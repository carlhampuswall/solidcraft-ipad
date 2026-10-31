//! Read-only native presentation mapping and explicit numeric transactions. CAD
//! evaluation, expressions and undo remain the upstream engine's responsibility.
use std::sync::Arc;

use serde_json::{Value, json};
use solidcraft_engine::{
    ComputedCommands, Sel,
    doc::{FeatureKind, expr::Kind},
};

use crate::{AdapterError, RadiusHandle, SceneSnapshot, SessionAdapter, Viewport, error};

pub(crate) struct WorkspaceCache {
    revision: u64,
    document_id: String,
    rows: Vec<Value>,
    assembly: Value,
}

#[derive(Clone)]
pub struct NumericEdit {
    pub token: u64,
    document_id: String,
    geometry_revision: u64,
    selection: Vec<Sel>,
    target: NumericTarget,
    pub units: String,
    pub expression: String,
    pub value: f64,
    pub(crate) preview: Option<Arc<SceneSnapshot>>,
}

#[derive(Clone)]
enum NumericTarget {
    Radius(RadiusHandle),
    Dimension { sketch: u64, constraint: String, parameter: String },
    NewDimension { sketch: u64, params: Value },
}

pub struct PreparedNumeric {
    pub expression: String,
    pub value: f64,
    pub model: Arc<SceneSnapshot>,
    transaction: Option<ComputedCommands>,
}

fn selected_body(selection: &[Sel], name: &str) -> bool {
    selection.iter().any(|s| match s {
        Sel::Body { name: n } | Sel::Face { body: n, .. } | Sel::Edge { body: n, .. } | Sel::Vertex { body: n, .. } => n == name,
        _ => false,
    })
}

impl SessionAdapter {
    /// Read the most recent pick through the existing measurement command. A
    /// scratch session keeps inspection out of live edits and document history.
    fn selection_edge_length_mm(&self, selection: &[Sel]) -> Option<f64> {
        let edge @ Sel::Edge { .. } = selection.last()? else { return None };
        let measured = self.session.scratch().execute("inspect.measure", &json!({"items":[edge]})).ok()?;
        let item = measured.get("items")?.get(0)?;
        item.get("circumference")
            .or_else(|| item.get("arc_length"))
            .or_else(|| item.get("length_mm"))?
            .as_f64()
            .filter(|length| length.is_finite() && *length >= 0.0)
    }

    fn radius_selected(&self, selection: &[Sel]) -> Option<RadiusHandle> {
        let handle = self.radius_handle()?;
        (!selection.is_empty()
            && selection.iter().all(|s| match s {
                Sel::Feature { id } => *id == handle.feature,
                Sel::Body { name } | Sel::Face { body: name, .. } | Sel::Edge { body: name, .. } | Sel::Vertex { body: name, .. } => {
                    name == &handle.body
                }
                _ => false,
            }))
        .then_some(handle)
    }

