//! Strict native reference validation before accepting a modelling proposal.
//! The shared evaluator owns identity resolution. Native forms require an exact
//! result instead of accepting its optional nearest-geometry recovery.

use solidcraft_engine::doc::{FeatureKind, face_point_state, naming, profile_selection_indices};

use crate::{AdapterError, SessionAdapter, error};

impl SessionAdapter {
    /// Check references in each feature's actual upstream state. This does not
    /// manufacture topology identities or change the saved feature definition.
    pub(crate) fn validate_model_references(&self) -> Result<(), AdapterError> {
        for feature in &self.session.doc.features {
            if feature.suppressed || self.session.model.result(feature.id).is_none_or(|result| result.skipped) {
                continue;
            }
            let fail = |detail: String| {
                error("unresolved_reference", format!("{}: {detail}. Edit the feature and explicitly reselect its input.", feature.name))
            };
            let before = self.session.model.state_before(feature.id);
            for (sketch, selection) in feature.kind.profile_sels() {
                let solved = before.sketch(sketch).ok_or_else(|| fail(format!("source sketch {sketch} is unavailable")))?;
                let indices = profile_selection_indices(solved, selection).map_err(|failure| fail(failure.to_string()))?;
                if indices.is_empty() {
                    return Err(fail("the selected sketch region is empty".into()));
                }
            }
            if let FeatureKind::Fillet { edges, body, .. } | FeatureKind::Chamfer { edges, body, .. } = &feature.kind {
                for (index, point) in edges.iter().enumerate() {
                    let name = feature
                        .edge_names
                        .get(index)
                        .filter(|name| !name.is_empty())
                        .ok_or_else(|| fail("an edge has no stored native identity".into()))?;
                    let resolved = before
                        .bodies
                        .iter()
                        .filter(|candidate| body.as_ref().is_none_or(|body| candidate.name == body.0))
                        .any(|candidate| naming::locate_edge(candidate, name, point.0).is_ok_and(|hit| hit.note(name).is_none()));
                    if !resolved {
                        return Err(fail(format!("edge `{name}` no longer resolves exactly")));
                    }
                }
            }
            let faces = face_point_state(&self.session.doc, &before, &feature.kind, feature.component);
            for (point, name) in feature.kind.face_points().into_iter().zip(&feature.face_names) {
                // Empty native entries can mean an explicit spatial extent point;
                // they are not a face reference and must retain that meaning.
                if !name.is_empty() && naming::locate_face(&faces, name, point).is_none_or(|hit| hit.note(name).is_some()) {
                    return Err(fail(format!("face `{name}` no longer resolves exactly")));
                }
            }
        }
        Ok(())
    }
}

/// These diagnostics describe a fallback reference, rather than a Boolean retry
/// or another warning that can still produce a valid current shape.
pub(crate) fn risky_reference_warning(warning: &str) -> bool {
    ["reference", "no longer exists", "nearest", "found by geometry", "was split", "was merged", "profile"].iter().any(|part| warning.contains(part))
}
