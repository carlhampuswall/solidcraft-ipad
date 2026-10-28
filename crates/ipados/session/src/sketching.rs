//! Presentation of native sketch state. The upstream model/solver remain authoritative.
use serde_json::{Value, json};
use solidcraft_engine::{
    Sel,
    geom::Plane,
    sketch::{CurveKind, Sketch},
};

use crate::SessionAdapter;

pub struct SketchSnapshot {
    pub id: u64,
    pub name: String,
    pub plane: Plane,
    pub sketch: Sketch,
    pub profiles: usize,
    pub solved: bool,
    pub dof: usize,
    pub glyphs: Vec<Value>,
}

impl SessionAdapter {
    pub(crate) fn sketch_snapshot(&self) -> Option<SketchSnapshot> {
        let id = self.session.active_sketch?;
        let world = self.session.world_state();
        let ss = world.sketch(id)?;
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

fn constraint_actions(sk: &Sketch, selection: &[Sel]) -> Vec<Value> {
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
