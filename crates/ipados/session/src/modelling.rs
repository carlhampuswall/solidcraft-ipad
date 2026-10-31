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
    pub(crate) selection_field: Option<String>,
    pub(crate) document_id: String,
    pub(crate) geometry_revision: u64,
    pub(crate) fields: Vec<Value>,
    pub(crate) preview: Option<Arc<SceneSnapshot>>,
    /// Display-only last validated pose while a newer Pencil proposal is pending.
    /// This never validates the current parameters or enables confirmation.
    pub(crate) last_preview: Option<Arc<SceneSnapshot>>,
    pub(crate) handle: Option<ModelHandle>,
    pub(crate) repair_source: Option<Arc<SceneSnapshot>>,
    pub(crate) repair_original: Option<Arc<SceneSnapshot>>,
    pub reply: Value,
    pub feedback: Vec<String>,
}

impl ModelEdit {
    pub(crate) fn body_pick_field(&self) -> Option<&str> {
        self.selection_field
            .as_deref()
            .filter(|key| self.fields.iter().any(|field| field["key"] == *key && matches!(field["kind"].as_str(), Some("body_pick" | "bodies_pick"))))
    }

    fn has_body_picks(&self) -> bool {
        self.fields.iter().any(|field| field["kind"] == "body_pick")
    }

    fn body_pick_selection(&self, field: &str) -> Vec<Sel> {
        // Keep the target visibly selected while adding tools. It also makes
        // the handoff observable through the host's ordinary selection reports.
        let mut names = vec![self.params["target"].clone()];
        if field == "tools" {
            names.extend(self.params["tools"].as_array().into_iter().flatten().cloned());
        }
        let mut selected = Vec::new();
        for name in names.iter().filter_map(Value::as_str) {
            let body = Sel::Body { name: name.into() };
            if !selected.contains(&body) {
                selected.push(body);
            }
        }
        selected
    }

    pub(crate) fn pick_body_target(&mut self, name: &str) -> Vec<Sel> {
        self.params["target"] = json!(name);
        if let Some(tools) = self.params["tools"].as_array_mut() {
            tools.retain(|tool| tool != name);
        }
        // Switch on the viewport lane: a second tap may precede host inspection.
        self.selection_field = Some("tools".into());
        self.version = self.version.saturating_add(1);
        self.preview = None;
        self.last_preview = None;
        self.body_pick_selection("tools")
    }
}

#[derive(Clone, PartialEq, Serialize)]
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

fn extrusion_operations(editing: bool) -> Vec<Value> {
    [
        ("ui.ipados.face_extrude.new_body", if editing { "new_body" } else { "new" }),
        ("ui.ipados.face_extrude.union", "join"),
        ("ui.ipados.face_extrude.intersect", "intersect"),
        ("ui.ipados.face_extrude.subtract", "cut"),
    ]
    .into_iter()
    .map(|(label, value)| option(label, json!(value)))
    .collect()
}

fn update_face_extrusion_target(params: &mut Value, previous: Option<&Value>) {
    let source = params.get("profile").unwrap_or(params);
    let Some(body) = source["face_body"].as_str() else { return };
    let previous_body = previous.and_then(|params| params.get("profile").unwrap_or(params)["face_body"].as_str());
    let targets = &params["targets"];
    if targets.as_array().is_none_or(|targets| targets.is_empty())
        || previous_body.is_some_and(|previous| previous != body && targets == &json!([previous]))
    {
        let targets = json!([body]);
        params["targets"] = targets;
    }
}

// Compare submitted targets with the current published form before inference.
// Unchanged targets may be only the intersecting subset of a wider cached scope.
fn invalidate_automatic_subtract_scope(previous: &Value, params: &mut Value) {
    if params["targets"] != previous["targets"]
        && let Some(fields) = params.as_object_mut()
    {
        fields.remove("_auto_subtract_scope");
    }
}

fn expression(fields: &mut Vec<Value>, key: &str, label: &str) {
    fields.push(field(key, label, "expression", Vec::new()));
}

