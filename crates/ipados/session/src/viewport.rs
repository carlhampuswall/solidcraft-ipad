//! Display state independent of the engine worker. All navigation uses the shared
//! stateless `view.navigate` implementation; CAD geometry is immutable here.
use solidcore_record::{Plane, Vec3};
use solidcraft_engine::geom::prelude::*;
use std::{collections::BTreeSet, sync::Arc};

use serde_json::{Value, json};
use solidcraft_engine::{
    Sel,
    geom::{Aabb3, Mesh},
    view,
};
use solidcraft_render::{Camera, Mat4, Rgb, Scene, SceneLine, SceneMesh};

use crate::input::Contacts;
use crate::{AdapterError, PickFilter, SceneSnapshot, ViewportMetrics, error};

pub struct RenderSnapshot {
    pub model: Arc<SceneSnapshot>,
    pub background: Option<(Rgb, Rgb)>,
    pub sketch_color: Option<Rgb>,
    pub constrained_green: bool,
    pub construction_dashes: bool,
    pub camera: Camera,
    pub view_proj: Mat4,
    pub guides: Arc<Scene>,
    pub highlight: Arc<Scene>,
    pub scene_revision: u64,
    pub highlight_revision: u64,
    pub guide_revision: u64,
    pub geometry_revision: u64,
    pub selected_bodies: Vec<String>,
    pub hovered_bodies: Vec<String>,
    pub selection_tint: [f32; 3],
    pub hover_tint: [f32; 3],
}

impl RenderSnapshot {
    /// Dashed construction strokes live in the camera-dependent feedback layer.
    pub fn model_line_visible(&self, line: &SceneLine) -> bool {
        !self.construction_dashes || !crate::construction::construction_stroke(line)
    }

    /// Preserve solver-state colours when applying the native appearance.
    pub fn model_line_color(&self, color: Rgb) -> Rgb {
        crate::appearance::sketch_line_color(color, self.sketch_color, self.constrained_green)
    }
}

struct HighlightKey {
    model_handle: Option<crate::modelling::ModelHandle>,
    model: Option<Arc<SceneSnapshot>>,
    selection: Vec<Sel>,
    hover: Option<Sel>,
    camera: Option<Camera>,
    scale: f64,
    height: f64,
    tool_revision: u64,
}

#[derive(Clone, Copy, PartialEq)]
struct GridStyle {
    visible: bool,
    integrated_axes: bool,
    colors: Option<[Rgb; 2]>,
}

impl Default for GridStyle {
    fn default() -> Self {
        Self { visible: true, integrated_axes: true, colors: None }
    }
}

/// Native viewport interaction state. It owns no CAD session and can continue navigating
/// the last valid immutable scene while the authoritative engine is evaluating another.
pub struct Viewport {
    pub(crate) camera: Camera,
    pub(crate) metrics: ViewportMetrics,
    pub(crate) model: Option<Arc<SceneSnapshot>>,
    pub(crate) selection: Vec<Sel>,
    pub(crate) hover: Option<Sel>,
    pub(crate) filter: PickFilter,
    pub(crate) projection: Option<crate::sketch_projection::ProjectionPicking>,
    pub(crate) multiple: bool,
    pub(crate) contacts: Contacts,
    pub(crate) numeric: Option<crate::NumericEdit>,
    pub(crate) modelling: Option<crate::ModelEdit>,
    pub(crate) origin: crate::origin::OriginReferences,
    background: Option<Rgb>,
    sketch_color: Option<Rgb>,
    constrained_green: bool,
    pub(crate) construction_dashes: bool,
    grid: GridStyle,
    generation: u64,
    pub(crate) metrics_epoch: u64,
    pub(crate) camera_epoch: u64,
    pub(crate) planar: bool,
    pub(crate) touch_orbit_follows_viewpoint: bool,
    pub(crate) two_finger_twist: bool,
    pub(crate) sketch_rotation_snap: bool,
    pub(crate) direct_handles: bool,
    pub(crate) drag_step_mm: f64,
    pub(crate) automatic_subtract: bool,
    pub(crate) cancel_tools_on_dismiss: bool,
    pub(crate) plane_preview: Option<Plane>,
    pub(crate) plane_camera: Option<(Camera, bool)>,
    guide_plane: Option<Plane>,
    pub(crate) updating: bool,
    scene_revision: u64,
    highlight_revision: u64,
    highlight_key: Option<HighlightKey>,
    guide_revision: u64,
    guide_camera: Option<Camera>,
    guide_metrics: ViewportMetrics,
    guide_grid: GridStyle,
    guides: Arc<Scene>,
    highlight: Arc<Scene>,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            camera: Camera::default(),
            metrics: ViewportMetrics::default(),
            model: None,
            selection: Vec::new(),
            hover: None,
            filter: PickFilter::default(),
            projection: None,
            multiple: true,
            contacts: Contacts::default(),
            numeric: None,
            modelling: None,
            origin: crate::origin::OriginReferences::default(),
            background: None,
            sketch_color: None,
            constrained_green: true,
            construction_dashes: true,
            grid: GridStyle::default(),
            generation: 0,
            metrics_epoch: 0,
            camera_epoch: 0,
            planar: false,
            touch_orbit_follows_viewpoint: true,
            two_finger_twist: true,
            sketch_rotation_snap: true,
            direct_handles: true,
            drag_step_mm: 0.1,
            automatic_subtract: true,
            cancel_tools_on_dismiss: true,
            plane_preview: None,
            plane_camera: None,
            guide_plane: None,
            updating: false,
            scene_revision: 1,
            highlight_revision: 1,
            highlight_key: None,
            guide_revision: 1,
            guide_camera: None,
            guide_metrics: ViewportMetrics::default(),
            guide_grid: GridStyle::default(),
            guides: Arc::default(),
            highlight: Arc::default(),
        }
    }
}

impl Viewport {
    pub fn is_updating(&self) -> bool {
        self.updating
    }

    pub fn selection(&self) -> &[Sel] {
        &self.selection
    }

    pub(crate) fn is_selected(&self, item: &Sel) -> bool {
        self.selection.iter().any(|selected| same(selected, item))
    }

    /// Request generations are issued before evaluation, and checked at publication.
    /// Closing/replacing a session invalidates outstanding work without kernel preemption.
    pub fn begin_update(&mut self, generation: u64) {
        self.numeric = None;
        self.modelling = None;
        self.generation = generation;
        self.updating = true;
        self.hover = None;
        self.contacts.clear();
        self.changed();
    }

    pub fn install(&mut self, generation: u64, model: Arc<SceneSnapshot>) -> bool {
        if generation != self.generation {
            return false;
        }
        let first = self.model.is_none();
        if model.sketch.is_none() || self.model.as_ref().is_some_and(|old| old.document_id != model.document_id) {
            self.contacts.sketch.construction = false;
        }
        let changed = self.model.as_ref().is_none_or(|old| old.document_id != model.document_id || old.geometry_revision != model.geometry_revision);
        if changed {
            self.projection = None;
            // Shared topology indices and picked points have no persistent identity contract.
            self.selection.clear();
            self.hover = None;
        }
        if changed {
            self.contacts.clear();
            self.numeric = None;
            self.modelling = None;
        }
        if model.sketch.is_none()
            || self.constraint_tool().is_some()
                && self.model.as_ref().is_some_and(|old| {
                    old.document_id != model.document_id || old.sketch.as_ref().map(|s| s.id) != model.sketch.as_ref().map(|s| s.id)
                })
        {
            self.contacts.sketch.tool = "select".into();
        }
        let old_plane = self.model.as_ref().and_then(|m| m.sketch_plane);
        self.plane_preview = None;
        self.plane_camera = None;
        self.origin.set_choosing(false);
        if old_plane != model.sketch_plane {
            self.planar = model.sketch_plane.is_some();
            if let Some(plane) = model.sketch_plane {
                self.align_sketch_plane(plane);
            }
        }
        self.model = Some(model);
        self.origin.dirty = true;
        self.updating = false;
        if first {
            self.fit();
        }
        self.changed();
        true
    }

