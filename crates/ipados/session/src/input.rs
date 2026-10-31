//! Source-aware interaction ownership. UIKit sends values, never retained touches.
//! All camera changes delegate to the existing engine navigation implementation.
use std::collections::BTreeMap;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use solidcraft_engine::{Sel, geom::Vec3};

use crate::{AdapterError, PreparedTool, RadiusHandle, SceneSnapshot, ToolEvent, ToolOperation, ToolPhase, ToolWork, Viewport, error};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputSource {
    Pencil,
    Finger,
    Indirect,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum InputPhase {
    Began,
    Moved,
    Ended,
    Cancelled,
    Hover,
    Scroll,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InputEvent {
    pub source: InputSource,
    pub phase: InputPhase,
    pub id: u64,
    pub x_points: f64,
    pub y_points: f64,
    pub x_pixels: f64,
    pub y_pixels: f64,
    pub scale: f64,
    pub timestamp: f64,
    pub pressure: Option<f64>,
    pub altitude: Option<f64>,
    #[serde(default)]
    pub azimuth: Option<f64>,
    #[serde(default)]
    pub predicted: bool,
    #[serde(default)]
    pub display_preview: bool,
    /// Host sequence breaks equal-timestamp ties. Zero is reserved for legacy replay.
    #[serde(default)]
    pub sequence: u64,
    #[serde(default)]
    pub viewport_epoch: Option<u64>,
    #[serde(default)]
    pub camera_epoch: Option<u64>,
    #[serde(default)]
    pub document_id: Option<String>,
    #[serde(default)]
    pub geometry_revision: Option<u64>,
    #[serde(default)]
    pub consumed: bool,
    /// Native label bounds can exceed the fallback point-based hit target.
    #[serde(default)]
    pub sketch_annotation: Option<String>,
    #[serde(default)]
    pub shift: bool,
    /// UIKit click/tap count; zero preserves legacy single-pick replay.
    #[serde(default)]
    pub tap_count: u32,
    /// 0 primary, 1 secondary orbit, 2 middle pan.
    #[serde(default)]
    pub button: u8,
    #[serde(default)]
    pub scroll: f64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Owner {
    Ui,
    Navigation,
    PencilCandidate,
    Tool,
    SketchTool,
    Pointer,
    ModelDismiss,
    Suppressed,
}
struct Contact {
    source: InputSource,
    owner: Owner,
    start: [f64; 2],
    current: [f64; 2],
    began: f64,
    timestamp: f64,
    moved: bool,
    button: u8,
    revision: Option<u64>,
    model_token: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EditState {
    Candidate,
    Dragging,
    Suspended,
    Rebase,
    Committing,
}
struct Drag {
    contact: u64,
    work: ToolWork,
    state: EditState,
    anchor: Vec3,
    anchor_radius: f64,
    preview_version: Option<u64>,
}

#[derive(Default)]
pub(crate) struct Contacts {
    items: BTreeMap<u64, Contact>,
    pub(crate) navigating: bool,
    orbit_pivot: Option<Vec3>,
    twist_snap_offset: f64,
    twist_snapped: bool,
    sequence: u64,
    pub(crate) next_interaction: u64,
    drag: Option<Drag>,
    pub(crate) pending: Option<ToolWork>,
    pub(crate) sketch: crate::sketch_input::SketchInput,
    pub(crate) previous_sketch_tool: String,
    pub preview: Option<Arc<SceneSnapshot>>,
    pub tool_event: Option<ToolEvent>,
    pub tool_revision: u64,
    pub radius_tool: bool,
    dismissed_model_token: Option<u64>,
}

impl Contacts {
    fn fingers(&self) -> Vec<(u64, [f64; 2])> {
        self.items.iter().filter(|(_, c)| c.owner == Owner::Navigation).map(|(id, c)| (*id, c.current)).collect()
    }
    fn event(&mut self, phase: ToolPhase, point: Option<Vec3>) {
        if let Some(d) = &self.drag {
            self.tool_event = Some(ToolEvent { interaction: d.work.interaction, phase, point });
            self.tool_revision = self.tool_revision.saturating_add(1);
        }
    }
    fn cancel_edit(&mut self) {
        self.event(ToolPhase::Cancel, None);
        self.drag = None;
        self.pending = None;
        self.preview = None;
        self.sketch.cancel();
    }
    pub fn clear(&mut self) {
        self.cancel_edit();
        self.items.clear();
        self.navigating = false;
        self.orbit_pivot = None;
        self.twist_snap_offset = 0.0;
        self.twist_snapped = false;
    }
    fn rebase_fingers(&mut self) {
        // Count transitions have no camera delta; subsequent movement starts here.
        self.orbit_pivot = None;
        self.twist_snap_offset = 0.0;
        self.twist_snapped = false;
        for c in self.items.values_mut().filter(|c| c.owner == Owner::Navigation) {
            c.start = c.current;
        }
    }
    fn suspend(&mut self) {
        self.sketch.cancel();
        self.pending = None;
        self.preview = None;
        // Camera movement invalidates a stationary selection candidate as well:
        // lifting later must not pick whatever the camera moved underneath it.
        for c in self.items.values_mut() {
            c.model_token = None;
            if matches!(c.owner, Owner::PencilCandidate | Owner::ModelDismiss) {
                c.moved = true;
            }
        }
        if let Some(d) = &mut self.drag {
            if d.state == EditState::Committing {
                self.cancel_edit();
                return;
            }
            d.state = EditState::Suspended;
            d.work.version = d.work.version.saturating_add(1);
            d.preview_version = Some(d.work.version);
            self.pending = None;
            self.event(ToolPhase::Suspend, None);
        }
    }
    fn finish_navigation(&mut self) {
        if self.navigating {
            self.navigating = false;
            if let Some(d) = &mut self.drag
                && d.state == EditState::Suspended
            {
                d.state = EditState::Rebase;
            }
        }
    }
    pub fn diagnostic(&self) -> Value {
        json!({"contacts":self.items.len(),"ui_owned":self.items.values().filter(|c|c.owner==Owner::Ui).count(),
            "navigation":self.navigating,"edit":self.drag.as_ref().map(|d|d.state),
            "radius":self.drag.as_ref().and_then(|d|match &d.work.operation {ToolOperation::Radius{radius,..}=>Some(*radius),_=>None}),"tool_pending":self.pending.is_some(),
            "tool":if self.sketch.active() {self.sketch.tool.as_str()} else if self.radius_tool {"post_radius"} else {"select"},"tool_event":self.tool_event,
            "sketch":self.sketch.diagnostic(),"dismissed_model_token":self.dismissed_model_token})
    }
}

impl Viewport {
    fn snapped_twist(&mut self, angle: f64) -> f64 {
        let plane = self.plane_preview.or_else(|| self.model.as_ref().and_then(|model| model.sketch_plane));
        let Some(plane) = plane.filter(|_| self.planar && self.sketch_rotation_snap) else {
            self.contacts.twist_snap_offset = 0.0;
            self.contacts.twist_snapped = false;
            return angle;
        };
        if angle.abs() < 1e-12 {
            return angle;
        }
        let (right, up, _) = self.camera.basis();
        // Measure clockwise screen rotation from the sketch's own X axis, not
        // world yaw/roll: face-attached and rotated planes need the same detents.
        let current = -plane.x.dot(up).atan2(plane.x.dot(right));
        let raw = current + self.contacts.twist_snap_offset + angle;
        let nearest = (raw / std::f64::consts::FRAC_PI_4).round() * std::f64::consts::FRAC_PI_4;
        // A wider release band prevents jitter. Retain the unsnapped remainder
        // so many small samples can leave a detent and reverse without drift.
        let tolerance = if self.contacts.twist_snapped { 5.0_f64 } else { 3.0_f64 }.to_radians();
        self.contacts.twist_snapped = (raw - nearest).abs() <= tolerance;
        let target = if self.contacts.twist_snapped { nearest } else { raw };
        self.contacts.twist_snap_offset = raw - target;
        let difference = target - current;
        difference.sin().atan2(difference.cos())
    }
}

impl InputEvent {
    pub fn validate(&self) -> Result<(), AdapterError> {
        if ![self.x_points, self.y_points, self.x_pixels, self.y_pixels, self.scale, self.timestamp, self.scroll].iter().all(|v| v.is_finite())
            || !(0.1..=8.0).contains(&self.scale)
            || self.timestamp < 0.0
            || self.x_points.abs() > 1e6
            || self.y_points.abs() > 1e6
            || self.scroll.abs() > 1e4
            || self.pressure.is_some_and(|p| !p.is_finite() || !(0.0..=1.0).contains(&p))
            || self.altitude.is_some_and(|a| !a.is_finite() || !(0.0..=std::f64::consts::FRAC_PI_2).contains(&a))
            || self.azimuth.is_some_and(|a| !a.is_finite() || a.abs() > std::f64::consts::TAU)
            || self.document_id.as_ref().is_some_and(|id| id.len() > 128)
            || self.sketch_annotation.as_ref().is_some_and(|id| id.len() > 256)
            || (self.x_points * self.scale - self.x_pixels).abs() > 0.01
            || (self.y_points * self.scale - self.y_pixels).abs() > 0.01
            || (self.id == 0 && matches!(self.phase, InputPhase::Began | InputPhase::Moved | InputPhase::Ended))
            || self.button > 2
        {
            return Err(error("input", "invalid input coordinates or device values"));
        }
        Ok(())
    }
}

impl Viewport {
    /// Choose an object centre once per one-finger gesture, independent of the
    /// panned camera target and selection filter. Camera maths remains shared.
    fn finger_orbit_pivot(&self, start: [f64; 2]) -> Vec3 {
        let Some(model) = self.display_model() else { return self.camera.target };
        let vp = self.vp();
        if let Some(pixel) = self.metrics.pixel(start)
            && let Some(Sel::Body { name }) = model.picks.pick(&self.camera, vp, self.metrics, pixel, crate::PickFilter::Body)
            && let Some(body) = model.picks.bodies.iter().find(|body| body.name == name)
        {
            return body.mesh.bounds().center();
        }
        // Empty-space drags use the bodies in view, excluding distant offscreen
        // geometry that could otherwise make the visible object circle its pivot.
        let [width, height] = self.metrics.pixels();
        let bounds = model
            .picks
            .bodies
            .iter()
            .map(|body| body.mesh.bounds())
            .filter(|bounds| {
                crate::picking::project(vp, bounds.center(), [width, height])
                    .is_some_and(|p| (0.0..width).contains(&p[0]) && (0.0..height).contains(&p[1]))
            })
            .fold(solidcraft_engine::geom::Aabb3::EMPTY, |bounds, body| bounds.union(&body));
        if !bounds.is_empty() {
            bounds.center()
        } else if !model.bounds.is_empty() {
            model.bounds.center()
        } else {
            self.camera.target
        }
    }

    pub fn input(&mut self, e: InputEvent) -> Result<Value, AdapterError> {
        if let Err(failure) = e.validate() {
            self.cancel_interactions();
            return Err(failure);
        }
        let source = e.source;
        let phase = e.phase;
        let handled = self.route(e)?;
        let mut reply = self.diagnostic();
        reply["source"] = json!(source);
        reply["phase"] = json!(phase);
        reply["handled"] = json!(handled);
        Ok(reply)
    }

    pub fn cancel_interactions(&mut self) {
        self.contacts.clear();
        self.hover = None;
        self.changed();
    }

    pub fn take_tool_work(&mut self) -> Option<ToolWork> {
        self.contacts.pending.take()
    }
    pub fn tool_is_current(&self, work: &ToolWork) -> bool {
        !self.updating
            && self.model.as_ref().is_some_and(|m| m.document_id == work.document_id && m.geometry_revision == work.geometry_revision)
            && match &work.operation {
                ToolOperation::Modelling { token, params, .. } => self.modelling.as_ref().is_some_and(|e| e.token == *token && e.params == *params),
                _ => true,
            }
            && (self.contacts.sketch.current(work)
                || self.contacts.drag.as_ref().is_some_and(|d| {
                    d.work.interaction == work.interaction
                        && d.work.version == work.version
                        && matches!(d.state, EditState::Dragging | EditState::Committing)
                        && (d.state == EditState::Committing) == work.commit
                }))
    }
    pub fn publish_preview(&mut self, work: &ToolWork, model: Arc<SceneSnapshot>) -> bool {
        if !self.tool_is_current(work) || work.commit {
            return false;
        }
        if let ToolOperation::Modelling { params, .. } = &work.operation {
            if let Some(edit) = &mut self.modelling {
                edit.params = params.clone();
                edit.preview = Some(model);
                edit.last_preview = None;
            }
            if let Some(drag) = &mut self.contacts.drag {
                drag.preview_version = Some(work.version);
            }
            if self.contacts.drag.as_ref().is_some_and(|d| !self.contacts.items.contains_key(&d.contact)) {
                self.contacts.drag = None;
            }
        } else {
            self.contacts.preview = Some(model);
        }
        self.changed();
        true
    }
    /// Show completed geometry during continuous movement without validating an
    /// older proposal or moving the handle back from the latest actual sample.
    pub fn publish_model_progress(&mut self, work: &ToolWork, model: Arc<SceneSnapshot>) -> bool {
        let ToolOperation::Modelling { token, .. } = &work.operation else { return false };
        let Some(drag) = &mut self.contacts.drag else { return false };
        if self.updating
            || work.commit
            || drag.state != EditState::Dragging
            || drag.work.interaction != work.interaction
            || work.version >= drag.work.version
            || drag.preview_version.is_some_and(|version| work.version <= version)
            || !self.contacts.items.contains_key(&drag.contact)
            || !self.model.as_ref().is_some_and(|m| m.document_id == work.document_id && m.geometry_revision == work.geometry_revision)
        {
            return false;
        }
        let Some(edit) = self.modelling.as_mut().filter(|edit| edit.token == *token && edit.preview.is_none()) else { return false };
        edit.last_preview = Some(model);
        drag.preview_version = Some(work.version);
        self.changed();
        true
    }
    pub fn publish_model_tool_details(
        &mut self,
        work: &ToolWork,
        reply: Value,
        feedback: Vec<String>,
        handle: Option<crate::modelling::ModelHandle>,
    ) {
        if !self.tool_is_current(work) {
            return;
        }
        if matches!(work.operation, ToolOperation::Modelling { .. })
            && let Some(edit) = &mut self.modelling
        {
            edit.reply = reply;
            edit.feedback = feedback;
            if handle.is_some() {
                edit.handle = handle;
            }
        }
    }
    /// Publish one evaluated proposal, including inferred form values, before
    /// returning to the host. A final preview retires its drag, so its parameters
    /// must not be checked against that retired work a second time.
    pub fn publish_tool_preview(&mut self, prepared: PreparedTool) -> bool {
        let work = &prepared.work;
        if !self.tool_is_current(work) || work.commit {
            return false;
        }
        self.publish_model_tool_details(work, prepared.reply, prepared.feedback, prepared.handle);
        if !self.publish_preview(work, prepared.model) {
            return false;
        }
        if let Some(params) = prepared.model_params {
            if let Some(edit) = &mut self.modelling {
                edit.params = params.clone();
            }
            if let Some(drag) = &mut self.contacts.drag
                && let ToolOperation::Modelling { params: current, .. } = &mut drag.work.operation
            {
                *current = params;
            }
        }
        true
    }
    pub fn complete_tool(&mut self, work: &ToolWork, model: Arc<SceneSnapshot>) -> bool {
        if !self.tool_is_current(work) || !work.commit {
            return false;
        }
        let created_dimension = match &work.operation {
            ToolOperation::Sketch { commands, .. } if commands.iter().any(|(id, _)| id == "sketch.dimension") => {
                model.sketch.as_ref().and_then(|new| {
                    new.sketch
                        .constraints
                        .iter()
                        .find(|c| {
                            c.kind.is_dimension()
                                && self
                                    .model
                                    .as_ref()
                                    .and_then(|m| m.sketch.as_ref())
                                    .is_none_or(|old| !old.sketch.constraints.iter().any(|a| a.id == c.id))
                        })
                        .map(|c| c.id.clone())
                })
            }
            _ => None,
        };
        self.contacts.preview = None;
        self.contacts.drag = None;
        self.contacts.pending = None;
        self.contacts.sketch.complete(work, &model);
        self.selection.clear();
        if let Some(id) = created_dimension {
            self.selection.push(Sel::SketchConstraint { id });
        }
        self.hover = None;
        self.model = Some(model);
        self.changed();
        true
    }
    pub fn fail_tool(&mut self, work: &ToolWork) {
        if self.tool_is_current(work) {
            if let ToolOperation::Modelling { .. } = &work.operation
                && let Some(edit) = &mut self.modelling
            {
                edit.preview = None;
            }
            // A failed preview is still provisional. Preserve the last valid display
            // and contact anchors so the next actual sample can recover normally.
            if work.commit {
                self.contacts.cancel_edit();
            }
            self.changed();
        }
    }
    pub(crate) fn handle(&self) -> Option<&RadiusHandle> {
        self.model.as_ref()?.radius_handle.as_ref().filter(|_| self.contacts.radius_tool)
    }
    pub(crate) fn handle_point(&self) -> Option<Vec3> {
        if let Some(h) = self.modelling.as_ref().and_then(|e| e.handle.as_ref()) {
            return Some(h.point());
        }
        let h = self.handle()?;
        Some(h.point(self.numeric.as_ref().map_or_else(
            || {
                self.contacts.drag.as_ref().map_or(h.radius, |d| match &d.work.operation {
                    ToolOperation::Radius { radius, .. } => *radius,
                    _ => h.radius,
                })
            },
            |e| e.value,
        )))
    }
    pub(crate) fn plane_point(&self, point: [f64; 2], plane: solidcraft_engine::geom::Plane) -> Option<Vec3> {
        let p = self.metrics.pixel(point)?;
        let [w, h] = self.metrics.pixels();
        let (o, d) = self.camera.ray(p[0], p[1], w, h);
        if d.dot(plane.normal()).abs() < 1e-5 {
            return None;
        }
        plane.intersect_ray(o, d).filter(|p| p.is_finite() && p.len() < 1e9)
    }
    fn begin_tool(&mut self, e: &InputEvent, point: [f64; 2]) -> bool {
        if self
            .contacts
            .drag
            .as_ref()
            .is_some_and(|d| self.contacts.items.contains_key(&d.contact) || !matches!(d.work.operation, ToolOperation::Modelling { .. }))
        {
            return false;
        }
        if let Some(edit) = self.modelling.as_ref()
            && let Some(h) = edit.handle.clone()
        {
            let Some(p) = crate::picking::project(self.vp(), h.point(), self.metrics.pixels()) else {
                return false;
            };
            let Some(q) = self.metrics.pixel(point) else {
                return false;
            };
            if (p[0] - q[0]).hypot(p[1] - q[1]) / self.metrics.scale > if self.direct_handles { 24. } else { 16. } {
                return false;
            }
            let Some(plane) = h.interaction_plane(self.camera.back()) else {
                return false;
            };
            let Some(anchor) = self.plane_point(point, plane) else {
                return false;
            };
            let Some(model) = &self.model else {
                return false;
            };
            let Some(interaction) = self.contacts.next_interaction.checked_add(1) else {
                return false;
            };
            self.contacts.next_interaction = interaction;
            let work = ToolWork {
                interaction,
                version: 0,
                document_id: model.document_id.clone(),
                geometry_revision: model.geometry_revision,
                operation: ToolOperation::Modelling {
                    token: edit.token,
                    operation: edit.operation.clone(),
                    feature: edit.feature,
                    params: edit.params.clone(),
                },
                commit: false,
            };
            // A completed modelling contact may still have a preview in flight.
            // The new interaction supersedes that ticket without waiting for CAD.
            self.contacts.pending = None;
            self.contacts.drag =
                Some(Drag { contact: e.id, anchor, anchor_radius: h.distance, work, state: EditState::Candidate, preview_version: None });
            self.contacts.event(ToolPhase::Begin, Some(anchor));
            return true;
        }
        let Some(h) = self.handle().cloned() else { return false };
        let Some(p) = crate::picking::project(self.vp(), h.point(h.radius), self.metrics.pixels()) else { return false };
        let Some(q) = self.metrics.pixel(point) else { return false };
        if (p[0] - q[0]).hypot(p[1] - q[1]) / self.metrics.scale > 12.0 {
            return false;
        }
        let Some(anchor) = self.plane_point(point, h.plane) else { return false };
        let Some(model) = &self.model else { return false };
        let Some(interaction) = self.contacts.next_interaction.checked_add(1) else { return false };
        self.contacts.next_interaction = interaction;
        let work = ToolWork {
            interaction,
            version: 0,
            document_id: model.document_id.clone(),
            geometry_revision: model.geometry_revision,
            operation: ToolOperation::Radius { radius: h.radius, handle: h.clone() },
            commit: false,
        };
        self.select(Some(Sel::Body { name: h.body.clone() }), false);
        self.contacts.drag = Some(Drag { contact: e.id, anchor, anchor_radius: h.radius, work, state: EditState::Candidate, preview_version: None });
        self.contacts.event(ToolPhase::Begin, Some(anchor));
        if self.contacts.navigating {
            self.contacts.suspend();
        }
        true
    }
    fn update_tool(&mut self, e: &InputEvent, point: [f64; 2], moved: bool) {
        let Some(d) = &self.contacts.drag else { return };
        if d.contact != e.id {
            return;
        }
        if e.geometry_revision != Some(d.work.geometry_revision) {
            self.contacts.cancel_edit();
            return;
        }
        if self.metrics.pixel(point).is_none() || self.updating {
            self.contacts.cancel_edit();
            return;
        }
        let state = d.state;
        if e.camera_epoch.is_some_and(|epoch| epoch != self.camera_epoch) {
            return;
        }
        if state == EditState::Suspended || state == EditState::Committing {
            return;
        }
        let (plane, factor) = match &d.work.operation {
            ToolOperation::Radius { handle, .. } => (handle.plane, 1.),
            ToolOperation::Modelling { .. } => {
                let Some(h) = self.modelling.as_ref().and_then(|e| e.handle.as_ref()) else {
                    return;
                };
                let Some(plane) = h.interaction_plane(self.camera.back()) else {
                    return;
                };
                (plane, h.factor)
            }
            _ => return,
        };
        let Some(world) = self.plane_point(point, plane) else {
            self.contacts.cancel_edit();
            return;
        };
        if let Some(d) = &mut self.contacts.drag {
            if state == EditState::Rebase {
                d.anchor = world;
                if let ToolOperation::Radius { radius, .. } = &d.work.operation {
                    d.anchor_radius = *radius;
                }
                if let ToolOperation::Modelling { .. } = &d.work.operation
                    && let Some(h) = self.modelling.as_ref().and_then(|e| e.handle.as_ref())
                {
                    d.anchor_radius = h.distance;
                }
                d.state = EditState::Dragging;
                self.contacts.event(ToolPhase::Resume, Some(world));
                return;
            }
            if !moved {
                return;
            }
            let radius = self
                .modelling
                .as_ref()
                .and_then(|e| e.handle.as_ref())
                .map_or_else(|| d.anchor_radius + (world - d.anchor).dot(plane.x) / factor, |h| h.proposal(d.anchor, world, d.anchor_radius));
            // Quantise the proposed parameter, never the contact position. Keep
            // the original anchor so sub-step motion accumulates without drift.
            let linear = self.modelling.as_ref().and_then(|edit| edit.handle.as_ref()).is_none_or(|handle| !handle.angular && handle.units == "mm");
            let radius = if linear && self.drag_step_mm > 0. {
                let scale = 1. / self.drag_step_mm;
                (radius * scale).round() / scale
            } else {
                radius
            };
            match &mut d.work.operation {
                ToolOperation::Radius { radius: current, .. } => {
                    let radius = radius.clamp(0.5, 50.);
                    if (radius - *current).abs() < 1e-6 {
                        return;
                    }
                    *current = radius;
                }
                ToolOperation::Modelling { params, .. } => {
                    if !radius.is_finite() {
                        return;
                    }
                    let Some(handle) = self.modelling.as_ref().and_then(|e| e.handle.as_ref()) else {
                        return;
                    };
                    if (radius - handle.distance).abs() < 1e-6 {
                        return;
                    }
                    handle.set_parameter(params, radius);
                    if self.automatic_subtract && params.get("_auto_subtract").is_none() && params["operation"] == "new" {
                        params["_auto_subtract"] = json!(true);
                    }
                    if handle.angular {
                        d.anchor = world;
                        d.anchor_radius = radius;
                    }
                    if let Some(edit) = &mut self.modelling {
                        edit.params = params.clone();
                        edit.version = edit.version.saturating_add(1);
                        // Keep the last validated pose on screen while the bounded
                        // worker evaluates this newer proposal. It is display-only:
                        // preview=None still prevents Apply for the pending value.
                        if let Some(previous) = edit.preview.take() {
                            edit.last_preview = Some(previous);
                        }
                        if let Some(h) = &mut edit.handle {
                            h.distance = radius;
                        }
                    }
                }
                _ => return,
            }
            d.work.version = d.work.version.saturating_add(1);
            d.state = EditState::Dragging;
            self.contacts.pending = Some(d.work.clone());
            self.contacts.event(ToolPhase::Update, Some(world));
        }
    }
    fn end_tool(&mut self, e: &InputEvent) {
        let Some(d) = &mut self.contacts.drag else { return };
        if d.contact != e.id {
            return;
        }
        if matches!(d.work.operation, ToolOperation::Modelling { .. }) {
            if e.phase == InputPhase::Cancelled || matches!(d.state, EditState::Suspended | EditState::Rebase) {
                if let Some(edit) = &mut self.modelling {
                    edit.preview = None;
                    edit.last_preview = None;
                }
                self.contacts.cancel_edit();
            } else if d.state == EditState::Dragging {
                // Ending a handle only finalises its proposal. Apply still requires
                // the validated form and accepts one authoritative transaction.
                d.work.version = d.work.version.saturating_add(1);
                self.contacts.pending = Some(d.work.clone());
            } else {
                // Tapping the handle creates no proposal and must release capture.
                self.contacts.cancel_edit();
            }
            return;
        }
        let unchanged = match &d.work.operation {
            ToolOperation::Radius { handle, radius } => (radius - handle.radius).abs() < 1e-6,
            _ => true,
        };
        if e.phase == InputPhase::Cancelled || matches!(d.state, EditState::Suspended | EditState::Rebase) || unchanged {
            self.contacts.cancel_edit();
            return;
        }
        if d.state == EditState::Dragging {
            d.state = EditState::Committing;
            d.work.commit = true;
            d.work.version = d.work.version.saturating_add(1);
            self.contacts.pending = Some(d.work.clone());
            self.contacts.event(ToolPhase::Commit, None);
        } else {
            self.contacts.cancel_edit();
        }
    }

    fn route(&mut self, e: InputEvent) -> Result<bool, AdapterError> {
        // Prediction is optional visual data only. This host deliberately does not request
        // it: no prediction can advance contact anchors, intent, work tickets or history.
        if e.predicted {
            return Ok(false);
        }
        if e.sequence != 0 {
            if e.sequence <= self.contacts.sequence {
                return Ok(false);
            }
            self.contacts.sequence = e.sequence;
        }
        if e.phase == InputPhase::Cancelled && e.id == 0 {
            self.hover = None;
            self.changed();
            return Ok(true);
        }
        let point = [e.x_points, e.y_points];
        let context_ok = (e.scale - self.metrics.scale).abs() < 1e-6
            && e.viewport_epoch.is_none_or(|v| v == self.metrics_epoch)
            && e.document_id.as_ref().is_none_or(|id| self.model.as_ref().is_some_and(|m| &m.document_id == id));
        if !context_ok {
            self.cancel_interactions();
            return Ok(false);
        }
        let camera_current = e.camera_epoch.is_none_or(|epoch| epoch == self.camera_epoch);
        if e.phase == InputPhase::Hover {
            if !camera_current || e.display_preview {
                return Ok(false);
            }
            if self.contacts.drag.is_none() && !self.contacts.navigating && matches!(e.source, InputSource::Pencil | InputSource::Indirect) {
                if self.projection.is_some() {
                    if e.geometry_revision == self.model.as_ref().map(|m| m.geometry_revision) {
                        self.projection_hover(point);
                    }
                } else {
                    self.hover = e.geometry_revision.and_then(|r| self.pick(point, Some(r)));
                }
                self.changed();
                return Ok(true);
            }
            return Ok(false);
        }
        if e.phase == InputPhase::Scroll && e.source == InputSource::Indirect && self.metrics.pixel(point).is_some() {
            self.contacts.cancel_edit();
            self.navigate(&json!({"motion":"zoom","factor":(-e.scroll*0.01).exp().clamp(0.05,20.0)}))?;
            self.changed();
            return Ok(true);
        }
        if e.phase == InputPhase::Began {
            if self.contacts.items.contains_key(&e.id) {
                return Ok(false);
            }
            if self.contacts.items.len() >= 16 {
                self.cancel_interactions();
                return Ok(false);
            }
            let valid = !self.numeric_active()
                && (!e.display_preview || self.contacts.sketch.active() || self.modelling.is_some())
                && camera_current
                && !self.updating
                && e.geometry_revision.is_some()
                && self.model.as_ref().is_some_and(|m| Some(m.geometry_revision) == e.geometry_revision);
            let mut owner = if e.consumed || self.metrics.pixel(point).is_none() {
                Owner::Ui
            } else {
                match e.source {
                    InputSource::Finger => Owner::Navigation,
                    InputSource::Pencil if valid => Owner::PencilCandidate,
                    InputSource::Indirect
                        if (!self.numeric_active()
                            && (!e.display_preview
                                || valid && self.modelling.is_some() && (self.direct_handles || self.cancel_tools_on_dismiss)))
                            || e.button != 0 =>
                    {
                        Owner::Pointer
                    }
                    _ => Owner::Suppressed,
                }
            };
            if owner == Owner::PencilCandidate && self.contacts.items.values().any(|c| c.source == InputSource::Pencil && c.owner != Owner::Ui) {
                owner = Owner::Suppressed;
            }
            let direct = self.direct_handles
                && valid
                && self.modelling.as_ref().is_some_and(|edit| edit.handle.is_some())
                && (owner == Owner::Navigation && self.contacts.items.is_empty() || owner == Owner::Pointer && e.button == 0);
            if (owner == Owner::PencilCandidate || direct) && self.begin_tool(&e, point) {
                owner = Owner::Tool;
            }
            // A validated provisional solid has different topology from the
            // source document. The selection action reveals that source first;
            // never pick invisible source faces through a displayed preview.
            if matches!(owner, Owner::PencilCandidate | Owner::Pointer)
                && e.display_preview
                && self.modelling.as_ref().is_some_and(|e| e.preview.is_some() || e.last_preview.is_some())
            {
                owner = if self.cancel_tools_on_dismiss { Owner::ModelDismiss } else { Owner::Suppressed };
            }
            // A second finger turns a direct geometry grab into ordinary two-finger
            // navigation. Reject any in-flight sketch preview before rebasing both.
            if owner == Owner::Navigation
                && let Some(contact) = self.contacts.sketch.geometry_contact()
                && self.contacts.items.get(&contact).is_some_and(|c| c.source == InputSource::Finger && c.owner == Owner::SketchTool)
            {
                self.cancel_sketch_input();
                if let Some(contact) = self.contacts.items.get_mut(&contact) {
                    contact.owner = Owner::Navigation;
                }
            }
            if valid
                && e.button == 0
                && self.contacts.items.is_empty()
                && matches!(owner, Owner::PencilCandidate | Owner::Navigation | Owner::Pointer)
                && self.projection.is_none()
                && self.begin_dimension_drag(&e, point)
            {
                owner = Owner::SketchTool;
            }
            if (owner == Owner::PencilCandidate
                || valid
                    && e.button == 0
                    && self.contacts.items.is_empty()
                    && matches!(owner, Owner::Navigation | Owner::Pointer)
                    && (self.contacts.sketch.tool == "sketch.delete" || self.contacts.sketch.tool == "select" && self.direct_handles))
                && self.begin_sketch_input(&e, point)
            {
                owner = Owner::SketchTool;
            }
            self.contacts.items.insert(
                e.id,
                Contact {
                    source: e.source,
                    owner,
                    start: point,
                    current: point,
                    began: e.timestamp,
                    timestamp: e.timestamp,
                    moved: owner == Owner::PencilCandidate && self.contacts.navigating,
                    button: e.button,
                    revision: e.geometry_revision,
                    model_token: if valid { self.modelling.as_ref().map(|edit| edit.token) } else { None },
                },
            );
            if owner == Owner::Navigation {
                self.contacts.rebase_fingers();
                if self.contacts.fingers().len() > 1 {
                    for contact in self.contacts.items.values_mut().filter(|c| c.owner == Owner::Navigation) {
                        contact.moved = true;
                    }
                }
            }
            self.hover = None;
            self.changed();
            return Ok(!matches!(owner, Owner::Ui | Owner::Suppressed));
        }
        let Some(c) = self.contacts.items.get(&e.id) else { return Ok(false) };
        if c.source != e.source || e.timestamp < c.timestamp {
            self.cancel_interactions();
            return Err(error("input", "contact source changed or time reversed"));
        }
        let owner = c.owner;
        let old = c.current;
        let start = c.start;
        let button = c.button;
        let revision = c.revision;
        let model_token = c.model_token;
        let distance = (point[0] - c.start[0]).hypot(point[1] - c.start[1]);
        // Selection has no drag action. Tolerate small Pencil lift motion and
        // retain the target touched down on, as sketch selection already does.
        // Actual tool/sketch drags keep their more precise movement threshold.
        let pencil_tap = e.source == InputSource::Pencil && matches!(owner, Owner::PencilCandidate | Owner::ModelDismiss);
        let moved = c.moved || distance > if pencil_tap { 10.0 } else { 5.0 };
        let tap_point = if pencil_tap { start } else { point };
        let elapsed = e.timestamp - c.began;
        if matches!(owner, Owner::Tool | Owner::SketchTool) && self.metrics.pixel(point).is_none() {
            self.contacts.cancel_edit();
        }
        if let Some(c) = self.contacts.items.get_mut(&e.id) {
            c.current = point;
            c.timestamp = e.timestamp;
            c.moved = moved;
        }
        if matches!(e.phase, InputPhase::Ended | InputPhase::Cancelled) {
            // A handle gets first refusal for dragging, but a stationary Pencil
            // or pointer tap still selects the visible source underneath it.
            // Never pick source topology through an evaluated preview.
            let handle_tap = owner == Owner::Tool
                && !moved
                && !e.display_preview
                && matches!(e.source, InputSource::Pencil | InputSource::Indirect)
                && self.modelling.as_ref().is_some_and(|edit| edit.preview.is_none() && edit.last_preview.is_none())
                && self
                    .contacts
                    .drag
                    .as_ref()
                    .is_some_and(|drag| drag.state == EditState::Candidate && matches!(drag.work.operation, ToolOperation::Modelling { .. }));
            // End coordinates are actual samples too; never commit the last prediction.
            if owner == Owner::Tool {
                if !camera_current {
                    self.contacts.cancel_edit();
                }
                if e.phase == InputPhase::Ended
                    && self.contacts.drag.as_ref().is_some_and(|d| !matches!(d.state, EditState::Suspended | EditState::Rebase))
                {
                    self.update_tool(&e, point, moved);
                }
                self.end_tool(&e);
            }
            if owner == Owner::SketchTool {
                if !camera_current || revision != e.geometry_revision {
                    self.cancel_sketch_input();
                } else {
                    if e.phase == InputPhase::Ended {
                        self.update_sketch_input(&e, point, moved);
                    }
                    self.end_sketch_input(&e, moved);
                }
            }
            self.contacts.items.remove(&e.id);
            if owner == Owner::Navigation {
                self.contacts.rebase_fingers();
                if self.contacts.fingers().is_empty() {
                    self.contacts.finish_navigation();
                }
            }
            // A stationary primary tap on empty *displayed* space dismisses the
            // provisional form. Use body picking independent of the selection
            // filter, and the visible preview rather than hidden source faces.
            // Stale/consumed contacts and navigation never dismiss an edit.
            if self.cancel_tools_on_dismiss
                && self.model_active()
                && model_token.is_some()
                && model_token == self.modelling.as_ref().map(|edit| edit.token)
                && e.phase == InputPhase::Ended
                && !e.consumed
                && !moved
                && button == 0
                && (matches!(owner, Owner::Navigation | Owner::PencilCandidate | Owner::Pointer | Owner::ModelDismiss) || handle_tap)
                && camera_current
                && revision == e.geometry_revision
                && revision.is_some()
                && self.model.as_ref().is_some_and(|m| Some(m.geometry_revision) == revision)
                && !self.updating
                && !self.contacts.navigating
                && self.metrics.pixel(point).is_some()
                && let Some(pixel) = self.metrics.pixel(tap_point)
                && let Some(model) = self.display_model()
                && model.picks.pick(&self.camera, self.vp(), self.metrics, pixel, crate::PickFilter::Body).is_none()
                && model.picks.pick_profile(&self.camera, self.vp(), self.metrics, pixel).is_none()
                && self.pick_reference(tap_point).is_none()
            {
                let token = self.modelling.as_ref().map(|edit| edit.token);
                self.cancel_model();
                self.select(None, false);
                self.contacts.dismissed_model_token = token;
                self.changed();
                return Ok(true);
            }
            if e.phase == InputPhase::Ended
                && !moved
                && button == 0
                && (matches!(owner, Owner::PencilCandidate | Owner::Pointer)
                    || handle_tap
                    || (owner == Owner::Navigation
                        && (self.origin.choosing
                            || self.constraint_tool().is_some()
                            || self.projection.is_some()
                            || self.modelling.as_ref().is_some_and(|edit| edit.selection_field.is_some())
                            || self.pick_reference(point).is_some())))
                && camera_current
                && self.metrics.pixel(point).is_some()
                && revision == e.geometry_revision
                && !self.updating
                && !self.numeric_active()
                && revision.is_some()
                && self.model.as_ref().is_some_and(|m| Some(m.geometry_revision) == revision)
                && !self.contacts.navigating
            {
                if self.projection.is_some() {
                    self.projection_tap(tap_point);
                    self.changed();
                    return Ok(true);
                }
                let hit = self.pick(tap_point, revision);
                // Accumulate real picks, while an ordinary empty-space tap
                // clears the set without requiring a trip through Tools.
                let reattaching = self.modelling.as_ref().is_some_and(|edit| edit.operation == "sketch.redefine");
                let add = !reattaching && (e.shift || self.multiple && hit.is_some());
                if self.constraint_tool().is_some() {
                    // Ineligible geometry is still occupied space. Only a true
                    // miss dismisses the constraint tool.
                    self.constraint_pick(hit.or_else(|| self.pick_sketch_unfiltered(tap_point)))?;
                } else {
                    self.select_tap(hit, add, e.tap_count);
                }
            }
            self.changed();
            return Ok(!matches!(owner, Owner::Ui | Owner::Suppressed));
        }
        if e.phase != InputPhase::Moved {
            return Ok(false);
        }
        match owner {
            Owner::Tool => self.update_tool(&e, point, moved),
            Owner::SketchTool => {
                if !camera_current || revision != e.geometry_revision {
                    self.cancel_sketch_input();
                } else {
                    self.update_sketch_input(&e, point, moved);
                }
            }
            Owner::Navigation => {
                let fingers = self.contacts.fingers();
                let pencil = self.contacts.items.values().any(|c| c.source == InputSource::Pencil && c.owner != Owner::Ui);
                let threshold = if pencil && fingers.len() == 1 { 12.0 } else { 6.0 };
                let intentional = distance > threshold && (!pencil || fingers.len() == 2 || elapsed >= 0.08);
                let compatible = fingers.len() == 1
                    || (fingers.len() == 2 && (fingers[0].1[0] - fingers[1].1[0]).hypot(fingers[0].1[1] - fingers[1].1[1]) >= 24.0);
                if compatible && (self.contacts.navigating || intentional) {
                    if !self.contacts.navigating {
                        self.contacts.navigating = true;
                        self.contacts.suspend();
                    }
                    let delta = [point[0] - old[0], point[1] - old[1]];
                    if fingers.len() == 1 {
                        let mut params = json!({"motion":if self.planar {"pan"} else {"orbit"},"delta":delta,
                            "viewport":[self.metrics.width_points,self.metrics.height_points]});
                        if !self.planar && self.touch_orbit_follows_viewpoint {
                            let pivot = self.contacts.orbit_pivot.unwrap_or_else(|| self.finger_orbit_pivot(start));
                            self.contacts.orbit_pivot = Some(pivot);
                            // Keep the shared drag direction so the visible
                            // object follows the finger around its fixed centre.
                            params["free_orbit"] = json!(true);
                            params["pivot"] = json!(pivot);
                        }
                        self.navigate(&params)?;
                    } else if let Some((_, other)) = fingers.iter().find(|(id, _)| *id != e.id) {
                        self.navigate(&json!({"motion":"pan","delta":[delta[0]*0.5,delta[1]*0.5],"viewport":[self.metrics.width_points,self.metrics.height_points]}))?;
                        let before = (old[0] - other[0]).hypot(old[1] - other[1]);
                        let after = (point[0] - other[0]).hypot(point[1] - other[1]);
                        if before >= 24.0 && after >= 24.0 {
                            self.navigate(&json!({"motion":"zoom","factor":(before/after).clamp(0.05,20.0)}))?;
                            if self.two_finger_twist {
                                let previous_angle = (old[1] - other[1]).atan2(old[0] - other[0]);
                                let current_angle = (point[1] - other[1]).atan2(point[0] - other[0]);
                                let difference = current_angle - previous_angle;
                                // The shortest signed turn avoids a jump at ±pi.
                                let angle = self.snapped_twist(difference.sin().atan2(difference.cos()));
                                self.navigate(&json!({"motion":"roll","angle":angle,
                                    "anchor_points":[(point[0]+other[0])*0.5,(point[1]+other[1])*0.5]}))?;
                            }
                        }
                    }
                }
            }
            Owner::Pointer if button != 0 => {
                self.contacts.cancel_edit();
                let motion = if button == 2 || e.shift || self.planar { "pan" } else { "orbit" };
                self.navigate(&json!({"motion":motion,"delta":[point[0]-old[0],point[1]-old[1]],"viewport":[self.metrics.width_points,self.metrics.height_points]}))?;
            }
            _ => {}
        }
        self.changed();
        Ok(!matches!(owner, Owner::Ui | Owner::Suppressed))
    }
}
