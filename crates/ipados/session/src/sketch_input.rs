//! Native interaction state over immutable upstream sketches. Only completed command
//! proposals reach the engine worker; drawing samples never mutate a document.
use serde::Serialize;
use serde_json::{Value, json};
use solidcraft_engine::{
    Sel,
    geom::{Vec2, Vec3},
    sketch::{ConstraintKind, CurveKind, Shape, Sketch},
};
use std::collections::BTreeMap;

use crate::{AdapterError, InputEvent, InputPhase, SceneSnapshot, ToolOperation, ToolWork, Viewport, error};

#[derive(Clone)]
struct Placement {
    at: Vec2,
    argument: Value,
}

/// Associate a displayed size with its source edge without inferring a second
/// parameter model. Point-to-point dimensions may reference a line's endpoints.
pub(crate) fn dimension_on_curve(sk: &Sketch, kind: &ConstraintKind, index: usize) -> bool {
    match *kind {
        ConstraintKind::Length { l, .. } => l == index,
        ConstraintKind::Radius { c, .. } | ConstraintKind::Diameter { c, .. } => c == index,
        ConstraintKind::Distance { p, q, .. } | ConstraintKind::DistanceX { p, q, .. } | ConstraintKind::DistanceY { p, q, .. } => {
            matches!(sk.curves.get(index).map(|c| &c.kind), Some(CurveKind::Line { a, b }) if (*a == p && *b == q) || (*a == q && *b == p))
        }
        _ => false,
    }
}

#[derive(Clone, Serialize)]
pub(crate) struct Snap {
    kind: &'static str,
    at: Vec2,
    persistent: bool,
}

pub(crate) struct SketchInput {
    pub tool: String,
    points: Vec<Placement>,
    cursor: Option<Placement>,
    contact: Option<u64>,
    started_with_anchor: bool,
    line_chain: bool,
    work: Option<ToolWork>,
    after: Option<Placement>,
    moving: Option<(String, Vec2, Vec<String>)>,
    tap_selection: Option<Sel>,
    moving_dimension: Option<(String, Vec2)>,
    moving_measurement: Option<(String, Vec2)>,
    measurement_positions: BTreeMap<(String, u64, String), Vec2>,
    dimension: Option<Value>,
    suppressed: bool,
    pub snap: Option<Snap>,
    pub snapping: bool,
    pub automatic: bool,
    pub free_dimensions: bool,
    pub repeat_rectangles: bool,
    pub construction: bool,
}

impl Default for SketchInput {
    fn default() -> Self {
        Self {
            tool: "select".into(),
            points: Vec::new(),
            cursor: None,
            contact: None,
            started_with_anchor: false,
            line_chain: false,
            work: None,
            after: None,
            moving: None,
            tap_selection: None,
            moving_dimension: None,
            moving_measurement: None,
            measurement_positions: BTreeMap::new(),
            dimension: None,
            suppressed: false,
            snap: None,
            snapping: true,
            automatic: true,
            free_dimensions: true,
            repeat_rectangles: false,
            construction: false,
        }
    }
}

impl SketchInput {
    pub(crate) fn pending(&self) -> bool {
        self.work.is_some()
    }
    pub fn active(&self) -> bool {
        self.tool.starts_with("sketch.")
    }
    pub(crate) fn geometry_contact(&self) -> Option<u64> {
        self.moving.as_ref().and(self.contact)
    }
    pub fn dimension(&mut self, selection: &[Sel], params: &Value) -> Result<(), AdapterError> {
        let entities: Vec<_> = selection
            .iter()
            .filter_map(|s| match s {
                Sel::SketchPoint { id } | Sel::SketchCurve { id, .. } => Some(id.clone()),
                _ => None,
            })
            .collect();
        if entities.is_empty() || entities.len() > 2 {
            return Err(error("bad_params", "Select one or two sketch entities to dimension"));
        }
        let mut proposal = json!({"entities":entities,"type":params.get("dimension_type").and_then(Value::as_str).unwrap_or("auto")});
        if let Some(value) = params.get("value") {
            proposal["value"] = value.clone();
        }
        self.dimension = Some(proposal);
        Ok(())
    }
    pub fn cancel(&mut self) {
        self.line_chain = false;
        self.points.clear();
        self.cursor = None;
        self.contact = None;
        self.work = None;
        self.after = None;
        self.moving = None;
        self.tap_selection = None;
        self.moving_dimension = None;
        self.moving_measurement = None;
        self.snap = None;
    }
    pub fn current(&self, work: &ToolWork) -> bool {
        self.work.as_ref().is_some_and(|w| w.interaction == work.interaction && w.version == work.version && w.commit == work.commit)
    }
    pub fn diagnostic(&self) -> Value {
        let prompt = match self.tool.as_str() {
            "select" => "select_hint",
            tool if tool.starts_with("sketch.constraint.") => "constraint_pick",
            "sketch.move" => "move",
            "sketch.delete" => "delete_hint",
            "sketch.trim" => "trim",
            "sketch.extend" => "extend",
            "sketch.offset" => "offset_hint",
            "sketch.dimension" => "dimension_place",
            "sketch.point" => "point_place",
            "sketch.arc.three_point" if self.points.len() == 2 => "arc_through",
            "sketch.arc.three_point" if self.points.len() == 1 => "arc_end",
            "sketch.spline.fit_point" => "spline_point",
            _ if self.points.is_empty() => "first_point",
            _ => "second_point",
        };
        let feedback = self
            .points
            .first()
            .zip(self.cursor.as_ref())
            .map(|(a, b)| json!({"width":(b.at.x-a.at.x).abs(),"height":(b.at.y-a.at.y).abs(),"radius":b.at.dist(a.at)}));
        json!({"points":self.points.iter().map(|p|p.at).collect::<Vec<_>>(),"snap":self.snap,
            "snapping":self.snapping,"auto_constraints":self.automatic,"prompt":prompt,"feedback":feedback,
            "pending":self.work.is_some(),"tool":self.tool,"free_dimensions":self.free_dimensions,"repeat_rectangles":self.repeat_rectangles,"construction":self.construction})
    }
    pub fn complete(&mut self, work: &ToolWork, model: &SceneSnapshot) {
        if !self.current(work) {
            return;
        }
        self.points.clear();
        if self.tool == "sketch.rectangle.two_point"
            && !self.repeat_rectangles
            && matches!(&work.operation, ToolOperation::Sketch { commands, .. } if commands.iter().any(|(id, _)| id == "sketch.rectangle.two_point"))
        {
            self.tool = "select".into();
        }
        if self.tool == "sketch.line"
            && let Some(mut next) = self.after.take()
        {
            // The committed endpoint is a genuine engine reference, not its old screen position.
            if let (Some(snapshot), Some(reference)) = (&model.sketch, next.argument.as_str())
                && let Some(index) = snapshot.sketch.resolve_point(reference)
                && let Some(point) = snapshot.sketch.point(index)
            {
                next.at = point;
            }
            self.points.push(next);
            self.line_chain = true;
        }
        self.cursor = None;
        self.contact = None;
        self.work = None;
        self.moving = None;
        self.tap_selection = None;
        self.moving_dimension = None;
        self.moving_measurement = None;
        self.snap = None;
    }
}