    /// A controlled failure keeps the last scene visible, but picking stays disabled until
    /// a current snapshot is published. Never present an old mesh as current geometry.
    pub fn fail_update(&mut self, generation: u64) {
        if generation == self.generation {
            self.updating = true;
            self.selection.clear();
            self.hover = None;
            self.changed();
        }
    }

    pub fn set_metrics(&mut self, metrics: ViewportMetrics) -> Result<(), AdapterError> {
        let initial = self.metrics.width_points <= 0.0 || self.metrics.height_points <= 0.0;
        let metrics = metrics.validate()?;
        if self.metrics == metrics {
            return Ok(());
        }
        let previous = self.metrics;
        self.metrics_epoch = self.metrics_epoch.saturating_add(1);
        self.metrics = metrics;
        if self.metrics.width_points > 0.0 && self.metrics.height_points > 0.0 {
            if initial {
                self.fit();
            } else {
                // Preserve framing against the shorter viewport axis as panels or
                // the keyboard change its aspect. Reverse the same correction on
                // expansion; layout alone never accumulates an extra model zoom.
                // Use the shared navigation path, including its supported bounds.
                let aspect = |m: ViewportMetrics| (m.width_points / m.height_points).clamp(0.01, 1.0);
                let mut factor = aspect(previous) / aspect(self.metrics);
                while !(0.05..=20.0).contains(&factor) {
                    let step = factor.clamp(0.05, 20.0);
                    self.navigate(&json!({"motion":"zoom","factor":step}))?;
                    factor /= step;
                }
                if (factor - 1.0).abs() > f64::EPSILON {
                    self.navigate(&json!({"motion":"zoom","factor":factor}))?;
                }
            }
        }
        self.contacts.clear();
        self.hover = None;
        self.changed();
        Ok(())
    }

