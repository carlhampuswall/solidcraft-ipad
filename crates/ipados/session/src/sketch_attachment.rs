//! Reviewable face reattachment through the existing sketch.redefine command.
use serde_json::{Value, json};
use solidcraft_engine::{Sel, doc::FeatureKind};

use crate::{AdapterError, ModelEdit, SessionAdapter, error};

impl SessionAdapter {
    /// Called on a scratch session rolled immediately before the sketch. Later
    /// bodies must never become candidates for a circular sketch dependency.
    pub(crate) fn sketch_attachment_form(&self, edit: &mut ModelEdit) -> Result<(), AdapterError> {
        let feature = edit.feature.and_then(|id| self.session.doc.feature(id));
        if !feature.is_some_and(|feature| matches!(feature.kind, FeatureKind::Sketch { .. }) && !feature.suppressed) {
            return Err(error("selection", "ui.ipados.attachment.choose_sketch"));
        }
        let world = self.session.world_state();
        let mut options = Vec::new();
        let frames = self.session.placed_document().body_frames_with_source(&self.session.model.state());
        for body in world.bodies.iter().filter(|body| self.session.body_visible(&body.name)) {
            let Some((_, _, _, path, _)) = frames.iter().find(|(name, ..)| name == &body.name) else { continue };
            let names = solidcraft_engine::doc::naming::face_names(body);
            for (index, spot) in solidcraft_engine::doc::naming::face_spots(body).into_iter().enumerate() {
                let Some(spot) = spot.filter(|spot| matches!(spot.surface, Some(solidcraft_engine::kernel::FaceSurface::Plane { .. }))) else {
                    continue;
                };
                if options.len() >= 256 {
                    break;
                }
                let name = names.get(index).cloned().unwrap_or_else(|| index.to_string());
                options.push(json!({"label":format!("{} · {name}", body.name),
                    "value":{"face":spot.point,"body":body.name,"index":index,"path":path}}));
            }
            if options.len() >= 256 {
                break;
            }
        }
        edit.params = json!({"plane":null});
        edit.fields = vec![json!({"key":"plane","label":"ui.ipados.attachment.face","kind":"reference","options":options})];
        edit.feedback = vec!["ui.ipados.attachment.hint".into()];
        Ok(())
    }

    pub(crate) fn sketch_attachment_selection(&self, edit: &ModelEdit, selection: &[Sel]) -> Value {
        let [Sel::Face { body, index, .. }] = selection else { return Value::Null };
        edit.fields
            .iter()
            .find(|field| field["key"] == "plane")
            .and_then(|field| field["options"].as_array())
            .and_then(|options| options.iter().find(|option| option["value"]["body"] == *body && option["value"]["index"] == *index))
            .map(|option| option["value"].clone())
            .unwrap_or(Value::Null)
    }

    pub(crate) fn sketch_attachment_command(&self, edit: &ModelEdit, params: &Value) -> Result<Value, AdapterError> {
        // Only the exact, upstream face payloads from this revision's form may
        // be accepted. Do not fall back to a nearest point or a later body.
        let picked = &params["plane"];
        let point = serde_json::from_value::<solidcraft_engine::geom::Vec3>(picked["face"].clone()).ok().filter(|point| point.is_finite());
        // Swift's JSON round trip can spell 10.0 as 10. Compare typed coordinates
        // and exact body/index/path identity, then send the canonical option to
        // the engine rather than the caller's re-encoded geometry payload.
        let plane = edit.fields.iter().filter_map(|field| field["options"].as_array()).flatten().find_map(|option| {
            let plane = &option["value"];
            (plane["body"] == picked["body"]
                && plane["index"] == picked["index"]
                && plane["path"] == picked["path"]
                && point.is_some()
                && serde_json::from_value::<solidcraft_engine::geom::Vec3>(plane["face"].clone()).ok() == point)
                .then_some(plane)
        });
        let plane = plane.ok_or_else(|| error("selection", "ui.ipados.attachment.choose_face"))?;
        let feature =
            edit.feature.and_then(|id| self.session.doc.feature(id)).ok_or_else(|| error("stale_edit", "ui.ipados.attachment.choose_sketch"))?;
        let FeatureKind::Sketch { plane: original, .. } = &feature.kind else {
            return Err(error("selection", "ui.ipados.attachment.choose_sketch"));
        };
        let mut command = json!({"sketch":feature.id,"plane":plane});
        if let Some(anchor) = solidcraft_engine::doc::face_anchor::plane_face_anchor(original) {
            command["author_path"] = json!(anchor.author_path);
        }
        // Non-face sketches retain the engine's explicit author-placement checks.
        Ok(command)
    }
}
