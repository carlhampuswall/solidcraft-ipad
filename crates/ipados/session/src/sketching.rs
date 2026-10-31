//! Presentation of native sketch state. The upstream model/solver remain authoritative.
use serde_json::{Value, json};
use solidcraft_engine::{
    Sel,
    geom::{Plane, Vec2},
    sketch::{Analysis, ConstraintKind, CurveKind, Hold, Sketch, measure_dimension, solve_with},
};

use crate::SessionAdapter;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct RectangleMeasurement {
    pub center: Vec2,
    pub preferred: bool,
}

/// Recognise isolated H/V-constrained rectangular loops once on the engine worker.
/// These are display hints only; dimensions and freedom still come from upstream.
fn rectangle_measurements(sk: &Sketch) -> BTreeMap<usize, RectangleMeasurement> {
    let mut incident: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for (index, curve) in sk.curves.iter().enumerate() {
        if let CurveKind::Line { a, b } = curve.kind {
            incident.entry(a).or_default().push(index);
            incident.entry(b).or_default().push(index);
        }
    }
    let axes: BTreeMap<_, _> = sk
        .constraints
        .iter()
        .filter(|c| !c.driven)
        .filter_map(|c| match c.kind {
            ConstraintKind::Horizontal { l } => Some((l, true)),
            ConstraintKind::Vertical { l } => Some((l, false)),
            _ => None,
        })
        .collect();
    let mut rectangles = BTreeMap::new();
    let mut visited = BTreeSet::new();
    for (start, curve) in sk.curves.iter().enumerate() {
        if visited.contains(&start) {
            continue;
        }
        let CurveKind::Line { a: first, .. } = curve.kind else { continue };
        let mut edges = Vec::new();
        let (mut edge, mut point) = (start, first);
        for _ in 0..4 {
            if !axes.contains_key(&edge) {
                break;
            }
            let Some(CurveKind::Line { a, b }) = sk.curves.get(edge).map(|c| &c.kind) else { break };
            let next = if *a == point { *b } else { *a };
            let (Some(p), Some(q)) = (sk.point(point), sk.point(next)) else { break };
            let Some(direction) = (q - p).normalized() else { break };
            edges.push((edge, (p + q) * 0.5, direction));
            let Some(neighbours) = incident.get(&next) else { break };
            let [one, two] = neighbours.as_slice() else { break };
            edge = if *one == edge { *two } else { *one };
            point = next;
            if edge == start {
                break;
            }
        }
        let [(a, ma, da), (b, _, db), (c, mc, dc), (d, _, dd)] = edges.as_slice() else { continue };
        if edge != start || point != first || da.dot(*db).abs() > 1e-8 || da.dot(*dc) > -1.0 + 1e-8 || db.dot(*dd) > -1.0 + 1e-8 {
            continue;
        }
        let center = (*ma + *mc) * 0.5;
        for ((i, m, direction), (opposite, other, _)) in edges.iter().zip(edges.iter().cycle().skip(2)) {
            // Horizontal sides prefer top; vertical sides prefer right, independent
            // of drawing order and endpoint direction.
            let order = if direction.x.abs() >= direction.y.abs() { m.y.total_cmp(&other.y) } else { m.x.total_cmp(&other.x) };
            rectangles.insert(*i, RectangleMeasurement { center, preferred: order.is_gt() || (order.is_eq() && i < opposite) });
        }
        visited.extend([*a, *b, *c, *d]);
    }
    rectangles
}