    pub fn command(&mut self, id: &str, params: &Value) -> Result<Value, AdapterError> {
        if id == "view.sketch_projection" {
            return self.projection_command(params);
        }
        if id == "view.tool_dismissal" {
            let enabled = params["cancel_on_dismiss"].as_bool().ok_or_else(|| error("bad_params", "Expected a boolean cancel_on_dismiss."))?;
            self.cancel_interactions();
            self.cancel_tools_on_dismiss = enabled;
            return Ok(self.diagnostic());
        }
        if id == "view.background" {
            let color: Option<[u8; 3]> = serde_json::from_value(params["color"].clone()).map_err(|e| error("bad_params", e.to_string()))?;
            let grid: Option<[[u8; 3]; 2]> = serde_json::from_value(params["grid"].clone()).map_err(|e| error("bad_params", e.to_string()))?;
            let sketch: Option<[u8; 3]> = serde_json::from_value(params["sketch"].clone()).map_err(|e| error("bad_params", e.to_string()))?;
            let constrained_green = params
                .get("constrained_green")
                .map(|value| value.as_bool().ok_or_else(|| error("bad_params", "constrained_green must be a boolean")))
                .transpose()?
                .unwrap_or(self.constrained_green);
            self.background = color.map(|[r, g, b]| Rgb(r, g, b));
            self.sketch_color = sketch.map(|[r, g, b]| Rgb(r, g, b));
            self.constrained_green = constrained_green;
            let colors = grid.map(|colors| colors.map(|[r, g, b]| Rgb(r, g, b)));
            if self.grid.colors != colors {
                self.grid.colors = colors;
                self.changed();
            }
            return Ok(self.diagnostic());
        }
        if id == "view.grid" {
            let visible: bool = serde_json::from_value(params["visible"].clone()).map_err(|e| error("bad_params", e.to_string()))?;
            let integrated_axes = params
                .get("integrated_axes")
                .map(|value| serde_json::from_value::<bool>(value.clone()))
                .transpose()
                .map_err(|e| error("bad_params", e.to_string()))?
                .unwrap_or(self.grid.integrated_axes);
            if self.grid.visible != visible || self.grid.integrated_axes != integrated_axes {
                self.grid.visible = visible;
                self.grid.integrated_axes = integrated_axes;
                self.changed();
            }
            return Ok(self.diagnostic());
        }
        if id == "view.automatic_subtract" {
            let enabled = params["enabled"].as_bool().ok_or_else(|| error("bad_params", "ui.ipados.handles.boolean_required"))?;
            self.cancel_interactions();
            self.automatic_subtract = enabled;
            self.changed();
            return Ok(self.diagnostic());
        }
        if id == "view.construction_dashes" {
            let enabled = params.get("enabled").and_then(Value::as_bool).ok_or_else(|| error("bad_params", "enabled must be boolean"))?;
            self.construction_dashes = enabled;
            self.highlight_key = None;
            self.changed();
            return Ok(json!({"enabled":enabled}));
        }
        if id == "view.sketch_rotation_snap" {
            let enabled = params["enabled"].as_bool().ok_or_else(|| error("bad_params", "enabled must be a boolean"))?;
            self.cancel_interactions();
            self.sketch_rotation_snap = enabled;
            return Ok(self.diagnostic());
        }
        if id == "view.drag_precision" {
            let step = params["step_mm"]
                .as_f64()
                .filter(|step| step.is_finite() && (*step == 0. || (0.001..=10.).contains(step)))
                .ok_or_else(|| error("bad_params", "ui.ipados.workspace.drag_resolution_invalid"))?;
            self.cancel_interactions();
            self.drag_step_mm = step;
            return Ok(self.diagnostic());
        }
        if id == "view.tool_handles" {
            let enabled = params["enabled"].as_bool().ok_or_else(|| error("bad_params", "ui.ipados.handles.boolean_required"))?;
            self.cancel_interactions();
            self.direct_handles = enabled;
            self.highlight_key = None;
            self.changed();
            return Ok(self.diagnostic());
        }
        if (self.numeric.is_some() && id.starts_with("select.")) || ((self.numeric.is_some() || self.modelling.is_some()) && id.starts_with("tool."))
        {
            return Err(error("disabled", "ui.ipados.workspace.finish_edit"));
        }
        if id == "input.cancel" || id == "tool.cancel" {
            if id == "tool.cancel" {
                self.projection = None;
            }
            self.cancel_interactions();
            if id == "tool.cancel" && self.constraint_tool().is_some() {
                self.contacts.sketch.tool = "select".into();
                self.selection.clear();
                self.changed();
            }
            return Ok(self.diagnostic());
        }
        if id == "tool.constraint" {
            self.start_constraint(params)?;
            return Ok(self.diagnostic());
        }
        if self.sketch_command(id, params)? {
            return Ok(self.diagnostic());
        }
        if self.origin_command(id, params)? {
            return Ok(self.diagnostic());
        }
        if id == "view.sketch_plane" {
            if self.model.as_ref().is_some_and(|m| m.sketch.is_some()) {
                return Err(error("disabled", "Finish the active sketch first"));
            }
            let plane: Plane =
                serde_json::from_value(params.get("plane").cloned().unwrap_or(Value::Null)).map_err(|e| error("bad_params", e.to_string()))?;
            let checked = Plane::new(plane.origin, plane.x, plane.y)
                .filter(|p| p.origin.len() < 1e9)
                .ok_or_else(|| error("bad_params", "Invalid sketch plane"))?;
            self.cancel_interactions();
            self.origin.set_choosing(false);
            if self.plane_camera.is_none() {
                self.plane_camera = Some((self.camera, self.planar));
            }
            self.plane_preview = Some(checked);
            self.planar = true;
            self.align_sketch_plane(checked);
            self.changed();
            return Ok(self.diagnostic());
        }
        if id == "view.cancel_plane" {
            self.cancel_interactions();
            self.origin.set_choosing(false);
            if let Some((camera, planar)) = self.plane_camera.take() {
                self.camera = camera;
                self.planar = planar;
            }
            self.plane_preview = None;
            self.clear_hidden_reference_selection();
            self.changed();
            return Ok(self.diagnostic());
        }
        if id == "tool.set" || id == "tool.previous" {
            self.projection = None;
            if id == "tool.previous" && self.model.as_ref().is_some_and(|m| m.sketch.is_some()) {
                let tool = if self.contacts.sketch.active() {
                    self.contacts.previous_sketch_tool = self.contacts.sketch.tool.clone();
                    "select".to_string()
                } else if self.contacts.previous_sketch_tool.is_empty() {
                    "sketch.line".to_string()
                } else {
                    self.contacts.previous_sketch_tool.clone()
                };
                self.cancel_interactions();
                self.contacts.sketch.tool = tool;
                self.contacts.radius_tool = false;
                self.changed();
                return Ok(self.diagnostic());
            }
            if let Some(tool) = params.get("tool").and_then(Value::as_str).filter(|s| s.starts_with("sketch.")) {
                if !matches!(
                    tool,
                    "sketch.point"
                        | "sketch.line"
                        | "sketch.rectangle.two_point"
                        | "sketch.circle.center"
                        | "sketch.arc.three_point"
                        | "sketch.spline.fit_point"
                        | "sketch.move"
                        | "sketch.dimension"
                        | "sketch.delete"
                        | "sketch.trim"
                        | "sketch.extend"
                        | "sketch.offset"
                ) {
                    return Err(error("bad_params", "unsupported native sketch tool"));
                }
                if self.model.as_ref().is_none_or(|m| m.sketch.is_none()) {
                    return Err(error("disabled", "Create or reopen a sketch first"));
                }
                self.cancel_interactions();
                if tool == "sketch.dimension" {
                    self.contacts.sketch.dimension(&self.selection, params)?;
                }
                self.contacts.sketch.tool = tool.into();
                self.contacts.previous_sketch_tool = tool.into();
                self.contacts.radius_tool = false;
                self.changed();
                return Ok(self.diagnostic());
            }
            let tool = if id == "tool.previous" {
                !self.contacts.radius_tool
            } else {
                match params.get("tool").and_then(Value::as_str) {
                    Some("select") => false,
                    Some("post_radius") => true,
                    _ => return Err(error("bad_params", "unsupported native tool")),
                }
            };
            self.cancel_interactions();
            if self.contacts.sketch.active() {
                self.contacts.previous_sketch_tool = self.contacts.sketch.tool.clone();
            }
            self.contacts.radius_tool = tool;
            self.contacts.sketch.tool = "select".into();
            self.changed();
            return Ok(self.diagnostic());
        }
        if id == "view.exit_planar" {
            if self.model.as_ref().is_some_and(|m| m.sketch.is_some()) {
                return Err(error("disabled", "Finish the active sketch before leaving its plane"));
            }
            self.cancel_interactions();
            self.planar = false;
            self.changed();
            return Ok(self.diagnostic());
        }
        if id.starts_with("view.") && id != "view.project" {
            self.cancel_interactions();
        }
        if let Some(standard) = solidcraft_engine::standard_view(id) {
            if self.planar {
                return Err(error("planar_view", "exit the sketch view explicitly before changing orientation"));
            }
            self.camera.set_view(standard);
        } else {
            match id {
                "view.navigate" => {
                    self.navigate(params)?;
                }
                "view.touch_twist" => {
                    let enabled = params.get("enabled").and_then(Value::as_bool).ok_or_else(|| error("bad_params", "enabled must be a boolean"))?;
                    self.two_finger_twist = enabled;
                }
                "view.touch_orbit" => {
                    self.touch_orbit_follows_viewpoint = params
                        .get("follows_viewpoint")
                        .and_then(Value::as_bool)
                        .ok_or_else(|| error("bad_params", "follows_viewpoint must be a boolean"))?;
                }
                "view.project" => {
                    let point: [f64; 3] = serde_json::from_value(params.get("point").cloned().unwrap_or(Value::Null))
                        .map_err(|e| error("bad_params", e.to_string()))?;
                    if !point.iter().all(|v| v.is_finite() && v.abs() <= 1e12) {
                        return Err(error("bad_params", "point must contain finite coordinates"));
                    }
                    let pixel = crate::picking::project(self.vp(), Vec3::new(point[0], point[1], point[2]), self.metrics.pixels())
                        .ok_or_else(|| error("projection", "point is outside the camera clipping range"))?;
                    return Ok(
                        json!({"points":[pixel[0]/self.metrics.scale+self.metrics.origin_points[0],pixel[1]/self.metrics.scale+self.metrics.origin_points[1]],
                        "pixels":pixel,"scale":self.metrics.scale,"geometry_revision":self.model.as_ref().map(|m|m.geometry_revision)}),
                    );
                }
                "view.fit" => self.fit(),
                "view.home" => {
                    if self.planar {
                        return Err(error("planar_view", "exit the sketch view explicitly before changing orientation"));
                    }
                    // Fixed origin-centred, orthographic isometric home (200 mm
                    // eye distance). Never derive its framing from model bounds.
                    self.camera = Camera { target: Vec3::ZERO, distance: 200.0, fov: 0.0, ..Camera::default() };
                }
                "view.perspective" => {
                    if self.model.as_ref().is_some_and(|m| m.sketch.is_some()) {
                        return Err(error("disabled", "Sketch editing uses a planar view"));
                    }
                    let enabled = params.get("enabled").and_then(Value::as_bool).ok_or_else(|| error("bad_params", "enabled must be a boolean"))?;
                    let half = self.camera.half_height();
                    self.camera.fov = if enabled { 0.6 } else { 0.0 };
                    self.camera.distance = if enabled { half / 0.3_f64.tan() } else { half / 0.4 };
                }
                "select.entity" => {
                    if self.updating {
                        return Err(error("disabled", "ui.ipados.updating"));
                    }
                    let entity: Sel = serde_json::from_value(params.get("entity").cloned().unwrap_or(Value::Null))
                        .map_err(|e| error("bad_params", e.to_string()))?;
                    let valid = self.model.as_ref().is_some_and(|m| match &entity {
                        Sel::Body { name } => m.picks.bodies.iter().any(|b| &b.name == name),
                        Sel::Occurrence { path } => m.occurrences.contains(path),
                        Sel::Feature { id } => m.feature_ids.contains(id),
                        Sel::Profile { sketch, index } => m.picks.profiles.iter().any(|p| p.sketch == *sketch && p.index == *index),
                        Sel::SketchPoint { id } => m.sketch.as_ref().is_some_and(|s| s.sketch.resolve_point(id).is_some()),
                        Sel::SketchCurve { id, sketch } => {
                            m.sketch.as_ref().is_some_and(|s| sketch.is_none_or(|id| id == s.id) && s.sketch.curve_index(id).is_some())
                        }
                        Sel::SketchConstraint { id } => m.sketch.as_ref().is_some_and(|s| s.sketch.constraints.iter().any(|c| &c.id == id)),
                        Sel::Plane { .. } | Sel::Axis { .. } | Sel::Vertex { .. } => self.reference_available(&entity),
                        _ => false,
                    });
                    if !valid {
                        return Err(error("stale_selection", "ui.ipados.workspace.invalidated"));
                    }
                    let add = params.get("additive").and_then(Value::as_bool).unwrap_or(self.multiple);
                    if self.constraint_tool().is_some() {
                        self.constraint_pick(Some(entity))?;
                    } else {
                        self.cancel_interactions();
                        self.select(Some(entity), add);
                    }
                }
                "select.entities" => {
                    if self.updating {
                        return Err(error("disabled", "The source scene is updating."));
                    }
                    let items: Vec<Sel> = serde_json::from_value(params["items"].clone()).map_err(|e| error("selection", e.to_string()))?;
                    if items.len() > 128 {
                        return Err(error("selection", "Selection exceeds its bounded list."));
                    }
                    if items
                        .iter()
                        .any(|item| !matches!(item,Sel::Occurrence{path} if self.model.as_ref().is_some_and(|m|m.occurrences.contains(path))))
                    {
                        return Err(error("stale_selection", "An endpoint occurrence is unavailable."));
                    }
                    self.selection.clear();
                    for item in items {
                        self.select(Some(item), true);
                    }
                }
                "select.filter" => {
                    self.filter = serde_json::from_value(params.get("filter").cloned().unwrap_or(Value::Null))
                        .map_err(|e| error("bad_params", e.to_string()))?;
                    self.hover = None;
                }
                "select.multiple" => {
                    self.multiple = params.get("enabled").and_then(Value::as_bool).ok_or_else(|| error("bad_params", "enabled must be a boolean"))?;
                }
                "select.clear" => {
                    self.selection.clear();
                    self.hover = None;
                }
                _ => return Err(error("unknown_command", format!("unsupported viewport command {id}"))),
            }
        }
        self.changed();
        Ok(self.diagnostic())
    }