/// Pointer distance uses the same upstream outline as native picking.
fn curve_distance(sketch: &Sketch, curve: usize, point: Vec2) -> Option<f64> {
    sketch.shape(curve).map(|shape| shape.dist(point)).or_else(|| {
        sketch
            .polyline(curve)
            .windows(2)
            .map(|pair| {
                let d = pair[1] - pair[0];
                let t = if d.len2() > 0.0 { ((point - pair[0]).dot(d) / d.len2()).clamp(0.0, 1.0) } else { 0.0 };
                (pair[0] + d * t).dist(point)
            })
            .min_by(f64::total_cmp)
    })
}

impl Viewport {
    /// Presentation-only anchor for finishing committed lines. Keep the active
    /// drawing contact and provisional evaluations clear of native controls.
    pub(crate) fn sketch_chain_end(&self) -> Option<[f64; 2]> {
        let input = &self.contacts.sketch;
        if input.tool != "sketch.line"
            || !input.line_chain
            || input.contact.is_some()
            || input.work.is_some()
            || input.suppressed
            || self.contacts.navigating
            || self.updating
        {
            return None;
        }
        let snapshot = self.model.as_ref()?.sketch.as_ref()?;
        let end = input.points.last()?;
        let position = crate::picking::project(self.vp(), snapshot.plane.to_world(end.at), self.metrics.pixels())?;
        Some([position[0] / self.metrics.scale + self.metrics.origin_points[0], position[1] / self.metrics.scale + self.metrics.origin_points[1]])
    }

    pub(crate) fn sketch_command(&mut self, id: &str, params: &Value) -> Result<bool, AdapterError> {
        match id {
            "tool.construction" => {
                let enabled = params.get("enabled").and_then(Value::as_bool).ok_or_else(|| error("bad_params", "enabled must be boolean"))?;
                if self.model.as_ref().is_none_or(|model| model.sketch.is_none()) {
                    return Err(error("disabled", "Create or reopen a sketch first"));
                }
                self.cancel_interactions();
                self.contacts.sketch.construction = enabled;
            }
            "tool.propose" => {
                let command = params.get("command").and_then(Value::as_str).ok_or_else(|| error("bad_params", "A sketch command is required"))?;
                let parameters = params
                    .get("params")
                    .filter(|p| p.is_object())
                    .cloned()
                    .ok_or_else(|| error("bad_params", "Sketch command parameters must be an object"))?;
                let commit = params.get("commit").and_then(Value::as_bool).ok_or_else(|| error("bad_params", "commit must be boolean"))?;
                if !command.starts_with("sketch.") || matches!(command, "sketch.create" | "sketch.edit" | "sketch.finish" | "sketch.cancel") {
                    return Err(error("bad_params", "Use the sketch lifecycle controls for this command"));
                }
                self.propose_sketch(command, parameters, commit)?;
            }
            "tool.snapping" | "tool.auto_constraints" => {
                let enabled = params.get("enabled").and_then(Value::as_bool).ok_or_else(|| error("bad_params", "enabled must be boolean"))?;
                if id == "tool.snapping" {
                    self.propose_sketch("sketch.options", json!({"snap":enabled}), true)?;
                } else {
                    self.cancel_interactions();
                    self.contacts.sketch.automatic = enabled;
                }
            }
            "tool.free_dimensions" | "tool.repeat_rectangles" => {
                let enabled = params.get("enabled").and_then(Value::as_bool).ok_or_else(|| error("bad_params", "enabled must be boolean"))?;
                self.cancel_interactions();
                if id == "tool.free_dimensions" {
                    self.contacts.sketch.free_dimensions = enabled;
                    self.contacts.tool_revision = self.contacts.tool_revision.saturating_add(1);
                } else {
                    self.contacts.sketch.repeat_rectangles = enabled;
                }
            }
            "tool.finish_chain" => {
                if self.contacts.sketch.tool == "sketch.spline.fit_point" && self.contacts.sketch.points.len() >= 2 {
                    self.queue_sketch(true)?;
                } else {
                    self.contacts.sketch.cancel();
                    self.contacts.pending = None;
                    self.contacts.preview = None;
                }
            }
            _ => return Ok(false),
        }
        self.changed();
        Ok(true)
    }

    pub(crate) fn propose_sketch(&mut self, command: &str, parameters: Value, commit: bool) -> Result<(), AdapterError> {
        self.cancel_interactions();
        let model = self.model.as_ref().ok_or_else(|| error("sketch", "No document"))?;
        let sketch = model.sketch.as_ref().ok_or_else(|| error("sketch", "No active sketch"))?;
        let interaction = self.contacts.next_interaction.checked_add(1).ok_or_else(|| error("revision_limit", "interaction id exhausted"))?;
        self.contacts.next_interaction = interaction;
        let work = ToolWork {
            interaction,
            version: 1,
            document_id: model.document_id.clone(),
            geometry_revision: model.geometry_revision,
            operation: ToolOperation::Sketch { sketch: sketch.id, commands: vec![(command.into(), parameters)] },
            commit,
        };
        self.contacts.sketch.work = Some(work.clone());
        self.contacts.pending = Some(work);
        Ok(())
    }