pub struct SketchSnapshot {
    pub id: u64,
    pub name: String,
    pub plane: Plane,
    pub sketch: Sketch,
    pub profiles: usize,
    pub solved: bool,
    pub dof: usize,
    pub glyphs: Vec<Value>,
    pub(crate) rectangle_measurements: BTreeMap<usize, RectangleMeasurement>,
    pub(crate) free_measurements: Vec<(usize, ConstraintKind, &'static str)>,
}

impl SessionAdapter {
    pub(crate) fn sketch_snapshot(&self) -> Option<SketchSnapshot> {
        let id = self.session.active_sketch?;
        let world = self.session.world_state();
        let ss = world.sketch(id)?;
        // Analyse sizes on the engine worker, once per immutable publication. A
        // floating rigid shape still has positional freedom, so neither sketch
        // DOF nor curve_determined alone says whether its length can change.
        let freedom = if ss.report.point_determined.len() == ss.sketch.points.len() && ss.report.curve_determined.len() == ss.sketch.curves.len() {
            ss.report.clone()
        } else {
            solve_with(&mut ss.sketch.clone(), &Hold::default(), None, Analysis::Freedom, None)
        };
        // A plain sketch with only its origin locked can scale freely. Avoid a
        // separate solve for every edge in large, unconstrained imports.
        let freely_scalable = ss.sketch.constraints.iter().all(|c| c.driven)
            && ss.sketch.patterns.is_empty()
            && ss.sketch.links.is_empty()
            && ss.sketch.offsets.is_empty()
            && ss.sketch.freeform_offsets.is_empty()
            && ss.sketch.wires.is_empty()
            && ss.sketch.equations.is_empty()
            && ss.sketch.curves.iter().all(|c| !c.fixed && c.link.is_none())
            && ss.sketch.locked_points().iter().enumerate().all(|(index, locked)| index == 0 || !locked);
        let free_measurements = ss
            .sketch
            .curves
            .iter()
            .enumerate()
            .filter_map(|(index, curve)| {
                if !freedom.keeps_orientation() || freedom.dof == 0 || freedom.curve_determined.get(index) == Some(&true) {
                    return None;
                }
                let (mut kind, dimension) = match curve.kind {
                    CurveKind::Line { .. } => (ConstraintKind::Length { l: index, value: 0.0 }, "length"),
                    CurveKind::Circle { .. } | CurveKind::Arc { .. } => (ConstraintKind::Radius { c: index, value: 0.0 }, "radius"),
                    _ => return None,
                };
                let value = measure_dimension(&ss.sketch, &kind).filter(|v| v.is_finite() && *v > 0.0)?;
                match &mut kind {
                    ConstraintKind::Length { value: size, .. } | ConstraintKind::Radius { value: size, .. } => *size = value,
                    _ => return None,
                }
                if freely_scalable {
                    return Some((index, kind, dimension));
                }
                let mut probe = ss.sketch.clone();
                probe.add_constraint(kind.clone(), None).ok()?;
                let report = solve_with(&mut probe, &Hold::default(), None, Analysis::Freedom, None);
                (report.keeps_orientation() && report.dof < freedom.dof).then_some((index, kind, dimension))
            })
            .collect();
        let glyphs = if self.session.model.is_stale() {
            Vec::new()
        } else {
            self.session
                .scratch()
                .execute("sketch.glyphs", &json!({"sketch":id}))
                .ok()
                .and_then(|v| v["glyphs"].as_array().cloned())
                .unwrap_or_default()
        };
        Some(SketchSnapshot {
            glyphs,
            rectangle_measurements: rectangle_measurements(&ss.sketch),
            free_measurements,
            id,
            name: ss.name.clone(),
            plane: ss.plane,
            sketch: ss.sketch.clone(),
            profiles: ss.profiles.len(),
            solved: ss.report.keeps_orientation(),
            dof: ss.report.dof,
        })
    }

    pub fn preview_sketch_plane(&self, plane: Value) -> Result<Plane, crate::AdapterError> {
        let mut scratch = self.session.scratch();
        if scratch.active_sketch.is_some() {
            return Err(crate::error("disabled", "Finish the active sketch before selecting another plane."));
        }
        scratch.execute("sketch.create", &json!({"plane":plane}))?;
        solidcraft_engine::view::open_sketch_plane(&scratch).ok_or_else(|| crate::error("plane", "The selected sketch plane could not be evaluated."))
    }

