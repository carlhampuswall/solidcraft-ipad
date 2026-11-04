//! A small native tool contract over upstream scratch evaluation and command history.
//! Sketch, modelling and assembly handles share this preview contract; no document
//! mutation or independent history lives in the input/viewport worker.
use solidcore_record::{Plane, Vec3};
use solidcraft_engine::geom::prelude::*;
use std::sync::Arc;

use serde::Serialize;
use serde_json::{Value, json};
use solidcraft_engine::{
    ComputedCommands,
    doc::{FeatureKind, expr::Kind},
};

use crate::{AdapterError, SceneSnapshot, SessionAdapter, error};

#[derive(Clone, Debug, Serialize)]
pub struct RadiusHandle {
    pub feature: u64,
    pub body: String,
    pub radius: f64,
    pub plane: Plane,
}

impl RadiusHandle {
    pub fn point(&self, radius: f64) -> Vec3 {
        self.plane.origin + self.plane.x * radius
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolPhase {
    Begin,
    Update,
    Suspend,
    Resume,
    Commit,
    Cancel,
}

/// Reusable lifecycle contract. Positions are actual world points on the tool's plane.
/// Predicted samples never enter this contract or the engine work mailbox.
#[derive(Clone, Debug, Serialize)]
pub struct ToolEvent {
    pub interaction: u64,
    pub phase: ToolPhase,
    pub point: Option<Vec3>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ToolWork {
    pub interaction: u64,
    pub version: u64,
    pub document_id: String,
    pub geometry_revision: u64,
    pub operation: ToolOperation,
    pub commit: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolOperation {
    Radius { handle: RadiusHandle, radius: f64 },
    Sketch { sketch: u64, commands: Vec<(String, Value)> },
    Modelling { token: u64, operation: String, feature: Option<u64>, params: Value },
}

impl ToolWork {
    fn commands(&self) -> Vec<(String, Value)> {
        match &self.operation {
            ToolOperation::Radius { handle, radius } => vec![("timeline.edit".into(), json!({"feature":handle.feature,"set":{"radius":radius}}))],
            ToolOperation::Sketch { commands, .. } => commands.clone(),
            ToolOperation::Modelling { .. } => Vec::new(),
        }
    }
}

pub struct PreparedTool {
    pub work: ToolWork,
    pub model: Arc<SceneSnapshot>,
    transaction: Option<ComputedCommands>,
    pub reply: Value,
    pub feedback: Vec<String>,
    pub handle: Option<crate::modelling::ModelHandle>,
    pub model_params: Option<Value>,
}

impl SessionAdapter {
    /// Read-only fixture metadata. Public expressions and the evaluated visible body
    /// must agree; arbitrary imported cylinders are deliberately not made editable.
    pub(crate) fn radius_handle(&self) -> Option<RadiusHandle> {
        let f = self.session.doc.features.iter().find(|f| f.name == "Viewport post" && !f.suppressed && f.component == 0)?;
        let FeatureKind::Cylinder { base, axis, radius, height, .. } = &f.kind else { return None };
        if Vec3::from(*axis).dist(Vec3::Z) > 1e-9
            || !self.session.body_visible("Post")
            || !self.session.world_state().ok()?.bodies.iter().any(|b| b.name == "Post")
        {
            return None;
        }
        let value = |e: &str| self.session.doc.eval(e, Kind::Length).ok();
        let radius = value(radius)?;
        let origin = Vec3::new(value(&base[0])?, value(&base[1])?, value(&base[2])? + value(height)?);
        (radius.is_finite() && (0.5..=50.0).contains(&radius) && origin.is_finite()).then_some(RadiusHandle {
            feature: f.id,
            body: "Post".into(),
            radius,
            plane: Plane { origin, ..Plane::XY },
        })
    }

    fn validate_tool(&self, work: &ToolWork) -> Result<(), AdapterError> {
        if work.document_id != self.session.document_id || work.geometry_revision != self.geometry_revision {
            return Err(error("stale_edit", "the parameter handle or document changed"));
        }
        match &work.operation {
            ToolOperation::Radius { handle, radius } => {
                if !radius.is_finite()
                    || !(0.5..=50.0).contains(radius)
                    || self.radius_handle().is_none_or(|h| h.feature != handle.feature || h.plane != handle.plane || h.radius != handle.radius)
                {
                    return Err(error("stale_edit", "the parameter handle or document changed"));
                }
            }
            ToolOperation::Sketch { sketch, commands } => {
                if self.session.active_sketch != Some(*sketch)
                    || commands.is_empty()
                    || commands.len() > 32
                    || commands.iter().any(|(id, _)| {
                        !id.starts_with("sketch.") || matches!(id.as_str(), "sketch.create" | "sketch.edit" | "sketch.finish" | "sketch.cancel")
                    })
                {
                    return Err(error("stale_edit", "the sketch or current tool changed"));
                }
            }
            ToolOperation::Modelling { .. } if work.commit => return Err(error("tool", "Confirm the modelling preview explicitly.")),
            ToolOperation::Modelling { .. } => {}
        }
        Ok(())
    }

    /// Runs only on the engine worker. Uses the upstream preview foundation (scratch
    /// session, shared command evaluator/cache and shared scene builder). Authoritative
    /// document/history remain untouched, even if this work is subsequently cancelled.
    pub fn prepare_tool(&self, work: ToolWork) -> Result<PreparedTool, AdapterError> {
        self.validate_tool(&work)?;
        if let ToolOperation::Modelling { token, operation, feature, params } = &work.operation {
            let mut edit = self.begin_model(*token, Some(operation), *feature, &[])?;
            // A handle sample is already the viewport's current form. Rebuilding
            // its metadata must not compare inferred targets with creation defaults
            // and mistake them for a newly submitted target-list edit.
            edit.params = params.clone();
            let prepared = self.prepare_model(&edit, params.clone(), false)?;
            return Ok(PreparedTool {
                work,
                model: prepared.model,
                transaction: None,
                reply: prepared.reply,
                feedback: prepared.feedback,
                handle: prepared.handle,
                model_params: Some(prepared.params),
            });
        }
        let commands = work.commands();
        let mut scratch = Self { session: self.session.scratch(), geometry_revision: self.geometry_revision, workspace_cache: None };
        // Primitive previews evaluate only through their sketch. The native live drag
        // path already solves just that sketch and retains the previous downstream model.
        // Committed work always evaluates the original full timeline.
        if !work.commit
            && let ToolOperation::Sketch { sketch, .. } = &work.operation
            && !commands.iter().any(|(id, p)| id == "sketch.drag" && p["live"] == true)
        {
            scratch.execute("timeline.roll_to", json!({"feature":sketch}))?;
        }
        for (id, params) in &commands {
            let reply = scratch.execute(id, params.clone())?;
            if id == "sketch.drag"
                && reply.value["reached"].as_f64() == Some(0.0)
                && reply.value["retry"] != true
                && params.get("from") != params.get("to")
            {
                return Err(error("fixed_geometry", "The sketch solver could not move this geometry within its constraints."));
            }
        }
        if let ToolOperation::Sketch { sketch, .. } = &work.operation {
            let state = scratch.session.model.state();
            let ss = state.sketch(*sketch).ok_or_else(|| error("solve_unavailable", "The edited sketch has no solver result."))?;
            if !ss.report.keeps_orientation() {
                return Err(error(
                    "sketch_solve",
                    ss.report.failure(|ids| {
                        if ids.is_empty() {
                            "The sketch solver could not satisfy this proposal.".into()
                        } else {
                            format!("The sketch solver could not satisfy constraints {ids} for this proposal.")
                        }
                    }),
                ));
            }
        }
        let model = scratch.snapshot()?;
        let transaction = if work.commit { Some(self.session.scratch().compute_commands(&commands, None)?) } else { None };
        Ok(PreparedTool { work, model, transaction, reply: Value::Null, feedback: Vec::new(), handle: None, model_params: None })
    }

    /// The bridge checks the interaction ticket while holding the viewport lock before
    /// this short, evaluation-free acceptance. Cancellation cannot race the acceptance.
    pub fn accept_tool(&mut self, mut prepared: PreparedTool) -> Result<Arc<SceneSnapshot>, AdapterError> {
        self.validate_tool(&prepared.work)?;
        let transaction = prepared.transaction.take().ok_or_else(|| error("tool", "preview cannot be committed"))?;
        let revision = self.geometry_revision.checked_add(1).ok_or_else(|| error("revision_limit", "geometry revision exhausted"))?;
        let model = Arc::get_mut(&mut prepared.model).ok_or_else(|| error("tool", "prepared scene must have unique ownership"))?;
        self.session.accept_commands(transaction)?;
        self.geometry_revision = revision;
        model.geometry_revision = revision;
        model.revision = self.session.revision;
        Ok(prepared.model)
    }
}