    /// Single camera path for native gestures and explicit navigation commands.
    pub(crate) fn navigate(&mut self, params: &Value) -> Result<(), AdapterError> {
        // Native multitouch adds continuous image rotation using the shared
        // Camera operations. Other navigation stays on the engine's stateless path.
        if params.get("motion").and_then(Value::as_str) == Some("roll") {
            let angle = params
                .get("angle")
                .and_then(Value::as_f64)
                .filter(|a| a.is_finite() && a.abs() <= std::f64::consts::PI)
                .ok_or_else(|| error("bad_params", "roll angle must be finite and within ±pi radians"))?;
            let anchor = params
                .get("anchor_points")
                .map(|value| {
                    let point: [f64; 2] = serde_json::from_value(value.clone())
                        .map_err(|_| error("bad_params", "anchor_points must contain two finite coordinates"))?;
                    if !point.iter().all(|v| v.is_finite() && v.abs() <= 1e6) {
                        return Err(error("bad_params", "anchor_points must be finite and within ±1000000 points"));
                    }
                    Ok(point)
                })
                .transpose()?;
            let (right, up, _) = self.camera.basis();
            let plane = Plane { origin: self.camera.target, x: right, y: up };
            let pivot = anchor.and_then(|point| self.plane_point(point, plane)).map(|point| (point, self.camera.frame_coords(point)));
            self.camera.roll_by(angle);
            if let Some((point, coords)) = pivot {
                self.camera.place(point, coords);
            }
            return Ok(());
        }
        let mut p = params.clone();
        let obj = p.as_object_mut().ok_or_else(|| error("bad_params", "navigation parameters must be an object"))?;
        if self.planar && obj.get("motion").and_then(Value::as_str) == Some("orbit") {
            obj.insert("motion".into(), json!("pan"));
        }
        obj.insert("camera".into(), json!(self.camera));
        let value = solidcraft_engine::navigate_camera(&p)?;
        self.camera = serde_json::from_value(value["camera"].clone()).map_err(|e| error("camera", e.to_string()))?;
        if value["fit"] == true {
            self.fit();
        }
        Ok(())
    }

    /// Fit uses shared Camera::fit and navigation zoom. The aspect correction is native
    /// layout policy, not another camera implementation. Small models use shared zoom's
    /// supported 1 µm distance floor; large fits retain the engine's f64 camera math.
    fn fit(&mut self) {
        let Some(model) = self.display_model().cloned() else {
            self.camera = Camera::default();
            return;
        };
        if model.bounds.is_empty() {
            self.camera = Camera::default();
            if self.planar
                && let Some(plane) = model.sketch_plane
            {
                self.align_sketch_plane(plane);
            }
            return;
        }
        self.fit_bounds(model.bounds);
    }

    pub(crate) fn fit_bounds(&mut self, bounds: Aabb3) {
        if bounds.is_empty() {
            return;
        }
        self.camera.fit(&bounds);
        let r = bounds.diagonal() * 0.5;
        let aspect = (self.metrics.width_points / self.metrics.height_points.max(1.0)).clamp(0.01, 1.0);
        let target_distance = if self.camera.fov > 0.0 {
            r.max(1e-4) / ((self.camera.fov * 0.5).tan() * aspect).atan().sin() * 1.1
        } else {
            r.max(1e-4) / (0.4 * aspect) * 1.15
        };
        // Fit the shared camera's supported distance range, including below its ordinary
        // minimum fit radius. No changes to desktop/web Camera::fit defaults.
        self.camera.distance = target_distance.clamp(1e-3, 1e12);
    }

    pub(crate) fn vp(&self) -> Mat4 {
        let bounds = self.reference_bounds();
        self.camera.view_proj_bounds(self.metrics.width_points / self.metrics.height_points.max(1.0), &bounds)
    }

    pub(crate) fn pick(&self, points: [f64; 2], expected: Option<u64>) -> Option<Sel> {
        let model = self.model.as_ref()?;
        if self.updating || expected.is_some_and(|r| r != model.geometry_revision) {
            return None;
        }
        let pixel = self.metrics.pixel(points)?;
        if model.sketch.is_some() {
            return self.pick_sketch(points);
        }
        let filter = if self.origin.choosing {
            PickFilter::Face
        } else if self.modelling.as_ref().is_some_and(|edit| edit.selection_field.as_deref() == Some("targets") || edit.body_pick_field().is_some()) {
            PickFilter::Body
        } else if self.modelling.as_ref().is_some_and(|edit| edit.selection_field.as_deref() == Some("edges")) {
            PickFilter::Edge
        } else if self
            .modelling
            .as_ref()
            .is_some_and(|edit| matches!(edit.selection_field.as_deref(), Some("profile" | "faces")) || edit.operation == "sketch.redefine")
        {
            PickFilter::Face
        } else {
            self.filter
        };
        let picked = model.picks.pick(&self.camera, self.vp(), self.metrics, pixel, filter);
        if !self.origin.choosing
            && self.modelling.as_ref().is_none_or(|edit| edit.operation != "sketch.redefine")
            && matches!(filter, PickFilter::Auto | PickFilter::Face)
            && !matches!(picked, Some(Sel::Edge { .. } | Sel::Vertex { .. }))
            && let Some(profile) = model.picks.pick_profile(&self.camera, self.vp(), self.metrics, pixel)
        {
            return Some(profile);
        }
        // Explicit topology filters retain their target. In the plane chooser,
        // reference planes compete with body faces using the rendered depth.
        if (self.origin.choosing || picked.is_none())
            && self.modelling.as_ref().is_none_or(|edit| edit.body_pick_field().is_none())
            && let Some(reference) = self.pick_reference(points)
        {
            return Some(reference);
        }
        if filter == PickFilter::Occurrence {
            let Sel::Body { name } = picked? else {
                return None;
            };
            return model.occurrence_paths.get(&name).map(|path| Sel::Occurrence { path: path.clone() });
        }
        picked
    }

