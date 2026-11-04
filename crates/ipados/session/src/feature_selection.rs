//! Selection feedback projected from existing evaluator states, without evaluating
//! or mutating the document on the display lane.
use solidcore_record::Vec3;
use std::collections::{BTreeMap, BTreeSet};

use solidcraft_engine::{
    Sel,
    doc::{ModelState, construct::ConstructGeom, naming, persistent_name::FaceName},
    view,
};

use crate::SessionAdapter;

#[derive(Default)]
pub(crate) struct FeatureFeedback {
    pub bodies: Vec<String>,
    pub lines: Vec<Vec<Vec3>>,
    pub construct: Option<ConstructGeom>,
}

impl SessionAdapter {
    /// History emphasis is presentation only: retain the viewport selection and
    /// resolve ownership through evaluated bodies and persistent face names.
    pub(crate) fn selection_features(&self, selection: &[Sel]) -> BTreeSet<u64> {
        let mut selected = BTreeSet::new();
        if selection.is_empty() {
            return selected;
        }
        let world = self.session.world_state().ok();
        for item in selection {
            let body_name = match item {
                Sel::Body { name } => Some(name),
                Sel::Face { body, .. } | Sel::Edge { body, .. } | Sel::Vertex { body, .. } => Some(body),
                _ => None,
            };
            if let Some(body) = body_name.and_then(|name| world.as_ref()?.bodies.iter().find(|body| &body.name == name)) {
                let faces: Vec<usize> = match item {
                    Sel::Face { index, .. } => vec![*index],
                    Sel::Edge { index, .. } => {
                        body.mesh().edge_faces.get(*index).map(|faces| faces.iter().map(|i| *i as usize).collect()).unwrap_or_default()
                    }
                    _ => Vec::new(),
                };
                let mut origins = BTreeSet::new();
                if !faces.is_empty() {
                    let names = naming::face_names(body);
                    for index in faces {
                        if names.get(index).is_some()
                            && naming::face_name_exact(body, index)
                            && let Some(origin) = names.get(index).and_then(|name| FaceName::parse(name).ok()).and_then(|name| name.made_by())
                        {
                            origins.insert(origin.0);
                        }
                    }
                }
                if origins.is_empty() {
                    origins.insert(body.feature);
                }
                selected.extend(origins);
            }
            match item {
                Sel::Feature { id } => {
                    selected.insert(*id);
                }
                Sel::Profile { sketch, .. } | Sel::ExternalSketchCurve { sketch, .. } | Sel::ExternalSketchPoint { sketch, .. } => {
                    selected.insert(*sketch);
                }
                Sel::SketchCurve { sketch, .. } => {
                    selected.extend(sketch.or(self.session.active_sketch));
                }
                Sel::SketchPoint { .. } | Sel::SketchMidpoint { .. } | Sel::SketchConstraint { .. } => {
                    selected.extend(self.session.active_sketch);
                }
                Sel::Plane { name } | Sel::Axis { name } | Sel::ConstructionPoint { name } => {
                    selected.extend(self.session.doc.find_feature(name).map(|feature| feature.id));
                }
                Sel::Occurrence { path } => {
                    if !path.is_empty()
                        && let Ok(paths) = self.assembly_body_paths()
                        && let Some(world) = &world
                    {
                        selected.extend(
                            world.bodies.iter().filter(|body| paths.get(&body.name).is_some_and(|p| p.starts_with(path))).map(|body| body.feature),
                        );
                    }
                }
                _ => {}
            }
        }
        selected.retain(|id| self.session.doc.feature(*id).is_some_and(|feature| !self.hidden_internal_profile(feature)));
        selected
    }

    pub(crate) fn feature_feedback(&self, world: &ModelState) -> BTreeMap<u64, FeatureFeedback> {
        let mut feedback = BTreeMap::new();
        let mut previous = BTreeMap::new();
        for result in &self.session.model.results {
            if result.skipped || result.error.is_some() || result.stale {
                continue;
            }
            // A modifying feature often retains the creator's ModelBody.feature.
            // Compare authoritative before/after identities to include only the
            // definitions this feature actually created or changed.
            let current: BTreeMap<_, _> = result.state.bodies.iter().map(|body| ((body.component, body.name.as_str()), body)).collect();
            let changed: BTreeSet<_> = current
                .iter()
                .filter(|(key, body)| {
                    previous.get(*key).is_none_or(|old: &&solidcraft_engine::doc::ModelBody| {
                        !old.body.same(&body.body) || old.source_transform != body.source_transform
                    })
                })
                .map(|(key, _)| *key)
                .collect();
            let bodies = world
                .bodies
                .iter()
                .filter(|body| changed.contains(&(body.component, body.definition_name())) && self.session.body_visible(&body.name))
                .map(|body| body.name.clone())
                .collect();
            feedback.insert(result.id, FeatureFeedback { bodies, ..FeatureFeedback::default() });
            previous = current;
        }
        for sketch in world.sketches.iter().filter(|sketch| {
            // A consumed sketch may be outlined temporarily on selection. Keep
            // explicit item/folder/occurrence hiding authoritative.
            view::sketch_shown(&self.session, sketch.feature)
                || (!self.session.visibility.hidden_sketches.contains(&sketch.feature)
                    && !self.session.feature_folder_hidden(sketch.feature, "sketches")
                    && self.session.doc.feature(sketch.feature).is_some_and(|feature| {
                        let path = self
                            .session
                            .active_occurrence_path
                            .as_ref()
                            .filter(|_| self.session.active_component == feature.component)
                            .cloned()
                            .unwrap_or_else(|| self.session.doc.first_path(feature.component));
                        self.session.occurrence_visible(&path)
                    }))
        }) {
            if let Some(item) = feedback.get_mut(&sketch.feature) {
                item.lines = view::sketch_lines(&sketch.sketch, &sketch.plane, false, &sketch.report.curve_determined)
                    .into_iter()
                    .map(|(points, _, _)| points)
                    .collect();
            }
        }
        for construct in world.construct.iter().filter(|construct| self.session.construction_visible(&construct.name)) {
            if let Some(item) = feedback.get_mut(&construct.feature) {
                item.construct = Some(construct.geom.clone());
            }
        }
        feedback
    }
}
