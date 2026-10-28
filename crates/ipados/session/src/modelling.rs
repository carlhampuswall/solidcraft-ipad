//! Native forms over the shared command registry. Only provisional presentation
//! state lives here; documents, topology, evaluation and history remain upstream.
use std::sync::Arc;

use serde::Serialize;
use serde_json::{Value, json};
use solidcraft_engine::{
    ComputedCommands, Sel,
    doc::{FeatureKind, ProfileSel, capture_profile_selection},
};

use crate::{AdapterError, SceneSnapshot, SessionAdapter, Viewport, error};

const TOOLS: &[&str] = &[
    "solid.extrude",
    "solid.offset_face",
    "solid.fillet",
    "solid.chamfer",
    "solid.revolve",
    "solid.shell",
    "solid.hole",
    "solid.combine",
    "solid.pattern.rectangular",
    "solid.pattern.circular",
    "solid.pattern.path",
    "solid.mirror",
    "solid.sweep",
    "solid.loft",
    "parameters.change",
    "parameters.add",
];

#[derive(Clone)]
pub struct ModelEdit {
    pub token: u64,
    pub version: u64,
    pub operation: String,
    pub feature: Option<u64>,
    pub params: Value,
    pub(crate) document_id: String,
    pub(crate) geometry_revision: u64,
    pub(crate) fields: Vec<Value>,
    pub(crate) preview: Option<Arc<SceneSnapshot>>,
    /// Display-only last validated pose while a newer Pencil proposal is pending.
    /// This never validates the current parameters or enables confirmation.
    pub(crate) last_preview: Option<Arc<SceneSnapshot>>,
    pub(crate) handle: Option<ModelHandle>,
    pub(crate) repair_source: Option<Arc<SceneSnapshot>>,
    pub reply: Value,
    pub feedback: Vec<String>,
}

#[derive(Clone, Serialize)]
pub struct ModelHandle {
    pub origin: solidcraft_engine::geom::Vec3,
    pub normal: solidcraft_engine::geom::Vec3,
    pub distance: f64,
    pub factor: f64,
    pub parameter: String,
    pub units: String,
    pub angular: bool,
    pub arm: solidcraft_engine::geom::Vec3,
    pub size: f64,
}
impl ModelHandle {
    pub(crate) fn point(&self) -> solidcraft_engine::geom::Vec3 {
        if self.angular {
            let angle = self.distance.to_radians();
            self.origin + (self.arm * angle.cos() + self.normal.cross(self.arm) * angle.sin()) * self.size
        } else {
            self.origin + self.normal * self.distance * self.factor
        }
    }
    pub(crate) fn interaction_plane(&self, back: solidcraft_engine::geom::Vec3) -> Option<solidcraft_engine::geom::Plane> {
        Some(if self.angular {
            solidcraft_engine::geom::Plane { origin: self.origin, x: self.arm, y: self.normal.cross(self.arm) }
        } else {
            solidcraft_engine::geom::Plane { origin: self.origin, x: self.normal, y: back.cross(self.normal).normalized()? }
        })
    }
    pub(crate) fn proposal(&self, anchor: solidcraft_engine::geom::Vec3, point: solidcraft_engine::geom::Vec3, value: f64) -> f64 {
        if self.angular {
            let a = anchor - self.origin;
            let b = point - self.origin;
            value + self.normal.dot(a.cross(b)).atan2(a.dot(b)).to_degrees()
        } else {
            value + (point - anchor).dot(self.normal) / self.factor
        }
    }
    pub(crate) fn set_parameter(&self, params: &mut Value, value: f64) {
        let expr = json!(format!("{value} {}", self.units));
        if let Some((parent, key)) = self.parameter.split_once('.') {
            params[parent][key] = expr;
        } else {
            params[&self.parameter] = expr;
        }
    }
}

pub struct PreparedModel {
    pub model: Arc<SceneSnapshot>,
    pub params: Value,
    pub(crate) version: u64,
    pub(crate) handle: Option<ModelHandle>,
    pub(crate) transaction: Option<ComputedCommands>,
    pub reply: Value,
    pub feedback: Vec<String>,
}

fn option(label: impl Into<String>, value: Value) -> Value {
    json!({"label":label.into(),"value":value})
}
fn field(key: &str, label: &str, kind: &str, options: Vec<Value>) -> Value {
    let known = matches!(
        key,
        "profile"
            | "targets"
            | "body"
            | "target"
            | "bodies"
            | "tools"
            | "edges"
            | "faces"
            | "distance"
            | "radius"
            | "diameter"
            | "depth"
            | "thickness"
            | "angle"
            | "axis"
            | "direction"
            | "operation"
            | "keep_tools"
            | "path_sketch"
            | "path_curves"
            | "sections"
            | "twist"
            | "scale"
            | "dir1"
            | "count1"
            | "spacing1"
            | "count"
            | "plane"
            | "name"
            | "expression"
            | "unit"
            | "extent.direction"
            | "extent.distance"
            | "chamfer_mode"
            | "distance2"
            | "through_all"
            | "extent.through_all"
            | "flip"
            | "source_mode"
            | "features"
            | "spacing"
            | "orientation"
    );
    let label = if known { format!("ui.ipados.modelling.field.{}", key.replace('.', "_")) } else { label.to_string() };
    json!({"key":key,"label":label,"kind":kind,"options":options})
}
fn choices(values: &[&str]) -> Vec<Value> {
    values.iter().map(|v| option(*v, json!(v))).collect()
}
fn expression(fields: &mut Vec<Value>, key: &str, label: &str) {
    fields.push(field(key, label, "expression", Vec::new()));
}

fn boolean_choices() -> Vec<Value> {
    vec![option("false", json!(false)), option("true", json!(true))]
}

fn profile_reference(sketch: &solidcraft_engine::doc::SolvedSketch, indices: &[usize]) -> Result<Value, AdapterError> {
    let captured = capture_profile_selection(sketch, indices).map_err(|e| error("profile", e.to_string()))?;
    let profiles = match captured {
        ProfileSel::Curves { loops } => json!(loops.iter().map(|curves| json!({"curves":curves})).collect::<Vec<_>>()),
        ProfileSel::All => json!("all"),
        ProfileSel::Indices { indices } => json!(indices),
        ProfileSel::Points { points } => json!(points.iter().map(|point| json!({"point":point})).collect::<Vec<_>>()),
        captured => serde_json::to_value(captured).map_err(|e| error("profile", e.to_string()))?,
    };
    Ok(json!({"sketch":sketch.feature,"profiles":profiles}))
}

