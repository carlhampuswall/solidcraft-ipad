//! Safe native integration: one engine owner and immutable, revisioned viewport data.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

mod input;
mod picking;
mod viewport;

pub use input::{InputEvent, InputPhase, InputSource};
pub use picking::{PickFilter, PickScene, ViewportMetrics};
pub use solidcraft_engine::geom::Vec3 as WorldPoint;
pub use viewport::{RenderSnapshot, Viewport};

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
pub struct SceneSnapshot {
    pub scene: Arc<Scene>,
    pub picks: PickScene,
    pub bounds: Aabb3,
    pub document_id: String,
    pub revision: u64,
    pub geometry_revision: u64,
    pub body_count: usize,
    pub triangle_count: usize,
    pub volume_mm3: f64,
}

/// The only mutable CAD document/session owner. Camera/input live in `Viewport` so
/// navigation and lifecycle suspension never wait for evaluation or tessellation.
pub struct SessionAdapter {
    session: Session,
    geometry_revision: u64,
}

impl Default for SessionAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionAdapter {
    pub fn new() -> Self {
        Self { session: Session::new(Document::new("SolidCraft")), geometry_revision: 1 }
    }

    pub fn execute(&mut self, id: &str, params: Value) -> Result<CommandReply, AdapterError> {
        let before = (self.session.document_id.clone(), self.session.revision);
        let doc = self.session.doc.clone();
        let result = self.session.execute(id, &params);
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
        if let Some(failure) = self.session.model.results.iter().find_map(|result| result.error.as_ref()) {
            return Err(error("evaluation", failure.clone()));
        }
        // No camera-dependent guides or selection tint in the stable model upload.
        let selection = std::mem::take(&mut self.session.selection);
        let scene = view::scene_with_guides(&self.session, &solidcraft_render::Camera::default(), None, view::Guides { grid: false, axes: false });
        self.session.selection = selection;
        let world = self.session.world_state();
        let names: Vec<String> = world.bodies.iter().filter(|b| self.session.body_visible(&b.name)).map(|b| b.name.clone()).collect();
        let bounds = view::bounds(&self.session);
        let picks = PickScene::new(&scene, names, bounds)?;
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
            body_count: scene.meshes.len(),
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
