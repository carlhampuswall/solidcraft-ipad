//! Platform-sized coordinates and topology-aware picking on shared engine tessellation.
//! The egui picker cannot be linked by the native host. Reuse its underlying MeshIndex,
//! Camera and B-rep face/edge mappings, with only screen tolerance/occlusion policy here.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use solidcraft_engine::{
    Sel,
    geom::{Aabb3, Mesh, MeshIndex, Plane, Region2, Vec3},
};
use solidcraft_render::{Camera, Mat4, Scene};

use crate::{AdapterError, error};

#[derive(Clone, Copy, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ViewportMetrics {
    pub width_points: f64,
    pub height_points: f64,
    pub scale: f64,
    #[serde(default)]
    pub origin_points: [f64; 2],
}

impl Default for ViewportMetrics {
    fn default() -> Self {
        Self { width_points: 0.0, height_points: 0.0, scale: 1.0, origin_points: [0.0; 2] }
    }
}

impl ViewportMetrics {
    pub fn validate(self) -> Result<Self, AdapterError> {
        if ![self.width_points, self.height_points, self.scale, self.origin_points[0], self.origin_points[1]].iter().all(|x| x.is_finite())
            || !(0.0..=16384.0).contains(&self.width_points)
            || !(0.0..=16384.0).contains(&self.height_points)
            || !(0.1..=8.0).contains(&self.scale)
            || self.width_points * self.scale > 16384.0
            || self.height_points * self.scale > 16384.0
        {
            return Err(error("viewport", "invalid viewport size, origin or scale"));
        }
        Ok(self)
    }

    /// Window points to drawable pixels. UIKit normally supplies view-local points, with
    /// zero origin; explicit origins support embedded viewports and deterministic tests.
    pub fn pixel(self, points: [f64; 2]) -> Option<[f64; 2]> {
        let p = [(points[0] - self.origin_points[0]) * self.scale, (points[1] - self.origin_points[1]) * self.scale];
        let [w, h] = self.pixels();
        (self.width_points > 0.0 && self.height_points > 0.0 && p.iter().all(|x| x.is_finite()) && p[0] >= 0.0 && p[1] >= 0.0 && p[0] < w && p[1] < h)
            .then_some(p)
    }