    pub fn workspace(&mut self, selection: &[Sel], radius_tool: bool, numeric: bool) -> Value {
        if self.workspace_cache.as_ref().is_none_or(|c| c.revision != self.geometry_revision || c.document_id != self.session.document_id) {
            let mut rows = Vec::new();
            let row = |id: String, parent: Option<String>, label: String, kind: &str, entity: Value| json!({"id":id,"parent_id":parent,"label":label,"kind":kind,"entity":entity,"selected":false});
            rows.push(row("component:0".into(), None, self.session.doc.name.clone(), "component", Value::Null));
            for c in &self.session.doc.components {
                rows.push(row(format!("component:{}", c.id), Some(format!("component:{}", c.parent)), c.name.clone(), "component", Value::Null));
            }
            for f in &self.session.doc.features {
                let kind = if matches!(&f.kind, FeatureKind::Sketch { .. }) { "sketch" } else { "feature" };
                rows.push(row(
                    format!("feature:{}", f.id),
                    Some(format!("component:{}", f.component)),
                    f.name.clone(),
                    kind,
                    json!(Sel::Feature { id: f.id }),
                ));
            }
            // World-state owners are authoritative, including occurrence placements;
            // never infer body ownership by parsing feature or body names.
            let paths = self.assembly_body_paths().unwrap_or_default();
            for b in &self.session.world_state().bodies {
                rows.push(row(
                    format!("body:{}", b.name),
                    Some(paths.get(&b.name).map(|p| crate::assembly::path_id(p)).unwrap_or_else(|| format!("component:{}", b.component))),
                    b.name.clone(),
                    "body",
                    json!(Sel::Body { name: b.name.clone() }),
                ));
            }
            for row in &mut rows {
                if row["kind"] == "component" {
                    let component = row["entity"]
                        .as_object()
                        .and_then(|o| o.get("component"))
                        .and_then(Value::as_u64)
                        .or_else(|| row["id"].as_str().and_then(|id| id.strip_prefix("component:")).and_then(|id| id.parse::<u64>().ok()))
                        .unwrap_or(0);
                    row["assembly"] = json!({"component":component,"active":self.session.active_component==component,"actions":if component==0{vec![json!({"label":"ui.ipados.assembly.root","command":"component.activate","params":{"component":0}})]}else{vec![json!({"label":"ui.ipados.assembly.rename","operation":"component.rename","params":{"component":component,"name":row["label"]}})]}});
                }
            }
            let assembly = match self.assembly_browser() {
                Ok((mut assembly_rows, state)) => {
                    rows.append(&mut assembly_rows);
                    state
                }
                Err(failure) => {
                    json!({"diagnostics":[failure.message],"active_component":self.session.active_component,"active_name":self.session.doc.name,"active_path":[],"tools":[],"actions":[]})
                }
            };
            self.organise_browser(&mut rows);
            self.workspace_cache =
                Some(WorkspaceCache { revision: self.geometry_revision, document_id: self.session.document_id.clone(), rows, assembly });
        }
        let mut rows = self.workspace_cache.as_ref().map(|c| c.rows.clone()).unwrap_or_default();
        for row in &mut rows {
            row["selected"] = json!(match row["kind"].as_str() {
                Some("body") => row["label"].as_str().is_some_and(|n| selected_body(selection, n)),
                _ => selection.iter().any(|s| json!(s) == row["entity"]),
            });
            if row["assembly"].is_object() {
                row["assembly"]["active"] = json!(if row["kind"] == "component" {
                    row["assembly"]["component"] == json!(self.session.active_component)
                } else {
                    row["assembly"]["path"] == json!(self.session.active_occurrence_path.clone().unwrap_or_default())
                });
            }
        }
        let sketch = self.sketch_workspace(selection);
        let planes: Vec<_> = self
            .session
            .doc
            .features
            .iter()
            .filter(|f| matches!(f.kind, FeatureKind::ConstructionPlane { .. }))
            .map(|f| json!({"name":f.name}))
            .collect();
        let selected = self.radius_selected(selection);
        let reason = if numeric {
            Some("ui.ipados.workspace.finish_edit")
        } else if self.radius_handle().is_none() {
            Some("ui.ipados.workspace.no_radius")
        } else if selected.is_none() {
            Some("ui.ipados.workspace.select_post")
        } else {
            None
        };
        let radius = selected.and_then(|h| {
            let f = self.session.doc.features.iter().find(|f| f.id == h.feature)?;
            let FeatureKind::Cylinder { radius, .. } = &f.kind else { return None };
            Some(json!({"feature":h.feature,"expression":radius,"value":h.radius,"units":"mm"}))
        });
        let mut tools = vec![
            json!({"id":"select","enabled":!numeric,"reason":if numeric {Some("ui.ipados.workspace.finish_edit")} else {None}}),
            json!({"id":"post_radius","enabled":reason.is_none() && self.session.active_sketch.is_none(),"reason":reason}),
        ];
        if self.session.active_sketch.is_some() {
            tools.extend(
                [
                    "sketch.line",
                    "sketch.rectangle.two_point",
                    "sketch.circle.center",
                    "sketch.arc.three_point",
                    "sketch.spline.fit_point",
                    "sketch.move",
                    "sketch.dimension_text",
                    "sketch.delete",
                    "sketch.trim",
                    "sketch.extend",
                ]
                .into_iter()
                .map(|id| json!({"id":id,"enabled":!numeric,"reason":if numeric {Some("ui.ipados.workspace.finish_edit")} else {None}})),
            );
        }
        let selection_edge_length_mm = self.selection_edge_length_mm(selection);
        json!({"title":self.session.doc.name,"dirty":self.session.is_dirty(),"document_id":self.session.document_id,"revision":self.session.revision,
            "geometry_revision":self.geometry_revision,"can_undo":!numeric && !self.session.undo.is_empty(),"can_redo":!numeric && !self.session.redo.is_empty(),
            "rows":rows,"tools":tools,"assembly":self.workspace_cache.as_ref().map(|c|c.assembly.clone()),
            "sketch":sketch,"planes":planes,"modelling":self.modelling_workspace(),"active_tool":if radius_tool {"post_radius"} else {"select"},"selection":selection,"selection_edge_length_mm":selection_edge_length_mm,"radius":radius})
    }

