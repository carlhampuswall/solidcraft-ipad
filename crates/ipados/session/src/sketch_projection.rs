//! Native presentation and transaction composition over the upstream sketch commands.
use serde::Deserialize;
use serde_json::{Value, json};
use solidcraft_engine::{doc::FeatureKind, sketch::CurveKind};
use std::collections::BTreeSet;

use crate::{AdapterError, SceneSnapshot, SessionAdapter, error};
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
                        json!({"id":c.id,"kind":"curve","type":kind,"polyline":points})
                    })
                    .collect();
                entities.extend(
                    sk.points
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| *i != 0 && !used.contains(i))
                        .map(|(_, p)| json!({"id":p.id,"kind":"point","type":"point","polyline":[p.pos]})),
                );
                (!entities.is_empty()).then(|| json!({"id":f.id,"name":f.name,"entities":entities}))
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
            .filter(|e| request.all || e["id"].as_str().is_some_and(|id| selected.contains(id)))
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