    pub fn pixels(self) -> [f64; 2] {
        [self.width_points * self.scale, self.height_points * self.scale]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PickFilter {
    #[default]
    Auto,
    Body,
    Occurrence,
    Face,
    Edge,
    Vertex,
}

pub(crate) struct PickBody {
    pub name: String,
    pub mesh: Arc<Mesh>,
    index: MeshIndex,
}

pub(crate) struct PickProfile {
    pub sketch: u64,
    pub index: usize,
    pub plane: Plane,
    region: Region2,
    pub fill: Arc<Mesh>,
    pub boundaries: Vec<Vec<Vec3>>,
}

pub struct PickScene {
    bounds: Aabb3,
    pub(crate) bodies: Vec<PickBody>,
    pub(crate) profiles: Vec<PickProfile>,
}

impl PickScene {
    /// Endpoint connectivity stays within the picked body/occurrence. Never use
    /// intermediate tessellation samples: an edge touching a middle is separate.
    pub(crate) fn connected_edges(&self, selection: &Sel) -> Option<Vec<Sel>> {
        let Sel::Edge { body, index, .. } = selection else { return None };
        let mesh = &self.bodies.iter().find(|candidate| &candidate.name == body)?.mesh;
        let ends = mesh
            .edges
            .iter()
            .enumerate()
            .filter(|(i, _)| mesh.edge_pickable(*i))
            .filter_map(|(i, points)| Some((i, [*points.first()?, *points.last()?])))
            .collect::<Vec<_>>();
        endpoint_chain(&ends, *index)?
            .into_iter()
            .map(|edge| {
                if edge == *index {
                    return Some(selection.clone());
                }
                let points = mesh.edges.get(edge)?;
                Some(Sel::Edge { body: body.clone(), index: edge, point: *points.get(points.len() / 2)? })
            })
            .collect()
    }

    /// In the same order as Scene::meshes, including occurrence-qualified engine names.
    pub fn body_names(&self) -> Vec<String> {
        self.bodies.iter().map(|b| b.name.clone()).collect()
    }

    /// Match desktop surface and circular-rim selection across kernel splits.
    /// Keep the original pick first, including the placement point tools consume.
    pub(crate) fn grouped_selection(&self, selection: &Sel) -> Vec<Sel> {
        let (Sel::Face { body, index, .. } | Sel::Edge { body, index, .. }) = selection else { return vec![selection.clone()] };
        let Some(body_mesh) = self.bodies.iter().find(|candidate| &candidate.name == body) else { return vec![selection.clone()] };
        let mesh = &body_mesh.mesh;
        if matches!(selection, Sel::Edge { .. }) {
            let mut rim = mesh.circular_rim(*index);
            rim.sort_by_key(|edge| usize::from(edge != index));
            return rim
                .into_iter()
                .filter_map(|edge| {
                    if edge == *index {
                        return Some(selection.clone());
                    }
                    let points = mesh.edges.get(edge)?;
                    Some(Sel::Edge { body: body.clone(), index: edge, point: *points.get(points.len() / 2)? })
                })
                .collect();
        }
        let mut patch = mesh.surface_patch(*index);
        patch.sort_by_key(|face| usize::from(face != index));
        patch
            .into_iter()
            .filter_map(|face| {
                if face == *index {
                    return Some(selection.clone());
                }
                mesh.triangles.iter().zip(&mesh.tri_face).find_map(|(triangle, id)| {
                    if *id as usize != face {
                        return None;
                    }
                    let [a, b, c] = mesh.tri(triangle)?;
                    Some(Sel::Face { body: body.clone(), index: face, point: (a + b + c) / 3. })
                })
            })
            .collect()
    }

    pub(crate) fn new(scene: &Scene, names: Vec<String>, bounds: Aabb3) -> Result<Self, AdapterError> {
        if scene.meshes.len() != names.len() {
            return Err(error("entity_mapping", "visible bodies and renderer meshes differ"));
        }
        Ok(Self {
            bounds,
            bodies: scene.meshes.iter().zip(names).map(|(m, name)| PickBody { name, index: MeshIndex::new(&m.mesh), mesh: m.mesh.clone() }).collect(),
            profiles: Vec::new(),
        })
    }

    /// Called on the engine worker with the same visible, world-space sketch state
    /// used by the shared scene builder. Profile detection and fill stay off input.
    pub(crate) fn add_profiles(&mut self, sketch: &solidcraft_engine::doc::SolvedSketch) {
        for (index, profile) in sketch.profiles.iter().enumerate() {
            let region = &profile.region;
            let mut fill = Mesh::default();
            for tri in region.triangulate(1e-2) {
                let Ok(start) = u32::try_from(fill.positions.len()) else { break };
                if start > u32::MAX - 3 {
                    break;
                }
                fill.positions.extend(tri.map(|p| sketch.plane.to_world(p)));
                fill.normals.extend([sketch.plane.normal(); 3]);
                fill.triangles.push([start, start + 1, start + 2]);
            }
            let boundaries = std::iter::once(&region.outer)
                .chain(&region.holes)
                .map(|boundary| {
                    let mut points: Vec<_> = boundary.polyline(1e-2).into_iter().map(|p| sketch.plane.to_world(p)).collect();
                    if let Some(first) = points.first().copied() {
                        points.push(first);
                    }
                    points
                })
                .collect();
            self.profiles.push(PickProfile {
                sketch: sketch.feature,
                index,
                plane: sketch.plane,
                region: region.clone(),
                fill: Arc::new(fill),
                boundaries,
            });
        }
    }

    pub(crate) fn pick_profile(&self, camera: &Camera, vp: Mat4, metrics: ViewportMetrics, pixel: [f64; 2]) -> Option<Sel> {
        let [w, h] = metrics.pixels();
        let (origin, direction) = self.ray(camera, pixel[0], pixel[1], w, h);
        let solid_depth = self.nearest(origin, direction).map(|(depth, _, _)| depth);
        self.profiles
            .iter()
            .filter_map(|profile| {
                let point = profile.plane.intersect_ray(origin, direction)?;
                let depth = (point - origin).dot(direction);
                let epsilon = camera.distance.abs().max(point.len()).max(1.) * 1e-7;
                if depth < 0.
                    || solid_depth.is_some_and(|solid| depth > solid + epsilon)
                    || project(vp, point, [w, h]).is_none()
                    || !profile.region.contains(profile.plane.to_local(point))
                {
                    return None;
                }
                Some((depth, profile))
            })
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.sketch.cmp(&b.1.sketch)).then(a.1.index.cmp(&b.1.index)))
            .map(|(_, profile)| Sel::Profile { sketch: profile.sketch, index: profile.index })
    }