/// Presentation eligibility for the upstream feature-replay path. Body sources
/// use a separate mode and never masquerade as replayed feature definitions.
fn replayable(kind: &FeatureKind) -> bool {
    matches!(
        kind,
        FeatureKind::Extrude { .. }
            | FeatureKind::Revolve { .. }
            | FeatureKind::Fillet { .. }
            | FeatureKind::Chamfer { .. }
            | FeatureKind::Box { .. }
            | FeatureKind::Cylinder { .. }
            | FeatureKind::Sphere { .. }
            | FeatureKind::Torus { .. }
            | FeatureKind::Cone { .. }
            | FeatureKind::Hole { .. }
            | FeatureKind::Sweep { .. }
            | FeatureKind::Loft { .. }
            | FeatureKind::Pipe { .. }
    ) || matches!(kind, FeatureKind::Pattern { bodies, .. } | FeatureKind::Mirror { bodies, .. } if bodies.is_empty())
}

fn path_fields(fields: &mut Vec<Value>, sketches: Vec<Value>, curves: Vec<Value>, prefix: &str) {
    fields.push(field(&format!("{prefix}path_sketch"), "Path sketch", "reference", sketches));
    fields.push(field("path_curves", "Ordered path curves", "references", curves));
}

impl SessionAdapter {
    fn replay_source_options(&self, native_ids: bool) -> Vec<Value> {
        self.session
            .doc
            .features
            .iter()
            .filter(|feature| {
                !feature.suppressed
                    && replayable(&feature.kind)
                    && self.session.model.result(feature.id).is_some_and(|result| !result.skipped && result.error.is_none())
            })
            .map(|feature| option(feature.name.clone(), if native_ids { json!(feature.id) } else { json!(feature.name) }))
            .collect()
    }

    fn model_path_options(&self) -> (Vec<Value>, Vec<Value>) {
        let state = self.session.model.state();
        let sketches = state.sketches.iter().map(|sketch| option(sketch.name.clone(), json!(sketch.feature))).collect();
        let curves = state
            .sketches
            .iter()
            .flat_map(|sketch| {
                sketch
                    .sketch
                    .curves
                    .iter()
                    .map(move |curve| option(format!("{} · {}", sketch.name, curve.id), json!({"sketch":sketch.feature,"curve":curve.id})))
            })
            .collect();
        (sketches, curves)
    }

    pub fn modelling_workspace(&self) -> Value {
        let timeline: Vec<_> = self
            .session
            .doc
            .features
            .iter()
            .enumerate()
            .map(|(order, f)| {
                let result = self.session.model.result(f.id);
                json!({"id":f.id,"name":f.name,"kind":f.kind.type_name(),"order":order,"suppressed":f.suppressed,
                "error":result.and_then(|r|r.error.as_ref()),"warning":result.and_then(|r|r.warning.as_ref()),"stale":result.is_some_and(|r|r.stale)})
            })
            .collect();
        let parameters: Vec<_> = self
            .session
            .doc
            .param_rows()
            .into_iter()
            .map(|p| json!({"name":p.name,"expression":p.expression,"unit":p.unit,"value":p.value}))
            .collect();
        json!({"tools":TOOLS.iter().map(|id|json!({"id":id,"label":format!("cmd.{id}")})).collect::<Vec<_>>(),"timeline":timeline,"parameters":parameters})
    }