// Native length fields publish the engine's default unit for insertion controls;
// the host never has to infer a dimension from its label or expression text.
fn length_expression(fields: &mut Vec<Value>, key: &str, label: &str) {
    let mut descriptor = field(key, label, "expression", Vec::new());
    descriptor["units"] = json!(solidcraft_engine::doc::expr::Kind::Length.default_unit());
    fields.push(descriptor);
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

fn extrusion_face_reference(selection: &[Sel]) -> Result<Option<Value>, AdapterError> {
    let mut body = None;
    let mut points = Vec::new();
    for selected in selection {
        if let Sel::Face { body: name, point, .. } = selected {
            if body.is_some_and(|body| body != name) {
                return Err(error("selection", "ui.ipados.face_extrude.same_body"));
            }
            if !point.is_finite() || points.len() >= 256 {
                return Err(error("selection", "ui.ipados.face_extrude.choose_source"));
            }
            body = Some(name);
            points.push(*point);
        }
    }
    Ok(body.map(|body| json!({"faces":points,"face_body":body})))
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
    // Runs on the engine worker. Only native world-space meshes and profile
    // regions participate; the authoritative document is never modified here.
    fn automatic_subtract(&self, edit: &ModelEdit, params: &mut Value) {
        if edit.operation != "solid.extrude"
            || edit.feature.is_some()
            || params["_auto_subtract"] != true
            || !matches!(params["operation"].as_str(), Some("new" | "cut"))
            || params["direction"] == "symmetric"
        {
            return;
        }
        let mut proposal = edit.clone();
        proposal.params = params.clone();
        let Some(handle) = self.extrusion_handle(&proposal) else { return };
        let length = (handle.distance * handle.factor).abs();
        if !length.is_finite() || length < 1e-6 {
            return;
        }
        let direction = handle.normal * (handle.distance * handle.factor).signum();
        let world = self.session.world_state();
        let mut origins = vec![handle.origin];
        let source = params.get("profile").unwrap_or(params);
        if let Some(sketch) = source["sketch"].as_u64().and_then(|id| world.sketch(id)) {
            let value = &source["profiles"];
            let selection = serde_json::from_value::<ProfileSel>(value.clone()).ok().or_else(|| {
                let loops = value.as_array()?.iter().map(|value| serde_json::from_value(value["curves"].clone()).ok()).collect::<Option<Vec<_>>>()?;
                Some(ProfileSel::Curves { loops })
            });
            if let Some(indices) = selection.and_then(|selection| solidcraft_engine::doc::profile_selection_indices(sketch, &selection).ok()) {
                for profile in indices.iter().take(256).filter_map(|index| sketch.profiles.get(*index)) {
                    origins.push(sketch.plane.to_world(profile.region.interior_point()));
                    for triangle in profile.region.triangulate(1e-2).into_iter().take(256_usize.saturating_sub(origins.len())) {
                        origins.push(sketch.plane.to_world((triangle[0] + triangle[1] + triangle[2]) / 3.));
                    }
                }
            }
        }
        let scope = params.get("_auto_subtract_scope").unwrap_or(&params["targets"]).clone();
        let explicit = scope.as_array().filter(|names| !names.is_empty());
        let epsilon = length.min(1e-3) * 0.1;
        let targets: Vec<_> = world
            .bodies
            .iter()
            .filter(|body| {
                self.session.body_visible(&body.name) && explicit.is_none_or(|names| names.iter().any(|name| name.as_str() == Some(&body.name)))
            })
            .filter(|body| {
                let mesh = body.mesh();
                let inside = mesh.inside_index();
                let index = solidcraft_engine::geom::MeshIndex::new(&mesh);
                origins.iter().any(|origin| {
                    let start = *origin + direction * epsilon;
                    inside.contains(start)
                        || index
                            .raycast(&mesh, start, direction)
                            .is_some_and(|(distance, _)| distance + epsilon < length && inside.contains(start + direction * (distance + epsilon)))
                })
            })
            .map(|body| body.name.clone())
            .collect();
        params["_auto_subtract_scope"] = scope.clone();
        params["operation"] = json!(if targets.is_empty() { "new" } else { "cut" });
        params["targets"] = if targets.is_empty() { scope } else { json!(targets) };
    }

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
        let sources: Vec<_> = state.sketches.iter().filter(|sketch| !self.internal_profile(sketch.feature)).collect();
        let sketches = sources.iter().map(|sketch| option(sketch.name.clone(), json!(sketch.feature))).collect();
        let curves = sources
            .into_iter()
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

    fn internal_profile(&self, id: u64) -> bool {
        self.session.doc.feature(id).is_some_and(|feature| feature.is_internal_profile())
    }

    pub fn modelling_workspace(&self) -> Value {
        let marker = self.session.doc.marker.unwrap_or(self.session.doc.features.len());
        let position = self.session.doc.features.iter().take(marker).filter(|feature| !feature.is_internal_profile()).count();
        let timeline: Vec<_> = self
            .session
            .doc
            .features
            .iter()
            .enumerate()
            .filter(|(_, feature)| !feature.is_internal_profile())
            .enumerate()
            .map(|(order, (document_order, f))| {
                let result = self.session.model.result(f.id);
                json!({"id":f.id,"name":f.name,"kind":f.kind.type_name(),"order":order,"document_order":document_order,"suppressed":f.suppressed,
                "error":result.and_then(|r|r.error.as_ref()),"warning":result.and_then(|r|r.warning.as_ref()),"stale":result.is_some_and(|r|r.stale),
                "can_reattach":matches!(f.kind, FeatureKind::Sketch { .. }) && !f.suppressed})
            })
            .collect();
        let parameters: Vec<_> = self
            .session
            .doc
            .param_rows()
            .into_iter()
            .map(|p| json!({"name":p.name,"expression":p.expression,"unit":p.unit,"value":p.value}))
            .collect();
        json!({"tools":TOOLS.iter().map(|id|json!({"id":id,"label":format!("cmd.{id}")})).collect::<Vec<_>>(),"timeline":timeline,
            "position":position,"parameters":parameters})
    }

    pub fn begin_model(&self, token: u64, operation: Option<&str>, feature: Option<u64>, selection: &[Sel]) -> Result<ModelEdit, AdapterError> {
        if self.session.active_sketch.is_some() {
            return Err(error("disabled", "Finish the sketch before modelling."));
        }
        let reattach = operation == Some("sketch.redefine");
        if reattach && feature.is_none() {
            return Err(error("bad_params", "ui.ipados.attachment.choose_sketch"));
        }
        let mut context = Self { session: self.session.scratch(), geometry_revision: self.geometry_revision, workspace_cache: None };
        let mut params = json!({});
        let operation = if let Some(id) = feature {
            let position =
                self.session.doc.features.iter().position(|f| f.id == id).ok_or_else(|| error("stale_edit", "The feature no longer exists."))?;
            if !reattach {
                let inspect = context.execute("timeline.inspect", json!({"feature":id}))?.value;
                params = inspect["definition"].clone();
            }
            // Offer only geometry before the edited feature. No authoritative marker is moved.
            context.execute("timeline.roll_to", json!({"position":position}))?;
            if reattach { "sketch.redefine" } else { "timeline.edit" }
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
            selection_field: None,
            document_id: self.session.document_id.clone(),
            geometry_revision: self.geometry_revision,
            fields: Vec::new(),
            preview: None,
            last_preview: None,
            handle: None,
            repair_source: None,
            repair_original: None,
            reply: Value::Null,
            feedback: Vec::new(),
        };
        if reattach {
            context.sketch_attachment_form(&mut edit)?;
        } else if feature.is_some() {
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
                for ss in before.sketches.iter().filter(|sketch| !context.internal_profile(sketch.feature)) {
                    for (index, _) in ss.profiles.iter().enumerate() {
                        let profiles = capture_profile_selection(ss, &[index]).map_err(|e| error("profile", e.to_string()))?;
                        options.push(option(format!("{} · region {}", ss.name, index + 1), json!({"sketch":ss.feature,"profiles":profiles})));
                    }
                }
                edit.params["profile"] = json!({"sketch":edit.params["sketch"],"profiles":edit.params["profiles"]});
                if !edit.params["sketch"].as_u64().is_some_and(|id| context.internal_profile(id)) {
                    edit.fields.push(field("profile", "Reselect source profile", "reference", options));
                }
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
                for ss in context.session.model.state().sketches.iter().filter(|sketch| !context.internal_profile(sketch.feature)) {
                    for (index, _) in ss.profiles.iter().enumerate() {
                        let profiles = capture_profile_selection(ss, &[index]).map_err(|e| error("profile", e.to_string()))?;
                        options.push(option(format!("{} · region {}", ss.name, index + 1), json!({"sketch":ss.feature,"profiles":profiles})));
                    }
                }
                edit.fields.push(field("sections", "Ordered sections", "references", options));
            }
            if edit.params.get("operation").is_some() {
                let extrusion = self.session.doc.feature(feature.unwrap_or_default()).is_some_and(|f| matches!(f.kind, FeatureKind::Extrude { .. }));
                let operations = if extrusion { extrusion_operations(true) } else { choices(&["new_body", "join", "cut", "intersect"]) };
                edit.fields.push(field("operation", "Operation", if extrusion { "inline_choice" } else { "choice" }, operations));
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
        let combine = edit.operation == "solid.combine"
            || feature.and_then(|id| self.session.doc.feature(id)).is_some_and(|feature| matches!(feature.kind, FeatureKind::Combine { .. }));
        if combine {
            for field in &mut edit.fields {
                match field["key"].as_str() {
                    Some("target") => field["kind"] = json!("body_pick"),
                    Some("tools") => field["kind"] = json!("bodies_pick"),
                    _ => {}
                }
            }
            edit.selection_field = Some(if edit.params["target"].as_str().is_some_and(|name| !name.is_empty()) { "tools" } else { "target" }.into());
        }
        if reattach
            || combine && feature.is_some()
            || feature.is_some() && (self.session.model.results.iter().any(|r| r.error.is_some()) || self.validate_model_references().is_err())
        {
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
        if let Some(handle) = self.edge_treatment_handle(edit) {
            return Some(handle);
        }
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
        let distance = edit.params["distance"].as_str().or_else(|| edit.params["extent"]["distance"].as_str())?;
        // An automatic extrusion starts without a proposed depth. Keep its
        // handle on the source so the first drag supplies the actual distance.
        let distance = if edit.feature.is_none() && distance.trim().is_empty() {
            0.
        } else {
            self.session.doc.eval(distance, solidcraft_engine::doc::expr::Kind::Length).ok()?
        };
        let direction = edit.params["direction"].as_str().or_else(|| edit.params["extent"]["direction"].as_str()).unwrap_or("positive");
        let factor = match direction {
            "negative" => -1.,
            "symmetric" if extent["convention"] == "legacy_per_side" => 1.,
            "symmetric" => 0.5,
            _ => 1.,
        };
        let world = self.session.world_state();
        let source = edit.params.get("profile").unwrap_or(&edit.params);
        let face_point = source["faces"].as_array().and_then(|points| points.first()).or_else(|| source.get("face"));
        let (origin, normal, arm) = if let Some(point) = face_point {
            let point = serde_json::from_value::<solidcraft_engine::geom::Vec3>(point.clone()).ok()?;
            let body = world.body(source["face_body"].as_str()?)?;
            let index = solidcraft_engine::doc::appearance::face_index_at(body, point)?;
            let spot = solidcraft_engine::doc::naming::face_spot(body, index)?;
            let solidcraft_engine::kernel::FaceSurface::Plane { normal, .. } = spot.surface? else { return None };
            (spot.point, normal.vec(), solidcraft_engine::geom::Vec3::X)
        } else {
            let sid = edit.params["profile"]["sketch"].as_u64().or_else(|| edit.params["sketch"].as_u64())?;
            let ss = world.sketch(sid)?;
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
            (ss.plane.to_world(centre), ss.plane.normal(), ss.plane.x)
        };
        Some(ModelHandle {
            origin,
            normal,
            distance,
            factor,
            parameter: if edit.feature.is_some() { "extent.distance".into() } else { "distance".into() },
            units: "mm".into(),
            angular: false,
            arm,
            size: 20.,
        })
    }

    /// Display geometry only: dimensions and evaluation remain engine commands.
    /// Anchor to the source, since a blend replaces the selected edge in preview.
    fn edge_treatment_handle(&self, edit: &ModelEdit) -> Option<ModelHandle> {
        use solidcraft_engine::geom::Vec3;
        let native = edit.feature.and_then(|id| self.session.doc.feature(id));
        let shell = edit.operation == "solid.shell" || native.is_some_and(|f| matches!(f.kind, FeatureKind::Shell { .. }));
        let fillet = edit.operation == "solid.fillet" || native.is_some_and(|f| matches!(f.kind, FeatureKind::Fillet { .. }));
        let chamfer = edit.operation == "solid.chamfer" || native.is_some_and(|f| matches!(f.kind, FeatureKind::Chamfer { .. }));
        if !shell && !fillet && !chamfer {
            return None;
        }
        let source = edit.feature.map_or_else(|| self.session.world_state(), |id| self.session.model.state_before(id));
        let reference = edit.params[if shell { "faces" } else { "edges" }].as_array()?.first()?;
        let name = edit.params["body"].as_str().or_else(|| reference["body"].as_str())?;
        let body = source.body(name)?;
        let mesh = body.mesh();
        let (origin, normal) = if shell {
            let point = serde_json::from_value::<Vec3>(reference.clone()).ok()?;
            let index = solidcraft_engine::doc::appearance::face_index_at(body, point)?;
            let spot = solidcraft_engine::doc::naming::face_spot(body, index)?;
            let solidcraft_engine::kernel::FaceSurface::Plane { normal, .. } = spot.surface? else { return None };
            (spot.point, normal.vec())
        } else {
            let index = if let Some(index) = reference["index"].as_u64() {
                usize::try_from(index).ok()?
            } else {
                let point = serde_json::from_value::<Vec3>(reference.clone()).ok()?;
                mesh.edges
                    .iter()
                    .enumerate()
                    .filter_map(|(index, edge)| {
                        let distance = edge.windows(2).map(|segment| point.dist_to_segment(segment[0], segment[1])).fold(f64::MAX, f64::min);
                        (distance < 0.01).then_some((index, distance))
                    })
                    .min_by(|a, b| a.1.total_cmp(&b.1))?
                    .0
            };
            if !mesh.edge_pickable(index) {
                return None;
            }
            let edge = mesh.edges.get(index)?;
            let origin = if edge.len() == 2 { (*edge.first()? + *edge.last()?) * 0.5 } else { *edge.get(edge.len() / 2)? };
            // The exterior face-normal bisector keeps the knob off the solid.
            // Curved faces use their existing tessellation normal at the edge.
            let mut normal = Vec3::ZERO;
            for face in mesh.edge_faces.get(index)? {
                let nearest = mesh
                    .tri_face
                    .iter()
                    .enumerate()
                    .filter(|(_, id)| *id == face)
                    .filter_map(|(i, _)| mesh.triangles.get(i))
                    .flat_map(|tri| tri.iter())
                    .filter_map(|vertex| Some((mesh.positions.get(*vertex as usize)?, mesh.normals.get(*vertex as usize)?)))
                    .min_by(|(a, _), (b, _)| a.dist(origin).total_cmp(&b.dist(origin)))?;
                normal += *nearest.1;
            }
            (origin, normal.normalized()?)
        };
        let parameter = if shell {
            "thickness"
        } else if fillet {
            "radius"
        } else {
            "distance"
        };
        let distance = self.session.doc.eval(edit.params[parameter].as_str()?, solidcraft_engine::doc::expr::Kind::Length).ok()?;
        if !distance.is_finite() || distance <= 0. {
            return None;
        }
        Some(ModelHandle {
            origin,
            normal,
            distance,
            factor: 1.,
            parameter: parameter.into(),
            units: "mm".into(),
            angular: false,
            arm: Vec3::X,
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
        let selecting_targets = edit.selection_field.as_deref() == Some("targets");
        let mut refreshed = if edit.feature.is_some() || crate::assembly::supported(&edit.operation) || selecting_targets || edit.has_body_picks() {
            edit.clone()
        } else {
            self.begin_model(edit.token, Some(&edit.operation), None, selection)?
        };
        // Rebuilding a creation form must not reset the live handle's version.
        // Inspection runs after previews, including while its contact is down.
        refreshed.version = edit.version;
        refreshed.selection_field = edit.selection_field.clone();
        let canvas_profile = if params.is_none()
            && edit.feature.is_none()
            && !selecting_targets
            && matches!(edit.operation.as_str(), "solid.extrude" | "solid.revolve" | "solid.sweep")
            && selection.iter().any(|s| matches!(s, Sel::Profile { .. }) || edit.operation == "solid.extrude" && matches!(s, Sel::Face { .. }))
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
        let canvas_selection = params.is_none();
        refreshed.params = params.unwrap_or_else(|| edit.params.clone());
        if canvas_selection && let Some(field) = edit.body_pick_field() {
            let bodies: Vec<_> = selection
                .iter()
                .filter_map(|selected| match selected {
                    Sel::Body { name } => Some(json!(name)),
                    _ => None,
                })
                .collect();
            if field == "target" {
                refreshed.params["target"] = bodies.first().cloned().unwrap_or(Value::Null);
                let target = refreshed.params["target"].clone();
                if let Some(tools) = refreshed.params["tools"].as_array_mut() {
                    tools.retain(|tool| tool != &target);
                }
            } else {
                refreshed.params["tools"] = json!(bodies.into_iter().filter(|body| body != &refreshed.params["target"]).collect::<Vec<_>>());
            }
        }
        if edit.operation == "sketch.redefine" && canvas_selection && edit.selection_field.as_deref() == Some("plane") {
            refreshed.params["plane"] = self.sketch_attachment_selection(edit, selection);
        }
        invalidate_automatic_subtract_scope(&edit.params, &mut refreshed.params);
        if selecting_targets && canvas_selection {
            if let Some(fields) = refreshed.params.as_object_mut() {
                fields.remove("_auto_subtract_scope");
            }
            refreshed.params["targets"] = json!(
                selection
                    .iter()
                    .filter_map(|selected| match selected {
                        Sel::Body { name } => Some(name.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            );
        }
        if let Some((faces, body)) = canvas_faces {
            refreshed.params["faces"] = faces;
            refreshed.params["body"] = body;
        }
        // Creation used to rebuild the options but retain the old edge list.
        // Explicit source picking replaces only the picked reference field.
        let treatment = matches!(edit.operation.as_str(), "solid.fillet" | "solid.chamfer" | "solid.shell")
            || edit
                .feature
                .and_then(|id| self.session.doc.feature(id))
                .is_some_and(|feature| matches!(feature.kind, FeatureKind::Fillet { .. } | FeatureKind::Chamfer { .. } | FeatureKind::Shell { .. }));
        if canvas_selection && treatment && (edit.feature.is_none() || matches!(edit.selection_field.as_deref(), Some("edges" | "faces"))) {
            let key = if refreshed.params.get("thickness").is_some() { "faces" } else { "edges" };
            let bodies = selection
                .iter()
                .filter_map(|s| match s {
                    Sel::Edge { body, .. } | Sel::Face { body, .. } => Some(body),
                    _ => None,
                })
                .collect::<std::collections::BTreeSet<_>>();
            if bodies.len() > 1 {
                return Err(error("selection", "Select edges or removal faces from one body at a time."));
            }
            refreshed.params[key] = json!(
                selection
                    .iter()
                    .filter_map(|s| match s {
                        Sel::Edge { body, index, point } if key == "edges" =>
                            Some(if edit.feature.is_some() { json!(point) } else { json!({"body":body,"index":index}) }),
                        Sel::Face { point, .. } if key == "faces" => Some(json!(point)),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            );
            if let Some(body) = selection.iter().find_map(|s| match s {
                Sel::Edge { body, .. } | Sel::Face { body, .. } => Some(body),
                _ => None,
            }) {
                refreshed.params["body"] = json!(body);
            }
        }
        if let Some(profile) = canvas_profile {
            refreshed.params["profile"] = profile;
        }
        if edit.operation == "solid.extrude" && !selecting_targets {
            update_face_extrusion_target(&mut refreshed.params, Some(&edit.params));
        }
        self.update_face_extrusion_direction(edit, &mut refreshed.params);
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
        if refreshed.selection_field.is_some() {
            refreshed.handle = None;
        }
        Ok(refreshed)
    }

    fn update_face_extrusion_direction(&self, edit: &ModelEdit, params: &mut Value) {
        if params["operation"] == edit.params["operation"]
            && params["profile"] == edit.params["profile"]
            && params["targets"] == edit.params["targets"]
        {
            return;
        }
        let source = params.get("profile").unwrap_or(params);
        let face_source = edit.operation == "solid.extrude" && (source.get("faces").is_some() || source.get("face").is_some());
        let attached_extrusion = edit.feature.and_then(|id| self.session.doc.feature(id)).is_some_and(|feature| {
            let FeatureKind::Extrude { sketch, .. } = &feature.kind else { return false };
            self.session
                .doc
                .feature(sketch.0)
                .is_some_and(|sketch| matches!(&sketch.kind, FeatureKind::Sketch { plane: solidcraft_engine::doc::PlaneRef::Face { .. }, .. }))
        });
        let sketch_source = edit.operation == "solid.extrude" && source["sketch"].as_u64().is_some();
        if !face_source && !attached_extrusion && !sketch_source {
            return;
        }
        let extent = if edit.feature.is_some() { &params["extent"] } else { &*params };
        let previous = if edit.feature.is_some() { &edit.params["extent"] } else { &edit.params };
        if extent["direction"] != previous["direction"] || extent["direction"] == "symmetric" {
            return;
        }
        let Some(distance) = extent["distance"]
            .as_str()
            .and_then(|expression| self.session.doc.eval(expression, solidcraft_engine::doc::expr::Kind::Length).ok())
            .or_else(|| extent["distance"].as_f64())
            .filter(|distance| distance.is_finite() && *distance != 0.)
        else {
            return;
        };
        let inward = match params["operation"].as_str() {
            Some("cut" | "intersect") => true,
            Some("new" | "new_body" | "join") => false,
            _ => return,
        };
        let negative = if face_source || attached_extrusion {
            inward
        } else if inward {
            let mut probe = edit.clone();
            probe.params = params.clone();
            let Some(handle) = self.extrusion_handle(&probe) else { return };
            let world = self.session.world_state();
            let targets = params["targets"].as_array();
            let enters = |sign: f64| {
                world.bodies.iter().any(|body| {
                    targets.is_some_and(|names| names.iter().any(|name| name.as_str() == Some(&body.name)))
                        && body.mesh().contains(handle.origin + handle.normal * sign * distance.abs().min(1e-3))
                })
            };
            match (enters(1.), enters(-1.)) {
                (false, true) => true,
                (true, false) => false,
                _ => return,
            }
        } else {
            return;
        };
        let direction = if (distance > 0.) == negative { "negative" } else { "positive" };
        let extent = if edit.feature.is_some() { params.get_mut("extent") } else { Some(params) };
        if let Some(fields) = extent.and_then(Value::as_object_mut) {
            fields.insert("direction".into(), json!(direction));
        }
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
            .filter(|f| matches!(f.kind, FeatureKind::Sketch { .. }) && !f.is_internal_profile())
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
                Sel::Face { body, .. } | Sel::Edge { body, .. } | Sel::Vertex { body, .. } if edit.operation == "solid.combine" => Some(json!(body)),
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
        for ss in state.sketches.iter().filter(|sketch| !self.internal_profile(sketch.feature)) {
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
                length_expression(f, "distance", "Distance");
            }
            "solid.extrude" | "solid.revolve" | "solid.sweep" => {
                // This composite presentation input is expanded immediately before
                // calling the existing constructor; profiles and body-face extrusion
                // both retain the upstream command's reference semantics.
                let face_source = if edit.operation == "solid.extrude" { extrusion_face_reference(selection)? } else { None };
                if face_source.is_some() && selected_sketch.is_some() {
                    return Err(error("selection", "ui.ipados.face_extrude.mixed_sources"));
                }
                if let Some(source) = face_source {
                    let label = faces.iter().map(|face| face["label"].as_str().unwrap_or_default()).collect::<Vec<_>>().join(", ");
                    regions.push(option(label, source.clone()));
                    p["profile"] = source;
                } else if let Some(profile) = selected_profile {
                    if !regions.iter().any(|region| region["value"] == profile) {
                        let label = selected_sketch.and_then(|id| state.sketch(id)).map(|sketch| sketch.name.clone()).unwrap_or_default();
                        regions.push(option(label, profile.clone()));
                    }
                    p["profile"] = profile;
                } else if selected_sketch.is_some() && regions.len() == 1 {
                    p["profile"] = regions.first().map(|region| region["value"].clone()).unwrap_or(Value::Null);
                }
                let mut source = field("profile", "ui.ipados.modelling.profile", "reference", regions);
                if edit.operation == "solid.extrude" {
                    source["label"] = json!("ui.ipados.face_extrude.source");
                }
                f.push(source);
                if edit.operation == "solid.extrude" {
                    p["distance"] = json!("10 mm");
                    p["direction"] = json!("positive");
                    p["through_all"] = json!(false);
                    length_expression(f, "distance", "Distance");
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
                length_expression(f, key, key);
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
                length_expression(f, "thickness", "Wall thickness");
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
                let mut unique = Vec::new();
                for body in &selected_bodies {
                    if !unique.contains(body) {
                        unique.push(body.clone());
                    }
                }
                p["target"] = unique.first().cloned().unwrap_or(Value::Null);
                p["tools"] = json!(unique.iter().skip(1).collect::<Vec<_>>());
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
            let operations = if edit.operation == "solid.extrude" {
                extrusion_operations(false)
            } else {
                choices(if edit.operation == "solid.combine" { &["join", "cut", "intersect"] } else { &["new", "join", "cut", "intersect"] })
            };
            f.push(field("operation", "Operation", if edit.operation == "solid.extrude" { "inline_choice" } else { "choice" }, operations));
        }
        if matches!(edit.operation.as_str(), "solid.extrude" | "solid.revolve" | "solid.sweep" | "solid.loft" | "solid.hole") {
            p["targets"] = json!([]);
            if edit.operation == "solid.extrude" {
                update_face_extrusion_target(p, None);
            }
            f.push(field("targets", "Target bodies · required for join, cut or intersect", "references", bodies));
        }
        Ok(())
    }

    pub fn model_descriptor(&self, edit: &ModelEdit) -> Value {
        json!({"token":edit.token,"operation":edit.operation,"feature":edit.feature,"params":edit.params,"fields":edit.fields,"selection_field":edit.selection_field,"valid":edit.preview.is_some(),"handle":edit.handle,"reply":edit.reply,"feedback":edit.feedback})
    }

    fn validate_model(&self, edit: &ModelEdit) -> Result<(), AdapterError> {
        if edit.document_id != self.session.document_id || edit.geometry_revision != self.geometry_revision {
            return Err(error("stale_edit", "The model changed. Restart the operation and reselect its inputs."));
        }
        Ok(())
    }

    pub fn prepare_model(&self, edit: &ModelEdit, mut params: Value, commit: bool) -> Result<PreparedModel, AdapterError> {
        self.validate_model(edit)?;
        if crate::assembly::supported(&edit.operation) {
            return self.prepare_assembly(edit, params, commit);
        }
        if !params.is_object() || params.to_string().len() > 65536 {
            return Err(error("bad_params", "Invalid modelling form."));
        }
        invalidate_automatic_subtract_scope(&edit.params, &mut params);
        self.automatic_subtract(edit, &mut params);
        let mut p = params.clone();
        if let Some(fields) = p.as_object_mut() {
            fields.remove("_auto_subtract");
            fields.remove("_auto_subtract_scope");
        }
        if edit.operation == "solid.extrude" {
            update_face_extrusion_target(&mut p, None);
        }
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
        let source = p.get("profile").unwrap_or(&p);
        if edit.operation == "solid.extrude"
            && source.get("faces").is_none()
            && source.get("face").is_none()
            && source["sketch"].as_u64().is_none()
            && source["sketch"].as_str().is_none_or(str::is_empty)
        {
            return Err(error("selection", "ui.ipados.face_extrude.choose_source"));
        }
        if let Some(profile) = p.as_object_mut().and_then(|m| m.remove("profile")) {
            if edit.operation == "solid.extrude" && profile.get("faces").is_some() {
                for key in ["face", "sketch", "profiles"] {
                    if let Some(fields) = p.as_object_mut() {
                        fields.remove(key);
                    }
                }
                p["faces"] = profile["faces"].clone();
                p["face_body"] = profile["face_body"].clone();
            } else {
                p["sketch"] = profile["sketch"].clone();
                p["profiles"] = profile["profiles"].clone();
            }
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
        let (command, p) = if edit.operation == "sketch.redefine" {
            ("sketch.redefine", self.sketch_attachment_command(edit, &params)?)
        } else if let Some(feature) = edit.feature {
            if hole && let Some(targets) = p.as_object_mut().and_then(|m| m.remove("targets")) {
                p["participants"] = targets;
            }
            ("timeline.edit", json!({"feature":feature,"set":p}))
        } else {
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
        let reply = scratch.execute(command, p.clone())?.value;
        if let Some(r) = scratch.session.model.results.iter().find(|r| r.warning.is_some()) {
            return Err(error("evaluation_warning", format!("{}: {}", r.name, r.warning.as_deref().unwrap_or_default())));
        }
        scratch.validate_model_references()?;
        scratch.validate_joint_references()?;
        let mut model = scratch.snapshot()?;
        let mut feedback = if edit.operation == "sketch.redefine" { vec!["ui.ipados.attachment.preview_hint".into()] } else { Vec::new() };
        if !commit {
            let feature = edit.feature.or_else(|| reply["feature"].as_u64());
            if let Some(feature) =
                feature.filter(|id| scratch.session.doc.feature(*id).is_some_and(|f| matches!(f.kind, FeatureKind::Extrude { .. })))
            {
                feedback = self.extrusion_preview_scene(&scratch, command, &p, feature, &mut model)?;
            }
        }
        let transaction = if commit { Some(self.session.scratch().compute_commands(&[(command.into(), p)], None)?) } else { None };
        let mut updated = edit.clone();
        updated.params = params.clone();
        let offset =
            edit.operation == "solid.offset_face" || target_feature.is_some_and(|feature| matches!(feature.kind, FeatureKind::OffsetFace { .. }));
        // Joining can replace the selected face in the preview. Resolve creation
        // handles against the unchanged source, just like Offset Face.
        let source = params.get("profile").unwrap_or(&params);
        let face_extrude = edit.operation == "solid.extrude" && (source.get("faces").is_some() || source.get("face").is_some());
        let treatment = matches!(edit.operation.as_str(), "solid.fillet" | "solid.chamfer" | "solid.shell")
            || target_feature
                .is_some_and(|feature| matches!(feature.kind, FeatureKind::Fillet { .. } | FeatureKind::Chamfer { .. } | FeatureKind::Shell { .. }));
        let handle = if offset || face_extrude || treatment { self.extrusion_handle(&updated) } else { scratch.extrusion_handle(&updated) };
        Ok(PreparedModel { model, params, version: edit.version, handle, transaction, reply: Value::Null, feedback })
    }

    fn extrusion_preview_scene(
        &self,
        after: &Self,
        command: &str,
        params: &Value,
        feature: u64,
        model: &mut Arc<SceneSnapshot>,
    ) -> Result<Vec<String>, AdapterError> {
        use solidcraft_engine::{doc::Operation, view};
        use solidcraft_render::{Scene, SceneMesh};

        let Some(native) = after.session.doc.feature(feature) else { return Ok(Vec::new()) };
        let FeatureKind::Extrude { operation, .. } = &native.kind else { return Ok(Vec::new()) };
        let palette = view::Palette::DEFAULT;
        let (color, label) = match operation {
            Operation::NewBody => (view::colors::PROFILE, "ui.ipados.face_extrude.preview_new"),
            Operation::Join => (palette.axes[1], "ui.ipados.face_extrude.preview_union"),
            Operation::Intersect => (palette.body_selected, "ui.ipados.face_extrude.preview_intersect"),
            Operation::Cut => (palette.axes[0], "ui.ipados.face_extrude.preview_subtract"),
        };
        let result = after.session.world_state();
        let result_names = model.picks.body_names();
        let mut base = self.session.scratch();
        base.selection.clear();
        if command == "timeline.edit"
            && let Some(position) = base.doc.feature_index(feature)
        {
            base.execute("timeline.roll_to", &json!({"position":position}))?;
        }
        let before = base.world_state();
        let before_scene = view::scene_with_guides(&base, &solidcraft_render::Camera::default(), None, view::Guides { grid: false, axes: false });
        let before_names: Vec<_> = before.bodies.iter().filter(|body| base.body_visible(&body.name)).map(|body| body.name.clone()).collect();
        let changed = |name: &str| match (before.body(name), result.body(name)) {
            (Some(old), Some(new)) => {
                let old = old.mesh();
                let new = new.mesh();
                old.positions != new.positions || old.triangles != new.triangles
            }
            _ => true,
        };
        let mesh = |source: &SceneMesh, color, opacity| SceneMesh { mesh: source.mesh.clone(), color, opacity, face_colors: Vec::new() };
        let mut display = Scene { background: model.scene.background, ..Scene::default() };
        display.lines = model
            .scene
            .lines
            .iter()
            .map(|line| solidcraft_render::SceneLine {
                points: line.points.clone(),
                color: line.color,
                alpha: line.alpha,
                width: line.width,
                on_top: line.on_top,
            })
            .collect();
        let mut names = Vec::new();
        let mut bounds = model.bounds;
        for (source, name) in before_scene.meshes.iter().zip(&before_names) {
            bounds = bounds.union(&source.mesh.bounds());
            if matches!(operation, Operation::NewBody | Operation::Join) || !changed(name) {
                display.meshes.push(SceneMesh {
                    mesh: source.mesh.clone(),
                    color: source.color,
                    opacity: source.opacity,
                    face_colors: source.face_colors.clone(),
                });
            } else {
                display.meshes.push(mesh(source, palette.body, 0.16));
            }
            names.push(name.clone());
        }
        if !matches!(operation, Operation::Join) {
            for (source, name) in model.scene.meshes.iter().zip(&result_names).filter(|(_, name)| changed(name)) {
                let tint = if *operation == Operation::Cut { source.color } else { color };
                display.meshes.push(mesh(source, tint, source.opacity));
                names.push(name.clone());
            }
        }
        if matches!(operation, Operation::Join | Operation::Cut) {
            // Request the existing engine's New Body form for the tool. This is
            // presentation geometry only; the real Boolean result above remains
            // authoritative and is the only geometry that can be accepted.
            let mut tool = params.clone();
            if command == "timeline.edit" {
                tool["set"]["operation"] = json!("new_body");
            } else {
                tool["operation"] = json!("new");
            }
            let mut tool_source = self.session.scratch();
            if command == "timeline.edit"
                && let Some(position) = tool_source.doc.feature_index(feature).and_then(|position| position.checked_add(1))
            {
                tool_source.execute("timeline.roll_to", &json!({"position":position}))?;
            }
            let preview = tool_source.preview_world(&[(command.into(), tool)])?;
            for body in
                preview.after.bodies.iter().filter(|body| {
                    if command == "timeline.edit" { body.feature == feature } else { self.session.doc.feature(body.feature).is_none() }
                })
            {
                let shape = body.mesh();
                bounds = bounds.union(&shape.bounds());
                display.meshes.push(SceneMesh {
                    mesh: shape,
                    color,
                    face_colors: Vec::new(),
                    opacity: if *operation == Operation::Cut { 0.38 } else { 1.0 },
                });
                names.push(String::new());
            }
        }
        let snapshot = Arc::get_mut(model).ok_or_else(|| error("preview", "Prepared preview scene must be uniquely owned."))?;
        display.radius = snapshot.scene.radius.max(bounds.diagonal());
        snapshot.bounds = bounds;
        snapshot.preview_scene = Some(Arc::new(crate::PreviewScene { scene: Arc::new(display), names }));
        Ok(vec![label.into()])
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
        if let Some(field) = edit.body_pick_field() {
            self.selection = edit.body_pick_selection(field);
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
    pub fn begin_model(&mut self, mut edit: ModelEdit) -> Result<(), AdapterError> {
        if self.numeric.is_some() || self.modelling.is_some() || (self.updating && edit.repair_source.is_none()) {
            return Err(error("disabled", "Finish the current operation."));
        }
        self.cancel_interactions();
        if edit.repair_source.is_some() && !self.updating {
            edit.repair_original = self
                .model
                .as_ref()
                .filter(|model| model.document_id == edit.document_id && model.geometry_revision == edit.geometry_revision)
                .cloned();
        }
        if let Some(source) = &edit.repair_source {
            self.model = Some(source.clone());
            self.selection.clear();
            self.updating = false;
        }
        if let Some(field) = edit.body_pick_field() {
            self.selection = edit.body_pick_selection(field);
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
        if edit.operation == "sketch.redefine" && edit.selection_field.as_deref() == Some("plane") {
            let current = matches!(self.selection.as_slice(), [Sel::Face { body, index, .. }]
                if prepared.params["plane"]["body"] == *body && prepared.params["plane"]["index"] == *index);
            if !current {
                return Err(error("stale_preview", "ui.ipados.attachment.pick_changed"));
            }
        }
        if let Some(e) = &mut self.modelling {
            e.preview = Some(prepared.model.clone());
            e.selection_field = None;
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

    pub fn select_model_inputs(&mut self, token: u64, field: Option<&str>) -> Result<(), AdapterError> {
        let edit = self.model_edit(token)?;
        let field = field.or_else(|| {
            if edit.operation == "sketch.redefine" {
                Some("plane")
            } else if edit.has_body_picks() {
                Some("target")
            } else {
                (edit.operation == "solid.extrude" && edit.params["operation"] != "new" && edit.params["profile"].is_object()).then_some("targets")
            }
        });
        if field.is_some_and(|field| {
            !matches!(field, "profile" | "targets" | "target" | "tools" | "plane" | "edges" | "faces")
                || !edit.fields.iter().any(|input| input["key"] == field)
                || matches!(field, "target" | "tools") && !edit.has_body_picks()
        }) {
            return Err(error("bad_params", "Choose a modelling source or target field."));
        }
        self.fail_model(token)?;
        self.selection.clear();
        if edit.has_body_picks() {
            self.selection = edit.body_pick_selection(field.unwrap_or("target"));
        }
        if let Some(edit) = &mut self.modelling {
            edit.selection_field = field.map(str::to_string);
            if edit.operation == "sketch.redefine" {
                edit.params["plane"] = Value::Null;
            }
            edit.handle = None;
            edit.version = edit.version.saturating_add(1);
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
            self.updating = edit.repair_original.is_none();
            if let Some(original) = edit.repair_original {
                self.model = Some(original);
            }
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
        if let Some(edit) = self.modelling.as_ref().filter(|edit| edit.repair_source.is_some()) {
            self.updating = edit.repair_original.is_none();
            if let Some(original) = &edit.repair_original {
                self.model = Some(original.clone());
            }
            self.selection.clear();
        }
        self.modelling = None;
        self.cancel_interactions();
    }
}
