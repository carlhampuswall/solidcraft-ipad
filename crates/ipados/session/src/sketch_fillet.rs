//! Source-corner picking for the native live editor. The engine alone makes the arcs.
use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use solidcore_record::Vec2;
use solidcraft_engine::geom::prelude::*;
use solidcraft_engine::{
    Sel,
    sketch::{ConstraintKind, CurveKind, Sketch},
};

use crate::{AdapterError, ModelEdit, SessionAdapter, error, modelling::ModelHandle};

pub(crate) struct Corner {
    pub reference: Value,
    pub at: Vec2,
    direction: Vec2,
    suggested_radius: f64,
}

/// Connectivity comes from shared endpoints and actual Coincident constraints,
/// never from a screen tolerance or nearby, unrelated geometry.
pub(crate) fn corners(sk: &Sketch, selection: &[Sel]) -> Vec<Corner> {
    let mut neighbours: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for constraint in &sk.constraints {
        if let ConstraintKind::Coincident { p, q } = constraint.kind {
            neighbours.entry(p).or_default().push(q);
            neighbours.entry(q).or_default().push(p);
        }
    }
    let mut groups = BTreeMap::new();
    for &point in neighbours.keys() {
        if groups.contains_key(&point) {
            continue;
        }
        let mut pending = vec![point];
        groups.insert(point, point);
        while let Some(next) = pending.pop() {
            for &other in neighbours.get(&next).into_iter().flatten() {
                if let std::collections::btree_map::Entry::Vacant(entry) = groups.entry(other) {
                    entry.insert(point);
                    pending.push(other);
                }
            }
        }
    }
    let group = |point: usize| groups.get(&point).copied().unwrap_or(point);
    let curves: BTreeSet<_> = selection
        .iter()
        .filter_map(|s| match s {
            Sel::SketchCurve { id, .. } => Some(id.as_str()),
            _ => None,
        })
        .collect();
    let points: BTreeSet<_> = selection
        .iter()
        .filter_map(|s| match s {
            Sel::SketchPoint { id } => sk.resolve_point(id).map(group),
            _ => None,
        })
        .collect();
    let mut ends: BTreeMap<usize, Vec<(&str, usize, usize)>> = BTreeMap::new();
    for curve in &sk.curves {
        if !matches!(curve.kind, CurveKind::Line { .. } | CurveKind::Arc { .. }) || curve.infinite {
            continue;
        }
        let Some((a, b)) = curve.kind.ends() else { continue };
        for (near, far) in [(a, b), (b, a)] {
            let corner = group(near);
            if curves.contains(curve.id.as_str()) || points.contains(&corner) && !curve.construction {
                ends.entry(corner).or_default().push((&curve.id, near, far));
            }
        }
    }
    ends.values()
        .filter_map(|sides| {
            let [(a, near, far_a), (b, _, far_b)] = sides.as_slice() else { return None };
            if a == b {
                return None;
            }
            let at = sk.point(*near)?;
            let va = sk.point(*far_a)? - at;
            let vb = sk.point(*far_b)? - at;
            let away = |id: &str| {
                let shape = sk.shape(sk.curve_index(id)?)?;
                let (start, end) = shape.range();
                Some(if shape.point(start).dist(at) <= shape.point(end).dist(at) { shape.tangent(start) } else { -shape.tangent(end) })
            };
            let (da, db) = (away(a)?, away(b)?);
            let direction = (da + db).normalized()?;
            let cosine = da.dot(db).clamp(-1., 1.);
            let tangent = ((1. - cosine) / (1. + cosine)).sqrt();
            let suggested_radius = (va.len().min(vb.len()) * tangent * 0.1).min(2.);
            // Two curves can share both ends. Name the particular endpoint in
            // that case so the command rounds each corner once.
            let reference = if group(*far_a) == group(*far_b) { json!({"point":sk.points.get(*near)?.id}) } else { json!({"a":a,"b":b}) };
            (suggested_radius.is_finite() && suggested_radius > 1e-8).then_some(Corner { reference, at, direction, suggested_radius })
        })
        .take(500)
        .collect()
}