    fn nearest(&self, o: Vec3, d: Vec3) -> Option<(f64, usize, usize)> {
        self.bodies
            .iter()
            .enumerate()
            .filter_map(|(i, body)| body.index.raycast(&body.mesh, o, d).map(|(t, tri)| (t, i, tri)))
            .min_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)))
    }

    /// Filter priority is deterministic: vertex, edge, face (or explicitly the body).
    /// Only engine B-rep edges and their endpoints are selectable, never triangulation
    /// vertices or display-only mesh outlines. Coordinates are already in world space:
    /// Session::world_state and view::scene applied body/occurrence transforms once.
    pub fn pick(&self, camera: &Camera, vp: Mat4, metrics: ViewportMetrics, pixel: [f64; 2], filter: PickFilter) -> Option<Sel> {
        let [w, h] = metrics.pixels();
        if w < 1.0 || h < 1.0 {
            return None;
        }
        let (o, d) = self.ray(camera, pixel[0], pixel[1], w, h);
        let face = self.nearest(o, d).and_then(|(t, bi, ti)| {
            let body = self.bodies.get(bi)?;
            let point = o + d * t;
            project(vp, point, [w, h])?;
            if matches!(filter, PickFilter::Body | PickFilter::Occurrence) {
                Some(Sel::Body { name: body.name.clone() })
            } else {
                let index = *body.mesh.tri_face.get(ti)? as usize;
                Some(Sel::Face { body: body.name.clone(), index, point })
            }
        });
        if matches!(filter, PickFilter::Body | PickFilter::Occurrence | PickFilter::Face) {
            return face;
        }
        let tolerance = 8.0 * metrics.scale;
        // A conservative world-space cone, then exact screen tests on the candidates.
        let (o2, d2) = self.ray(camera, pixel[0] + tolerance * 1.5, pixel[1] + tolerance * 1.5, w, h);
        let (reach_a, reach_b) = (o.dist(o2) * 1.5, d.dist(d2) * 1.5);
        let mut edge: Option<(f64, usize, usize, Vec3)> = None;
        let mut vertex: Option<(f64, usize, usize, Vec3)> = None;
        for (bi, body) in self.bodies.iter().enumerate() {
            let mut candidates = Vec::new();
            body.index.segments_near(o, d, reach_a, reach_b, |ei, si| candidates.push((ei, si)));
            candidates.sort_unstable();
            for (ei, si) in candidates {
                if !body.mesh.edge_pickable(ei) {
                    continue;
                }
                let Some(points) = body.mesh.edges.get(ei) else { continue };
                let (Some(a), Some(b)) = (points.get(si), points.get(si + 1).or(points.get(si))) else { continue };
                if filter != PickFilter::Edge {
                    for p in [points.first(), points.last()].into_iter().flatten() {
                        let Some(q) = project(vp, *p, [w, h]) else { continue };
                        let dist = (q[0] - pixel[0]).hypot(q[1] - pixel[1]);
                        if dist <= tolerance && vertex.as_ref().is_none_or(|best| dist < best.0) && self.visible(camera, vp, metrics, *p) {
                            vertex = Some((dist, bi, ei, *p));
                        }
                    }
                }
                if filter == PickFilter::Vertex {
                    continue;
                }
                let (Some(qa), Some(qb)) = (project(vp, *a, [w, h]), project(vp, *b, [w, h])) else { continue };
                let (dx, dy) = (qb[0] - qa[0], qb[1] - qa[1]);
                let denom = dx * dx + dy * dy;
                let t = if denom > 1e-20 { (((pixel[0] - qa[0]) * dx + (pixel[1] - qa[1]) * dy) / denom).clamp(0.0, 1.0) } else { 0.0 };
                let dist = (qa[0] + dx * t - pixel[0]).hypot(qa[1] + dy * t - pixel[1]);
                // Perspective-correct interpolation on a projected B-rep polyline segment.
                let aw = vp.apply(*a)[3];
                let bw = vp.apply(*b)[3];
                let weight = (t / bw) / ((1.0 - t) / aw + t / bw);
                let point = *a + (*b - *a) * weight;
                if dist <= tolerance && edge.as_ref().is_none_or(|best| dist < best.0) && self.visible(camera, vp, metrics, point) {
                    edge = Some((dist, bi, ei, point));
                }
            }
        }
        if let Some((_, bi, _, point)) = vertex {
            return self.bodies.get(bi).map(|b| Sel::Vertex { body: b.name.clone(), point });
        }
        if let Some((_, bi, index, point)) = edge {
            return self.bodies.get(bi).map(|b| Sel::Edge { body: b.name.clone(), index, point });
        }
        (filter == PickFilter::Auto).then_some(face).flatten()
    }

    /// Start the shared screen ray on the same near plane the renderer uses. An
    /// orthographic zoom can place Camera::ray's usual origin inside the model;
    /// moving along that same ray to clip_bounds avoids selecting its back face.
    pub(crate) fn ray(&self, camera: &Camera, x: f64, y: f64, w: f64, h: f64) -> (Vec3, Vec3) {
        let (origin, direction) = camera.ray(x, y, w, h);
        let forward = -camera.back();
        let depth = (origin - camera.eye()).dot(forward);
        let near = camera.clip_bounds(&self.bounds).0;
        let along = direction.dot(forward);
        if along.abs() <= 1e-12 {
            return (origin, direction);
        }
        (origin + direction * ((near - depth) / along), direction)
    }

    pub(crate) fn visible(&self, camera: &Camera, vp: Mat4, metrics: ViewportMetrics, point: Vec3) -> bool {
        let [w, h] = metrics.pixels();
        let Some(q) = project(vp, point, [w, h]) else { return false };
        let (o, d) = self.ray(camera, q[0], q[1], w, h);
        let depth = (point - o).dot(d);
        // A numerical allowance, not a percentage of the whole design: a rear edge near
        // a front face must not win merely because the model is large.
        let epsilon = camera.distance.abs().max(point.len()).max(1.0) * 1e-7;
        self.nearest(o, d).is_none_or(|(t, _, _)| depth <= t + epsilon)
    }
}