    /// A double tap expands selection; it never toggles off the first tap's edge.
    pub(crate) fn select_tap(&mut self, hit: Option<Sel>, add: bool, tap_count: u32) {
        if tap_count != 2 || self.origin.choosing || self.modelling.is_some() || self.contacts.sketch.tool != "select" {
            self.select(hit, add);
            return;
        }
        let Some(hit) = hit else {
            self.select(None, add);
            return;
        };
        let connected = self.model.as_ref().and_then(|model| {
            if let Sel::SketchCurve { id, sketch } = &hit {
                let snapshot = model.sketch.as_ref()?;
                if sketch.is_some_and(|sketch| sketch != snapshot.id) {
                    return None;
                }
                let seed = snapshot.sketch.curve_index(id)?;
                let ends = snapshot
                    .sketch
                    .curves
                    .iter()
                    .enumerate()
                    .filter_map(|(index, curve)| {
                        if !solidcraft_engine::view::sketch_curve_visible(&snapshot.sketch, index) {
                            return None;
                        }
                        let (a, b) = curve.kind.ends()?;
                        Some((index, [snapshot.plane.to_world(snapshot.sketch.point(a)?), snapshot.plane.to_world(snapshot.sketch.point(b)?)]))
                    })
                    .collect::<Vec<_>>();
                return crate::picking::endpoint_chain(&ends, seed)?
                    .into_iter()
                    .map(|index| Some(Sel::SketchCurve { id: snapshot.sketch.curves.get(index)?.id.clone(), sketch: Some(snapshot.id) }))
                    .collect();
            }
            model.picks.connected_edges(&hit)
        });
        if !matches!(hit, Sel::SketchCurve { .. } | Sel::Edge { .. }) {
            self.select(Some(hit), add);
            return;
        }
        let Some(connected) = connected else { return };
        let missing: Vec<_> = connected.into_iter().filter(|piece| !add || !self.is_selected(piece)).collect();
        if (if add { self.selection.len() } else { 0 }) + missing.len() > 1024 {
            return;
        }
        if !add {
            self.selection.clear();
        }
        self.selection.extend(missing);
        self.hover = None;
        self.changed();
    }

    pub(crate) fn select(&mut self, hit: Option<Sel>, add: bool) {
        let body_field = self.modelling.as_ref().and_then(|edit| edit.body_pick_field()).map(str::to_string);
        if body_field.is_some() && hit.as_ref().is_some_and(|hit| !matches!(hit, Sel::Body { .. })) {
            return;
        }
        if body_field.as_deref() == Some("target")
            && let Some(Sel::Body { name }) = &hit
            && let Some(edit) = &mut self.modelling
        {
            self.selection = edit.pick_body_target(name);
            self.hover = None;
            self.changed();
            return;
        }
        if body_field.as_deref() == Some("tools")
            && let Some(Sel::Body { name }) = &hit
            && self.modelling.as_ref().is_some_and(|edit| edit.params["target"] == *name)
        {
            return;
        }
        let add = match body_field.as_deref() {
            Some("target") => false,
            Some("tools") => hit.is_some(),
            _ => add,
        };
        let previous = body_field.as_ref().map(|_| self.selection.clone());
        if !add {
            self.selection.clear();
        }
        if let Some(hit) = hit {
            let picked = self.model.as_ref().map_or_else(|| vec![hit.clone()], |model| model.picks.grouped_selection(&hit));
            if add && picked.iter().all(|piece| self.is_selected(piece)) {
                self.selection.retain(|selected| !picked.iter().any(|piece| same(selected, piece)));
            } else {
                let missing: Vec<_> = picked.into_iter().filter(|piece| !self.is_selected(piece)).collect();
                if self.selection.len().saturating_add(missing.len()) <= 1024 {
                    self.selection.extend(missing);
                }
            }
        }
        if previous.as_ref().is_some_and(|previous| previous != &self.selection)
            && let Some(edit) = &mut self.modelling
        {
            // A late preview must never validate an earlier set of body picks.
            edit.version = edit.version.saturating_add(1);
            edit.preview = None;
            edit.last_preview = None;
        }
        self.hover = None;
        self.changed();
    }

    pub fn snapshot(&self) -> Option<RenderSnapshot> {
        let authoritative = self.model.as_ref()?;
        let model = self.display_model().cloned()?;
        Some(RenderSnapshot {
            geometry_revision: authoritative.geometry_revision,
            background: self.background.map(|color| (color, color)).or(model.scene.background),
            sketch_color: self.sketch_color,
            constrained_green: self.constrained_green,
            construction_dashes: self.construction_dashes,
            model,
            camera: self.camera,
            view_proj: self.vp(),
            guides: self.guides.clone(),
            highlight: self.highlight.clone(),
            scene_revision: self.scene_revision,
            highlight_revision: self.highlight_revision,
            guide_revision: self.guide_revision,
            selected_bodies: self.selected_assembly_bodies(&self.selection),
            hovered_bodies: self.selected_assembly_bodies(&self.hover.iter().cloned().collect::<Vec<_>>()),
            selection_tint: view::Palette::DEFAULT.body_selected.f().map(|v| v as f32),
            hover_tint: view::Palette::DEFAULT.body_selected.f().map(|v| v as f32 * 0.2),
        })
    }
    fn selected_assembly_bodies(&self, selection: &[Sel]) -> Vec<String> {
        let mut names = Vec::new();
        let Some(model) = self.display_model() else {
            return names;
        };
        for selected in selection {
            match selected {
                Sel::Body { name } => names.push(name.clone()),
                Sel::Occurrence { path } => {
                    names.extend(model.occurrence_paths.iter().filter(|(_, p)| p.starts_with(path)).map(|(name, _)| name.clone()))
                }
                Sel::Feature { id } => {
                    if let Some(feedback) = model.feature_feedback.get(id) {
                        names.extend(feedback.bodies.iter().cloned());
                    }
                }
                _ => {}
            }
        }
        names.sort();
        names.dedup();
        names
    }

    pub fn diagnostic(&self) -> Value {
        let mut input = self.contacts.diagnostic();
        input["sketch"]["projection"] = self.projection_presentation();
        input["sketch"]["finish_chain_at"] = json!(self.sketch_chain_end());
        input["sketch"]["snapping"] =
            json!(self.contacts.sketch.snapping && self.model.as_ref().and_then(|m| m.sketch.as_ref()).is_none_or(|s| !s.sketch.view.no_snap));
        json!({"document_id":self.model.as_ref().map(|m| &m.document_id),
            "geometry_revision":self.model.as_ref().map(|m| m.geometry_revision),
            "revision":self.model.as_ref().map(|m| m.revision),"scene_revision":self.scene_revision,
            "selection":self.selection,"hover":self.hover,"filter":self.filter,"multiple":self.multiple,
            "updating":self.updating,"drag_step_mm":self.drag_step_mm,"camera":self.camera,"viewport_epoch":self.metrics_epoch,"camera_epoch":self.camera_epoch,"metrics":self.metrics,
            "volume_mm3":self.model.as_ref().map(|m|m.volume_mm3),"preview_volume_mm3":self.display_model().filter(|m| self.model.as_ref().is_none_or(|a| !Arc::ptr_eq(a,m))).map(|m|m.volume_mm3),"preview":self.contacts.preview.is_some() || self.numeric.as_ref().is_some_and(|e|e.preview.is_some()) || self.modelling.as_ref().is_some_and(|e|e.preview.is_some() || e.last_preview.is_some()),"preview_stale":self.modelling.as_ref().is_some_and(|e|e.preview.is_none() && e.last_preview.is_some()),"numeric_token":self.numeric.as_ref().map(|e|e.token),"model_token":self.modelling.as_ref().map(|e|e.token),
            "input":input,"tool_pending":self.contacts.diagnostic()["tool_pending"],"planar":self.planar,
            "origin":self.origin,"reference_labels":self.reference_labels(),
            "background":self.background.map(|color| [color.0,color.1,color.2]),
            "sketch_color":self.sketch_color.map(|color| [color.0,color.1,color.2]),
            "constrained_green":self.constrained_green,
            "grid":{"visible":self.grid.visible,"integrated_axes":self.grid.integrated_axes,"colors":self.grid.colors.map(|colors| colors.map(|color| [color.0,color.1,color.2]))},
            "touch_orbit":{"follows_viewpoint":self.touch_orbit_follows_viewpoint},
            "touch_twist":{"enabled":self.two_finger_twist},
            "sketch_rotation_snap":{"enabled":self.sketch_rotation_snap},
            "tool_dismissal":{"cancel_on_dismiss":self.cancel_tools_on_dismiss},
            "handle":self.handle(),"handle_point":self.handle_point(),"model_value":self.model_value(),"annotations":self.sketch_annotations(),"constraint_indicators":self.sketch_constraint_indicators()})
    }