    pub fn begin_model(&self, token: u64, operation: Option<&str>, feature: Option<u64>, selection: &[Sel]) -> Result<ModelEdit, AdapterError> {
        if self.session.active_sketch.is_some() {
            return Err(error("disabled", "Finish the sketch before modelling."));
        }
        let mut context = Self { session: self.session.scratch(), geometry_revision: self.geometry_revision, workspace_cache: None };
        let mut params = json!({});
        let operation = if let Some(id) = feature {
            let position =
                self.session.doc.features.iter().position(|f| f.id == id).ok_or_else(|| error("stale_edit", "The feature no longer exists."))?;
            let inspect = context.execute("timeline.inspect", json!({"feature":id}))?.value;
            params = inspect["definition"].clone();
            // Offer only geometry before the edited feature. No authoritative marker is moved.
            context.execute("timeline.roll_to", json!({"position":position}))?;
            "timeline.edit"
        } else {
            let id = operation.ok_or_else(|| error("bad_params", "Choose a modelling operation."))?;
            if !TOOLS.contains(&id) && !crate::assembly::supported(id) {
                return Err(error("unsupported", "This operation has no native modelling form."));
            }
            id
        };
        let mut edit = ModelEdit {
            token,
            version: 1,
            operation: operation.into(),
            feature,
            params,
            document_id: self.session.document_id.clone(),
            geometry_revision: self.geometry_revision,
            fields: Vec::new(),
            preview: None,
            last_preview: None,
            handle: None,
            repair_source: None,
            reply: Value::Null,
            feedback: Vec::new(),
        };
        if feature.is_some() {
            // A complete native definition is retained unchanged. Only real expression
            // leaves are exposed; reference payloads are never rewritten as display IDs.
            let inputs = self.session.doc.feature(feature.unwrap_or_default()).map(|f| f.kind.inputs()).unwrap_or_default();
            let rows = self.session.doc.param_rows();
            let input_names = self.session.doc.feature(feature.unwrap_or_default()).map(|f| f.param_names.clone()).unwrap_or_default();
            fn visit(
                value: &mut Value,
                path: String,
                inputs: &[(&str, String, solidcraft_engine::doc::expr::Kind)],
                rows: &[solidcraft_engine::doc::ParamRow],
                input_names: &[String],
                fields: &mut Vec<Value>,
            ) {
                match value {
                    Value::Object(map) => {
                        for (key, v) in map {
                            visit(v, if path.is_empty() { key.clone() } else { format!("{path}.{key}") }, inputs, rows, input_names, fields);
                        }
                    }
                    Value::String(text) => {
                        if let Some((label, _, kind)) = inputs.iter().find(|(_, e, _)| e == text) {
                            if let Some(row) = rows.iter().find(|r| input_names.contains(&r.name) && r.name == *text) {
                                *text = row.expression.clone();
                            }
                            let mut f = field(&path, label, "expression", Vec::new());
                            f["units"] = json!(kind.default_unit());
                            fields.push(f);
                        }
                    }
                    _ => {}
                }
            }
            visit(&mut edit.params, String::new(), &inputs, &rows, &input_names, &mut edit.fields);
            if let Some(native) = self.session.doc.feature(feature.unwrap_or_default())
                && matches!(
                    native.kind,
                    FeatureKind::Extrude { .. }
                        | FeatureKind::Revolve { .. }
                        | FeatureKind::Sweep { .. }
                        | FeatureKind::Loft { .. }
                        | FeatureKind::Hole { .. }
                )
                && edit.params.get("targets").is_none()
            {
                edit.params["targets"] = json!([]);
            }
            if let Some(native) = self.session.doc.feature(feature.unwrap_or_default())
                && matches!(native.kind, FeatureKind::Hole { .. })
            {
                edit.params["targets"] = json!(native.participants);
            }
            if let Some(native) = self.session.doc.feature(feature.unwrap_or_default()) {
                match &native.kind {
                    FeatureKind::Pattern { features, bodies, pattern } => {
                        edit.params["source_mode"] = json!(if bodies.is_empty() { "features" } else { "bodies" });
                        edit.params["features"] = json!(features);
                        edit.params["bodies"] = json!(bodies);
                        edit.fields.push(field("source_mode", "Source mode", "choice", choices(&["bodies", "features"])));
                        edit.fields.push(field("features", "Source features (replayed)", "references", context.replay_source_options(true)));
                        if let solidcraft_engine::doc::PatternKind::Path { path_sketch, path, orient, .. } = pattern {
                            let (sketches, curves) = context.model_path_options();
                            path_fields(&mut edit.fields, sketches, curves, "pattern.");
                            edit.params["path_curves"] =
                                json!(path.iter().map(|curve| json!({"sketch":path_sketch,"curve":curve})).collect::<Vec<_>>());
                            edit.params["orientation"] = json!(if *orient { "path" } else { "identical" });
                            edit.fields.push(field("orientation", "Orientation", "choice", choices(&["identical", "path"])));
                        }
                    }
                    FeatureKind::Mirror { features, bodies, .. } => {
                        edit.params["source_mode"] = json!(if bodies.is_empty() { "features" } else { "bodies" });
                        edit.params["features"] = json!(features);
                        edit.params["bodies"] = json!(bodies);
                        edit.fields.push(field("source_mode", "Source mode", "choice", choices(&["bodies", "features"])));
                        edit.fields.push(field("features", "Source features (replayed)", "references", context.replay_source_options(true)));
                    }
                    FeatureKind::Extrude { .. } => {
                        if edit.params["extent"].get("through_all").is_none() {
                            edit.params["extent"]["through_all"] = json!(false);
                        }
                        edit.fields.push(field("extent.through_all", "Through All", "choice", boolean_choices()));
                    }
                    FeatureKind::Hole { depth, .. } => {
                        edit.params["through_all"] = json!(depth.is_none());
                        if depth.is_none() {
                            edit.params["depth"] = json!("10 mm");
                            expression(&mut edit.fields, "depth", "Depth");
                        }
                        edit.fields.push(field("through_all", "Through All", "choice", boolean_choices()));
                    }
                    _ => {}
                }
            }
            let bodies: Vec<_> = context.session.world_state().bodies.iter().map(|b| option(b.name.clone(), json!(b.name))).collect();
            for key in ["targets", "bodies", "tools"] {
                if edit.params.get(key).is_some() {
                    edit.fields.push(field(key, key, "references", bodies.clone()));
                }
            }
            for key in ["target"] {
                if edit.params.get(key).is_some() {
                    edit.fields.push(field(key, key, "reference", bodies.clone()));
                }
            }
            let before = context.session.world_state();
            if edit.params.get("profiles").is_some() && edit.params.get("sketch").is_some() {
                let mut options = Vec::new();
                for ss in &before.sketches {
                    for (index, _) in ss.profiles.iter().enumerate() {
                        let profiles = capture_profile_selection(ss, &[index]).map_err(|e| error("profile", e.to_string()))?;
                        options.push(option(format!("{} · region {}", ss.name, index + 1), json!({"sketch":ss.feature,"profiles":profiles})));
                    }
                }
                edit.params["profile"] = json!({"sketch":edit.params["sketch"],"profiles":edit.params["profiles"]});
                edit.fields.push(field("profile", "Reselect source profile", "reference", options));
            }
            if edit.params.get("edges").is_some() {
                let mut options = Vec::new();
                for body in before.bodies.iter().filter(|b| edit.params["body"].as_str().is_none_or(|name| b.name == name)) {
                    let mesh = body.mesh();
                    for (index, edge) in mesh.edges.iter().enumerate().filter(|(i, _)| mesh.edge_pickable(*i)).take(256) {
                        if let Some(point) = edge.get(edge.len() / 2) {
                            options.push(option(format!("{} · edge {}", body.name, index), json!(point)));
                        }
                    }
                }
                edit.fields.push(field("edges", "Reselect source edges", "references", options));
            }
            if edit.params.get("faces").is_some() {
                let mut options = Vec::new();
                for body in before.bodies.iter().filter(|b| edit.params["body"].as_str().is_none_or(|name| b.name == name)) {
                    for name in solidcraft_engine::doc::naming::face_names(body).iter().take(256) {
                        if let Ok(point) = solidcraft_engine::doc::naming::named_face_point(&before, Some(&body.name), name) {
                            options.push(option(format!("{} · {name}", body.name), json!(point)));
                        }
                    }
                }
                let offset = self.session.doc.feature(feature.unwrap_or_default()).is_some_and(|f| matches!(f.kind, FeatureKind::OffsetFace { .. }));
                edit.fields.push(field("faces", if offset { "ui.ipados.handles.faces" } else { "Reselect removal faces" }, "references", options));
                if offset {
                    edit.fields.push(field("body", "Target body", "reference", bodies.clone()));
                }
            }
            if edit.params.get("extent").is_some() {
                edit.fields.push(field("extent.direction", "Direction", "choice", choices(&["positive", "negative", "symmetric"])));
            }
            if edit.params.get("sections").is_some() {
                let mut options = Vec::new();
                for ss in &context.session.model.state().sketches {
                    for (index, _) in ss.profiles.iter().enumerate() {
                        let profiles = capture_profile_selection(ss, &[index]).map_err(|e| error("profile", e.to_string()))?;
                        options.push(option(format!("{} · region {}", ss.name, index + 1), json!({"sketch":ss.feature,"profiles":profiles})));
                    }
                }
                edit.fields.push(field("sections", "Ordered sections", "references", options));
            }
            if edit.params.get("operation").is_some() {
                edit.fields.push(field("operation", "Operation", "choice", choices(&["new_body", "join", "cut", "intersect"])));
            }
            if edit.params.get("keep_tools").is_some() {
                edit.fields.push(field(
                    "keep_tools",
                    "Retain tool bodies",
                    "choice",
                    vec![option("Retain", json!(true)), option("Consume", json!(false))],
                ));
            }
            if let Some(native) = self.session.doc.feature(feature.unwrap_or_default())
                && matches!(native.kind, FeatureKind::Mirror { .. })
            {
                let options = ["XY", "XZ", "YZ"].into_iter().map(|label| option(label, json!({"type":"origin","name":label}))).collect();
                edit.fields.push(field("plane", "Mirror plane", "choice", options));
            }
            if edit.fields.is_empty() {
                return Err(error(
                    "unsupported",
                    "This feature is preserved but has no native editable numeric inputs. Use Edit sketch for sketches.",
                ));
            }
        } else if crate::assembly::supported(&edit.operation) {
            context.assembly_form(&mut edit, selection)?;
        } else {
            context.create_model_form(&mut edit, selection)?;
        }
        if feature.is_some() && (self.session.model.results.iter().any(|r| r.error.is_some()) || self.validate_model_references().is_err()) {
            let mut source = context.snapshot()?;
            let model = Arc::get_mut(&mut source).ok_or_else(|| error("preview", "The repair source must be uniquely owned."))?;
            model.geometry_revision = self.geometry_revision;
            model.document_id = self.session.document_id.clone();
            model.revision = self.session.revision;
            edit.repair_source = Some(source);
        }
        edit.handle = if crate::assembly::supported(&edit.operation) { self.assembly_handle(&edit) } else { self.extrusion_handle(&edit) };
        Ok(edit)
    }