    pub fn begin_numeric(&self, token: u64, selection: &[Sel]) -> Result<NumericEdit, AdapterError> {
        self.begin_numeric_for(token, selection, None)
    }

    pub fn begin_numeric_for(&self, token: u64, selection: &[Sel], dimension: Option<&str>) -> Result<NumericEdit, AdapterError> {
        let (target, expression, value, units) = if let Some(sketch) = self.session.active_sketch {
            let state = self.session.model.state();
            let ss = state.sketch(sketch).ok_or_else(|| error("stale_edit", "The active sketch is unavailable."))?;
            if let [Sel::SketchConstraint { id }] = selection {
                let c = ss
                    .sketch
                    .constraints
                    .iter()
                    .find(|c| &c.id == id && c.kind.is_dimension() && !c.driven)
                    .ok_or_else(|| error("disabled", "Select a driving dimension."))?;
                let parameter = c.param.clone().ok_or_else(|| error("disabled", "This dimension has no driving parameter."))?;
                let row = self
                    .session
                    .doc
                    .param_rows()
                    .into_iter()
                    .find(|r| r.name == parameter)
                    .ok_or_else(|| error("stale_edit", "The dimension parameter is unavailable."))?;
                (
                    NumericTarget::Dimension { sketch, constraint: id.clone(), parameter },
                    row.expression,
                    row.value.ok_or_else(|| error("numeric_validation", "The native dimension has no evaluated value."))?,
                    row.unit,
                )
            } else {
                let entities: Vec<_> = selection
                    .iter()
                    .filter_map(|s| match s {
                        Sel::SketchCurve { id, sketch: owner } if owner.is_none_or(|id| id == sketch) => Some(id.clone()),
                        Sel::SketchPoint { id } => Some(id.clone()),
                        Sel::SketchMidpoint { id } => Some(format!("mid:{id}")),
                        _ => None,
                    })
                    .collect();
                if entities.is_empty() || entities.len() != selection.len() {
                    return Err(error("disabled", "Select sketch geometry to dimension."));
                }
                let params = json!({"entities":entities,"type":dimension.unwrap_or("auto")});
                let mut scratch = self.session.scratch();
                let reply = scratch.execute("sketch.dimension", &params)?;
                let parameter =
                    reply["param"].as_str().ok_or_else(|| error("dimension", "The native dimension did not create a driving parameter."))?;
                let row = scratch
                    .doc
                    .param_rows()
                    .into_iter()
                    .find(|r| r.name == parameter)
                    .ok_or_else(|| error("dimension", "The dimension parameter is unavailable."))?;
                (
                    NumericTarget::NewDimension { sketch, params },
                    row.expression,
                    row.value.ok_or_else(|| error("numeric_validation", "The native dimension has no evaluated value."))?,
                    row.unit,
                )
            }
        } else {
            let handle = self.radius_selected(selection).ok_or_else(|| error("disabled", "ui.ipados.workspace.select_post"))?;
            let f = self
                .session
                .doc
                .features
                .iter()
                .find(|f| f.id == handle.feature)
                .ok_or_else(|| error("stale_edit", "ui.ipados.workspace.invalidated"))?;
            let FeatureKind::Cylinder { radius, .. } = &f.kind else { return Err(error("disabled", "ui.ipados.workspace.no_radius")) };
            (NumericTarget::Radius(handle.clone()), radius.to_string(), handle.radius, "mm".into())
        };
        Ok(NumericEdit {
            token,
            document_id: self.session.document_id.clone(),
            geometry_revision: self.geometry_revision,
            selection: selection.to_vec(),
            target,
            expression,
            value,
            units,
            preview: None,
        })
    }