    /// Presentation data uses the same camera and endpoint as the renderer/picker.
    /// The native host never projects CAD geometry or evaluates an expression.
    fn model_value(&self) -> Option<Value> {
        if !self.direct_handles || self.updating {
            return None;
        }
        let edit = self.modelling.as_ref()?;
        let handle = edit.handle.as_ref()?;
        let position = crate::picking::project(self.vp(), handle.point(), self.metrics.pixels())?;
        let position = [position[0] / self.metrics.scale, position[1] / self.metrics.scale];
        if !(0.0..=self.metrics.width_points).contains(&position[0]) || !(0.0..=self.metrics.height_points).contains(&position[1]) {
            return None;
        }
        let expression =
            if let Some((parent, key)) = handle.parameter.split_once('.') { &edit.params[parent][key] } else { &edit.params[&handle.parameter] };
        Some(
            json!({"token":edit.token,"parameter":handle.parameter,"position":position,"value":handle.distance,"units":handle.units,"expression":expression,"valid":edit.preview.is_some()}),
        )
    }

    fn draw_linear_handle(&self, scene: &mut Scene, p: Vec3, normal: Vec3) -> bool {
        let (right, up, _) = self.camera.basis();
        let Some(start) = crate::picking::project(self.vp(), p, self.metrics.pixels()) else { return false };
        let Some(unit) = crate::picking::project(self.vp(), p + right, self.metrics.pixels()) else { return false };
        let pixels_per_world = (unit[0] - start[0]).hypot(unit[1] - start[1]);
        if !pixels_per_world.is_finite() || pixels_per_world < 1e-9 {
            return false;
        }
        let size = self.metrics.scale / pixels_per_world;
        let axis = crate::picking::project(self.vp(), p + normal * size * 10., self.metrics.pixels())
            .and_then(|end| (right * (end[0] - start[0]) - up * (end[1] - start[1])).normalized())
            .unwrap_or(up);
        let side = self.camera.back().cross(axis);
        let line =
            |points| SceneLine { points, color: view::Palette::DEFAULT.body_selected, alpha: 1., width: 3.5 * self.metrics.scale, on_top: true };
        scene.lines.push(line(vec![p - axis * size * 19., p + axis * size * 19.]));
        for sign in [-1., 1.] {
            let tip = p + axis * size * 19. * sign;
            let base = p + axis * size * 11. * sign;
            scene.lines.push(line(vec![base - side * size * 7., tip, base + side * size * 7.]));
        }
        true
    }