    fn extrusion_handle(&self, edit: &ModelEdit) -> Option<ModelHandle> {
        let offset = edit.operation == "solid.offset_face"
            || edit.feature.and_then(|id| self.session.doc.feature(id)).is_some_and(|f| matches!(f.kind, FeatureKind::OffsetFace { .. }));
        if offset {
            return self.offset_handle(edit);
        }
        let extrusion = edit.operation == "solid.extrude"
            || edit.feature.and_then(|id| self.session.doc.feature(id)).is_some_and(|f| matches!(f.kind, FeatureKind::Extrude { .. }));
        if !extrusion {
            return None;
        }
        let extent = if edit.feature.is_some() { &edit.params["extent"] } else { &edit.params };
        if extent["through_all"] == true || ["to", "from", "distance2", "start_offset"].iter().any(|key| extent.get(key).is_some()) {
            return None;
        }
        let sid = edit.params["profile"]["sketch"].as_u64().or_else(|| edit.params["sketch"].as_u64())?;
        let world = self.session.world_state();
        let ss = world.sketch(sid)?;
        let distance = edit.params["distance"].as_str().or_else(|| edit.params["extent"]["distance"].as_str())?;
        let distance = self.session.doc.eval(distance, solidcraft_engine::doc::expr::Kind::Length).ok()?;
        let direction = edit.params["direction"].as_str().or_else(|| edit.params["extent"]["direction"].as_str()).unwrap_or("positive");
        let factor = match direction {
            "negative" => -1.,
            "symmetric" if extent["convention"] == "legacy_per_side" => 1.,
            "symmetric" => 0.5,
            _ => 1.,
        };
        let value = edit.params["profile"].get("profiles").or_else(|| edit.params.get("profiles"))?;
        let selection = serde_json::from_value::<ProfileSel>(value.clone()).ok().or_else(|| {
            let loops = value
                .as_array()?
                .iter()
                .map(|v| v["curves"].as_array().and_then(|a| a.iter().map(|v| v.as_str().map(str::to_string)).collect::<Option<Vec<_>>>()))
                .collect::<Option<Vec<_>>>()?;
            Some(ProfileSel::Curves { loops })
        })?;
        let indices = solidcraft_engine::doc::profile_selection_indices(ss, &selection).ok()?;
        let centre = ss.profiles.get(*indices.first()?)?.region.interior_point();
        Some(ModelHandle {
            origin: ss.plane.to_world(centre),
            normal: ss.plane.normal(),
            distance,
            factor,
            parameter: if edit.feature.is_some() { "extent.distance".into() } else { "distance".into() },
            units: "mm".into(),
            angular: false,
            arm: ss.plane.x,
            size: 20.,
        })
    }

    fn offset_handle(&self, edit: &ModelEdit) -> Option<ModelHandle> {
        // Resolve against the actual source state, never the offset preview.
        let source = edit.feature.map_or_else(|| self.session.world_state(), |id| self.session.model.state_before(id));
        let points = edit.params["faces"].as_array()?;
        if points.len() != 1 {
            return None;
        }
        let point = serde_json::from_value::<solidcraft_engine::geom::Vec3>(points.first()?.clone()).ok()?;
        let mut bodies = source.bodies.iter().filter(|body| {
            edit.params["body"].as_str().is_none_or(|name| body.name == name)
                && solidcraft_engine::doc::appearance::face_index_at(body, point).is_some()
        });
        let body = bodies.next()?;
        if bodies.next().is_some() {
            return None;
        }
        let index = solidcraft_engine::doc::appearance::face_index_at(body, point)?;
        let spot = solidcraft_engine::doc::naming::face_spot(body, index)?;
        let solidcraft_engine::kernel::FaceSurface::Plane { normal, .. } = spot.surface? else { return None };
        let distance = self.session.doc.eval(edit.params["distance"].as_str()?, solidcraft_engine::doc::expr::Kind::Length).ok()?;
        Some(ModelHandle {
            origin: spot.point,
            normal: normal.vec(),
            distance,
            factor: 1.,
            parameter: "distance".into(),
            units: "mm".into(),
            angular: false,
            arm: solidcraft_engine::geom::Vec3::X,
            size: 20.,
        })
    }

    pub fn refresh_model(&self, edit: &ModelEdit, selection: &[Sel], params: Option<Value>) -> Result<ModelEdit, AdapterError> {
        self.validate_model(edit)?;
        let mut refreshed = if edit.feature.is_some() || crate::assembly::supported(&edit.operation) {
            edit.clone()
        } else {
            self.begin_model(edit.token, Some(&edit.operation), None, selection)?
        };
        let canvas_profile = if params.is_none()
            && edit.feature.is_none()
            && matches!(edit.operation.as_str(), "solid.extrude" | "solid.revolve" | "solid.sweep")
            && selection.iter().any(|s| matches!(s, Sel::Profile { .. }))
        {
            refreshed.params.get("profile").cloned()
        } else {
            None
        };
        let canvas_faces = (params.is_none()
            && edit.feature.is_none()
            && edit.operation == "solid.offset_face"
            && selection.iter().any(|selection| matches!(selection, Sel::Face { .. })))
        .then(|| (refreshed.params["faces"].clone(), refreshed.params["body"].clone()));
        refreshed.params = params.unwrap_or_else(|| edit.params.clone());
        if let Some((faces, body)) = canvas_faces {
            refreshed.params["faces"] = faces;
            refreshed.params["body"] = body;
        }
        if let Some(profile) = canvas_profile {
            refreshed.params["profile"] = profile;
        }
        if crate::assembly::supported(&edit.operation) {
            self.assembly_context_change(&edit.params, &mut refreshed.params);
            let submitted = refreshed.params.clone();
            refreshed.fields.clear();
            refreshed.feedback.clear();
            self.assembly_form(&mut refreshed, selection)?;
            if let (Some(defaults), Some(submitted)) = (refreshed.params.as_object_mut(), submitted.as_object()) {
                defaults.extend(submitted.clone());
            }
        }
        refreshed.preview = if refreshed.params == edit.params { edit.preview.clone() } else { None };
        refreshed.last_preview = if refreshed.params == edit.params { edit.last_preview.clone() } else { None };
        if refreshed.params == edit.params && edit.preview.is_some() {
            refreshed.reply = edit.reply.clone();
            refreshed.feedback = edit.feedback.clone();
        }
        if refreshed.params != edit.params || refreshed.fields != edit.fields {
            refreshed.version = edit.version.checked_add(1).ok_or_else(|| error("revision_limit", "Interaction version exhausted."))?;
            refreshed.preview = None;
            refreshed.last_preview = None;
        }
        refreshed.handle =
            if crate::assembly::supported(&edit.operation) { self.assembly_handle(&refreshed) } else { self.extrusion_handle(&refreshed) };
        Ok(refreshed)
    }