    fn validate_numeric(&self, edit: &NumericEdit) -> Result<(), AdapterError> {
        let valid = match &edit.target {
            NumericTarget::Radius(handle) => {
                self.radius_handle().is_some_and(|h| h.feature == handle.feature && h.radius == handle.radius && h.plane == handle.plane)
            }
            NumericTarget::Dimension { sketch, constraint, parameter } => {
                self.session.active_sketch == Some(*sketch)
                    && self.session.model.state().sketch(*sketch).is_some_and(|ss| {
                        ss.sketch.constraints.iter().any(|c| &c.id == constraint && c.param.as_ref() == Some(parameter) && !c.driven)
                    })
            }
            NumericTarget::NewDimension { sketch, .. } => self.session.active_sketch == Some(*sketch),
        };
        if edit.document_id != self.session.document_id || edit.geometry_revision != self.geometry_revision || !valid {
            return Err(error("stale_edit", "ui.ipados.workspace.invalidated"));
        }
        Ok(())
    }

    pub fn prepare_numeric(&self, edit: &NumericEdit, expression: &str, commit: bool) -> Result<PreparedNumeric, AdapterError> {
        self.validate_numeric(edit)?;
        if expression.len() > 1024 || expression.trim().is_empty() {
            return Err(error("numeric_validation", "ui.ipados.workspace.numeric_required"));
        }
        let (command, params) = match &edit.target {
            NumericTarget::Radius(handle) => {
                let value = self.session.doc.eval(expression, Kind::Length).map_err(|e| error("numeric_validation", e.to_string()))?;
                if !value.is_finite() || !(0.5..=50.0).contains(&value) {
                    return Err(error("numeric_validation", "ui.ipados.workspace.radius_range"));
                }
                ("timeline.edit", json!({"feature":handle.feature,"set":{"radius":expression}}))
            }
            NumericTarget::Dimension { parameter, .. } => ("parameters.change", json!({"name":parameter,"expression":expression})),
            NumericTarget::NewDimension { params, .. } => {
                let mut p = params.clone();
                p["value"] = json!(expression);
                ("sketch.dimension", p)
            }
        };
        let mut scratch = Self { session: self.session.scratch(), geometry_revision: self.geometry_revision, workspace_cache: None };
        let reply = scratch.execute(command, params.clone()).map_err(|e| error("numeric_validation", e.message))?;
        // Values and units come from the same native parameter used by the solver.
        let value = match &edit.target {
            NumericTarget::Radius(_) => scratch.radius_handle().map(|h| h.radius),
            NumericTarget::Dimension { parameter, .. } => {
                scratch.session.doc.param_rows().into_iter().find(|r| &r.name == parameter).and_then(|r| r.value)
            }
            NumericTarget::NewDimension { .. } => reply.value["param"]
                .as_str()
                .and_then(|parameter| scratch.session.doc.param_rows().into_iter().find(|r| r.name == parameter))
                .and_then(|r| r.value),
        }
        .ok_or_else(|| error("numeric_validation", "The native parameter did not evaluate."))?;
        // `parameters.change` accepts zero as a general distance. A radial dimension
        // must also satisfy the native circle creator's radius validation; reusing
        // that public constructor keeps its exact range/tolerance in the engine.
        let parameter = match &edit.target {
            NumericTarget::Dimension { parameter, .. } => Some(parameter.as_str()),
            NumericTarget::NewDimension { .. } => reply.value["param"].as_str(),
            NumericTarget::Radius(_) => None,
        };
        if let Some(sketch) = scratch.sketch_snapshot()
            && let Some(c) = sketch.sketch.constraints.iter().find(|c| c.param.as_deref() == parameter && parameter.is_some())
            && matches!(c.kind, solidcraft_engine::sketch::ConstraintKind::Radius { .. } | solidcraft_engine::sketch::ConstraintKind::Diameter { .. })
        {
            for curve in c.kind.indices().1 {
                if let Some(radius) = sketch.sketch.radius(curve) {
                    solidcraft_engine::sketch::Sketch::default()
                        .add_circle(solidcraft_engine::geom::Vec2::ZERO, radius, None, None)
                        .map_err(|e| error("numeric_validation", e.to_string()))?;
                }
            }
        }
        if let Some(id) = scratch.session.active_sketch {
            let state = scratch.session.model.state();
            let sketch = state.sketch(id).ok_or_else(|| error("solve_unavailable", "The edited sketch has no solver result."))?;
            if !sketch.report.keeps_orientation() {
                return Err(error(
                    "numeric_validation",
                    sketch.report.failure(|ids| {
                        if ids.is_empty() {
                            "The sketch solver could not satisfy this dimension.".into()
                        } else {
                            format!("The sketch solver could not satisfy constraints {ids} for this dimension.")
                        }
                    }),
                ));
            }
        }
        let model = scratch.snapshot()?;
        let commands = [(command.into(), params)];
        let transaction = if commit { Some(self.session.scratch().compute_commands(&commands, None)?) } else { None };
        Ok(PreparedNumeric { expression: expression.into(), value, model, transaction })
    }

