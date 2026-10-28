//! Display state independent of the engine worker. All navigation uses the shared
//! stateless `view.navigate` implementation; CAD geometry is immutable here.
use std::sync::Arc;

use serde_json::{Value, json};
use solidcraft_engine::{
    Sel,
    geom::{Aabb3, Mesh, Plane, Vec3},
    view,
};
use solidcraft_render::{Camera, Mat4, Scene, SceneLine, SceneMesh};

use crate::input::Contacts;
use crate::{AdapterError, PickFilter, SceneSnapshot, ViewportMetrics, error};

pub struct RenderSnapshot {
    pub model: Arc<SceneSnapshot>,
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

struct HighlightKey {
    model: Option<Arc<SceneSnapshot>>,
    selection: Vec<Sel>,
    hover: Option<Sel>,
    camera: Option<Camera>,
    scale: f64,
    height: f64,
    tool_revision: u64,
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
    pub(crate) multiple: bool,
    pub(crate) contacts: Contacts,
    pub(crate) numeric: Option<crate::NumericEdit>,
    pub(crate) modelling: Option<crate::ModelEdit>,
    pub(crate) origin: crate::origin::OriginReferences,
    generation: u64,
    pub(crate) metrics_epoch: u64,
    pub(crate) camera_epoch: u64,
    pub(crate) planar: bool,
    pub(crate) touch_orbit_follows_viewpoint: bool,
    pub(crate) direct_handles: bool,
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
            multiple: false,
            contacts: Contacts::default(),
            numeric: None,
            modelling: None,
            origin: crate::origin::OriginReferences::default(),
            generation: 0,
            metrics_epoch: 0,
            camera_epoch: 0,
            planar: false,
            touch_orbit_follows_viewpoint: true,
            direct_handles: true,
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
        let changed = self.model.as_ref().is_none_or(|old| old.document_id != model.document_id || old.geometry_revision != model.geometry_revision);
        if changed {
            // Shared topology indices and picked points have no persistent identity contract.
            self.selection.clear();
            self.hover = None;
        }
        if changed {
            self.contacts.clear();
            self.numeric = None;
            self.modelling = None;
        }
        if model.sketch.is_none() {
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
            self.cancel_interactions();
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
                    "sketch.line"
                        | "sketch.rectangle.two_point"
                        | "sketch.circle.center"
                        | "sketch.arc.three_point"
                        | "sketch.spline.fit_point"
                        | "sketch.move"
                        | "sketch.dimension"
                        | "sketch.trim"
                        | "sketch.extend"
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
                    self.camera.set_view(solidcraft_render::StandardView::Iso);
                    self.fit();
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
                    self.cancel_interactions();
                    let add = params.get("additive").and_then(Value::as_bool).unwrap_or(self.multiple);
                    self.select(Some(entity), add);
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
        let filter = if self.origin.choosing { PickFilter::Face } else { self.filter };
        let picked = model.picks.pick(&self.camera, self.vp(), self.metrics, pixel, filter);
        if !self.origin.choosing
            && matches!(filter, PickFilter::Auto | PickFilter::Face)
            && !matches!(picked, Some(Sel::Edge { .. } | Sel::Vertex { .. }))
            && let Some(profile) = model.picks.pick_profile(&self.camera, self.vp(), self.metrics, pixel)
        {
            return Some(profile);
        }
        // Explicit topology filters retain their target. In the plane chooser,
        // reference planes compete with body faces using the rendered depth.
        if (self.origin.choosing || picked.is_none())
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

    pub(crate) fn select(&mut self, hit: Option<Sel>, add: bool) {
        if !add {
            self.selection.clear();
        }
        if let Some(hit) = hit {
            if let Some(index) = self.selection.iter().position(|s| same(s, &hit)) {
                if add {
                    self.selection.remove(index);
                }
            } else if self.selection.len() < 1024 {
                self.selection.push(hit);
            }
        }
        self.hover = None;
        self.changed();
    }

    pub fn snapshot(&self) -> Option<RenderSnapshot> {
        let authoritative = self.model.as_ref()?;
        let model = self.display_model().cloned()?;
        Some(RenderSnapshot {
            geometry_revision: authoritative.geometry_revision,
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
                _ => {}
            }
        }
        names.sort();
        names.dedup();
        names
    }

    pub fn diagnostic(&self) -> Value {
        let mut input = self.contacts.diagnostic();
        input["sketch"]["snapping"] =
            json!(self.contacts.sketch.snapping && self.model.as_ref().and_then(|m| m.sketch.as_ref()).is_none_or(|s| !s.sketch.view.no_snap));
        json!({"document_id":self.model.as_ref().map(|m| &m.document_id),
            "geometry_revision":self.model.as_ref().map(|m| m.geometry_revision),
            "revision":self.model.as_ref().map(|m| m.revision),"scene_revision":self.scene_revision,
            "selection":self.selection,"hover":self.hover,"filter":self.filter,"multiple":self.multiple,
            "updating":self.updating,"camera":self.camera,"viewport_epoch":self.metrics_epoch,"camera_epoch":self.camera_epoch,"metrics":self.metrics,
            "volume_mm3":self.model.as_ref().map(|m|m.volume_mm3),"preview_volume_mm3":self.display_model().filter(|m| self.model.as_ref().is_none_or(|a| !Arc::ptr_eq(a,m))).map(|m|m.volume_mm3),"preview":self.contacts.preview.is_some() || self.numeric.as_ref().is_some_and(|e|e.preview.is_some()) || self.modelling.as_ref().is_some_and(|e|e.preview.is_some() || e.last_preview.is_some()),"preview_stale":self.modelling.as_ref().is_some_and(|e|e.preview.is_none() && e.last_preview.is_some()),"numeric_token":self.numeric.as_ref().map(|e|e.token),"model_token":self.modelling.as_ref().map(|e|e.token),
            "input":input,"tool_pending":self.contacts.diagnostic()["tool_pending"],"planar":self.planar,
            "origin":self.origin,"reference_labels":self.reference_labels(),
            "touch_orbit":{"follows_viewpoint":self.touch_orbit_follows_viewpoint},
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
        self.update_reference_feedback();
        self.scene_revision = self.scene_revision.saturating_add(1);
        if self.guide_camera != Some(self.camera) {
            self.camera_epoch = self.camera_epoch.saturating_add(1);
        }
        let plane = self.plane_preview.or_else(|| self.model.as_ref().and_then(|m| m.sketch_plane));
        if self.guide_camera != Some(self.camera) || self.guide_metrics != self.metrics || self.guide_plane != plane || self.origin.dirty {
            self.guide_revision = self.guide_revision.saturating_add(1);
            let mut scene = guides(&self.camera, self.metrics, plane);
            if plane.is_none() {
                self.draw_references(&mut scene);
            }
            self.guides = Arc::new(scene);
            self.origin.dirty = false;
            self.guide_camera = Some(self.camera);
            self.guide_metrics = self.metrics;
            self.guide_plane = plane;
        }
        let vertex = self.handle().is_some()
            || self.modelling.as_ref().is_some_and(|e| e.handle.is_some())
            || self.display_model().is_some_and(|m| m.sketch.is_some())
            || self.selection.iter().chain(self.hover.iter()).any(|s| matches!(s, Sel::Vertex { .. }));
        let camera = vertex.then_some(self.camera);
        let height = if vertex { self.metrics.height_points } else { 0.0 };
        let stale = self.highlight_key.as_ref().is_none_or(|key| {
            let same_model = match (&key.model, self.display_model()) {
                (Some(a), Some(b)) => Arc::ptr_eq(a, b),
                (None, None) => true,
                _ => false,
            };
            !same_model
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
        self.sketch_feedback(&mut scene);
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
        for (sel, color) in self.selection.iter().map(|s| (s, selected)).chain(self.hover.iter().map(|s| (s, hover))) {
            if let Sel::Profile { sketch, index } = sel {
                if let Some(profile) = model.picks.profiles.iter().find(|p| p.sketch == *sketch && p.index == *index) {
                    scene.meshes.push(SceneMesh { mesh: profile.fill.clone(), color, face_colors: Vec::new(), opacity: 0.3 });
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
                    if let Some(points) = mesh.edges.get(*index) {
                        scene.lines.push(line(points.clone()));
                    }
                }
                Sel::Face { index, .. } => {
                    let mut face = Mesh::default();
                    for (ti, tri) in mesh.triangles.iter().enumerate().filter(|(i, _)| mesh.tri_face.get(*i) == Some(&(*index as u32))) {
                        let Some(pts) = mesh.tri(tri) else { continue };
                        let Ok(start) = u32::try_from(face.positions.len()) else { break };
                        if start > u32::MAX - 3 {
                            break;
                        }
                        face.positions.extend(pts);
                        face.normals.extend(tri.iter().map(|i| mesh.normals.get(*i as usize).copied().unwrap_or(Vec3::Z)));
                        face.triangles.push([start, start + 1, start + 2]);
                        face.tri_face.push(*index as u32);
                        let _ = ti;
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
        _ => a == b,
    }
}

/// Reference grid uses the shared spacing/fade/plane helpers. The orientation triad
/// is anchored in the canvas corner and rotates with this same shared camera basis.
fn guides(camera: &Camera, metrics: ViewportMetrics, sketch_plane: Option<Plane>) -> Scene {
    let mut scene = Scene::default();
    let palette = view::Palette::DEFAULT;
    let (minor, major) = view::grid_step(camera.half_height());
    let plane = view::grid_plane(camera.to_frame(camera.back()), view::GridPlane::Ground);
    let (u, v) = sketch_plane.map(|p| (p.x, p.y)).unwrap_or_else(|| plane.axes());
    let origin = sketch_plane.map_or(Vec3::ZERO, |p| p.origin);
    let target = if sketch_plane.is_some() { camera.target - origin } else { camera.to_frame(camera.target) };
    let center = u * ((target.dot(u) / major).round() * major) + v * ((target.dot(v) / major).round() * major);
    let slant = if sketch_plane.is_some() { camera.back().dot(u.cross(v)) } else { camera.to_frame(camera.back()).dot(u.cross(v)) };
    let ext = view::grid_ext(camera.half_height(), major, slant);
    let fade = view::GridFade::new(camera.half_height(), slant, ext, ext);
    let count = ((fade.end / minor) as i64).clamp(1, 200);
    for i in -count..=count {
        let offset = i as f64 * minor;
        let color = if (offset / major).fract().abs() < 1e-9 { palette.grid_major } else { palette.grid };
        for (along, across) in [(u, v), (v, u)] {
            for (a, b, alpha) in fade.pieces(offset) {
                scene.grid.push(SceneLine {
                    points: [a, b]
                        .into_iter()
                        .map(|n| {
                            let p = center + across * offset + along * n;
                            if sketch_plane.is_some() { origin + p } else { camera.from_frame(p) }
                        })
                        .collect(),
                    color,
                    alpha,
                    width: metrics.scale,
                    on_top: false,
                });
            }
        }
    }
    if let Some(plane) = sketch_plane {
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