impl SessionAdapter {
    pub(crate) fn sketch_fillet_form(&self, edit: &mut ModelEdit, selection: &[Sel]) -> Result<(), AdapterError> {
        let id = self.session.active_sketch.ok_or_else(|| error("selection", "Create or reopen a sketch first"))?;
        let world = self.session.world_state()?;
        let sketch = world.sketch(id).ok_or_else(|| error("selection", "The active sketch is unavailable"))?;
        let references: Vec<_> = selection
            .iter()
            .filter_map(|selected| match selected {
                Sel::SketchCurve { id, sketch: source }
                    if source.is_none_or(|source| source == sketch.feature) && sketch.sketch.curve_index(id).is_some() =>
                {
                    Some(id)
                }
                Sel::SketchPoint { id } if sketch.sketch.resolve_point(id).is_some() => Some(id),
                _ => None,
            })
            .collect();
        let candidates = corners(&sketch.sketch, selection);
        let radius = candidates.first().map_or(1., |corner| corner.suggested_radius);
        edit.params = json!({"sketch":id,"edges":references,"radius":format!("{radius} mm"),"_fillet_anchor":candidates.first().map(|corner| &corner.reference)});
        edit.fields = vec![
            json!({"key":"edges","label":"ui.ipados.modelling.field.edges","kind":"references","options":references.iter().map(|id|json!({"label":id,"value":id})).collect::<Vec<_>>()}),
            json!({"key":"radius","label":"radius","kind":"expression","units":"mm","context":"length","options":[]}),
        ];
        Ok(())
    }

    fn fillet_selection(&self, params: &Value) -> Result<Vec<Sel>, AdapterError> {
        let id = self
            .session
            .active_sketch
            .filter(|id| params["sketch"].as_u64() == Some(*id))
            .ok_or_else(|| error("stale_edit", "The active sketch changed"))?;
        let world = self.session.world_state()?;
        let sketch = world.sketch(id).ok_or_else(|| error("selection", "The active sketch is unavailable"))?;
        params["edges"]
            .as_array()
            .filter(|edges| edges.len() <= 500)
            .ok_or_else(|| error("selection", "Choose at most 500 sketch edges"))?
            .iter()
            .map(|value| {
                let reference = value.as_str().ok_or_else(|| error("selection", "Choose sketch edges or corners"))?;
                if sketch.sketch.curve_index(reference).is_some() {
                    Ok(Sel::SketchCurve { id: reference.into(), sketch: Some(id) })
                } else if sketch.sketch.resolve_point(reference).is_some() {
                    Ok(Sel::SketchPoint { id: reference.into() })
                } else {
                    Err(error("stale_edit", "The selected sketch geometry changed"))
                }
            })
            .collect()
    }

    pub(crate) fn sketch_fillet_command(&self, params: &Value) -> Result<Value, AdapterError> {
        let selection = self.fillet_selection(params)?;
        let world = self.session.world_state()?;
        let sketch =
            self.session.active_sketch.and_then(|id| world.sketch(id)).ok_or_else(|| error("selection", "The active sketch is unavailable"))?;
        let corners = corners(&sketch.sketch, &selection);
        if corners.is_empty() {
            return Err(error("selection", "ui.ipados.sketch.fillet_hint"));
        }
        Ok(json!({"sketch":sketch.feature,"points":corners.iter().map(|corner| &corner.reference).collect::<Vec<_>>(),"radius":params["radius"]}))
    }

    pub(crate) fn refresh_sketch_fillet_anchor(&self, edit: &mut ModelEdit) -> Result<(), AdapterError> {
        let selection = self.fillet_selection(&edit.params)?;
        let world = self.session.world_state()?;
        let sketch =
            self.session.active_sketch.and_then(|id| world.sketch(id)).ok_or_else(|| error("selection", "The active sketch is unavailable"))?;
        let candidates = corners(&sketch.sketch, &selection);
        edit.params["_fillet_anchor"] = candidates
            .iter()
            .find(|corner| corner.reference == edit.params["_fillet_anchor"])
            .or_else(|| candidates.first())
            .map(|corner| corner.reference.clone())
            .unwrap_or(Value::Null);
        Ok(())
    }

    pub(crate) fn sketch_fillet_handle(&self, edit: &ModelEdit) -> Option<ModelHandle> {
        let selection = self.fillet_selection(&edit.params).ok()?;
        let world = self.session.world_state().ok()?;
        let sketch = world.sketch(self.session.active_sketch?)?;
        let corners = corners(&sketch.sketch, &selection);
        let corner = corners.iter().find(|corner| corner.reference == edit.params["_fillet_anchor"]).or_else(|| corners.first())?;
        let radius = edit.params["radius"]
            .as_str()
            .and_then(|value| self.session.doc.eval(value, solidcraft_engine::doc::expr::Kind::Length).ok())
            .or_else(|| edit.params["radius"].as_f64())
            .filter(|r| r.is_finite() && *r > 0.)?;
        Some(ModelHandle {
            origin: sketch.plane.to_world(corner.at),
            normal: sketch.plane.x * corner.direction.x + sketch.plane.y * corner.direction.y,
            distance: radius,
            factor: 1.,
            parameter: "radius".into(),
            units: "mm".into(),
            angular: false,
            arm: sketch.plane.x,
            size: 1.,
        })
    }
}
