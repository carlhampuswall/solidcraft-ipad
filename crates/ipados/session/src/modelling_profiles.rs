//! Live source selection for existing extrusions; all geometry edits use engine commands.
use serde_json::{Value, json};
use solidcraft_engine::geom::prelude::*;
use solidcraft_engine::{
    Sel,
    doc::{FeatureKind, ProfileSel, profile_selection_indices},
};

use crate::{AdapterError, ModelEdit, SessionAdapter, error};

fn profile_selection(value: &Value) -> Option<ProfileSel> {
    serde_json::from_value(value.clone()).ok().or_else(|| {
        if value == "all" {
            return Some(ProfileSel::All);
        }
        let values = value.as_array()?;
        if values.iter().all(Value::is_u64) {
            return Some(ProfileSel::Indices { indices: values.iter().map(|v| usize::try_from(v.as_u64()?).ok()).collect::<Option<_>>()? });
        }
        if values.iter().all(|v| v.get("point").is_some()) {
            return Some(ProfileSel::Points {
                points: values.iter().map(|v| serde_json::from_value(v["point"].clone()).ok()).collect::<Option<_>>()?,
            });
        }
        Some(ProfileSel::Curves { loops: values.iter().map(|v| serde_json::from_value(v["curves"].clone()).ok()).collect::<Option<_>>()? })
    })
}

impl SessionAdapter {
    pub(crate) fn replaces_extrusion_profile(&self, edit: &ModelEdit, params: &Value) -> bool {
        let Some(feature) = edit.feature.and_then(|id| self.session.doc.feature(id)) else { return false };
        let FeatureKind::Extrude { sketch, .. } = &feature.kind else { return false };
        params["profile"]["faces"].is_array()
            || self.session.doc.feature(sketch.0).is_some_and(|f| f.is_internal_profile())
                && params["profile"]["sketch"].as_u64().is_some_and(|id| id != sketch.0)
    }

    pub(crate) fn restore_extrusion_sources(&self, edit: &mut ModelEdit) -> Result<(), AdapterError> {
        let Some(feature) = edit.feature else { return Ok(()) };
        let before = self.session.model.state_before(feature);
        let world = solidcraft_engine::doc::world_state(&self.session.placed_document(), &before).map_err(solidcraft_engine::EngineError::from)?;
        let source = &edit.params["profile"];
        let mut selection = Vec::new();
        if let Some(sketch) = source["sketch"].as_u64().and_then(|id| world.sketch(id)) {
            let indices =
                profile_selection(&source["profiles"]).and_then(|selection| profile_selection_indices(sketch, &selection).ok()).unwrap_or_default();
            let internal = self.session.doc.feature(sketch.feature).is_some_and(|f| f.is_internal_profile());
            for index in indices.into_iter().take(1024) {
                if internal {
                    let Some(profile) = sketch.profiles.get(index) else { continue };
                    let point = sketch.plane.to_world(profile.region.interior_point());
                    let candidates: Vec<_> = world
                        .bodies
                        .iter()
                        .filter_map(|body| {
                            let index = solidcraft_engine::doc::appearance::face_index_at(body, point)?;
                            let spot = solidcraft_engine::doc::naming::face_spot(body, index)?;
                            let solidcraft_engine::kernel::FaceSurface::Plane { normal, .. } = spot.surface? else { return None };
                            (normal.vec().dot(sketch.plane.normal()) > 1. - 1e-9).then(|| Sel::Face { body: body.name.clone(), index, point })
                        })
                        .collect();
                    if let [face] = candidates.as_slice()
                        && !selection.contains(face)
                    {
                        selection.push(face.clone());
                    }
                } else {
                    selection.push(Sel::Profile { sketch: sketch.feature, index });
                }
            }
            // Keep the native profile unchanged until the first real tap. In
            // particular, opening an All-profile edit must retain its intent.
        } else if let Some(points) = source["faces"].as_array() {
            for point in points {
                let Some(body) = source["face_body"].as_str().and_then(|name| world.body(name)) else { continue };
                let point = serde_json::from_value(point.clone()).map_err(|e| error("selection", format!("{e}")))?;
                if let Some(index) = solidcraft_engine::doc::appearance::face_index_at(body, point) {
                    selection.push(Sel::Face { body: body.name.clone(), index, point });
                }
            }
        }
        edit.source_selection = selection;
        Ok(())
    }