    /// Use the engine's existing inspection reply rather than a parallel entity schema.
    pub(crate) fn sketch_workspace(&mut self, selection: &[Sel]) -> Value {
        let Some(id) = self.session.active_sketch else { return Value::Null };
        let Ok(mut value) = self.session.execute("sketch.inspect", &json!({"sketch":id})) else { return Value::Null };
        let state = self.session.world_state();
        let Some(ss) = state.sketch(id) else { return value };
        value["plane"] = json!(ss.plane);
        value["solved"] = json!(ss.report.keeps_orientation());
        value["report"] = json!(ss.report);
        let sk = &ss.sketch;
        value["projection_sources"] = json!(self.projection_sources());
        value["projection_links"] = json!(
            sk.links
                .iter()
                .map(|l| {
                    let label = match &l.source {
                        solidcraft_engine::sketch::LinkSource::SketchCurve { sketch, curve } => {
                            self.session.doc.feature(*sketch).map(|f| format!("{} · {curve}", f.name))
                        }
                        solidcraft_engine::sketch::LinkSource::SketchPoint { sketch, point } => {
                            self.session.doc.feature(*sketch).map(|f| format!("{} · {point}", f.name))
                        }
                        _ => None,
                    }
                    .unwrap_or_else(|| l.source.describe());
                    json!({"id":l.id,"label":label})
                })
                .collect::<Vec<_>>()
        );
        if let Some(constraints) = value.get_mut("constraints").and_then(Value::as_array_mut) {
            for row in constraints {
                let Some(c) = row["id"].as_str().and_then(|id| sk.constraints.iter().find(|c| c.id == id)) else { continue };
                row["dimension"] = json!(c.kind.is_dimension());
                row["driven"] = json!(c.driven);
                row["units"] = json!(if c.kind.is_angle() { "deg" } else { "mm" });
                row["value"] =
                    json!(solidcraft_engine::sketch::measure_dimension(sk, &c.kind).map(|n| if c.kind.is_angle() { n.to_degrees() } else { n }));
                let (points, curves) = c.kind.indices();
                let entities: Vec<_> = points
                    .iter()
                    .filter_map(|i| sk.points.get(*i).map(|p| p.id.clone()))
                    .chain(curves.iter().filter_map(|i| sk.curves.get(*i).map(|c| c.id.clone())))
                    .collect();
                row["entities"] = json!(entities);
                row["selected"] = json!(selection.iter().any(|s| matches!(s, Sel::SketchConstraint { id } if id == &c.id)));
                if c.kind.is_dimension()
                    && let Some(frame) = solidcraft_engine::sketch::dim_frame(sk, &c.kind)
                {
                    row["at"] = json!(solidcraft_engine::sketch::dim_layout(&frame, c.text, 1.0, solidcraft_engine::geom::Vec2::ZERO).text);
                }
            }
        }
        value["eligible_constraints"] = json!(constraint_actions(sk, selection));
        let refs: Vec<_> = selection
            .iter()
            .filter_map(|s| match s {
                Sel::SketchCurve { id, .. } | Sel::SketchPoint { id } => Some(id.as_str()),
                _ => None,
            })
            .collect();
        let mut dimensions: Vec<&str> = Vec::new();
        match refs.as_slice() {
            [one] => {
                if let Some(c) = sk.curve_index(one).and_then(|i| sk.curves.get(i)) {
                    match c.kind {
                        CurveKind::Line { .. } => dimensions.extend(["length", "horizontal", "vertical", "angle"]),
                        CurveKind::Circle { .. } | CurveKind::Arc { .. } => dimensions.extend(["radius", "diameter"]),
                        _ => {}
                    }
                }
            }
            [a, b] => {
                let pa = sk.resolve_point(a).is_some();
                let pb = sk.resolve_point(b).is_some();
                let ca = sk.curve_index(a).and_then(|i| sk.curves.get(i));
                let cb = sk.curve_index(b).and_then(|i| sk.curves.get(i));
                if pa && pb {
                    dimensions.extend(["distance", "horizontal", "vertical"]);
                } else if ca.is_some() && cb.is_some() {
                    dimensions.push(
                        if ca.is_some_and(|c| matches!(c.kind, CurveKind::Line { .. }))
                            && cb.is_some_and(|c| matches!(c.kind, CurveKind::Line { .. }))
                        {
                            "angle"
                        } else {
                            "distance"
                        },
                    );
                } else if pa && cb.is_some() || pb && ca.is_some() {
                    dimensions.push("distance");
                }
            }
            _ => {}
        }
        value["eligible_dimensions"] = json!(dimensions);
        value
    }
}

pub(crate) fn constraint_actions(sk: &Sketch, selection: &[Sel]) -> Vec<Value> {
    let ids: Vec<_> = selection
        .iter()
        .filter_map(|s| match s {
            Sel::SketchCurve { id, .. } | Sel::SketchPoint { id } => Some(id.clone()),
            _ => None,
        })
        .collect();
    let mut out = Vec::new();
    let mut add = |name: &str, mut params: Value| {
        params["if_redundant"] = json!("refuse");
        out.push(json!({"command":format!("sketch.constraint.{name}"),"params":params}));
    };
    if ids.len() == 1 {
        if ids.first().and_then(|id| sk.curve_index(id)).and_then(|i| sk.curves.get(i)).is_some_and(|c| matches!(c.kind, CurveKind::Line { .. })) {
            for mode in ["horizontal", "vertical"] {
                add("horizontal_vertical", json!({"line":ids[0],"mode":mode}));
            }
        }
        add("fix", json!({"entities":ids}));
    } else if let [a, b] = ids.as_slice() {
        let pa = sk.resolve_point(a).is_some();
        let pb = sk.resolve_point(b).is_some();
        let ca = sk.curve_index(a).and_then(|i| sk.curves.get(i));
        let cb = sk.curve_index(b).and_then(|i| sk.curves.get(i));
        let line = |c: Option<&solidcraft_engine::sketch::Curve>| c.is_some_and(|c| matches!(c.kind, CurveKind::Line { .. }));
        let round =
            |c: Option<&solidcraft_engine::sketch::Curve>| c.is_some_and(|c| matches!(c.kind, CurveKind::Circle { .. } | CurveKind::Arc { .. }));
        if pa && (pb || cb.is_some()) || pb && ca.is_some() || line(ca) && line(cb) {
            add("coincident", json!({"a":a,"b":b}));
        }
        if line(ca) && line(cb) {
            for name in ["parallel", "perpendicular", "equal"] {
                add(name, json!({"a":a,"b":b}));
            }
        }
        if pa && line(cb) || pb && line(ca) || line(ca) && line(cb) {
            add("midpoint", json!({"a":a,"b":b}));
        }
        if round(ca) && round(cb) {
            for name in ["concentric", "equal"] {
                add(name, json!({"a":a,"b":b}));
            }
        }
        if ca.is_some() && cb.is_some() && (round(ca) || round(cb)) {
            add("tangent", json!({"a":a,"b":b}));
        }
        add("fix", json!({"entities":ids}));
    }
    out
}
