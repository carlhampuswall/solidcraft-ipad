//! Display state independent of the engine worker. All navigation uses the shared
//! stateless `view.navigate` implementation; CAD geometry is immutable here.
use std::sync::Arc;

use serde_json::{Value, json};
use solidcraft_engine::{
    Sel,
    geom::{Mesh, Vec3},
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
    generation: u64,
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
            generation: 0,
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
    pub fn selection(&self) -> &[Sel] {
        &self.selection
    }

    /// Request generations are issued before evaluation, and checked at publication.
    /// Closing/replacing a session invalidates outstanding work without kernel preemption.
    pub fn begin_update(&mut self, generation: u64) {
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
        self.model = Some(model);
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
        self.metrics = metrics.validate()?;
        if initial && self.metrics.width_points > 0.0 && self.metrics.height_points > 0.0 {
            self.fit();
        }
        self.contacts.clear();
        self.hover = None;
        self.changed();
        Ok(())
    }

    pub fn command(&mut self, id: &str, params: &Value) -> Result<Value, AdapterError> {
        if let Some(standard) = solidcraft_engine::standard_view(id) {
            self.camera.set_view(standard);
        } else {
            match id {
                "view.navigate" => {
                    let mut p = params.clone();
                    let obj = p.as_object_mut().ok_or_else(|| error("bad_params", "navigation parameters must be an object"))?;
                    obj.insert("camera".into(), json!(self.camera));
                    let value = solidcraft_engine::navigate_camera(&p)?;
                    self.camera = serde_json::from_value(value["camera"].clone()).map_err(|e| error("camera", e.to_string()))?;
                    if value["fit"] == true {
                        self.fit();
                    }
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
                    self.camera.set_view(solidcraft_render::StandardView::Iso);
                    self.fit();
                }
                "view.perspective" => {
                    let enabled = params.get("enabled").and_then(Value::as_bool).ok_or_else(|| error("bad_params", "enabled must be a boolean"))?;
                    let half = self.camera.half_height();
                    self.camera.fov = if enabled { 0.6 } else { 0.0 };
                    self.camera.distance = if enabled { half / 0.3_f64.tan() } else { half / 0.4 };
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

    /// Fit uses shared Camera::fit and navigation zoom. The aspect correction is native
    /// layout policy, not another camera implementation. Small models use shared zoom's
    /// supported 1 µm distance floor; large fits retain the engine's f64 camera math.
    fn fit(&mut self) {
        let Some(model) = &self.model else {
            self.camera = Camera::default();
            return;
        };
        if model.bounds.is_empty() {
            self.camera = Camera::default();
            return;
        }
        self.camera.fit(&model.bounds);
        let r = model.bounds.diagonal() * 0.5;
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
        let bounds = self.model.as_ref().map(|m| m.bounds).unwrap_or_default();
        self.camera.view_proj_bounds(self.metrics.width_points / self.metrics.height_points.max(1.0), &bounds)
    }

    pub(crate) fn pick(&self, points: [f64; 2], expected: Option<u64>) -> Option<Sel> {
        let model = self.model.as_ref()?;
        if self.updating || expected.is_some_and(|r| r != model.geometry_revision) {
            return None;
        }
        let pixel = self.metrics.pixel(points)?;
        model.picks.pick(&self.camera, self.vp(), self.metrics, pixel, self.filter)
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
        let model = self.model.clone()?;
        Some(RenderSnapshot {
            geometry_revision: model.geometry_revision,
            model,
            camera: self.camera,
            view_proj: self.vp(),
            guides: self.guides.clone(),
            highlight: self.highlight.clone(),
            scene_revision: self.scene_revision,
            highlight_revision: self.highlight_revision,
            guide_revision: self.guide_revision,
            selected_bodies: self
                .selection
                .iter()
                .filter_map(|s| match s {
                    Sel::Body { name } => Some(name.clone()),
                    _ => None,
                })
                .collect(),
            hovered_bodies: self
                .hover
                .iter()
                .filter_map(|s| match s {
                    Sel::Body { name } => Some(name.clone()),
                    _ => None,
                })
                .collect(),
            selection_tint: view::Palette::DEFAULT.body_selected.f().map(|v| v as f32),
            hover_tint: view::Palette::DEFAULT.body_selected.f().map(|v| v as f32 * 0.2),
        })
    }

    pub fn diagnostic(&self) -> Value {
        json!({"document_id":self.model.as_ref().map(|m| &m.document_id),
            "geometry_revision":self.model.as_ref().map(|m| m.geometry_revision),
            "revision":self.model.as_ref().map(|m| m.revision),"scene_revision":self.scene_revision,
            "selection":self.selection,"hover":self.hover,"filter":self.filter,"multiple":self.multiple,
            "updating":self.updating,"camera":self.camera})
    }

    pub(crate) fn changed(&mut self) {
        self.scene_revision = self.scene_revision.saturating_add(1);
        if self.guide_camera != Some(self.camera) || self.guide_metrics != self.metrics {
            self.guide_revision = self.guide_revision.saturating_add(1);
            self.guides = Arc::new(guides(&self.camera, self.metrics));
            self.guide_camera = Some(self.camera);
            self.guide_metrics = self.metrics;
        }
        let vertex = self.selection.iter().chain(self.hover.iter()).any(|s| matches!(s, Sel::Vertex { .. }));
        let camera = vertex.then_some(self.camera);
        let height = if vertex { self.metrics.height_points } else { 0.0 };
        let stale = self.highlight_key.as_ref().is_none_or(|key| {
            let same_model = match (&key.model, &self.model) {
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
        });
        if stale {
            self.highlight_revision = self.highlight_revision.saturating_add(1);
            self.highlight = Arc::new(self.highlights());
            self.highlight_key = Some(HighlightKey {
                model: self.model.clone(),
                selection: self.selection.clone(),
                hover: self.hover.clone(),
                camera,
                scale: self.metrics.scale,
                height,
            });
        }
    }

    fn highlights(&self) -> Scene {
        let mut scene = Scene::default();
        let Some(model) = &self.model else { return scene };
        let selected = view::Palette::DEFAULT.body_selected;
        let hover = view::Palette::DEFAULT.sketch_construction;
        for (sel, color) in self.selection.iter().map(|s| (s, selected)).chain(self.hover.iter().map(|s| (s, hover))) {
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
fn guides(camera: &Camera, metrics: ViewportMetrics) -> Scene {
    let mut scene = Scene::default();
    let palette = view::Palette::DEFAULT;
    let (minor, major) = view::grid_step(camera.half_height());
    let plane = view::grid_plane(camera.to_frame(camera.back()), view::GridPlane::Ground);
    let (u, v) = plane.axes();
    let target = camera.to_frame(camera.target);
    let center = u * ((target.dot(u) / major).round() * major) + v * ((target.dot(v) / major).round() * major);
    let slant = camera.to_frame(camera.back()).dot(u.cross(v));
    let ext = view::grid_ext(camera.half_height(), major, slant);
    let fade = view::GridFade::new(camera.half_height(), slant, ext, ext);
    let count = ((fade.end / minor) as i64).clamp(1, 200);
    for i in -count..=count {
        let offset = i as f64 * minor;
        let color = if (offset / major).fract().abs() < 1e-9 { palette.grid_major } else { palette.grid };
        for (along, across) in [(u, v), (v, u)] {
            for (a, b, alpha) in fade.pieces(offset) {
                scene.grid.push(SceneLine {
                    points: vec![camera.from_frame(center + across * offset + along * a), camera.from_frame(center + across * offset + along * b)],
                    color,
                    alpha,
                    width: metrics.scale,
                    on_top: false,
                });
            }
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
