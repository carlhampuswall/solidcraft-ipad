//! Native presentation and picking of existing engine reference selections.
//! These guides own no document geometry and never create a sketch themselves.
use std::sync::Arc;

use serde::Serialize;
use serde_json::{Value, json};
use solidcraft_engine::{
    Sel,
    geom::{Aabb3, Mesh, Plane, Vec3},
    view::Palette,
};
use solidcraft_render::{Rgb, Scene, SceneLine, SceneMesh};

use crate::{AdapterError, Viewport, error, picking::project};

pub(crate) const ITEMS: [&str; 7] = ["O", "X", "Y", "Z", "XY", "XZ", "YZ"];

#[derive(Serialize)]
pub(crate) struct OriginReferences {
    pub visible: [bool; 7],
    pub choosing: bool,
    #[serde(skip)]
    overrides: [Option<bool>; 7],
    #[serde(skip)]
    pub dirty: bool,
    #[serde(skip)]
    selection: Vec<Sel>,
    #[serde(skip)]
    hover: Option<Sel>,
    #[serde(skip)]
    active: bool,
}

impl Default for OriginReferences {
    fn default() -> Self {
        Self { visible: [false; 7], choosing: false, overrides: [None; 7], dirty: true, selection: Vec::new(), hover: None, active: false }
    }
}

impl OriginReferences {
    fn refresh_visibility(&mut self) {
        self.visible = self.overrides.map(|value| value.unwrap_or(self.choosing));
        self.dirty = true;
    }

    pub fn set_choosing(&mut self, choosing: bool) {
        self.choosing = choosing;
        self.refresh_visibility();
    }

    pub fn update_feedback(&mut self, selection: &[Sel], hover: &Option<Sel>, active: bool) {
        if self.selection != selection || self.hover != *hover || self.active != active {
            self.selection = selection.to_vec();
            self.hover = hover.clone();
            self.active = active;
            self.dirty = true;
        }
    }
}

impl Viewport {
    pub(crate) fn clear_hidden_reference_selection(&mut self) {
        self.selection = self
            .selection
            .iter()
            .filter(|entity| match entity {
                Sel::Plane { .. } | Sel::Axis { .. } => self.reference_available(entity),
                Sel::Vertex { body, .. } if body.is_empty() => self.reference_available(entity),
                _ => true,
            })
            .cloned()
            .collect();
    }

    pub(crate) fn origin_command(&mut self, id: &str, params: &Value) -> Result<bool, AdapterError> {
        match id {
            "view.origin" => {
                let visible = match params.get("visible") {
                    Some(Value::Null) => None,
                    Some(Value::Bool(value)) => Some(*value),
                    _ => return Err(error("bad_params", "ui.ipados.sketch.reference_visibility_boolean")),
                };
                let range = if let Some(group) = params.get("group") {
                    if params.get("item").is_some() {
                        return Err(error("bad_params", "ui.ipados.sketch.reference_unknown"));
                    }
                    match group.as_str() {
                        Some("axes") => 1..4,
                        Some("planes") => 4..7,
                        _ => return Err(error("bad_params", "ui.ipados.sketch.reference_unknown")),
                    }
                } else if let Some(item) = params.get("item") {
                    let index = item
                        .as_str()
                        .and_then(|name| ITEMS.iter().position(|item| *item == name))
                        .ok_or_else(|| error("bad_params", "ui.ipados.sketch.reference_unknown"))?;
                    index..index + 1
                } else {
                    0..ITEMS.len()
                };
                self.origin.overrides.get_mut(range).ok_or_else(|| error("bad_params", "ui.ipados.sketch.reference_unknown"))?.fill(visible);
                self.origin.refresh_visibility();
                self.selection.retain(|s| match s {
                    Sel::Plane { .. } | Sel::Axis { .. } => false,
                    Sel::Vertex { body, .. } => !body.is_empty(),
                    _ => true,
                });
                self.cancel_interactions();
            }
            "view.choose_sketch_plane" => {
                if self.updating || self.numeric.is_some() || self.modelling.is_some() || self.model.as_ref().is_none_or(|m| m.sketch.is_some()) {
                    return Err(error("disabled", "ui.ipados.workspace.finish_edit"));
                }
                self.cancel_interactions();
                if self.plane_camera.is_none() {
                    self.plane_camera = Some((self.camera, self.planar));
                }
                self.plane_preview = None;
                self.origin.set_choosing(true);
                self.contacts.radius_tool = false;
                self.planar = false;
                self.selection.clear();
                // Origin references may lie outside a model moved far from zero.
                self.camera.set_view(solidcraft_render::StandardView::Iso);
                self.fit_bounds(self.reference_bounds());
            }
            _ => return Ok(false),
        }
        self.origin.dirty = true;
        self.changed();
        Ok(true)
    }