    pub(crate) fn changed(&mut self) {
        let plane = self.plane_preview.or_else(|| self.model.as_ref().and_then(|m| m.sketch_plane));
        let grid = GuideGrid::new(&self.camera, plane);
        let automatic = [Vec3::X, Vec3::Y, Vec3::Z].map(|axis| self.grid.integrated_axes && self.grid.visible && grid.contains_axis(axis));
        self.origin.set_grid_axes(automatic);
        self.update_reference_feedback();
        self.scene_revision = self.scene_revision.saturating_add(1);
        if self.guide_camera != Some(self.camera) {
            self.camera_epoch = self.camera_epoch.saturating_add(1);
        }
        if self.guide_camera != Some(self.camera)
            || self.guide_metrics != self.metrics
            || self.guide_plane != plane
            || self.guide_grid != self.grid
            || self.origin.dirty
        {
            self.guide_revision = self.guide_revision.saturating_add(1);
            let colors = std::array::from_fn(|i| {
                self.reference_color(&Sel::Axis { name: crate::origin::ITEMS[i + 1].to_string() }, view::Palette::DEFAULT.axes[i])
            });
            let axes = std::array::from_fn(|i| self.origin.axis_visible(i, self.grid.visible));
            let mut scene = guides(&self.camera, self.metrics, plane, self.grid, axes, colors);
            if plane.is_none() {
                self.draw_references(&mut scene);
            }
            self.guides = Arc::new(scene);
            self.origin.dirty = false;
            self.guide_camera = Some(self.camera);
            self.guide_metrics = self.metrics;
            self.guide_grid = self.grid;
            self.guide_plane = plane;
        }
        let vertex = self.handle().is_some()
            || self.construction_dashes && self.display_model().is_some_and(|m| m.scene.lines.iter().any(crate::construction::construction_stroke))
            || self.modelling.as_ref().is_some_and(|e| e.handle.is_some())
            || self.display_model().is_some_and(|m| m.sketch.is_some())
            || self.selection.iter().chain(self.hover.iter()).any(|s| match s {
                Sel::Vertex { .. } => true,
                Sel::Feature { id } => self.display_model().and_then(|m| m.feature_feedback.get(id)).is_some_and(|f| f.construct.is_some()),
                _ => false,
            });
        let camera = vertex.then_some(self.camera);
        let height = if vertex { self.metrics.height_points } else { 0.0 };
        let model_handle = self.modelling.as_ref().and_then(|edit| edit.handle.as_ref());
        let stale = self.highlight_key.as_ref().is_none_or(|key| {
            let same_model = match (&key.model, self.display_model()) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            };
            !same_model
                || key.model_handle.as_ref() != model_handle
                || key.selection.len() != self.selection.len()
                || !key.selection.iter().zip(&self.selection).all(|(a, b)| same(a, b))
                || !match (&key.hover, &self.hover) {
                    (Some(a), Some(b)) => same(a, b),
                    (None, None) => true,
                    _ => false,
                }
                || key.camera != camera
                || key.scale != self.metrics.scale
                || key.height != height
                || key.tool_revision != self.contacts.tool_revision
        });
        if stale {
            self.highlight_revision = self.highlight_revision.saturating_add(1);
            self.highlight = Arc::new(self.highlights());
            self.highlight_key = Some(HighlightKey {
                model_handle: model_handle.cloned(),
                model: self.display_model().cloned(),
                selection: self.selection.clone(),
                hover: self.hover.clone(),
                camera,
                scale: self.metrics.scale,
                height,
                tool_revision: self.contacts.tool_revision,
            });
        }
    }

    fn highlights(&self) -> Scene {
        let mut scene = Scene::default();
        if self.construction_dashes
            && let Some(model) = self.display_model()
        {
            let source = model.preview_scene.as_ref().map_or(model.scene.as_ref(), |preview| preview.scene.as_ref());
            let unit = 2.0 * self.camera.half_height() / self.metrics.height_points.max(1.0);
            for line in source.lines.iter().filter(|line| crate::construction::construction_stroke(line)) {
                crate::construction::dash_line(&mut scene, line, unit);
            }
        }
        self.sketch_feedback(&mut scene);
        self.projection_feedback(&mut scene);
        // Selection still belongs to the authoritative source revision while a
        // provisional shape changes its topology. Draw its real source edges/faces.
        let source = if self.modelling.is_some() { self.model.as_ref() } else { self.display_model() };
        let Some(model) = source else { return scene };
        if let Some(display) = self.display_model() {
            let size = (self.camera.half_height() * 0.08).clamp(2., 30.);
            for frame in &display.joint_frames {
                let origin = Vec3::new(frame[3][0], frame[3][1], frame[3][2]);
                for (i, color) in view::Palette::DEFAULT.axes.iter().enumerate() {
                    let axis = Vec3::new(frame[i][0], frame[i][1], frame[i][2]);
                    scene.lines.push(SceneLine {
                        points: vec![origin, origin + axis * size],
                        color: *color,
                        alpha: 1.,
                        width: 2. * self.metrics.scale,
                        on_top: true,
                    });
                }
            }
        }
        if let Some(p) = self.handle_point() {
            let (right, up, _) = self.camera.basis();
            let size = 7.0 * 2.0 * self.camera.half_height() / self.metrics.height_points.max(1.0);
            let arrow = self.direct_handles
                && self
                    .modelling
                    .as_ref()
                    .and_then(|edit| edit.handle.as_ref())
                    .is_some_and(|h| !h.angular && self.draw_linear_handle(&mut scene, p, h.normal));
            for axis in [right, up].into_iter().filter(|_| !arrow) {
                scene.lines.push(SceneLine {
                    points: vec![p - axis * size, p + axis * size],
                    color: view::Palette::DEFAULT.body_selected,
                    alpha: 1.0,
                    width: 3.0 * self.metrics.scale,
                    on_top: true,
                });
            }
            if let Some(h) = self.modelling.as_ref().and_then(|e| e.handle.as_ref()) {
                if h.angular {
                    let points = (0..=64)
                        .map(|i| {
                            let angle = f64::from(i) * std::f64::consts::TAU / 64.;
                            h.origin + (h.arm * angle.cos() + h.normal.cross(h.arm) * angle.sin()) * h.size
                        })
                        .collect();
                    scene.lines.push(SceneLine {
                        points,
                        color: view::Palette::DEFAULT.body_selected,
                        alpha: 1.,
                        width: 2. * self.metrics.scale,
                        on_top: true,
                    });
                }
                scene.lines.push(SceneLine {
                    points: vec![h.origin, p],
                    color: view::Palette::DEFAULT.body_selected,
                    alpha: 1.,
                    width: 2. * self.metrics.scale,
                    on_top: true,
                });
            }
        }
        let selected = view::Palette::DEFAULT.body_selected;
        let hover = view::Palette::DEFAULT.sketch_construction;
        let mut highlighted_faces = BTreeSet::new();
        let mut highlighted_edges = BTreeSet::new();
        for (sel, color) in self.selection.iter().map(|s| (s, selected)).chain(self.hover.iter().map(|s| (s, hover))) {
            if let Sel::Feature { id } = sel {
                if let Some(feedback) = model.feature_feedback.get(id) {
                    for points in &feedback.lines {
                        scene.lines.push(SceneLine { points: points.clone(), color, alpha: 1., width: 3. * self.metrics.scale, on_top: false });
                    }
                    for name in &feedback.bodies {
                        if let Some(body) = model.picks.bodies.iter().find(|body| &body.name == name) {
                            for (_, points) in body.mesh.edges.iter().enumerate().filter(|(i, _)| body.mesh.shaded_edge_visible(*i)) {
                                scene.lines.push(SceneLine {
                                    points: points.clone(),
                                    color,
                                    alpha: 1.,
                                    width: 3. * self.metrics.scale,
                                    on_top: false,
                                });
                            }
                        }
                    }
                    if let Some(construct) = &feedback.construct {
                        use solidcraft_engine::doc::construct::ConstructGeom;
                        let size = self.camera.half_height() * 0.2;
                        let lines = match construct {
                            ConstructGeom::Plane(plane) => vec![vec![
                                plane.origin,
                                plane.origin + plane.x * size,
                                plane.origin + (plane.x + plane.y) * size,
                                plane.origin + plane.y * size,
                                plane.origin,
                            ]],
                            ConstructGeom::Axis { origin, dir } => vec![vec![*origin - *dir * size, *origin + *dir * size]],
                            ConstructGeom::Point(point) => {
                                let (right, up, _) = self.camera.basis();
                                let radius = 6. * 2. * self.camera.half_height() / self.metrics.height_points.max(1.);
                                [right, up].into_iter().map(|axis| vec![*point - axis * radius, *point + axis * radius]).collect()
                            }
                        };
                        for points in lines {
                            scene.lines.push(SceneLine { points, color, alpha: 1., width: 3. * self.metrics.scale, on_top: false });
                        }
                    }
                }
                continue;
            }
            if let Sel::Profile { sketch, index } = sel {
                if let Some(profile) = model.picks.profiles.iter().find(|p| p.sketch == *sketch && p.index == *index) {
                    scene.meshes.push(SceneMesh {
                        mesh: profile.fill.clone(),
                        color,
                        face_colors: Vec::new(),
                        opacity: 0.3,
                        closed: false,
                        hatch: solidcraft_render::hatch::Hatch(0),
                    });
                    for boundary in &profile.boundaries {
                        scene.lines.push(SceneLine { points: boundary.clone(), color, alpha: 1., width: 3. * self.metrics.scale, on_top: false });
                    }
                }
                continue;
            }
            let name = match sel {
                Sel::Body { name } => name,
                Sel::Face { body, .. } | Sel::Edge { body, .. } | Sel::Vertex { body, .. } => body,
                _ => continue,
            };
            let Some(body) = model.picks.bodies.iter().find(|b| &b.name == name) else { continue };
            let mesh = &body.mesh;
            let line = |points| SceneLine { points, color, alpha: 1.0, width: 3.0 * self.metrics.scale, on_top: false };
            match sel {
                Sel::Body { .. } => {
                    for (ei, points) in mesh.edges.iter().enumerate().filter(|(i, _)| mesh.shaded_edge_visible(*i)) {
                        let _ = ei;
                        scene.lines.push(line(points.clone()));
                    }
                }
                Sel::Edge { index, .. } => {
                    for edge in mesh.circular_rim(*index).into_iter().filter(|edge| highlighted_edges.insert((name, *edge))) {
                        if let Some(points) = mesh.edges.get(edge) {
                            scene.lines.push(line(points.clone()));
                        }
                    }
                }
                Sel::Face { index, .. } => {
                    let faces: BTreeSet<_> = mesh.surface_patch(*index).into_iter().filter(|face| highlighted_faces.insert((name, *face))).collect();
                    if faces.is_empty() {
                        continue;
                    }
                    let mut face = Mesh::default();
                    for (tri, id) in mesh.triangles.iter().zip(&mesh.tri_face).filter(|(_, id)| faces.contains(&(**id as usize))) {
                        let Some(pts) = mesh.tri(tri) else { continue };
                        let Ok(start) = u32::try_from(face.positions.len()) else { break };
                        if start > u32::MAX - 3 {
                            break;
                        }
                        face.positions.extend(pts);
                        face.normals.extend(tri.iter().map(|i| mesh.normals.get(*i as usize).copied().unwrap_or(Vec3::Z)));
                        face.triangles.push([start, start + 1, start + 2]);
                        face.tri_face.push(*id);
                    }
                    scene.meshes.push(SceneMesh::solid(Arc::new(face), color));
                }
                Sel::Vertex { point, .. } => {
                    let (r, u, _) = self.camera.basis();
                    let size = 6.0 * self.metrics.scale * 2.0 * self.camera.half_height() / self.metrics.pixels()[1].max(1.0);
                    for axis in [r, u] {
                        scene.lines.push(line(vec![*point - axis * size, *point + axis * size]));
                    }
                }
                _ => {}
            }
        }
        scene
    }

    fn align_sketch_plane(&mut self, plane: Plane) {
        self.camera = self.camera.looking_from(plane.normal());
        let (right, up, _) = self.camera.basis();
        self.camera.roll_by(plane.x.dot(up).atan2(plane.x.dot(right)));
        self.camera.target = plane.origin;
        self.camera.fov = 0.0;
    }
}

