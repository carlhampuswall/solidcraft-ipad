//! Safe native integration: one engine owner and immutable, revisioned viewport data.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

mod assembly;
mod assembly_references;
mod browser;
mod constraint_input;
mod construction;
mod editing;
mod feature_selection;
mod input;
mod modelling;
mod origin;
mod picking;
mod recovery;
mod references;
mod sketch_attachment;
mod sketch_input;
mod sketch_projection;
mod sketching;
mod viewport;
mod workspace;

pub use editing::{PreparedTool, RadiusHandle, ToolEvent, ToolOperation, ToolPhase, ToolWork};
pub use input::{InputEvent, InputPhase, InputSource};
pub use modelling::{ModelEdit, PreparedModel};
pub use picking::{PickFilter, PickScene, ViewportMetrics};
pub use sketch_projection::SketchProjection;
pub use sketching::SketchSnapshot;
pub use solidcraft_engine::geom::Vec3 as WorldPoint;
pub use viewport::{RenderSnapshot, Viewport};
pub use workspace::{NumericEdit, PreparedNumeric};

use std::sync::Arc;

use serde::Serialize;
use serde_json::{Value, json};
use solidcraft_engine::{EngineError, Sel, Session, doc::Document, geom::Aabb3, view};
use solidcraft_render::Scene;

#[derive(Debug, Serialize, thiserror::Error)]
#[error("{message}")]
pub struct AdapterError {
    pub code: &'static str,
    pub message: String,
}

pub(crate) fn error(code: &'static str, message: impl Into<String>) -> AdapterError {
    AdapterError { code, message: message.into() }
}

impl From<EngineError> for AdapterError {
    fn from(error: EngineError) -> Self {
        let code = match &error {
            EngineError::UnknownCommand(_) => "unknown_command",
            EngineError::Disabled(_, _) => "disabled",
            EngineError::BadParams { .. } => "bad_params",
            EngineError::Internal(_, _) => "internal",
            EngineError::Other(_) => "engine",
        };
        Self { code, message: error.to_string() }
    }
}

#[derive(Debug, Serialize)]
pub struct CommandReply {
    pub value: Value,
    pub document_id: String,
    pub revision: u64,
    pub geometry_revision: u64,
    pub changed: bool,
}

/// Scene and picking index are built together on the evaluation worker, from the same
/// visible world state. No engine borrow or mutable geometry crosses to the renderer.
#[derive(Clone)]
pub struct NativeMeshInstance {
    pub body: String,
    pub mesh: Arc<solidcraft_engine::geom::Mesh>,
    pub transform: solidcraft_engine::doc::Mat,
}

pub struct SceneSnapshot {
    pub scene: Arc<Scene>,
    /// Display-only operation colours and material context. Picking, volume and
    /// accepted documents continue to use the evaluated result above.
    pub preview_scene: Option<Arc<PreviewScene>>,
    pub picks: PickScene,
    pub bounds: Aabb3,
    pub document_id: String,
    pub revision: u64,
    pub geometry_revision: u64,
    pub body_count: usize,
    pub triangle_count: usize,
    pub volume_mm3: f64,
    pub radius_handle: Option<RadiusHandle>,
    pub feature_ids: Vec<u64>,
    pub(crate) feature_feedback: std::collections::BTreeMap<u64, feature_selection::FeatureFeedback>,
    pub sketch: Option<SketchSnapshot>,
    pub(crate) projection_sources: Vec<sketch_projection::ProjectionSource>,
    pub sketch_plane: Option<solidcraft_engine::geom::Plane>,
    pub construction_planes: Vec<(String, solidcraft_engine::geom::Plane)>,
    pub occurrence_paths: std::collections::BTreeMap<String, Vec<u64>>,
    pub occurrences: Vec<Vec<u64>>,
    pub joint_frames: Vec<solidcraft_engine::doc::Mat>,
    pub instances: Vec<NativeMeshInstance>,
}

pub struct PreviewScene {
    pub scene: Arc<Scene>,
    /// Empty names are non-pickable preview tool/context meshes.
    pub names: Vec<String>,
}

/// The only mutable CAD document/session owner. Camera/input live in `Viewport` so
/// navigation and lifecycle suspension never wait for evaluation or tessellation.
pub struct SessionAdapter {
    session: Session,
    geometry_revision: u64,
    workspace_cache: Option<workspace::WorkspaceCache>,
}

impl Default for SessionAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionAdapter {
    pub fn new() -> Self {
        Self { session: Session::new(Document::new("SolidCraft")), geometry_revision: 1, workspace_cache: None }
    }