    /// Dimension text is a direct handle in Select and Move. Its original position
    /// and the contact anchor are distinct, so grabbing off-centre never jumps.
    pub(crate) fn begin_dimension_drag(&mut self, event: &InputEvent, screen: [f64; 2]) -> bool {
        let state = &self.contacts.sketch;
        if !matches!(state.tool.as_str(), "select" | "sketch.move") || state.contact.is_some() || state.work.is_some() || self.contacts.navigating {
            return false;
        }
        let annotations = self.sketch_annotations();
        let Some(annotation) = annotations.as_array().and_then(|annotations| {
            annotations
                .iter()
                .filter_map(|annotation| {
                    let position = annotation.get("points")?.as_array()?;
                    let distance = (position.first()?.as_f64()? - screen[0]).hypot(position.get(1)?.as_f64()? - screen[1]);
                    let hit = event.sketch_annotation.as_ref().map_or(distance <= 22.0, |id| annotation["id"].as_str() == Some(id));
                    hit.then_some((distance, annotation))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, annotation)| annotation)
        }) else {
            return false;
        };
        let Some(id) = annotation["id"].as_str().map(str::to_owned) else { return false };
        let Some(snapshot) = self.model.as_ref().and_then(|m| m.sketch.as_ref()) else { return false };
        let measurement = id.strip_prefix("edge:");
        let (frame, text, selection) = if let Some(curve) = measurement {
            let Some((index, kind, _)) =
                self.measurements(snapshot).find(|(index, _, _)| snapshot.sketch.curves.get(*index).is_some_and(|c| c.id == curve))
            else {
                return false;
            };
            if snapshot.sketch.curves.get(index).is_none_or(|c| c.id != curve) {
                return false;
            }
            let Some(frame) = solidcraft_engine::sketch::dim_frame(&snapshot.sketch, &kind) else { return false };
            (frame, self.measurement_text(snapshot, curve, &kind), Sel::SketchCurve { id: curve.into(), sketch: Some(snapshot.id) })
        } else {
            let Some(constraint) = snapshot.sketch.constraints.iter().find(|c| c.id == id) else { return false };
            let Some(frame) = solidcraft_engine::sketch::dim_frame(&snapshot.sketch, &constraint.kind) else { return false };
            (frame, constraint.text, Sel::SketchConstraint { id: id.clone() })
        };
        let Some(world) = self.plane_point(screen, snapshot.plane) else { return false };
        let local = snapshot.plane.to_local(world);
        let unit = 2.0 * self.camera.half_height() / self.metrics.height_points.max(1.0);
        let text = solidcraft_engine::sketch::dim_layout(&frame, text, unit, Vec2::ZERO).text;
        let Some(interaction) = self.contacts.next_interaction.checked_add(1) else { return false };
        self.contacts.next_interaction = interaction;
        if !self.is_selected(&selection) {
            self.select(Some(selection), self.multiple || event.shift);
        }
        let state = &mut self.contacts.sketch;
        state.cancel();
        state.contact = Some(event.id);
        if let Some(curve) = measurement {
            state.moving_measurement = Some((curve.into(), text));
        } else {
            state.moving_dimension = Some((id, text));
        }
        let placement = Placement { at: local, argument: json!([local.x, local.y]) };
        state.points.push(placement.clone());
        state.cursor = Some(placement);
        true
    }

    pub(crate) fn begin_sketch_input(&mut self, event: &InputEvent, screen: [f64; 2]) -> bool {
        if self.constraint_tool().is_some() || self.projection.is_some() {
            return false;
        }
        let direct = self.contacts.sketch.tool == "select" && self.direct_handles;
        if (!self.contacts.sketch.active() && !direct)
            || self.contacts.sketch.contact.is_some()
            || self.contacts.sketch.work.as_ref().is_some_and(|w| w.commit)
            || self.contacts.navigating
        {
            return false;
        }
        let Some(snapshot) = self.model.as_ref().and_then(|m| m.sketch.as_ref()) else {
            return false;
        };
        let Some(world) = self.plane_point(screen, snapshot.plane) else {
            return false;
        };
        let local = snapshot.plane.to_local(world);
        if direct || matches!(self.contacts.sketch.tool.as_str(), "sketch.move" | "sketch.delete" | "sketch.trim" | "sketch.extend" | "sketch.offset")
        {
            let Some(selection) = self.pick_sketch(screen) else {
                return false;
            };
            if (direct && !matches!(selection, Sel::SketchPoint { .. } | Sel::SketchCurve { .. }))
                || (self.contacts.sketch.tool == "sketch.offset" && !matches!(selection, Sel::SketchCurve { .. }))
            {
                return false;
            }
            let modifying = matches!(self.contacts.sketch.tool.as_str(), "sketch.trim" | "sketch.extend");
            let entity = match &selection {
                Sel::SketchPoint { id } if modifying => {
                    let Some(point) = snapshot.sketch.resolve_point(id) else {
                        return false;
                    };
                    let Some(curve) = snapshot.sketch.curves.iter().find(|c| c.kind.uses(point)) else {
                        return false;
                    };
                    curve.id.clone()
                }
                Sel::SketchPoint { id } | Sel::SketchCurve { id, .. } | Sel::SketchConstraint { id } => id.clone(),
                _ => return false,
            };
            let grouped = self.is_selected(&selection) || self.multiple || event.shift;
            let with = self
                .selection
                .iter()
                .filter_map(|s| match s {
                    Sel::SketchCurve { id, .. } | Sel::SketchPoint { id } if grouped && id != &entity => Some(id.clone()),
                    _ => None,
                })
                .collect();
            if direct {
                // A tap still toggles selection. Wait for actual movement before
                // selecting the grabbed entity for a drag of the current group.
                self.contacts.sketch.tap_selection = Some(selection);
            } else if !self.is_selected(&selection) {
                self.select(Some(selection), self.multiple || event.shift);
            }
            self.contacts.sketch.moving = Some((entity, local, with));
        }
        let Some(interaction) = self.contacts.next_interaction.checked_add(1) else {
            return false;
        };
        self.contacts.next_interaction = interaction;
        let placement = self.sketch_placement(local, event.shift);
        let state = &mut self.contacts.sketch;
        state.contact = Some(event.id);
        state.started_with_anchor = !state.points.is_empty();
        if state.points.is_empty() {
            state.points.push(placement.clone());
        }
        state.cursor = Some(placement);
        true
    }

    pub(crate) fn update_sketch_input(&mut self, event: &InputEvent, screen: [f64; 2], moved: bool) {
        if self.contacts.sketch.contact != Some(event.id) || self.contacts.navigating || self.updating {
            return;
        }
        if self.contacts.sketch.moving.is_some() && !moved {
            return;
        }
        if let Some(selection) = self.contacts.sketch.tap_selection.take()
            && !self.is_selected(&selection)
        {
            self.select(Some(selection), self.multiple || event.shift);
        }
        let Some(snapshot) = self.model.as_ref().and_then(|m| m.sketch.as_ref()) else {
            return;
        };
        let Some(world) = self.plane_point(screen, snapshot.plane) else {
            self.cancel_sketch_input();
            return;
        };
        let placement = self.sketch_placement(snapshot.plane.to_local(world), event.shift);
        self.contacts.sketch.cursor = Some(placement);
        if self.contacts.sketch.moving_measurement.is_none()
            && let Err(_failure) = self.queue_sketch(false)
        {
            // Incomplete/degenerate candidates have no command proposal. The last valid
            // evaluated preview stays visible until another valid placement or cancellation.
        }
        self.contacts.tool_revision = self.contacts.tool_revision.saturating_add(1);
    }