    fn create_model_form(&self, edit: &mut ModelEdit, selection: &[Sel]) -> Result<(), AdapterError> {
        if matches!(edit.operation.as_str(), "solid.fillet" | "solid.chamfer" | "solid.shell") {
            let bodies: std::collections::BTreeSet<_> = selection
                .iter()
                .filter_map(|s| match s {
                    Sel::Edge { body, .. } | Sel::Face { body, .. } => Some(body),
                    _ => None,
                })
                .collect();
            if bodies.len() > 1 {
                return Err(error("selection", "Select edges or removal faces from one body at a time."));
            }
        }
        let state = self.session.model.state();
        let bodies: Vec<_> = self.session.world_state().bodies.iter().map(|b| option(b.name.clone(), json!(b.name))).collect();
        let sketches: Vec<_> = self
            .session
            .doc
            .features
            .iter()
            .filter(|f| matches!(f.kind, FeatureKind::Sketch { .. }))
            .map(|f| option(f.name.clone(), json!(f.id)))
            .collect();
        let selected_sketch = selection.iter().find_map(|s| match s {
            Sel::Feature { id } if state.sketch(*id).is_some() => Some(*id),
            Sel::Profile { sketch, .. } => Some(*sketch),
            _ => None,
        });
        let selected_profiles: Vec<_> = selection
            .iter()
            .filter_map(|s| match s {
                Sel::Profile { sketch, index } if Some(*sketch) == selected_sketch => Some(*index),
                _ => None,
            })
            .collect();
        let selected_profile = if selected_profiles.is_empty() {
            None
        } else {
            let sketch =
                selected_sketch.and_then(|id| state.sketch(id)).ok_or_else(|| error("stale_selection", "ui.ipados.workspace.invalidated"))?;
            Some(profile_reference(sketch, &selected_profiles)?)
        };
        let selected_bodies: Vec<_> = selection
            .iter()
            .filter_map(|s| match s {
                Sel::Body { name } => Some(json!(name)),
                _ => None,
            })
            .collect();
        let selected_features: Vec<_> = selection
            .iter()
            .filter_map(|selection| match selection {
                Sel::Feature { id } => self.session.doc.feature(*id).filter(|feature| replayable(&feature.kind)).map(|feature| json!(feature.name)),
                _ => None,
            })
            .collect();
        let feature_sources = self.replay_source_options(false);
        let (path_sketches, path_curves) = self.model_path_options();
        let mut regions = Vec::new();
        let mut sections = Vec::new();
        for ss in &state.sketches {
            for (index, p) in ss.profiles.iter().enumerate() {
                let value = profile_reference(ss, &[index])?;
                let label = format!("{} · region {} · {:.3} mm²", ss.name, index + 1, p.area);
                sections.push(option(label.clone(), value.clone()));
                if Some(ss.feature) == selected_sketch || selected_sketch.is_none() {
                    regions.push(option(label, value));
                }
            }
        }
        let edges: Vec<_> = selection
            .iter()
            .filter_map(|s| match s {
                Sel::Edge { body, index, .. } => Some(option(format!("{body} · edge {index}"), json!({"body":body,"index":index}))),
                _ => None,
            })
            .collect();
        let faces: Vec<_> = selection
            .iter()
            .filter_map(|s| match s {
                Sel::Face { body, index, point } => Some(option(format!("{body} · face {index}"), json!(point))),
                _ => None,
            })
            .collect();
        let selected_body = selection.iter().find_map(|s| match s {
            Sel::Body { name } | Sel::Face { body: name, .. } | Sel::Edge { body: name, .. } => Some(name.clone()),
            _ => None,
        });
        let f = &mut edit.fields;
        let p = &mut edit.params;
        match edit.operation.as_str() {
            "solid.offset_face" => {
                p["faces"] = json!(faces.iter().map(|face| face["value"].clone()).collect::<Vec<_>>());
                p["distance"] = json!("1 mm");
                if let Some(body) = selected_body {
                    p["body"] = json!(body);
                }
                f.push(field("faces", "ui.ipados.handles.faces", "references", faces));
                f.push(field("body", "Target body", "reference", bodies.clone()));
                expression(f, "distance", "Distance");
            }
            "solid.extrude" | "solid.revolve" | "solid.sweep" => {
                // This composite presentation input is expanded immediately before
                // calling the existing constructor; its profiles are native identities.
                if let Some(profile) = selected_profile {
                    if !regions.iter().any(|region| region["value"] == profile) {
                        let label = selected_sketch.and_then(|id| state.sketch(id)).map(|sketch| sketch.name.clone()).unwrap_or_default();
                        regions.push(option(label, profile.clone()));
                    }
                    p["profile"] = profile;
                } else if selected_sketch.is_some() && regions.len() == 1 {
                    p["profile"] = regions.first().map(|region| region["value"].clone()).unwrap_or(Value::Null);
                }
                f.push(field("profile", "ui.ipados.modelling.profile", "reference", regions));
                if edit.operation == "solid.extrude" {
                    p["distance"] = json!("10 mm");
                    p["direction"] = json!("positive");
                    p["through_all"] = json!(false);
                    expression(f, "distance", "Distance");
                    f.push(field("direction", "Direction", "choice", choices(&["positive", "negative", "symmetric"])));
                    f.push(field("through_all", "Through All", "choice", boolean_choices()));
                }
                if edit.operation == "solid.revolve" {
                    p["angle"] = json!("360 deg");
                    p["axis"] = json!("Z");
                    expression(f, "angle", "Angle");
                    f.push(field("axis", "Axis", "reference", choices(&["X", "Y", "Z", "x", "y"])));
                }
                if edit.operation == "solid.sweep" {
                    path_fields(f, path_sketches.clone(), path_curves.clone(), "");
                    p["path_curves"] = json!([]);
                    p["twist"] = json!("0 deg");
                    p["scale"] = json!("1");
                    expression(f, "twist", "Twist");
                    expression(f, "scale", "End scale");
                }
            }
            "solid.loft" => {
                f.push(field("sections", "Ordered sections", "references", sections));
                p["sections"] = json!([]);
            }
            "solid.fillet" | "solid.chamfer" => {
                f.push(field("edges", "Edges · select with Pencil before starting", "references", edges.clone()));
                p["edges"] = json!(edges.iter().map(|o| o["value"].clone()).collect::<Vec<_>>());
                if let Some(body) = selected_body.clone() {
                    p["body"] = json!(body);
                }
                let key = if edit.operation == "solid.fillet" { "radius" } else { "distance" };
                p[key] = json!("1 mm");
                expression(f, key, key);
                if edit.operation == "solid.fillet" {
                    p["fit"] = json!("exact");
                    p["type"] = json!("constant");
                    f.push(field("type", "Fillet mode", "choice", choices(&["constant", "chord", "curvature"])));
                } else {
                    p["chamfer_mode"] = json!("equal");
                    p["distance2"] = json!("1 mm");
                    p["angle"] = json!("45 deg");
                    f.push(field("chamfer_mode", "Chamfer mode", "choice", choices(&["equal", "two_distances", "angle"])));
                    expression(f, "distance2", "Second distance");
                    expression(f, "angle", "Angle");
                }
            }
            "solid.shell" => {
                f.push(field("faces", "Removal faces · select with Pencil before starting", "references", faces.clone()));
                p["faces"] = json!(faces.iter().map(|o| o["value"].clone()).collect::<Vec<_>>());
                p["thickness"] = json!("1 mm");
                expression(f, "thickness", "Wall thickness");
                p["direction"] = json!("inside");
                f.push(field("direction", "Direction", "choice", choices(&["inside", "outside", "both"])));
                if let Some(body) = selected_body.clone() {
                    p["body"] = json!(body);
                }
            }
            "solid.hole" => {
                f.push(field("sketch", "Placement sketch · circle centres or points", "reference", sketches.clone()));
                if let Some(id) = selected_sketch {
                    p["sketch"] = json!(id);
                }
                p["diameter"] = json!("5 mm");
                p["depth"] = json!("10 mm");
                p["through_all"] = json!(false);
                expression(f, "diameter", "Diameter");
                expression(f, "depth", "Depth");
                f.push(field("through_all", "Through All", "choice", boolean_choices()));
                p["type"] = json!("simple");
            }
            "solid.combine" => {
                f.push(field("target", "Target body", "reference", bodies.clone()));
                f.push(field("tools", "Tool bodies", "references", bodies.clone()));
                p["tools"] = json!([]);
                p["keep_tools"] = json!(true);
                f.push(field("keep_tools", "Retain tool bodies", "choice", vec![option("Retain", json!(true)), option("Consume", json!(false))]));
                p["operation"] = json!("join");
            }
            "solid.pattern.rectangular" | "solid.pattern.circular" | "solid.pattern.path" | "solid.mirror" => {
                p["source_mode"] = json!(if selected_features.is_empty() { "bodies" } else { "features" });
                p["bodies"] = json!(selected_bodies);
                p["features"] = json!(selected_features);
                f.push(field("source_mode", "Source mode", "choice", choices(&["bodies", "features"])));
                f.push(field("bodies", "Source bodies (body pattern)", "references", bodies.clone()));
                f.push(field("features", "Source features (replayed)", "references", feature_sources));
                if edit.operation == "solid.pattern.rectangular" {
                    p["dir1"] = json!([1, 0, 0]);
                    p["count1"] = json!("2");
                    p["spacing1"] = json!("20 mm");
                    expression(f, "count1", "Count");
                    expression(f, "spacing1", "Spacing");
                    f.push(field(
                        "dir1",
                        "Direction",
                        "choice",
                        vec![option("X", json!([1, 0, 0])), option("Y", json!([0, 1, 0])), option("Z", json!([0, 0, 1]))],
                    ));
                }
                if edit.operation == "solid.pattern.circular" {
                    p["count"] = json!("3");
                    p["angle"] = json!("360 deg");
                    p["axis"] = json!("Z");
                    expression(f, "count", "Count");
                    expression(f, "angle", "Angle");
                    f.push(field("axis", "Axis", "choice", choices(&["X", "Y", "Z"])));
                }
                if edit.operation == "solid.pattern.path" {
                    path_fields(f, path_sketches, path_curves, "");
                    p["path_curves"] = json!([]);
                    p["count"] = json!("3");
                    p["spacing"] = json!("20 mm");
                    p["orientation"] = json!("identical");
                    expression(f, "count", "Count");
                    expression(f, "spacing", "Spacing");
                    f.push(field("orientation", "Orientation", "choice", choices(&["identical", "path"])));
                }
                if edit.operation == "solid.mirror" {
                    p["plane"] = json!("YZ");
                    f.push(field("plane", "Mirror plane", "choice", choices(&["XY", "XZ", "YZ"])));
                }
            }
            "parameters.change" => {
                let rows = self.session.doc.param_rows();
                f.push(field(
                    "name",
                    "Parameter",
                    "reference",
                    rows.iter().map(|r| option(format!("{} = {}", r.name, r.expression), json!(r.name))).collect(),
                ));
                if let Some(row) = rows.first() {
                    p["name"] = json!(row.name);
                    p["expression"] = json!(row.expression);
                }
                expression(f, "expression", "Expression");
            }
            "parameters.add" => {
                p["name"] = json!("length");
                p["expression"] = json!("10 mm");
                p["unit"] = json!("mm");
                f.push(field("name", "Name", "text", Vec::new()));
                expression(f, "expression", "Expression");
                f.push(field("unit", "Units", "choice", choices(&["mm", "deg", ""])));
            }
            _ => return Err(error("unsupported", "No native form for this operation.")),
        }
        if matches!(edit.operation.as_str(), "solid.extrude" | "solid.revolve" | "solid.sweep" | "solid.loft" | "solid.combine") {
            if p.get("operation").is_none() {
                p["operation"] = json!("new");
            }
            f.push(field(
                "operation",
                "Operation",
                "choice",
                choices(if edit.operation == "solid.combine" { &["join", "cut", "intersect"] } else { &["new", "join", "cut", "intersect"] }),
            ));
        }
        if matches!(edit.operation.as_str(), "solid.extrude" | "solid.revolve" | "solid.sweep" | "solid.loft" | "solid.hole") {
            p["targets"] = json!([]);
            f.push(field("targets", "Target bodies · required for join, cut or intersect", "references", bodies));
        }
        Ok(())
    }

