//! Native presentation and transaction composition over the upstream sketch commands.
use serde::Deserialize;
use serde_json::{Value, json};
use solidcraft_engine::{doc::FeatureKind, sketch::CurveKind};
use std::collections::BTreeSet;

use crate::{AdapterError, SceneSnapshot, SessionAdapter, Viewport, error};
use solidcraft_engine::geom::Vec3;
use std::sync::Arc;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SketchProjection {
    pub document_id: String,
    pub geometry_revision: u64,
    pub sketch: u64,
    pub source: u64,
    pub entities: Vec<String>,
    pub all: bool,
    pub construction: bool,
    pub linked: bool,
}

impl SessionAdapter {
    /// Earlier authored, evaluated sketches only. The engine resolves all placements.
    pub(crate) fn projection_sources(&self) -> Vec<Value> {
        let Some(active) = self.session.active_sketch else { return Vec::new() };
        let state = self.session.model.state();
        let total_curves: usize = self.session.doc.features.iter().filter_map(|f| state.sketch(f.id)).map(|s| s.sketch.curves.len()).sum();
        let sample_budget = (8192 / total_curves.max(1)).clamp(2, 64);
        self.session
            .doc
            .features
            .iter()
            .take_while(|f| f.id != active)
            .filter(|f| !f.suppressed && !f.is_internal_profile() && matches!(f.kind, FeatureKind::Sketch { .. }))
            .filter_map(|f| {
                let ss = state.sketch(f.id)?;
                let sk = &ss.sketch;
                let used: BTreeSet<_> = sk.curves.iter().flat_map(|c| c.kind.point_ids()).collect();
                let mut entities: Vec<_> = sk
                    .curves
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        let kind = match c.kind {
                            CurveKind::Line { .. } => "line",
                            CurveKind::Circle { .. } => "circle",
                            CurveKind::Arc { .. } => "arc",
                            CurveKind::Ellipse { .. } => "ellipse",
                            CurveKind::Spline { .. } => "spline",
                            CurveKind::Conic { .. } => "conic",
                        };
                        // Bound display samples; exact projection always uses the engine curve.
                        let line = sk.polyline(i);
                        let stride = line.len().div_ceil(sample_budget).max(1);
                        let mut points: Vec<_> = line.iter().step_by(stride).copied().collect();
                        if let Some(last) = line.last() {
                            points.push(*last);
                        }
                        json!({"id":c.id,"kind":"curve","type":kind,"polyline":points,"all_geometry":true})
                    })
                    .collect();
                entities.extend(
                    sk.points.iter().enumerate().map(
                        |(i, p)| json!({"id":p.id,"kind":"point","type":"point","polyline":[p.pos],"all_geometry":i != 0 && !used.contains(&i)}),
                    ),
                );
                (!entities.is_empty()).then(|| json!({"id":f.id,"name":f.name,"entities":entities}))
            })
            .collect()
    }

    pub(crate) fn projection_geometry(&self) -> Vec<ProjectionSource> {
        let world = self.session.world_state();
        self.projection_sources()
            .iter()
            .filter_map(|source| {
                let id = source["id"].as_u64()?;
                let plane = world.sketch(id)?.plane;
                let entities = source["entities"]
                    .as_array()?
                    .iter()
                    .filter_map(|entity| {
                        let local: Vec<solidcraft_engine::geom::Vec2> = serde_json::from_value(entity["polyline"].clone()).ok()?;
                        Some(ProjectionEntity {
                            id: entity["id"].as_str()?.into(),
                            point: entity["kind"] == "point",
                            all_geometry: entity["all_geometry"] == true,
                            points: local.into_iter().map(|p| plane.to_world(p)).collect(),
                        })
                    })
                    .collect();
                Some(ProjectionSource { id, name: source["name"].as_str()?.into(), entities })
            })
            .collect()
    }

    /// Validate the pinned form, then compose Project and Construction on scratch.
    /// Nothing is adopted until every command and the resulting scene have succeeded.
    pub fn project_sketch(&mut self, request: SketchProjection) -> Result<(Value, Arc<SceneSnapshot>), AdapterError> {
        if request.document_id != self.session.document_id
            || request.geometry_revision != self.geometry_revision
            || self.session.active_sketch != Some(request.sketch)
        {
            return Err(error("stale_result", "ui.ipados.projection.stale"));
        }
        if request.entities.len() > 1000 {
            return Err(error("bad_params", "ui.ipados.projection.limit"));
        }
        let source = self
            .projection_sources()
            .into_iter()
            .find(|s| s["id"] == request.source)
            .ok_or_else(|| error("bad_params", "ui.ipados.projection.source_missing"))?;
        let available = source["entities"].as_array().ok_or_else(|| error("bad_params", "ui.ipados.projection.source_missing"))?;
        let selected: BTreeSet<_> = request.entities.iter().map(String::as_str).collect();
        if !request.all && selected.iter().any(|id| !available.iter().any(|e| e["id"] == *id)) {
            return Err(error("bad_params", "ui.ipados.projection.source_missing"));
        }
        let refs: Vec<_> = available
            .iter()
            .filter(|e| if request.all { e["all_geometry"] == true } else { e["id"].as_str().is_some_and(|id| selected.contains(id)) })
            .map(|e| {
                if e["kind"] == "point" { json!({"sketch":request.source,"point":e["id"]}) } else { json!({"sketch":request.source,"curve":e["id"]}) }
            })
            .collect();
        if refs.is_empty() || refs.len() > 1000 {
            return Err(error("bad_params", "ui.ipados.projection.limit"));
        }
        let mut commands: Vec<(String, Value)> = vec![("sketch.project".into(), json!({"refs":refs,"projection_link":request.linked}))];
        let mut scratch = self.session.scratch();
        let reply = scratch.execute(&commands[0].0, &commands[0].1)?;
        let curves: Vec<_> =
            reply["links"].as_array().into_iter().flatten().flat_map(|l| l["curves"].as_array().into_iter().flatten().cloned()).collect();
        if request.construction && !curves.is_empty() {
            commands.push(("sketch.construction".into(), json!({"curves":curves,"value":true})));
        }
        let transaction = self.session.scratch().compute_commands(&commands, None)?;
        // Build display data before changing the authoritative design.
        for (id, params) in commands.iter().skip(1) {
            scratch.execute(id, params)?;
        }
        let revision = self.geometry_revision.checked_add(1).ok_or_else(|| error("revision_limit", "geometry revision exhausted"))?;
        let mut display = Self { session: scratch, geometry_revision: revision, workspace_cache: None };
        let mut snapshot = display.snapshot()?;
        let model = Arc::get_mut(&mut snapshot).ok_or_else(|| error("projection", "prepared scene must have unique ownership"))?;
        let replies = self.session.accept_commands(transaction)?;
        // One confirmation is one undo step, including Construction.
        self.session.undo.truncate(self.session.undo.len().saturating_sub(replies.len().saturating_sub(1)));
        self.geometry_revision = revision;
        model.revision = self.session.revision;
        Ok((reply, snapshot))
    }
}