    pub fn execute(&mut self, id: &str, params: Value) -> Result<CommandReply, AdapterError> {
        let before = (self.session.document_id.clone(), self.session.revision);
        let doc = self.session.doc.clone();
        // Native history navigation still uses the engine's marker command and
        // evaluation, but must neither record an undo step nor discard redo.
        // Move both stacks aside first, so even a full undo stack loses no entry.
        let history = matches!(id, "timeline.roll_to" | "timeline.rollback")
            .then(|| (std::mem::take(&mut self.session.undo), std::mem::take(&mut self.session.redo)));
        let result = if matches!(id, "file.save" | "file.save_as") {
            self.save_with_recovery(id, &params)
        } else {
            self.session.execute(id, &params).map_err(AdapterError::from)
        };
        if let Some((undo, redo)) = history {
            self.session.undo = undo;
            self.session.redo = redo;
        }
        // select.set increments the engine's general view revision but does not change
        // geometry. Other revision changes include visibility and occurrence placement.
        let geometry_changed = !Arc::ptr_eq(&doc, &self.session.doc)
            || before.0 != self.session.document_id
            || (before.1 != self.session.revision && !id.starts_with("select.") && !id.starts_with("view."));
        if geometry_changed {
            self.geometry_revision = self.geometry_revision.checked_add(1).ok_or_else(|| error("revision_limit", "geometry revision exhausted"))?;
        }
        let value = result?;
        Ok(CommandReply {
            value,
            changed: geometry_changed,
            document_id: self.session.document_id.clone(),
            revision: self.session.revision,
            geometry_revision: self.geometry_revision,
        })
    }

    /// Prepare and persist a browser project before replacing the live document.
    /// A bad import, failed evaluation or unwritable destination leaves history,
    /// selection and the current document untouched. All file semantics remain
    /// in the upstream commands; this is only a native publication transaction.
    pub fn replace_document(&mut self, source: Option<&str>, name: &str, destination: &str) -> Result<(Value, Arc<SceneSnapshot>), AdapterError> {
        if name.trim().is_empty() || name.len() > 512 || destination.is_empty() || destination.len() >= 4096 {
            return Err(error("bad_params", "invalid project name or destination"));
        }
        let revision = self.geometry_revision.checked_add(1).ok_or_else(|| error("revision_limit", "geometry revision exhausted"))?;
        let mut candidate = Self::new();
        let reply = match source {
            Some(path) => candidate.execute("doc.open", json!({"path":path}))?.value,
            None => candidate.execute("file.new", json!({"name":name}))?.value,
        };
        // Check the potentially large import-warning reply before adoption:
        // response overflow must never look like a failed project switch after
        // the live document has already changed. Leave room for viewport data.
        if serde_json::to_vec(&reply).map_err(|e| error("response", e.to_string()))?.len() > 512 * 1024 {
            return Err(error("response_limit", "project import diagnostics exceed 512 KiB"));
        }
        candidate.geometry_revision = revision;
        let mut snapshot = candidate.snapshot()?;
        let saved = candidate.execute("file.save_as", json!({"path":destination}))?;
        // Save As may rename a design named after its previous file. Retain
        // prepared geometry and normalise the publication to one new revision.
        candidate.geometry_revision = revision;
        if saved.changed {
            snapshot = candidate.snapshot()?;
        }
        self.session = candidate.session;
        self.geometry_revision = revision;
        self.workspace_cache = None;
        Ok((reply, snapshot))
    }

    /// Selection in `Viewport` consists of real engine `Sel`s. Synchronise via the
    /// existing command before an engine action that may consume the selection.
    pub fn selection(&mut self, items: &[Sel]) -> Result<(), AdapterError> {
        if self.session.selection != items {
            self.execute("select.set", json!({"items":items}))?;
        }
        Ok(())
    }

    /// Retains Step 2's small integration proof for diagnostics.
    pub fn proof(&mut self) -> Result<Arc<SceneSnapshot>, AdapterError> {
        self.require_empty()?;
        self.execute("solid.box", json!({"length":40,"width":30,"height":10,"operation":"new","name":"Foundation","body_name":"Foundation"}))?;
        self.snapshot()
    }

    /// Repeatable development design, entirely through the shared command registry.
    /// Two separate bodies overlap from some views; a visible sketch exercises lines.
    pub fn fixture(&mut self) -> Result<Arc<SceneSnapshot>, AdapterError> {
        self.require_empty()?;
        self.execute(
            "solid.box",
            json!({"length":40,"width":30,"height":10,"corner":[-20,-15,0],"operation":"new","name":"Viewport base","body_name":"Base"}),
        )?;
        self.execute("solid.cylinder", json!({"radius":7,"height":25,"base":[10,4,10],"operation":"new","name":"Viewport post","body_name":"Post"}))?;
        self.execute("sketch.create", json!({"plane":"XY","name":"Reference sketch"}))?;
        self.execute("sketch.rectangle.two_point", json!({"p0":[-32,-12],"p1":[-24,6]}))?;
        self.execute("sketch.finish", json!({}))?;
        self.snapshot()
    }