    pub(crate) fn end_sketch_input(&mut self, event: &InputEvent, moved: bool) {
        if self.contacts.sketch.contact != Some(event.id) {
            return;
        }
        if event.phase == InputPhase::Cancelled || self.contacts.navigating {
            self.cancel_sketch_input();
            return;
        }
        if let Some((curve, _)) = self.contacts.sketch.moving_measurement.clone() {
            if moved
                && let Some(model) = &self.model
                && let Some(snapshot) = &model.sketch
                && let Some((_, kind, _)) =
                    self.measurements(snapshot).find(|(index, _, _)| snapshot.sketch.curves.get(*index).is_some_and(|c| c.id == curve))
                && let Some(text) = self.measurement_text(snapshot, &curve, &kind)
            {
                let key = (model.document_id.clone(), snapshot.id, curve);
                // Presentation-only measurements never create a driving constraint
                // or history entry. Keep this session cache bounded.
                if self.contacts.sketch.measurement_positions.len() >= 256 && !self.contacts.sketch.measurement_positions.contains_key(&key) {
                    self.contacts.sketch.measurement_positions.pop_first();
                }
                self.contacts.sketch.measurement_positions.insert(key, text);
            }
            self.cancel_sketch_input();
            return;
        }
        self.contacts.sketch.contact = None;
        let anchored = self.contacts.sketch.started_with_anchor;
        let tool = self.contacts.sketch.tool.as_str();
        if matches!(tool, "sketch.point" | "sketch.dimension" | "sketch.delete" | "sketch.trim" | "sketch.extend") {
            let _ = self.queue_sketch(true);
            return;
        }
        if self.contacts.sketch.moving.is_some() || self.contacts.sketch.moving_dimension.is_some() {
            if moved {
                if self.queue_sketch(true).is_err() {
                    self.cancel_sketch_input();
                }
            } else {
                if let Some(selection) = self.contacts.sketch.tap_selection.take() {
                    self.select_tap(Some(selection), self.multiple || event.shift, event.tap_count);
                }
                self.cancel_sketch_input();
            }
            return;
        }
        let Some(cursor) = self.contacts.sketch.cursor.clone() else {
            return;
        };
        if tool == "sketch.spline.fit_point" || tool == "sketch.arc.three_point" {
            if (anchored || moved) && self.contacts.sketch.points.last().is_none_or(|p| p.at != cursor.at) {
                self.contacts.sketch.points.push(cursor);
            }
            if tool == "sketch.arc.three_point" && self.contacts.sketch.points.len() == 3 {
                let _ = self.queue_sketch(true);
            } else if tool == "sketch.spline.fit_point" {
                let _ = self.queue_sketch(false);
            } else {
                self.contacts.preview = None;
                self.contacts.pending = None;
                self.contacts.sketch.work = None;
            }
        } else if (anchored || moved) && self.queue_sketch(true).is_err() {
            // A tap at the anchor only places an anchor; it is never a zero-length entity.
            self.contacts.pending = None;
            self.contacts.preview = None;
            self.contacts.sketch.work = None;
        }
    }

    pub(crate) fn cancel_sketch_input(&mut self) {
        self.contacts.sketch.cancel();
        self.contacts.pending = None;
        self.contacts.preview = None;
        self.contacts.tool_revision = self.contacts.tool_revision.saturating_add(1);
    }