fn same(a: &Sel, b: &Sel) -> bool {
    match (a, b) {
        (Sel::Body { name: a }, Sel::Body { name: b }) => a == b,
        (Sel::Face { body: a, index: ai, .. }, Sel::Face { body: b, index: bi, .. })
        | (Sel::Edge { body: a, index: ai, .. }, Sel::Edge { body: b, index: bi, .. }) => a == b && ai == bi,
        (Sel::Vertex { body: a, point: ap }, Sel::Vertex { body: b, point: bp }) => a == b && ap.dist(*bp) < 1e-9,
        // Omitted sketch IDs refer to the active sketch. Its annotation and
        // picker include the ID, so both representations name the same curve.
        (Sel::SketchCurve { id: a, sketch: sa }, Sel::SketchCurve { id: b, sketch: sb }) => a == b && (sa.is_none() || sb.is_none() || sa == sb),
        _ => a == b,
    }
}

/// One world-space plane and fade for neutral lines, coloured axes and picking.
struct GuideGrid {
    plane: Plane,
    center: Vec3,
    minor: f64,
    major: f64,
    fade: view::GridFade,
}

impl GuideGrid {
    fn new(camera: &Camera, sketch_plane: Option<Plane>) -> Self {
        let plane = sketch_plane.unwrap_or_else(|| {
            let (x, y) = view::grid_plane(camera.to_frame(camera.back()), view::GridPlane::Ground).axes();
            Plane { origin: Vec3::ZERO, x: camera.from_frame(x), y: camera.from_frame(y) }
        });
        let (minor, major) = view::grid_step(camera.half_height());
        let target = camera.target - plane.origin;
        let center =
            plane.origin + plane.x * ((target.dot(plane.x) / major).round() * major) + plane.y * ((target.dot(plane.y) / major).round() * major);
        let slant = camera.back().dot(plane.normal());
        let ext = view::grid_ext(camera.half_height(), major, slant);
        Self { plane, center, minor, major, fade: view::GridFade::new(camera.half_height(), slant, ext, ext) }
    }

    fn contains_axis(&self, axis: Vec3) -> bool {
        self.plane.normal().dot(axis).abs() < 1e-9 && self.plane.normal().dot(self.plane.origin).abs() < 1e-9
    }

    fn axis_index(axis: Vec3, fallback: usize) -> usize {
        [Vec3::X, Vec3::Y, Vec3::Z].iter().position(|world| axis.dot(*world).abs() > 1.0 - 1e-9).unwrap_or(fallback)
    }
}

impl Viewport {
    pub(crate) fn axis_drawn_in_grid(&self, axis: Vec3) -> bool {
        self.grid.integrated_axes && self.grid.visible && GuideGrid::new(&self.camera, None).contains_axis(axis)
    }

    pub(crate) fn reference_axis_segment(&self, axis: Vec3, legacy_extent: f64) -> Option<[Vec3; 2]> {
        if !self.grid.integrated_axes {
            return Some([Vec3::ZERO, axis * legacy_extent]);
        }
        if self.axis_drawn_in_grid(axis) {
            let grid = GuideGrid::new(&self.camera, None);
            let along = grid.center.dot(axis);
            let offset = (grid.center - axis * along).len();
            let extent2 = grid.fade.end * grid.fade.end - offset * offset;
            if extent2 <= 0.0 {
                return None;
            }
            let extent = extent2.sqrt();
            return Some([axis * (along - extent), axis * (along + extent)]);
        }
        // Only a currently visible, containing grid extends an axis. Explicit
        // off-plane references and references with the grid hidden stay short.
        Some([Vec3::ZERO, axis * legacy_extent])
    }
}

/// Reference grid uses shared spacing/fade/plane helpers. Its zero lines carry
/// the active plane's axes; the independent corner triad still shows all XYZ.
fn guides(camera: &Camera, metrics: ViewportMetrics, sketch_plane: Option<Plane>, grid: GridStyle, axes: [bool; 3], axis_colors: [Rgb; 3]) -> Scene {
    let mut scene = Scene::default();
    let palette = view::Palette::DEFAULT;
    let [grid_minor, grid_major] = grid.colors.unwrap_or([palette.grid, palette.grid_major]);
    let GuideGrid { plane, center, minor, major, fade } = GuideGrid::new(camera, sketch_plane);
    let (u, v) = (plane.x, plane.y);
    let count = ((fade.end / minor) as i64).clamp(1, 200);
    for i in (-count..=count).filter(|_| grid.visible) {
        let offset = i as f64 * minor;
        let color = if (offset / major).fract().abs() < 1e-9 { grid_major } else { grid_minor };
        for (along, across) in [(u, v), (v, u)] {
            let index = GuideGrid::axis_index(along, if along == u { 0 } else { 1 });
            let zero = (center - plane.origin).dot(across) + offset;
            if grid.integrated_axes && axes[index] && zero.abs() < minor * 1e-6 {
                continue;
            }
            for (a, b, alpha) in fade.pieces(offset) {
                scene.grid.push(SceneLine {
                    points: [a, b].into_iter().map(|n| center + across * offset + along * n).collect(),
                    color,
                    alpha,
                    width: metrics.scale,
                    on_top: false,
                });
            }
        }
    }
    if grid.integrated_axes {
        // Generate zero lines directly so axes remain present after a large pan,
        // independently of the neutral grid's bounded line count.
        for (fallback, (along, across)) in [(u, v), (v, u)].into_iter().enumerate() {
            let index = GuideGrid::axis_index(along, fallback);
            if !axes[index] || !grid.visible {
                continue;
            }
            let offset = (plane.origin - center).dot(across);
            for (a, b, alpha) in fade.pieces(offset) {
                scene.lines.push(SceneLine {
                    points: [a, b].into_iter().map(|n| center + across * offset + along * n).collect(),
                    color: axis_colors[index],
                    alpha,
                    width: 1.5 * metrics.scale,
                    on_top: false,
                });
            }
        }
    } else if let Some(plane) = sketch_plane {
        let extent = camera.half_height() * 2.0;
        for (axis, color) in [plane.x, plane.y].into_iter().zip(palette.axes) {
            scene.lines.push(SceneLine {
                points: vec![plane.origin - axis * extent, plane.origin + axis * extent],
                color,
                alpha: 1.0,
                width: 1.5 * metrics.scale,
                on_top: false,
            });
        }
    }
    let [w, h] = metrics.pixels();
    if w > 0.0 && h > 0.0 {
        let unit = 2.0 * camera.half_height() / h;
        let (r, u, _) = camera.basis();
        let origin = camera.target - r * ((w * 0.5 - 40.0 * metrics.scale) * unit) - u * ((h * 0.5 - 40.0 * metrics.scale) * unit);
        for (axis, color) in [Vec3::X, Vec3::Y, Vec3::Z].into_iter().zip(palette.axes) {
            scene.lines.push(SceneLine {
                points: vec![origin, origin + axis * (25.0 * metrics.scale * unit)],
                color,
                alpha: 1.0,
                width: 2.5 * metrics.scale,
                on_top: true,
            });
        }
    }
    scene
}