pub(crate) struct ProjectionSource {
    pub id: u64,
    pub name: String,
    pub entities: Vec<ProjectionEntity>,
}
pub(crate) struct ProjectionEntity {
    pub id: String,
    pub point: bool,
    pub all_geometry: bool,
    pub points: Vec<Vec3>,
}
#[derive(Default)]
pub(crate) struct ProjectionPicking {
    source: Option<u64>,
    selected: BTreeSet<String>,
    filter: String,
    hover: Option<(u64, String)>,
}

impl Viewport {
    pub(crate) fn projection_command(&mut self, params: &Value) -> Result<Value, AdapterError> {
        let action = params["action"].as_str().ok_or_else(|| error("bad_params", "ui.ipados.projection.stale"))?;
        if action == "begin" {
            if self.updating
                || self.numeric_active()
                || self.model_active()
                || self.model.as_ref().is_none_or(|m| m.sketch.is_none() || m.projection_sources.is_empty())
            {
                return Err(error("disabled", "ui.ipados.projection.source_missing"));
            }
            self.cancel_interactions();
            self.contacts.sketch.tool = "select".into();
            self.selection.clear();
            self.hover = None;
            self.projection = Some(ProjectionPicking { filter: "auto".into(), ..ProjectionPicking::default() });
            if let Some(model) = &self.model {
                let bounds = model.projection_sources.iter().flat_map(|s| &s.entities).flat_map(|e| &e.points).fold(model.bounds, |mut bounds, p| {
                    bounds.add(*p);
                    bounds
                });
                self.fit_bounds(bounds);
            }
        } else if action == "cancel" {
            self.projection = None;
            self.cancel_interactions();
        } else {
            let model = self.model.as_ref().ok_or_else(|| error("disabled", "ui.ipados.projection.stale"))?;
            let picking = self.projection.as_mut().ok_or_else(|| error("disabled", "ui.ipados.projection.stale"))?;
            match action {
                "choose" => {
                    picking.source = None;
                    picking.selected.clear();
                }
                "next" => {
                    let index = model.projection_sources.iter().position(|s| Some(s.id) == picking.source);
                    let next = index.map_or(0, |i| (i + 1).checked_rem(model.projection_sources.len()).unwrap_or(0));
                    let source = model.projection_sources.get(next);
                    picking.source = source.map(|s| s.id);
                    picking.selected.clear();
                }
                "clear" => picking.selected.clear(),
                "all" => {
                    if let Some(source) = model.projection_sources.iter().find(|s| Some(s.id) == picking.source) {
                        picking.selected = source.entities.iter().filter(|e| e.all_geometry).map(|e| e.id.clone()).collect();
                    }
                }
                "filter" => {
                    let filter = params["filter"]
                        .as_str()
                        .filter(|f| ["auto", "curves", "points"].contains(f))
                        .ok_or_else(|| error("bad_params", "ui.ipados.projection.stale"))?;
                    picking.filter = filter.into();
                }
                _ => return Err(error("bad_params", "ui.ipados.projection.stale")),
            }
            picking.hover = None;
        }
        self.contacts.tool_revision = self.contacts.tool_revision.saturating_add(1);
        self.changed();
        Ok(self.diagnostic())
    }