    fn queue_sketch(&mut self, commit: bool) -> Result<(), AdapterError> {
        let model = self.model.as_ref().ok_or_else(|| error("sketch", "No document"))?;
        let snapshot = model.sketch.as_ref().ok_or_else(|| error("sketch", "No active sketch"))?;
        let state = &self.contacts.sketch;
        let first = state.points.first().ok_or_else(|| error("sketch", "Place a first point"))?;
        let last = state.cursor.as_ref().unwrap_or(first);
        let automatic = state.automatic && !state.suppressed;
        let arg = |p: &Placement| {
            if automatic || p.argument.as_str().is_some_and(|s| s.starts_with("ipad_line_")) { p.argument.clone() } else { json!([p.at.x, p.at.y]) }
        };
        let nonzero = first.at.dist(last.at) > 0.0;
        let mut after = None;
        let mut command = if let Some((dimension, text)) = &state.moving_dimension {
            let at = *text + (last.at - first.at);
            ("sketch.dimension_text".into(), json!({"dimension":dimension,"at":[at.x,at.y]}))
        } else {
            match state.tool.as_str() {
                "sketch.point" => {
                    // Endpoint references are not point-command arguments.
                    // Coordinates plus connect create a new coincident point.
                    let point = if automatic && last.argument.as_str().is_some_and(|s| s.starts_with("mid:") || s.starts_with("on:")) {
                        last.argument.clone()
                    } else {
                        json!([last.at.x, last.at.y])
                    };
                    ("sketch.point".into(), json!({"point":point,"connect":automatic}))
                }
                "select" | "sketch.move" => {
                    let (entity, from, with) = state.moving.as_ref().ok_or_else(|| error("sketch", "Select geometry to move"))?;
                    if *from == last.at {
                        return Err(error("sketch", "No movement"));
                    }
                    if snapshot.sketch.constraints.iter().any(|c| &c.id == entity && c.kind.is_dimension()) {
                        ("sketch.dimension_text".into(), json!({"dimension":entity,"at":[last.at.x,last.at.y]}))
                    } else {
                        ("sketch.drag".into(), json!({"entity":entity,"from":[from.x,from.y],"to":[last.at.x,last.at.y],"with":with,"live":!commit}))
                    }
                }
                "sketch.dimension" => {
                    let mut proposal = state.dimension.clone().ok_or_else(|| error("sketch", "Select entities to dimension"))?;
                    proposal["text_at"] = json!([last.at.x, last.at.y]);
                    ("sketch.dimension".into(), proposal)
                }
                "sketch.delete" => {
                    let (entity, _, _) = state.moving.as_ref().ok_or_else(|| error("sketch", "Select a curve"))?;
                    ("sketch.delete".into(), json!({"entities":[entity]}))
                }
                "sketch.trim" | "sketch.extend" => {
                    let (entity, _, _) = state.moving.as_ref().ok_or_else(|| error("sketch", "Select a curve"))?;
                    (state.tool.clone(), json!({"curve":entity,"at":[last.at.x,last.at.y]}))
                }
                "sketch.offset" => {
                    let (entity, _, _) = state.moving.as_ref().ok_or_else(|| error("sketch", "Select a curve"))?;
                    let curve = snapshot
                        .sketch
                        .curves
                        .iter()
                        .position(|curve| &curve.id == entity)
                        .ok_or_else(|| error("sketch", "The source curve is no longer available"))?;
                    // Measure the pointer against the upstream curve presentation. The
                    // engine alone constructs the exact offset and connected chain.
                    let distance =
                        curve_distance(&snapshot.sketch, curve, last.at).ok_or_else(|| error("sketch", "The source curve has no outline"))?;
                    if !distance.is_finite() || distance <= 1e-8 {
                        return Err(error("sketch", "Drag away from the source edge"));
                    }
                    ("sketch.offset".into(), json!({"curves":[entity],"distance":distance,"side":[last.at.x,last.at.y],"chain":true}))
                }
                "sketch.line" if nonzero => {
                    let id = format!("ipad_line_{}_{}", model.geometry_revision, self.contacts.next_interaction);
                    after = Some(Placement { at: last.at, argument: json!(format!("{id}.end")) });
                    ("sketch.line".into(), json!({"points":[arg(first),arg(last)],"ids":[id],"infer":automatic,"connect":automatic}))
                }
                "sketch.rectangle.two_point" if first.at.x != last.at.x && first.at.y != last.at.y => {
                    ("sketch.rectangle.two_point".into(), json!({"p0":[first.at.x,first.at.y],"p1":[last.at.x,last.at.y],"connect":automatic}))
                }
                "sketch.circle.center" if nonzero => {
                    let mut params = json!({"center":arg(first),"radius":first.at.dist(last.at),"connect":automatic});
                    if automatic && last.argument.is_string() {
                        params["through"] = last.argument.clone();
                    }
                    ("sketch.circle.center".into(), params)
                }
                "sketch.arc.three_point" if state.points.len() >= 2 => {
                    let second = state.points.get(1).ok_or_else(|| error("sketch", "Place arc end"))?;
                    let through = if commit { state.points.get(2).unwrap_or(last) } else { last };
                    ("sketch.arc.three_point".into(), json!({"start":arg(first),"end":arg(second),"through":arg(through),"connect":automatic}))
                }
                "sketch.spline.fit_point" => {
                    let mut points = state.points.clone();
                    if !commit && points.last().is_none_or(|p| p.at != last.at) {
                        points.push(last.clone());
                    }
                    if points.len() < 2 {
                        return Err(error("sketch", "Place another spline point"));
                    }
                    ("sketch.spline.fit_point".into(), json!({"points":points.iter().map(arg).collect::<Vec<_>>(),"connect":automatic}))
                }
                _ => return Err(error("sketch", "Place the next distinct point")),
            }
        };
        if matches!(
            command.0.as_str(),
            "sketch.line" | "sketch.rectangle.two_point" | "sketch.circle.center" | "sketch.arc.three_point" | "sketch.spline.fit_point"
        ) {
            command.1["construction"] = json!(state.construction);
        }
        let version = state.work.as_ref().map_or(1, |w| w.version.saturating_add(1));
        let work = ToolWork {
            interaction: self.contacts.next_interaction,
            version,
            document_id: model.document_id.clone(),
            geometry_revision: model.geometry_revision,
            operation: ToolOperation::Sketch { sketch: snapshot.id, commands: vec![command] },
            commit,
        };
        self.contacts.sketch.work = Some(work.clone());
        self.contacts.sketch.after = after;
        self.contacts.pending = Some(work);
        Ok(())
    }