    pub(crate) fn extrusion_source_reference(&self, edit: &ModelEdit, selection: &[Sel]) -> Result<Value, AdapterError> {
        if selection.is_empty() {
            return Ok(Value::Null);
        }
        if selection.len() > 256 {
            return Err(error("selection", "ui.ipados.face_extrude.choose_source"));
        }
        if selection.iter().all(|s| matches!(s, Sel::Face { .. })) {
            return super::modelling::extrusion_face_reference(selection)?.ok_or_else(|| error("selection", "ui.ipados.face_extrude.choose_source"));
        }
        let Some(Sel::Profile { sketch, .. }) = selection.first() else { return Err(error("selection", "ui.ipados.face_extrude.choose_source")) };
        let indices = selection
            .iter()
            .map(|s| match s {
                Sel::Profile { sketch: id, index } if id == sketch => Ok(*index),
                _ => Err(error("selection", "ui.ipados.face_extrude.choose_source")),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let before = self.session.model.state_before(edit.feature.unwrap_or_default());
        let solved = before.sketch(*sketch).ok_or_else(|| error("stale_selection", "ui.ipados.workspace.invalidated"))?;
        super::modelling::profile_reference(solved, &indices)
    }

    /// Capture replacement face boundaries before the existing feature using
    /// the engine's own face constructor. The temporary solid is removed after
    /// its linked profile has been assigned to the original extrusion.
    pub(crate) fn replacement_profile_commands(
        &self,
        edit: &ModelEdit,
        source: &Value,
        mut definition: Value,
        commit: bool,
    ) -> Result<Vec<(String, Value)>, AdapterError> {
        let feature = edit.feature.ok_or_else(|| error("stale_edit", "The feature no longer exists."))?;
        let native = self.session.doc.feature(feature).ok_or_else(|| error("stale_edit", "The feature no longer exists."))?;
        let position = self.session.doc.feature_index(feature).ok_or_else(|| error("stale_edit", "The feature no longer exists."))?;
        let mut scratch = self.session.scratch();
        let mut commands = Vec::new();
        let reply = {
            let mut run = |id: &str, params: Value| -> Result<Value, AdapterError> {
                let reply = scratch.execute(id, &params)?;
                commands.push((id.into(), params));
                Ok(reply)
            };
            run("timeline.roll_to", json!({"position":position}))?;
            let mut author = json!({"component":native.component});
            if self.session.active_component == native.component
                && let Some(path) = &self.session.active_occurrence_path
            {
                author["path"] = json!(path);
                author["occurrence"] = json!(path.last());
            }
            run("component.activate", author)?;
            let mut capture = source.clone();
            capture["operation"] = json!("new");
            capture["distance"] = json!("1 mm");
            run("solid.extrude", capture)?
        };
        let added: Vec<_> = scratch
            .doc
            .features
            .iter()
            .filter(|f| self.session.doc.feature(f.id).is_none() && matches!(f.kind, FeatureKind::Extrude { .. }))
            .collect();
        let [temporary] = added.as_slice() else { return Err(error("selection", "ui.ipados.face_extrude.choose_source")) };
        let FeatureKind::Extrude { sketch, profiles, .. } = &temporary.kind else {
            return Err(error("selection", "ui.ipados.face_extrude.choose_source"));
        };
        if source["faces"].is_array() {
            definition["sketch"] = json!(sketch);
            definition["profiles"] = json!(profiles);
        }
        commands.push(("timeline.edit".into(), json!({"feature":feature,"set":definition})));
        if let FeatureKind::Extrude { sketch, .. } = &native.kind
            && self.session.doc.feature(sketch.0).is_some_and(|f| f.is_internal_profile())
        {
            // Let the engine's existing deletion policy collect an unused old
            // helper, including all of its non-modelling references.
            commands.push(("timeline.suppress".into(), json!({"feature":reply["feature"],"suppressed":true})));
            commands.push(("timeline.edit".into(), json!({"feature":reply["feature"],"set":{"sketch":sketch,"profiles":{"type":"all"}}})));
        }
        commands.push(("timeline.delete".into(), json!({"feature":reply["feature"]})));
        // Restore authoring context; these commands never enter native history.
        let mut active = json!({"component":self.session.active_component});
        if let Some(path) = &self.session.active_occurrence_path {
            active["path"] = json!(path);
            active["occurrence"] = json!(path.last());
        }
        commands.push(("component.activate".into(), active));
        // Follow the original marker's frontier identity across insertion and
        // helper cleanup. Previews stop at the edited feature.
        let marker = if edit.rollback {
            if commit { json!({"position":"end"}) } else { json!({"feature":feature}) }
        } else if self.session.doc.marker.is_none() {
            json!({"position":"end"})
        } else {
            let frontier = self.session.doc.marker.and_then(|marker| marker.checked_sub(1)).and_then(|index| self.session.doc.features.get(index));
            let frontier =
                frontier.filter(|f| !matches!(&native.kind, FeatureKind::Extrude { sketch, .. } if sketch.0 == f.id && f.is_internal_profile()));
            match frontier {
                Some(feature) => json!({"feature":feature.id}),
                None => json!({"position":self.session.doc.marker.unwrap_or_default()}),
            }
        };
        commands.push(("timeline.roll_to".into(), marker));
        Ok(commands)
    }
}