    pub fn model_descriptor(&self, edit: &ModelEdit) -> Value {
        json!({"token":edit.token,"operation":edit.operation,"feature":edit.feature,"params":edit.params,"fields":edit.fields,"valid":edit.preview.is_some(),"handle":edit.handle,"reply":edit.reply,"feedback":edit.feedback})
    }

    fn validate_model(&self, edit: &ModelEdit) -> Result<(), AdapterError> {
        if edit.document_id != self.session.document_id || edit.geometry_revision != self.geometry_revision {
            return Err(error("stale_edit", "The model changed. Restart the operation and reselect its inputs."));
        }
        Ok(())
    }

    pub fn prepare_model(&self, edit: &ModelEdit, params: Value, commit: bool) -> Result<PreparedModel, AdapterError> {
        self.validate_model(edit)?;
        if crate::assembly::supported(&edit.operation) {
            return self.prepare_assembly(edit, params, commit);
        }
        if !params.is_object() || params.to_string().len() > 65536 {
            return Err(error("bad_params", "Invalid modelling form."));
        }
        let mut p = params.clone();
        let target_feature = edit.feature.and_then(|id| self.session.doc.feature(id));
        if edit.operation == "solid.offset_face" || target_feature.is_some_and(|feature| matches!(feature.kind, FeatureKind::OffsetFace { .. })) {
            let source = edit.feature.map_or_else(|| self.session.world_state(), |id| self.session.model.state_before(id));
            let faces = p["faces"]
                .as_array()
                .filter(|faces| !faces.is_empty() && faces.len() <= 256)
                .ok_or_else(|| error("selection", "ui.ipados.handles.choose_faces"))?;
            let points = faces
                .iter()
                .map(|point| {
                    serde_json::from_value::<solidcraft_engine::geom::Vec3>(point.clone())
                        .ok()
                        .filter(|point| point.is_finite())
                        .ok_or_else(|| error("selection", "ui.ipados.handles.choose_faces"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            let mut bodies = source.bodies.iter().filter(|body| {
                p["body"].as_str().is_none_or(|name| body.name == name)
                    && points.iter().all(|point| solidcraft_engine::doc::appearance::face_index_at(body, *point).is_some())
            });
            if bodies.next().is_none() || bodies.next().is_some() {
                return Err(error("selection", "ui.ipados.handles.choose_faces"));
            }
        }
        let sources = target_feature.is_some_and(|feature| matches!(feature.kind, FeatureKind::Pattern { .. } | FeatureKind::Mirror { .. }))
            || matches!(edit.operation.as_str(), "solid.pattern.rectangular" | "solid.pattern.circular" | "solid.pattern.path" | "solid.mirror");
        if sources {
            let mode = p.as_object_mut().and_then(|fields| fields.remove("source_mode"));
            let mode = mode
                .as_ref()
                .and_then(Value::as_str)
                .unwrap_or_else(|| if p["bodies"].as_array().is_some_and(|bodies| !bodies.is_empty()) { "bodies" } else { "features" });
            let selected = p
                .get(mode)
                .and_then(Value::as_array)
                .filter(|selected| !selected.is_empty() && selected.len() <= 1000)
                .ok_or_else(|| error("selection", "Choose at least one source in the selected body or feature mode."))?;
            if !matches!(mode, "bodies" | "features") {
                return Err(error("selection", "Choose body or feature sources."));
            }
            if mode == "features" {
                let limit = edit
                    .feature
                    .and_then(|id| self.session.doc.feature_index(id))
                    .unwrap_or(self.session.doc.marker.unwrap_or(self.session.doc.features.len()));
                let mut features = Vec::with_capacity(selected.len());
                for selected in selected {
                    let feature = if edit.feature.is_some() {
                        selected
                            .as_u64()
                            .and_then(|id| self.session.doc.feature(id))
                            .or_else(|| selected.as_str().and_then(|name| self.session.doc.seed_feature(name)))
                    } else {
                        selected.as_str().and_then(|name| self.session.doc.seed_feature(name))
                    }
                    .filter(|feature| {
                        replayable(&feature.kind)
                            && !feature.suppressed
                            && self.session.doc.feature_index(feature.id).is_some_and(|index| index < limit)
                    })
                    .ok_or_else(|| error("selection", "A selected source feature is unavailable in this feature's upstream history."))?;
                    features.push(if edit.feature.is_some() { json!(feature.id) } else { json!(feature.name) });
                }
                p["features"] = json!(features);
            }
            let inactive = if mode == "features" { "bodies" } else { "features" };
            if edit.feature.is_some() {
                p[inactive] = json!([]);
            } else if let Some(fields) = p.as_object_mut() {
                fields.remove(inactive);
            }
        }
        if let Some(mode) = p.as_object_mut().and_then(|m| m.remove("chamfer_mode")) {
            let fields = p.as_object_mut().ok_or_else(|| error("bad_params", "Invalid chamfer parameters."))?;
            if mode != "two_distances" {
                fields.remove("distance2");
            }
            if mode != "angle" {
                fields.remove("angle");
            }
        }
        if let Some(profile) = p.as_object_mut().and_then(|m| m.remove("profile")) {
            p["sketch"] = profile["sketch"].clone();
            p["profiles"] = profile["profiles"].clone();
        }
        let combine = target_feature.is_some_and(|f| matches!(f.kind, FeatureKind::Combine { .. })) || edit.operation == "solid.combine";
        let hole = target_feature.is_some_and(|f| matches!(f.kind, FeatureKind::Hole { .. })) || edit.operation == "solid.hole";
        if hole && let Some(through_all) = p.as_object_mut().and_then(|fields| fields.remove("through_all")) {
            let through_all = through_all.as_bool().ok_or_else(|| error("bad_params", "Through All must be a Boolean choice."))?;
            if through_all {
                if edit.feature.is_some() {
                    p["depth"] = Value::Null;
                } else if let Some(fields) = p.as_object_mut() {
                    fields.remove("depth");
                }
            } else if p.get("depth").is_none_or(Value::is_null) {
                return Err(error("bad_params", "Enter a hole depth or choose Through All."));
            }
        }
        let path_pattern = target_feature
            .is_some_and(|feature| matches!(feature.kind, FeatureKind::Pattern { pattern: solidcraft_engine::doc::PatternKind::Path { .. }, .. }));
        if path_pattern && p.get("pattern").is_none_or(|pattern| !pattern.is_object()) {
            return Err(error("bad_params", "A path pattern requires its native pattern definition."));
        }
        if let Some(curves) = p.as_object_mut().and_then(|fields| fields.remove("path_curves")) {
            let path_sketch = if path_pattern { p["pattern"]["path_sketch"].clone() } else { p["path_sketch"].clone() };
            let list = curves
                .as_array()
                .filter(|list| !list.is_empty() && list.len() <= 1000)
                .ok_or_else(|| error("selection", "Choose an ordered path of 1 to 1000 curves."))?;
            if list.iter().any(|curve| curve["sketch"] != path_sketch || curve["curve"].as_str().is_none()) {
                return Err(error("selection", "Path curves must belong to the selected path sketch."));
            }
            let path = json!(list.iter().map(|curve| curve["curve"].clone()).collect::<Vec<_>>());
            if path_pattern {
                p["pattern"]["path"] = path;
            } else {
                p["path"] = path;
            }
        }
        if path_pattern && let Some(orientation) = p.as_object_mut().and_then(|fields| fields.remove("orientation")) {
            p["pattern"]["orient"] = match orientation.as_str() {
                Some("identical") => json!(false),
                Some("path") => json!(true),
                _ => return Err(error("bad_params", "Choose identical or path orientation.")),
            };
        }
        if !combine
            && (p["operation"].as_str().is_some_and(|o| matches!(o, "join" | "cut" | "intersect")) || hole)
            && p["targets"].as_array().is_none_or(|t| t.is_empty())
        {
            return Err(error("selection", "Choose explicit target bodies for this operation."));
        }
        let (command, p) = if let Some(feature) = edit.feature {
            if hole && let Some(targets) = p.as_object_mut().and_then(|m| m.remove("targets")) {
                p["participants"] = targets;
            }
            ("timeline.edit", json!({"feature":feature,"set":p}))
        } else {
            if let Some(profile) = p.as_object_mut().and_then(|m| m.remove("profile")) {
                p["sketch"] = profile["sketch"].clone();
                p["profiles"] = profile["profiles"].clone();
            }
            if let Some(curves) = p.as_object_mut().and_then(|m| m.remove("path_curves")) {
                let list = curves.as_array().ok_or_else(|| error("selection", "Choose an ordered path."))?;
                if list.iter().any(|c| c["sketch"] != p["path_sketch"]) {
                    return Err(error("selection", "Path curves must belong to the selected path sketch."));
                }
                p["path"] = json!(list.iter().map(|c| c["curve"].clone()).collect::<Vec<_>>());
            }
            if p.get("operation").and_then(Value::as_str).is_some_and(|o| matches!(o, "join" | "cut" | "intersect"))
                && edit.operation != "solid.combine"
                && p["targets"].as_array().is_none_or(|t| t.is_empty())
            {
                return Err(error("selection", "Choose explicit target bodies for this operation."));
            }
            if edit.operation == "solid.hole" && p["targets"].as_array().is_none_or(|t| t.is_empty()) {
                return Err(error("selection", "Choose the bodies to drill."));
            }
            (edit.operation.as_str(), p)
        };
        // Reject references that no longer resolve exactly before the evaluator's
        // optional lenient recovery can choose a different profile.
        let mut scratch = Self { session: self.session.scratch(), geometry_revision: self.geometry_revision, workspace_cache: None };
        scratch.execute(command, p.clone())?;
        if let Some(r) = scratch.session.model.results.iter().find(|r| r.warning.is_some()) {
            return Err(error("evaluation_warning", format!("{}: {}", r.name, r.warning.as_deref().unwrap_or_default())));
        }
        scratch.validate_model_references()?;
        scratch.validate_joint_references()?;
        let model = scratch.snapshot()?;
        let transaction = if commit { Some(self.session.scratch().compute_commands(&[(command.into(), p)], None)?) } else { None };
        let mut updated = edit.clone();
        updated.params = params.clone();
        let offset =
            edit.operation == "solid.offset_face" || target_feature.is_some_and(|feature| matches!(feature.kind, FeatureKind::OffsetFace { .. }));
        let handle = if offset { self.extrusion_handle(&updated) } else { scratch.extrusion_handle(&updated) };
        Ok(PreparedModel { model, params, version: edit.version, handle, transaction, reply: Value::Null, feedback: Vec::new() })
    }

    pub fn accept_model(&mut self, edit: &ModelEdit, mut prepared: PreparedModel) -> Result<Arc<SceneSnapshot>, AdapterError> {
        self.validate_model(edit)?;
        let transaction = prepared.transaction.take().ok_or_else(|| error("preview", "A preview cannot be committed."))?;
        let revision = self.geometry_revision.checked_add(1).ok_or_else(|| error("revision_limit", "Revision exhausted."))?;
        let model = Arc::get_mut(&mut prepared.model).ok_or_else(|| error("preview", "Prepared scene must be uniquely owned."))?;
        self.session.accept_commands(transaction)?;
        self.geometry_revision = revision;
        model.geometry_revision = revision;
        model.revision = self.session.revision;
        Ok(prepared.model)
    }
}

impl Viewport {
    pub fn update_model(&mut self, edit: ModelEdit) -> Result<(), AdapterError> {
        let current = self.model_edit(edit.token)?;
        if edit.params != current.params || edit.version != current.version {
            self.cancel_interactions();
        }
        self.modelling = Some(edit);
        self.changed();
        Ok(())
    }
    pub fn model_active(&self) -> bool {
        self.modelling.is_some()
    }
    pub fn model_edit(&self, token: u64) -> Result<ModelEdit, AdapterError> {
        self.modelling
            .as_ref()
            .filter(|e| {
                e.token == token && self.model.as_ref().is_some_and(|m| m.document_id == e.document_id && m.geometry_revision == e.geometry_revision)
            })
            .cloned()
            .ok_or_else(|| error("stale_edit", "The modelling interaction was cancelled or the document changed."))
    }
    pub fn begin_model(&mut self, edit: ModelEdit) -> Result<(), AdapterError> {
        if self.numeric.is_some() || self.modelling.is_some() || (self.updating && edit.repair_source.is_none()) {
            return Err(error("disabled", "Finish the current operation."));
        }
        self.cancel_interactions();
        if let Some(source) = &edit.repair_source {
            self.model = Some(source.clone());
            self.selection.clear();
            self.updating = false;
        }
        self.modelling = Some(edit);
        self.changed();
        Ok(())
    }
    pub fn publish_model(&mut self, token: u64, prepared: &PreparedModel) -> Result<(), AdapterError> {
        let edit = self.model_edit(token)?;
        if edit.version != prepared.version {
            return Err(error("stale_preview", "A newer modelling proposal replaced this result."));
        }
        if let Some(e) = &mut self.modelling {
            e.preview = Some(prepared.model.clone());
            e.last_preview = None;
            e.params = prepared.params.clone();
            e.handle = prepared.handle.clone();
            e.reply = prepared.reply.clone();
            e.feedback = prepared.feedback.clone();
        }
        self.changed();
        Ok(())
    }
    pub fn fail_model(&mut self, token: u64) -> Result<(), AdapterError> {
        self.model_edit(token)?;
        self.cancel_interactions();
        if let Some(e) = &mut self.modelling {
            e.preview = None;
            e.last_preview = None;
        }
        self.changed();
        Ok(())
    }
    /// Invalidate confirmation before a numeric/slider evaluation while retaining
    /// its last validated display. Selection reveal uses fail_model instead.
    pub fn pend_model(&mut self, token: u64) -> Result<(), AdapterError> {
        self.model_edit(token)?;
        self.cancel_interactions();
        if let Some(edit) = &mut self.modelling
            && let Some(previous) = edit.preview.take()
        {
            edit.last_preview = Some(previous);
        }
        self.changed();
        Ok(())
    }
    pub fn finish_model(&mut self, token: u64, model: Option<Arc<SceneSnapshot>>) -> Result<(), AdapterError> {
        let edit = self.model_edit(token)?;
        self.modelling = None;
        self.cancel_interactions();
        if model.is_none() && edit.repair_source.is_some() {
            self.updating = true;
            self.selection.clear();
        }
        if let Some(model) = model {
            self.model = Some(model);
            self.selection.clear();
            self.hover = None;
        }
        self.changed();
        Ok(())
    }
    pub fn cancel_model(&mut self) {
        if self.modelling.as_ref().is_some_and(|e| e.repair_source.is_some()) {
            self.updating = true;
            self.selection.clear();
        }
        self.modelling = None;
        self.cancel_interactions();
    }
}