    fn sketch_placement(&mut self, local: Vec2, suppressed: bool) -> Placement {
        let mut placement = Placement { at: local, argument: json!([local.x, local.y]) };
        self.contacts.sketch.snap = None;
        self.contacts.sketch.suppressed = suppressed;
        let Some(snapshot) = self.model.as_ref().and_then(|m| m.sketch.as_ref()) else {
            return placement;
        };
        if suppressed
            || self.contacts.sketch.moving.is_some()
            || self.contacts.sketch.moving_dimension.is_some()
            || self.contacts.sketch.moving_measurement.is_some()
            || !self.contacts.sketch.snapping
            || snapshot.sketch.view.no_snap
            || matches!(
                self.contacts.sketch.tool.as_str(),
                "sketch.delete" | "sketch.trim" | "sketch.extend" | "sketch.dimension" | "sketch.move" | "sketch.offset"
            )
        {
            return placement;
        }
        let sk = &snapshot.sketch;
        let tolerance = 9.0 * 2.0 * self.camera.half_height() / self.metrics.height_points.max(1.0);
        let mut best: Option<(f64, Vec2, Value, &'static str)> = None;
        let mut offer = |point: Vec2, argument: Value, kind| {
            let distance = point.dist(local);
            if distance <= tolerance && best.as_ref().is_none_or(|b| distance < b.0) {
                best = Some((distance, point, argument, kind));
            }
        };
        offer(Vec2::ZERO, json!("origin"), "origin");
        let centers: std::collections::BTreeSet<usize> = sk
            .curves
            .iter()
            .filter_map(|curve| match curve.kind {
                solidcraft_engine::sketch::CurveKind::Circle { c, .. }
                | solidcraft_engine::sketch::CurveKind::Arc { c, .. }
                | solidcraft_engine::sketch::CurveKind::Ellipse { c, .. } => Some(c),
                _ => None,
            })
            .collect();
        for (index, point) in sk.points.iter().enumerate() {
            offer(point.pos, json!(point.id), if centers.contains(&index) { "center" } else { "point" });
        }
        if best.is_none()
            && self.contacts.sketch.tool != "sketch.point"
            && let Some((point, a, b)) = solidcraft_engine::sketch::nearest_curve_intersection(sk, local, tolerance)
            && let (Some(a), Some(b)) = (sk.curves.get(a), sk.curves.get(b))
        {
            best = Some((point.dist(local), point, json!(format!("x:{}+{}:{},{}", a.id, b.id, point.x, point.y)), "intersection"));
        }
        if best.is_none() {
            for (index, curve) in sk.curves.iter().enumerate() {
                let Some(shape) = sk.shape(index) else {
                    continue;
                };
                let candidate = match shape {
                    Shape::Line { a, b } => Some(((a + b) * 0.5, json!(format!("mid:{}", curve.id)), "midpoint")),
                    Shape::Round { c, .. } => Some((c, json!(format!("{}.center", curve.id)), "center")),
                };
                if let Some((point, arg, kind)) = candidate
                    && point.dist(local) <= tolerance
                {
                    best = Some((point.dist(local), point, arg, kind));
                    break;
                }
            }
        }
        if best.is_none() && self.contacts.sketch.tool == "sketch.point" {
            for (index, curve) in sk.curves.iter().enumerate() {
                if !solidcraft_engine::view::sketch_curve_visible(sk, index) {
                    continue;
                }
                let Some(shape) = sk.shape(index) else { continue };
                let point = shape.project(local);
                let distance = point.dist(local);
                if distance <= tolerance && shape.param_on(point, 1e-8).is_some() && best.as_ref().is_none_or(|b| distance < b.0) {
                    best = Some((distance, point, json!(format!("on:{}:{},{}", curve.id, point.x, point.y)), "on_curve"));
                }
            }
        }
        if best.is_none() {
            let anchor = self.contacts.sketch.points.last().map(|p| p.at);
            if let Some(anchor) = anchor {
                if (anchor.y - local.y).abs() <= tolerance && (anchor.x - local.x).abs() > tolerance {
                    best = Some(((anchor.y - local.y).abs(), Vec2::new(local.x, anchor.y), Value::Null, "horizontal"));
                } else if (anchor.x - local.x).abs() <= tolerance && (anchor.y - local.y).abs() > tolerance {
                    best = Some(((anchor.x - local.x).abs(), Vec2::new(anchor.x, local.y), Value::Null, "vertical"));
                }
            }
            if best.is_none() && local.x.abs() <= tolerance {
                best = Some((local.x.abs(), Vec2::new(0.0, local.y), Value::Null, "y_axis"));
            }
            if best.is_none() && local.y.abs() <= tolerance {
                best = Some((local.y.abs(), Vec2::new(local.x, 0.0), Value::Null, "x_axis"));
            }
        }
        if let Some((_, point, argument, kind)) = best {
            let persistent = self.contacts.sketch.automatic
                && if self.contacts.sketch.tool == "sketch.rectangle.two_point" {
                    matches!(kind, "point" | "origin")
                } else {
                    !argument.is_null() || (self.contacts.sketch.tool == "sketch.line" && matches!(kind, "horizontal" | "vertical"))
                };
            placement = Placement { at: point, argument: if argument.is_null() { json!([point.x, point.y]) } else { argument } };
            self.contacts.sketch.snap = Some(Snap { kind, at: point, persistent });
        }
        placement
    }

    pub(crate) fn pick_sketch(&self, screen: [f64; 2]) -> Option<Sel> {
        self.pick_sketch_filtered(screen, true)
    }

    pub(crate) fn pick_sketch_unfiltered(&self, screen: [f64; 2]) -> Option<Sel> {
        self.pick_sketch_filtered(screen, false)
    }

    fn pick_sketch_filtered(&self, screen: [f64; 2], constraint_filter: bool) -> Option<Sel> {
        let snapshot = self.model.as_ref()?.sketch.as_ref()?;
        let point = snapshot.plane.to_local(self.plane_point(screen, snapshot.plane)?);
        let tolerance = 10.0 * 2.0 * self.camera.half_height() / self.metrics.height_points.max(1.0);
        let curves_only = matches!(self.contacts.sketch.tool.as_str(), "sketch.delete" | "sketch.offset");
        if !curves_only
            && (!constraint_filter || self.constraint_tool().is_none())
            && let Some((id, _, _)) = self.visible_glyphs().into_iter().find(|(_, _, at)| at.dist(point) <= tolerance)
        {
            return Some(Sel::SketchConstraint { id });
        }
        let unit = 2.0 * self.camera.half_height() / self.metrics.height_points.max(1.0);
        if !curves_only
            && (!constraint_filter || self.constraint_tool().is_none())
            && let Some(c) = snapshot
                .sketch
                .constraints
                .iter()
                .filter_map(|c| {
                    let frame = solidcraft_engine::sketch::dim_frame(&snapshot.sketch, &c.kind)?;
                    let at = solidcraft_engine::sketch::dim_layout(&frame, c.text, unit, Vec2::ZERO).text;
                    (at.dist(point) <= tolerance * 1.8).then_some((at.dist(point), c))
                })
                .min_by(|a, b| a.0.total_cmp(&b.0))
                .map(|(_, c)| c)
        {
            return Some(Sel::SketchConstraint { id: c.id.clone() });
        }
        if let Some(p) = snapshot
            .sketch
            .points
            .iter()
            .filter(|p| {
                self.contacts.sketch.tool != "sketch.offset"
                    && (!curves_only
                        || p.id != "origin"
                            && snapshot.sketch.resolve_point(&p.id).is_some_and(|i| !snapshot.sketch.curves.iter().any(|c| c.kind.uses(i))))
            })
            .filter(|p| {
                !constraint_filter || self.constraint_tool() != Some("coincident") || !self.is_selected(&Sel::SketchPoint { id: p.id.clone() })
            })
            .filter(|p| p.pos.dist(point) <= tolerance && (!constraint_filter || self.constraint_accepts(&Sel::SketchPoint { id: p.id.clone() })))
            .min_by(|a, b| a.pos.dist(point).total_cmp(&b.pos.dist(point)))
        {
            return Some(Sel::SketchPoint { id: p.id.clone() });
        }
        snapshot
            .sketch
            .curves
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                let distance = curve_distance(&snapshot.sketch, i, point);
                distance.map(|d| (d, c))
            })
            .filter(|(d, c)| {
                *d <= tolerance && (!constraint_filter || self.constraint_accepts(&Sel::SketchCurve { id: c.id.clone(), sketch: Some(snapshot.id) }))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, c)| Sel::SketchCurve { id: c.id.clone(), sketch: Some(snapshot.id) })
    }

    pub(crate) fn sketch_feedback(&self, scene: &mut solidcraft_render::Scene) {
        let Some(snapshot) = self.display_model().and_then(|m| m.sketch.as_ref()) else {
            return;
        };
        let color = solidcraft_engine::view::Palette::DEFAULT.body_selected;
        let width = 2.5 * self.metrics.scale;
        let marker = |scene: &mut solidcraft_render::Scene, p: Vec3| {
            let size = 6.0 * 2.0 * self.camera.half_height() / self.metrics.height_points.max(1.0);
            for axis in [snapshot.plane.x, snapshot.plane.y] {
                scene.lines.push(solidcraft_render::SceneLine {
                    points: vec![p - axis * size, p + axis * size],
                    color,
                    alpha: 1.0,
                    width,
                    on_top: true,
                });
            }
        };
        let unit = 2.0 * self.camera.half_height() / self.metrics.height_points.max(1.0);
        // Standalone points have no curve stroke in the shared model scene.
        // Keep their markers a constant screen size, including scratch previews.
        let used: std::collections::BTreeSet<_> = snapshot.sketch.curves.iter().flat_map(|c| c.kind.point_ids()).collect();
        for (_, point) in snapshot.sketch.points.iter().enumerate().filter(|(i, p)| p.id != "origin" && !used.contains(i)) {
            if point.link.is_some() && snapshot.sketch.view.hide_projected {
                continue;
            }
            let palette = solidcraft_engine::view::Palette::DEFAULT;
            let point_color = if point.link.is_some() {
                palette.sketch_projected
            } else if point.fixed {
                palette.sketch_fixed
            } else {
                palette.sketch
            };
            for axis in [snapshot.plane.x, snapshot.plane.y] {
                let at = snapshot.plane.to_world(point.pos);
                scene.lines.push(solidcraft_render::SceneLine {
                    points: vec![at - axis * unit * 3.5, at + axis * unit * 3.5],
                    color: point_color,
                    alpha: 1.0,
                    width: self.metrics.scale * 2.0,
                    on_top: true,
                });
            }
        }
        let measurements: Vec<_> = self
            .measurements(snapshot)
            .filter_map(|(index, kind, _)| {
                let curve = snapshot.sketch.curves.get(index)?;
                let text = self.measurement_text(snapshot, &curve.id, &kind);
                Some((kind, text))
            })
            .collect();
        for (kind, text) in snapshot.sketch.constraints.iter().map(|c| (&c.kind, c.text)).chain(measurements.iter().map(|(kind, text)| (kind, *text)))
        {
            if let Some(frame) = solidcraft_engine::sketch::dim_frame(&snapshot.sketch, kind) {
                let layout = solidcraft_engine::sketch::dim_layout(&frame, text, unit, Vec2::ZERO);
                for line in layout.lines {
                    scene.lines.push(solidcraft_render::SceneLine {
                        points: line.into_iter().map(|p| snapshot.plane.to_world(p)).collect(),
                        color,
                        alpha: 0.8,
                        width: self.metrics.scale,
                        on_top: true,
                    });
                }
                for (tip, direction) in layout.arrows {
                    for sign in [-1.0, 1.0] {
                        let end = tip - direction * (unit * 7.0) + direction.perp() * (unit * sign * 3.0);
                        scene.lines.push(solidcraft_render::SceneLine {
                            points: vec![snapshot.plane.to_world(tip), snapshot.plane.to_world(end)],
                            color,
                            alpha: 0.8,
                            width: self.metrics.scale,
                            on_top: true,
                        });
                    }
                }
            }
        }
        for (_, _, at) in self.visible_glyphs() {
            let size = unit * 5.0;
            let points = [Vec2::new(0.0, size), Vec2::new(size, 0.0), Vec2::new(0.0, -size), Vec2::new(-size, 0.0), Vec2::new(0.0, size)];
            scene.lines.push(solidcraft_render::SceneLine {
                points: points.into_iter().map(|p| snapshot.plane.to_world(at + p)).collect(),
                color,
                alpha: 1.0,
                width: self.metrics.scale * 2.0,
                on_top: true,
            });
        }
        for selection in self.selection.iter().chain(self.hover.iter()) {
            match selection {
                Sel::SketchPoint { id } => {
                    if let Some(i) = snapshot.sketch.resolve_point(id)
                        && let Some(p) = snapshot.sketch.point(i)
                    {
                        marker(scene, snapshot.plane.to_world(p));
                    }
                }
                Sel::SketchCurve { id, .. } => {
                    if let Some(i) = snapshot.sketch.curve_index(id) {
                        let construction = self.construction_dashes && snapshot.sketch.curves.get(i).is_some_and(|curve| curve.construction);
                        let line = solidcraft_render::SceneLine {
                            points: snapshot.sketch.polyline(i).iter().map(|p| snapshot.plane.to_world(*p)).collect(),
                            color: if construction { solidcraft_engine::view::Palette::DEFAULT.sketch_construction } else { color },
                            alpha: 1.0,
                            width,
                            on_top: true,
                        };
                        if construction {
                            crate::construction::dash_line(scene, &line, unit);
                        } else {
                            scene.lines.push(line);
                        }
                        if let Some(curve) = snapshot.sketch.curves.get(i) {
                            for p in curve.kind.point_ids() {
                                if let Some(p) = snapshot.sketch.point(p) {
                                    marker(scene, snapshot.plane.to_world(p));
                                }
                            }
                        }
                    }
                }
                Sel::SketchConstraint { id } => {
                    if let Some(c) = snapshot.sketch.constraints.iter().find(|c| &c.id == id) {
                        for p in snapshot.sketch.constraint_points(&c.kind) {
                            if let Some(p) = snapshot.sketch.point(p) {
                                marker(scene, snapshot.plane.to_world(p));
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        if self.contacts.sketch.moving_dimension.is_none() && self.contacts.sketch.moving_measurement.is_none() {
            for point in &self.contacts.sketch.points {
                marker(scene, snapshot.plane.to_world(point.at));
            }
        }
        if let Some(snap) = &self.contacts.sketch.snap {
            marker(scene, snapshot.plane.to_world(snap.at));
        }
    }

    fn measurements<'a>(&self, snapshot: &'a crate::sketching::SketchSnapshot) -> impl Iterator<Item = (usize, ConstraintKind, &'static str)> + 'a {
        let selected_curve = match self.selection.as_slice() {
            [Sel::SketchCurve { id, sketch }] if sketch.is_none_or(|id| id == snapshot.id) => snapshot.sketch.curve_index(id),
            _ => None,
        };
        let show_all = self.contacts.sketch.free_dimensions;
        snapshot
            .free_measurements
            .iter()
            .filter(move |(index, _, _)| {
                (show_all && snapshot.rectangle_measurements.get(index).is_none_or(|side| side.preferred))
                    || (!show_all
                        && selected_curve == Some(*index)
                        && !snapshot.sketch.constraints.iter().any(|c| dimension_on_curve(&snapshot.sketch, &c.kind, *index)))
            })
            .cloned()
    }

    fn measurement_text(&self, snapshot: &crate::sketching::SketchSnapshot, curve: &str, kind: &ConstraintKind) -> Option<Vec2> {
        let state = &self.contacts.sketch;
        if let Some((id, text)) = &state.moving_measurement
            && id == curve
            && let (Some(first), Some(last)) = (state.points.first(), state.cursor.as_ref())
        {
            let frame = solidcraft_engine::sketch::dim_frame(&snapshot.sketch, kind)?;
            return Some(solidcraft_engine::sketch::encode_text(&frame, *text + (last.at - first.at)));
        }
        let model = self.model.as_ref()?;
        state.measurement_positions.get(&(model.document_id.clone(), snapshot.id, curve.into())).copied().or_else(|| {
            if !state.free_dimensions {
                return None;
            }
            let frame = solidcraft_engine::sketch::dim_frame(&snapshot.sketch, kind)?;
            let unit = 2.0 * self.camera.half_height() / self.metrics.height_points.max(1.0);
            let index = snapshot.sketch.curve_index(curve)?;
            if let Some(side) = snapshot.rectangle_measurements.get(&index)
                && let Some(offset) = solidcraft_engine::sketch::default_text(&frame, side.center, unit)
            {
                return Some(offset + offset.normalized().unwrap_or(Vec2::Y) * (14.0 * unit));
            }
            let offset = solidcraft_engine::sketch::default_text_offset(&frame, unit);
            // Keep the native 44-point label target clear of the edge's own
            // selection tolerance; showing a helper must not steal geometry drags.
            let text = offset + offset.normalized().unwrap_or(Vec2::Y) * (14.0 * unit);
            // A reference or partial dimension can coexist with a free size.
            // Put the new helper on the opposite side to keep both numbers usable.
            Some(if snapshot.sketch.constraints.iter().any(|c| dimension_on_curve(&snapshot.sketch, &c.kind, index)) { -text } else { text })
        })
    }

    pub(crate) fn sketch_annotations(&self) -> Value {
        let Some(snapshot) = self.display_model().and_then(|m| m.sketch.as_ref()) else {
            return json!([]);
        };
        let unit = 2.0 * self.camera.half_height() / self.metrics.height_points.max(1.0);
        let sk = &snapshot.sketch;
        let present = |id: &str, kind: &ConstraintKind, text, entity: Sel, dimension: Option<&str>, driven, selected| {
            let frame = solidcraft_engine::sketch::dim_frame(sk, kind)?;
            let layout = solidcraft_engine::sketch::dim_layout(&frame, text, unit, Vec2::ZERO);
            let position = crate::picking::project(self.vp(), snapshot.plane.to_world(layout.text), self.metrics.pixels())?;
            let value = solidcraft_engine::sketch::measure_dimension(sk, kind)?;
            Some(
                json!({"id":id,"points":[position[0]/self.metrics.scale+self.metrics.origin_points[0],position[1]/self.metrics.scale+self.metrics.origin_points[1]],
                "value":if kind.is_angle(){value.to_degrees()}else{value},"units":if kind.is_angle(){"deg"}else{"mm"},"driven":driven,
                "selected":selected,"entity":entity,"dimension_type":dimension}),
            )
        };
        let selected_curve = match self.selection.as_slice() {
            [Sel::SketchCurve { id, sketch }] if sketch.is_none_or(|id| id == snapshot.id) => sk.curve_index(id),
            _ => None,
        };
        let mut annotations: Vec<_> = sk
            .constraints
            .iter()
            .filter_map(|c| {
                let selected = self.selection.iter().any(|s| matches!(s, Sel::SketchConstraint { id } if id == &c.id))
                    || selected_curve.is_some_and(|index| dimension_on_curve(sk, &c.kind, index));
                present(&c.id, &c.kind, c.text, Sel::SketchConstraint { id: c.id.clone() }, None, c.driven, selected)
            })
            .collect();
        // Free sizes are presentation-only measurements until Apply.
        // The descriptor uses upstream measurement/layout; only sketch.dimension
        // creates a constraint, through the existing numeric transaction.
        for (index, kind, dimension) in self.measurements(snapshot) {
            if let Some(curve) = sk.curves.get(index)
                && let Some(annotation) = present(
                    &format!("edge:{}", curve.id),
                    &kind,
                    self.measurement_text(snapshot, &curve.id, &kind),
                    Sel::SketchCurve { id: curve.id.clone(), sketch: Some(snapshot.id) },
                    Some(dimension),
                    false,
                    selected_curve == Some(index),
                )
            {
                annotations.push(annotation);
            }
        }
        json!(annotations)
    }

    fn visible_glyphs(&self) -> Vec<(String, String, Vec2)> {
        let Some(snapshot) = self.display_model().and_then(|m| m.sketch.as_ref()) else {
            return Vec::new();
        };
        let unit = 2.0 * self.camera.half_height() / self.metrics.height_points.max(1.0);
        let mut placed: Vec<(String, String, Vec2)> = Vec::new();
        for glyph in &snapshot.glyphs {
            if glyph["dimension"].as_bool() == Some(true) {
                continue;
            }
            let Some(id) = glyph["id"].as_str() else {
                continue;
            };
            let Some(constraint) = snapshot.sketch.constraints.iter().find(|c| c.id == id) else {
                continue;
            };
            let points = snapshot.sketch.constraint_points(&constraint.kind);
            let interested = self.selection.iter().any(|selection| match selection {
                Sel::SketchConstraint { id: chosen } => chosen == id,
                Sel::SketchPoint { id } => snapshot.sketch.resolve_point(id).is_some_and(|p| points.contains(&p)),
                Sel::SketchCurve { id, .. } => snapshot
                    .sketch
                    .curve_index(id)
                    .and_then(|c| snapshot.sketch.curves.get(c))
                    .is_some_and(|c| c.kind.point_ids().iter().any(|p| points.contains(p))),
                _ => false,
            });
            if !interested {
                continue;
            }
            let Ok(anchor) = serde_json::from_value::<Vec2>(glyph["at"].clone()) else {
                continue;
            };
            let mut at = anchor + Vec2::new(0.0, unit * 18.0);
            // A bounded screen-space lane keeps neighbouring indicators apart. The
            // inspector remains the complete list, including very dense sketches.
            for n in 0..8 {
                let candidate = anchor + Vec2::new((n % 4) as f64 * unit * 20.0, (n / 4 + 1) as f64 * unit * 20.0);
                if !placed.iter().any(|(_, _, p)| p.dist(candidate) < unit * 18.0) {
                    at = candidate;
                    break;
                }
            }
            placed.push((id.into(), glyph["name"].as_str().unwrap_or("").into(), at));
            if placed.len() >= 32 {
                break;
            }
        }
        placed
    }

    pub(crate) fn sketch_constraint_indicators(&self) -> Value {
        let Some(snapshot) = self.display_model().and_then(|m| m.sketch.as_ref()) else {
            return json!([]);
        };
        json!(self.visible_glyphs().into_iter().filter_map(|(id,name,at)|{
            let position=crate::picking::project(self.vp(),snapshot.plane.to_world(at),self.metrics.pixels())?;
            Some(json!({"id":id,"name":name,"points":[position[0]/self.metrics.scale+self.metrics.origin_points[0],position[1]/self.metrics.scale+self.metrics.origin_points[1]],
                "selected":self.selection.iter().any(|s|matches!(s,Sel::SketchConstraint{id:chosen} if chosen==&id))}))
        }).collect::<Vec<_>>())
    }
}