/// Numerical coincidence in model units, independent of screen zoom. Sort only
/// endpoints, then visit their neighbours; crossings and control points are absent.
/// Refuse an oversized component atomically rather than selecting a partial chain.
pub(crate) fn endpoint_chain(edges: &[(usize, [Vec3; 2])], seed: usize) -> Option<Vec<usize>> {
    const TOLERANCE: f64 = 1e-7;
    let ends: BTreeMap<_, _> = edges.iter().copied().filter(|(_, points)| points.iter().all(|p| p.is_finite())).collect();
    let mut sorted: Vec<_> = ends.iter().flat_map(|(edge, points)| points.iter().map(move |point| (*point, *edge))).collect();
    sorted.sort_by(|a, b| a.0.x.total_cmp(&b.0.x).then(a.1.cmp(&b.1)));
    let mut found = BTreeSet::from([seed]);
    let mut todo = vec![seed];
    while let Some(edge) = todo.pop() {
        let Some(points) = ends.get(&edge) else { continue };
        for point in points {
            let start = sorted.partition_point(|(other, _)| other.x < point.x - TOLERANCE);
            for (other, next) in sorted.iter().skip(start).take_while(|(other, _)| other.x <= point.x + TOLERANCE) {
                if point.dist(*other) <= TOLERANCE && found.insert(*next) {
                    if found.len() > 1024 {
                        return None;
                    }
                    todo.push(*next);
                }
            }
        }
    }
    Some(std::iter::once(seed).chain(found.into_iter().filter(|edge| *edge != seed)).collect())
}

pub(crate) fn project(vp: Mat4, p: Vec3, [w, h]: [f64; 2]) -> Option<[f64; 2]> {
    let q = vp.apply(p);
    if !q.iter().all(|x| x.is_finite()) || q[3] <= 1e-12 {
        return None;
    }
    let z = q[2] / q[3];
    ((-1e-7..=1.0 + 1e-7).contains(&z)).then_some([(q[0] / q[3] + 1.0) * 0.5 * w, (1.0 - q[1] / q[3]) * 0.5 * h])
}