    fn references_active(&self) -> bool {
        self.model.as_ref().is_some_and(|m| m.sketch.is_none()) && self.plane_preview.is_none() && self.numeric.is_none() && self.modelling.is_none()
    }

    pub(crate) fn update_reference_feedback(&mut self) {
        self.origin.update_feedback(&self.selection, &self.hover, self.references_active());
    }

    fn reference_size(&self) -> f64 {
        self.model.as_ref().map_or(25.0, |m| (m.bounds.diagonal() * 0.4).clamp(25.0, 1e9))
    }

    fn reference_planes(&self) -> Vec<(String, Plane)> {
        if !self.references_active() {
            return Vec::new();
        }
        let mut planes: Vec<_> = [("XY", Plane::XY), ("XZ", Plane::XZ), ("YZ", Plane::YZ)]
            .into_iter()
            .zip(self.origin.visible.iter().skip(4))
            .filter(|(_, visible)| **visible)
            .map(|((name, plane), _)| (name.to_string(), plane))
            .collect();
        if self.origin.choosing
            && let Some(model) = &self.model
        {
            planes.extend(model.construction_planes.iter().cloned());
        }
        planes
    }

    fn corners(&self, name: &str, plane: Plane) -> [Vec3; 4] {
        let size = self.reference_size();
        let x = plane.x * size;
        // XZ's native sketch Y points down; origin guides extend toward world +Z.
        let y = plane.y * if name == "XZ" { -size } else { size };
        [plane.origin, plane.origin + x, plane.origin + x + y, plane.origin + y]
    }

    pub(crate) fn reference_bounds(&self) -> Aabb3 {
        let mut bounds = self.display_model().map(|m| m.bounds).unwrap_or_default();
        if self.references_active() {
            for (name, plane) in self.reference_planes() {
                for point in self.corners(&name, plane) {
                    bounds.add(point);
                }
            }
            for (index, axis) in [Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z].into_iter().enumerate() {
                if self.origin.visible.get(index) == Some(&true) {
                    bounds.add(Vec3::ZERO);
                    bounds.add(axis * self.reference_size() * 1.2);
                }
            }
        }
        bounds
    }

    pub(crate) fn reference_available(&self, entity: &Sel) -> bool {
        if !self.references_active() {
            return false;
        }
        match entity {
            Sel::Plane { name } => self.reference_planes().iter().any(|(n, _)| n == name),
            Sel::Axis { name } => {
                ITEMS.iter().position(|n| *n == name).is_some_and(|i| (1..4).contains(&i) && self.origin.visible.get(i) == Some(&true))
            }
            Sel::Vertex { body, point } => body.is_empty() && *point == Vec3::ZERO && self.origin.visible.first() == Some(&true),
            _ => false,
        }
    }

    pub(crate) fn pick_reference(&self, points: [f64; 2]) -> Option<Sel> {
        if !self.references_active() {
            return None;
        }
        let model = self.model.as_ref()?;
        let pixel = self.metrics.pixel(points)?;
        let [w, h] = self.metrics.pixels();
        let screen = |p| project(self.vp(), p, [w, h]);
        let visible = |p| model.picks.visible(&self.camera, self.vp(), self.metrics, p);
        let tolerance = 10.0 * self.metrics.scale;
        if !self.origin.choosing {
            if self.origin.visible.first() == Some(&true)
                && visible(Vec3::ZERO)
                && screen(Vec3::ZERO).is_some_and(|p| (p[0] - pixel[0]).hypot(p[1] - pixel[1]) <= tolerance)
            {
                return Some(Sel::Vertex { body: String::new(), point: Vec3::ZERO });
            }
            let start = screen(Vec3::ZERO)?;
            for (index, axis) in [Vec3::X, Vec3::Y, Vec3::Z].into_iter().enumerate() {
                if self.origin.visible.get(index + 1) != Some(&true) {
                    continue;
                }
                let end_world = axis * self.reference_size() * 1.2;
                let Some(end) = screen(end_world) else { continue };
                let delta = [end[0] - start[0], end[1] - start[1]];
                let length2 = delta[0] * delta[0] + delta[1] * delta[1];
                if length2 < tolerance * tolerance {
                    continue;
                }
                let t = (((pixel[0] - start[0]) * delta[0] + (pixel[1] - start[1]) * delta[1]) / length2).clamp(0.0, 1.0);
                if (pixel[0] - start[0] - t * delta[0]).hypot(pixel[1] - start[1] - t * delta[1]) <= tolerance && visible(end_world * t) {
                    return Some(Sel::Axis { name: ITEMS.get(index + 1)?.to_string() });
                }
            }
        }
        let (origin, direction) = model.picks.ray(&self.camera, pixel[0], pixel[1], w, h);
        self.reference_planes()
            .into_iter()
            .filter_map(|(name, plane)| {
                // Edge-on planes have no usable target area.
                if plane.normal().dot(direction).abs() < 0.04 {
                    return None;
                }
                let hit = plane.intersect_ray(origin, direction)?;
                let p = plane.to_local(hit);
                let y = if name == "XZ" { -p.y } else { p.y };
                let size = self.reference_size();
                let depth = (hit - origin).dot(direction);
                ((0.0..=size).contains(&p.x) && (0.0..=size).contains(&y) && screen(hit).is_some() && visible(hit))
                    .then_some((depth, Sel::Plane { name }))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, entity)| entity)
    }

