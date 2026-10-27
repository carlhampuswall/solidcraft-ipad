//! Safe, platform-independent ownership boundary around the shared CAD engine.
//!
//! A host serializes calls to one adapter on its evaluation worker. Snapshots own their
//! scene and retain shared meshes, so the renderer never borrows the mutable session.
//! Apple SDKs and the C ABI belong to the separate bridge crate.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]
#![forbid(unsafe_code)]

use serde::Serialize;
use serde_json::{Value, json};
use solidcraft_engine::{EngineError, Session, doc::Document, view};
use solidcraft_render::{Camera, Scene};

/// Structured failure returned to the platform host. `code` is stable within ABI version 1;
/// `message` is diagnostic text, never a machine-readable command contract.
#[derive(Debug, Serialize, thiserror::Error)]
#[error("{message}")]
pub struct AdapterError {
    pub code: &'static str,
    pub message: String,
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

/// A completed command is also the minimal document-change notification.
#[derive(Debug, Serialize)]
pub struct CommandReply {
    pub value: Value,
    pub document_id: String,
    pub revision: u64,
    /// Conservative geometry invalidation token, currently the shared session revision.
    pub geometry_revision: u64,
    /// Host scene invalidation, including camera, selection and failed command attempts.
    pub scene_revision: u64,
    pub changed: bool,
}

/// Immutable owned render input. Keep `(document_id, revision, scene_revision)` with
/// asynchronous render work; discard a result if any differs before presenting it.
pub struct RenderSnapshot {
    pub scene: Scene,
    pub camera: Camera,
    pub document_id: String,
    pub revision: u64,
    pub geometry_revision: u64,
    pub scene_revision: u64,
    pub body_count: usize,
    pub triangle_count: usize,
    pub volume_mm3: f64,
}

/// Owns exactly one real shared engine session and the shared camera.
/// Rust destruction releases the session, its history and model caches normally.
pub struct SessionAdapter {
    session: Session,
    camera: Camera,
    scene_revision: u64,
}

impl Default for SessionAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionAdapter {
    pub fn new() -> Self {
        Self { session: Session::new(Document::new("SolidCraft")), camera: Camera::default(), scene_revision: 1 }
    }

    /// Forward existing command IDs and JSON parameters unchanged. The platform boundary
    /// limits encoded requests before deserialization; the engine validates CAD parameters.
    pub fn execute(&mut self, id: &str, params: Value) -> Result<CommandReply, AdapterError> {
        self.scene_revision = self
            .scene_revision
            .checked_add(1)
            .ok_or_else(|| AdapterError { code: "scene_revision_limit", message: "the session scene revision is exhausted".into() })?;
        let before = (self.session.document_id.clone(), self.session.revision);
        // The shared command's camera slot reads the host's current camera and returns any
        // requested replacement. The adapter remains the single camera owner between calls.
        self.session.camera = Some(self.camera);
        let value = self.session.execute(id, &params)?;
        if let Some(camera) = self.session.camera.take() {
            self.camera = camera;
        }
        let changed = before != (self.session.document_id.clone(), self.session.revision);
        Ok(CommandReply {
            value,
            document_id: self.session.document_id.clone(),
            revision: self.session.revision,
            geometry_revision: self.session.revision,
            scene_revision: self.scene_revision,
            changed,
        })
    }

    /// Small startup integration proof: the real registry creates a timeline box, the
    /// document is evaluated without the model cache, then the shared view produces meshes.
    /// This is deliberately a one-shot operation on a fresh session, not a modelling API.
    pub fn proof(&mut self) -> Result<RenderSnapshot, AdapterError> {
        if !self.session.doc.features.is_empty() {
            return Err(AdapterError { code: "proof_requires_empty_document", message: "the foundation proof requires an empty document".into() });
        }
        self.execute(
            "solid.box",
            json!({"length": 40, "width": 30, "height": 10, "operation": "new", "name": "Foundation", "body_name": "Foundation"}),
        )?;
        self.session.model.no_cache = true;
        self.session.refresh();
        self.session.model.no_cache = false;
        self.camera.fit(&view::bounds(&self.session));
        let snapshot = self.snapshot()?;
        if snapshot.body_count != 1 || snapshot.triangle_count == 0 || (snapshot.volume_mm3 - 12_000.0).abs() > 1e-6 {
            return Err(AdapterError {
                code: "proof_geometry",
                message: "the engine proof did not produce the expected 40 × 30 × 10 mm solid".into(),
            });
        }
        Ok(snapshot)
    }

    /// Tessellation and scene construction can be expensive and belong on the same serial
    /// worker as command execution. The returned scene has no session lifetime dependency.
    pub fn snapshot(&self) -> Result<RenderSnapshot, AdapterError> {
        if let Some(error) = self.session.model.results.iter().find_map(|result| result.error.as_ref()) {
            return Err(AdapterError { code: "evaluation", message: error.clone() });
        }
        let scene = view::scene(&self.session, &self.camera);
        let triangle_count = scene
            .meshes
            .iter()
            .try_fold(0usize, |total, mesh| total.checked_add(mesh.mesh.triangles.len()))
            .ok_or_else(|| AdapterError { code: "geometry_limit", message: "the render scene has too many triangles".into() })?;
        let volume_mm3: f64 = scene.meshes.iter().map(|mesh| mesh.mesh.measure().volume.abs()).sum();
        if !volume_mm3.is_finite() {
            return Err(AdapterError { code: "invalid_geometry", message: "the engine render scene has a non-finite volume".into() });
        }
        Ok(RenderSnapshot {
            body_count: scene.meshes.len(),
            scene,
            camera: self.camera,
            document_id: self.session.document_id.clone(),
            revision: self.session.revision,
            geometry_revision: self.session.revision,
            scene_revision: self.scene_revision,
            triangle_count,
            volume_mm3,
        })
    }
}

#[cfg(test)]
mod tests;