    pub fn accept_numeric(&mut self, edit: &NumericEdit, mut prepared: PreparedNumeric) -> Result<Arc<SceneSnapshot>, AdapterError> {
        self.validate_numeric(edit)?;
        let transaction = prepared.transaction.take().ok_or_else(|| error("numeric", "A preview cannot be committed."))?;
        let revision = self.geometry_revision.checked_add(1).ok_or_else(|| error("revision_limit", "geometry revision exhausted"))?;
        let model = Arc::get_mut(&mut prepared.model).ok_or_else(|| error("numeric", "The prepared scene must have unique ownership."))?;
        self.session.accept_commands(transaction)?;
        self.geometry_revision = revision;
        model.geometry_revision = revision;
        model.revision = self.session.revision;
        Ok(prepared.model)
    }
}

impl Viewport {
    pub fn numeric_edit(&self, token: u64) -> Result<NumericEdit, AdapterError> {
        self.numeric
            .as_ref()
            .filter(|e| {
                e.token == token
                    && e.selection == self.selection
                    && self.model.as_ref().is_some_and(|m| m.document_id == e.document_id && m.geometry_revision == e.geometry_revision)
            })
            .cloned()
            .ok_or_else(|| error("stale_edit", "ui.ipados.workspace.invalidated"))
    }
    pub fn begin_numeric(&mut self, edit: NumericEdit) -> Result<(), AdapterError> {
        if self.numeric.is_some() || self.updating {
            return Err(error("disabled", "ui.ipados.workspace.finish_edit"));
        }
        self.cancel_interactions();
        self.numeric = Some(edit);
        self.changed();
        Ok(())
    }
    pub fn publish_numeric(&mut self, token: u64, prepared: &PreparedNumeric) -> Result<(), AdapterError> {
        self.numeric_edit(token)?;
        if let Some(edit) = &mut self.numeric {
            edit.expression = prepared.expression.clone();
            edit.value = prepared.value;
            edit.preview = Some(prepared.model.clone());
        }
        self.changed();
        Ok(())
    }
    pub fn finish_numeric(&mut self, token: u64, model: Option<Arc<SceneSnapshot>>) -> Result<(), AdapterError> {
        self.numeric_edit(token)?;
        self.numeric = None;
        if let Some(model) = model {
            self.model = Some(model);
            self.selection.clear();
            self.hover = None;
        }
        self.changed();
        Ok(())
    }
    pub fn numeric_active(&self) -> bool {
        self.numeric.is_some()
    }
    pub fn cancel_numeric(&mut self) {
        self.numeric = None;
        self.cancel_interactions();
    }
    pub fn radius_tool_active(&self) -> bool {
        self.contacts.radius_tool
    }
    pub(crate) fn display_model(&self) -> Option<&Arc<SceneSnapshot>> {
        self.modelling
            .as_ref()
            .and_then(|e| e.preview.as_ref().or(e.last_preview.as_ref()))
            .or(self.numeric.as_ref().and_then(|e| e.preview.as_ref()))
            .or(self.contacts.preview.as_ref())
            .or(self.model.as_ref())
    }
}