    fn reference_color(&self, entity: &Sel, color: Rgb) -> Rgb {
        if self.selection.contains(entity) {
            Palette::DEFAULT.body_selected
        } else if self.hover.as_ref() == Some(entity) {
            Palette::DEFAULT.sketch_construction
        } else {
            color
        }
    }

    pub(crate) fn draw_references(&self, scene: &mut Scene) {
        if !self.references_active() {
            return;
        }
        let palette = Palette::DEFAULT;
        for (name, plane) in self.reference_planes() {
            let entity = Sel::Plane { name: name.clone() };
            let base = match name.as_str() {
                "XY" => palette.axes[2],
                "XZ" => palette.axes[1],
                "YZ" => palette.axes[0],
                _ => palette.sketch_construction,
            };
            let color = self.reference_color(&entity, base);
            let points = self.corners(&name, plane);
            let mesh =
                Mesh { positions: points.to_vec(), normals: vec![plane.normal(); 4], triangles: vec![[0, 1, 2], [0, 2, 3]], ..Mesh::default() };
            scene.meshes.push(SceneMesh { mesh: Arc::new(mesh), color, face_colors: Vec::new(), opacity: if color != base { 0.4 } else { 0.16 } });
            scene.lines.push(SceneLine {
                points: points.into_iter().chain(points.first().copied()).collect(),
                color,
                alpha: 0.85,
                width: 1.5 * self.metrics.scale,
                on_top: false,
            });
        }
        for (index, axis) in [Vec3::X, Vec3::Y, Vec3::Z].into_iter().enumerate() {
            if self.origin.visible.get(index + 1) != Some(&true) {
                continue;
            }
            let Some(name) = ITEMS.get(index + 1) else { continue };
            let color = self.reference_color(&Sel::Axis { name: name.to_string() }, palette.axes[index]);
            scene.lines.push(SceneLine {
                points: vec![Vec3::ZERO, axis * self.reference_size() * 1.2],
                color,
                alpha: 1.0,
                width: 2.5 * self.metrics.scale,
                on_top: false,
            });
        }
        if self.origin.visible.first() == Some(&true) {
            let color = self.reference_color(&Sel::Vertex { body: String::new(), point: Vec3::ZERO }, palette.edge);
            let (right, up, _) = self.camera.basis();
            let radius = 4.0 * 2.0 * self.camera.half_height() / self.metrics.height_points.max(1.0);
            let positions: Vec<_> = (0..16)
                .map(|i| {
                    let angle = f64::from(i) * std::f64::consts::TAU / 16.0;
                    (right * angle.cos() + up * angle.sin()) * radius
                })
                .collect();
            let mesh =
                Mesh { positions, normals: vec![self.camera.back(); 16], triangles: (1..15).map(|i| [0, i, i + 1]).collect(), ..Mesh::default() };
            scene.meshes.push(SceneMesh::solid(Arc::new(mesh), color));
        }
    }

    pub(crate) fn reference_labels(&self) -> Vec<Value> {
        if !self.references_active() {
            return Vec::new();
        }
        let mut labels = Vec::new();
        let mut add = |name: &str, entity: Sel, world: Vec3| {
            if let Some(p) = project(self.vp(), world, self.metrics.pixels()) {
                labels.push(json!({"name":name,"entity":entity,"position":[p[0]/self.metrics.scale,p[1]/self.metrics.scale],"selected":self.selection.contains(&entity)}));
            }
        };
        for (name, plane) in self.reference_planes() {
            let corners = self.corners(&name, plane);
            add(&name, Sel::Plane { name: name.clone() }, corners[2]);
        }
        for (index, axis) in [Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z].into_iter().enumerate() {
            if self.origin.visible.get(index) != Some(&true) {
                continue;
            }
            let Some(name) = ITEMS.get(index) else { continue };
            let entity = if index == 0 { Sel::Vertex { body: String::new(), point: Vec3::ZERO } } else { Sel::Axis { name: name.to_string() } };
            add(name, entity, axis * self.reference_size() * 1.2);
        }
        labels
    }
}