    pub(crate) fn projection_presentation(&self) -> Value {
        let (Some(picking), Some(model)) = (&self.projection, &self.model) else { return Value::Null };
        let source = model.projection_sources.iter().find(|s| Some(s.id) == picking.source);
        json!({"document_id":model.document_id,"geometry_revision":model.geometry_revision,
            "sketch":model.sketch.as_ref().map(|s|s.id),"source":picking.source,
            "name":source.map(|s| &s.name),"selected":picking.selected,"filter":picking.filter,
            "source_count":model.projection_sources.len()})
    }

    fn projection_hit(&self, screen: [f64; 2]) -> Option<(u64, String)> {
        let picking = self.projection.as_ref()?;
        let model = self.model.as_ref()?;
        let pixel = self.metrics.pixel(screen)?;
        let vp = self.vp();
        let size = self.metrics.pixels();
        let mut best: Option<(bool, f64, u64, String)> = None;
        for source in model.projection_sources.iter().filter(|s| picking.source.is_none_or(|id| s.id == id)) {
            for entity in &source.entities {
                if picking.filter == "points" && !entity.point || picking.filter == "curves" && entity.point {
                    continue;
                }
                let projected: Vec<_> = entity.points.iter().filter_map(|p| crate::picking::project(vp, *p, size)).collect();
                let distance = if entity.point {
                    projected.first().map(|p| (p[0] - pixel[0]).hypot(p[1] - pixel[1]))
                } else {
                    projected
                        .windows(2)
                        .map(|p| {
                            let dx = p[1][0] - p[0][0];
                            let dy = p[1][1] - p[0][1];
                            let length = dx * dx + dy * dy;
                            let t = if length > 0. { (((pixel[0] - p[0][0]) * dx + (pixel[1] - p[0][1]) * dy) / length).clamp(0., 1.) } else { 0. };
                            (pixel[0] - p[0][0] - t * dx).hypot(pixel[1] - p[0][1] - t * dy)
                        })
                        .min_by(f64::total_cmp)
                };
                let Some(distance) = distance.filter(|d| *d <= if entity.point { 8. } else { 10. } * self.metrics.scale) else { continue };
                // Points take priority at endpoints/centres; Curves mode bypasses them.
                let candidate = (!entity.point, distance, source.id, entity.id.clone());
                if best.as_ref().is_none_or(|b| !candidate.0 && b.0 || candidate.0 == b.0 && candidate.1 < b.1) {
                    best = Some(candidate);
                }
            }
        }
        best.map(|(_, _, source, id)| (source, id))
    }

    pub(crate) fn projection_hover(&mut self, screen: [f64; 2]) {
        let hit = self.projection_hit(screen);
        if let Some(picking) = &mut self.projection {
            picking.hover = hit;
        }
        self.contacts.tool_revision = self.contacts.tool_revision.saturating_add(1);
    }
    pub(crate) fn projection_tap(&mut self, screen: [f64; 2]) {
        let hit = self.projection_hit(screen);
        if let (Some(picking), Some((source, id))) = (&mut self.projection, hit) {
            picking.source = Some(source);
            if !picking.selected.insert(id.clone()) {
                picking.selected.remove(&id);
            }
        }
        self.contacts.tool_revision = self.contacts.tool_revision.saturating_add(1);
    }

    pub(crate) fn projection_feedback(&self, scene: &mut solidcraft_render::Scene) {
        let (Some(picking), Some(model)) = (&self.projection, &self.model) else { return };
        let palette = solidcraft_engine::view::Palette::DEFAULT;
        let unit = 2. * self.camera.half_height() / self.metrics.height_points.max(1.);
        // Markers use the destination sketch axes; all positions come from engine world state.
        let plane = model.sketch_plane.unwrap_or(solidcraft_engine::geom::Plane::XY);
        for source in model.projection_sources.iter().filter(|s| picking.source.is_none_or(|id| s.id == id)) {
            for entity in &source.entities {
                if entity.point && picking.filter == "curves" {
                    continue;
                }
                let selected = picking.source == Some(source.id) && picking.selected.contains(&entity.id);
                let hover = picking.hover.as_ref().is_some_and(|(id, e)| *id == source.id && e == &entity.id);
                let color = if selected {
                    palette.body_selected
                } else if hover {
                    palette.sketch_fixed
                } else {
                    palette.sketch
                };
                let mut line = |points| {
                    scene.lines.push(solidcraft_render::SceneLine {
                        points,
                        color,
                        alpha: 1.,
                        width: if selected || hover { 3. * self.metrics.scale } else { 1.5 * self.metrics.scale },
                        on_top: true,
                    })
                };
                if entity.point {
                    if let Some(p) = entity.points.first() {
                        let size = unit * if selected || hover { 5. } else { 3. };
                        for axis in [plane.x, plane.y] {
                            line(vec![*p - axis * size, *p + axis * size]);
                        }
                    }
                } else {
                    line(entity.points.clone());
                }
            }
        }
    }
}