    fn require_empty(&self) -> Result<(), AdapterError> {
        if !self.session.doc.features.is_empty() {
            return Err(error("proof_requires_empty_document", "the development fixture requires an empty document"));
        }
        Ok(())
    }

    pub fn snapshot(&mut self) -> Result<Arc<SceneSnapshot>, AdapterError> {
        if let Some(result) = self.session.model.results.iter().find(|result| result.error.is_some()) {
            return Err(error("evaluation", format!("{}: {}", result.name, result.error.as_deref().unwrap_or_default())));
        }
        if self.session.active_sketch.is_none() {
            if let Some(result) = self.session.model.results.iter().find(|result| !result.skipped && result.stale) {
                return Err(error("stale_evaluation", format!("{} has not been regenerated for the current document.", result.name)));
            }
            if let Some(result) =
                self.session.model.results.iter().find(|result| result.warning.as_deref().is_some_and(references::risky_reference_warning))
            {
                return Err(error("unresolved_reference", format!("{}: {}", result.name, result.warning.as_deref().unwrap_or_default())));
            }
        }
        // No camera-dependent guides or selection tint in the stable model upload.
        let selection = std::mem::take(&mut self.session.selection);
        let scene = view::scene_with_guides(&self.session, &solidcraft_render::Camera::default(), None, view::Guides { grid: false, axes: false });
        self.session.selection = selection;
        let world = self.session.world_state();
        let names: Vec<String> = world.bodies.iter().filter(|b| self.session.body_visible(&b.name)).map(|b| b.name.clone()).collect();
        let mut bounds = view::bounds(&self.session);
        // Shared curve bounds omit standalone points. Include the active
        // sketch's visible points so Fit also works for a point-only sketch.
        for sketch in world.sketches.iter().filter(|sketch| self.session.active_sketch == Some(sketch.feature)) {
            for point in sketch.sketch.points.iter().filter(|p| p.id != "origin" && (p.link.is_none() || !sketch.sketch.view.hide_projected)) {
                bounds.add(sketch.plane.to_world(point.pos));
            }
        }
        let mut picks = PickScene::new(&scene, names, bounds)?;
        for sketch in world.sketches.iter().filter(|sketch| {
            view::sketch_shown(&self.session, sketch.feature)
                && !self.session.visibility.hidden_profiles.contains(&sketch.feature)
                && !sketch.sketch.view.hide_profile
        }) {
            picks.add_profiles(sketch);
        }
        let triangle_count = scene
            .meshes
            .iter()
            .try_fold(0usize, |n, m| n.checked_add(m.mesh.triangles.len()))
            .ok_or_else(|| error("geometry_limit", "too many render triangles"))?;
        let volume_mm3: f64 = scene.meshes.iter().map(|m| m.mesh.measure().volume.abs()).sum();
        if !volume_mm3.is_finite() {
            return Err(error("invalid_geometry", "non-finite scene volume"));
        }
        Ok(Arc::new(SceneSnapshot {
            instances: world
                .bodies
                .iter()
                .filter(|b| self.session.body_visible(&b.name))
                .map(|b| {
                    let (mesh, transform) = b.mesh_in_place();
                    NativeMeshInstance { body: b.name.clone(), mesh, transform }
                })
                .collect(),
            occurrence_paths: self.assembly_body_paths()?,
            occurrences: self.assembly_placements()?.into_iter().map(|p| p.path).collect(),
            joint_frames: self
                .session
                .doc
                .assembly
                .joints
                .iter()
                .filter(|j| !j.suppressed && self.session.joint_visible(j.id))
                .flat_map(|j| [&j.a, &j.b])
                .filter_map(|origin| self.session.doc.origin_world(&self.session.model.state(), origin).ok())
                .collect(),
            radius_handle: self.radius_handle(),
            feature_ids: self.session.doc.features.iter().map(|f| f.id).collect(),
            feature_feedback: self.feature_feedback(&world),
            sketch: self.sketch_snapshot(),
            projection_sources: self.projection_geometry(),
            sketch_plane: view::open_sketch_plane(&self.session),
            construction_planes: world
                .construct
                .iter()
                .filter_map(|c| match &c.geom {
                    solidcraft_engine::doc::construct::ConstructGeom::Plane(plane) if self.session.construction_visible(&c.name) => {
                        Some((c.name.clone(), *plane))
                    }
                    _ => None,
                })
                .collect(),
            body_count: scene.meshes.len(),
            preview_scene: None,
            scene: Arc::new(scene),
            picks,
            bounds,
            document_id: self.session.document_id.clone(),
            revision: self.session.revision,
            geometry_revision: self.geometry_revision,
            triangle_count,
            volume_mm3,
        }))
    }
}

#[cfg(test)]
mod tests;
