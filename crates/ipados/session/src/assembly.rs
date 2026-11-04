//! Assembly presentation over native component/occurrence and mechanism APIs.
//! Every placement and solve belongs to the shared Session and native document.
use solidcraft_engine::geom::prelude::*;
use std::collections::BTreeMap;

use serde_json::{Value, json};
use solidcore_record::Vec3;
use solidcraft_engine::{Sel, occurrences::OccurrencePlacement};

use crate::{AdapterError, ModelEdit, PreparedModel, SessionAdapter, error};

pub(crate) const TOOLS: &[&str] = &[
    "component.create",
    "component.rename",
    "component.from_bodies",
    "component.move_bodies",
    "occurrence.copy",
    "component.paste_new",
    "occurrence.move",
    "occurrence.drag",
    "occurrence.ground",
    "component.capture_position",
    "component.revert_position",
    "joint.origin",
    "joint.create",
    "joint.as_built",
    "joint.edit",
    "joint.limits",
    "joint.drive",
    "joint.reach",
];
pub(crate) fn supported(id: &str) -> bool {
    TOOLS.contains(&id)
}
fn option(label: impl Into<String>, value: Value) -> Value {
    json!({"label":label.into(),"value":value})
}
fn field(key: &str, kind: &str, options: Vec<Value>) -> Value {
    json!({"key":key,"label":format!("ui.ipados.assembly.field.{}",key.replace('.',"_")),"kind":kind,"options":options})
}
fn choices(values: &[&str]) -> Vec<Value> {
    values.iter().map(|v| option(*v, json!(v))).collect()
}
pub(crate) fn path_id(path: &[u64]) -> String {
    format!("occurrence:{}", path.iter().map(u64::to_string).collect::<Vec<_>>().join("/"))
}

impl SessionAdapter {
    pub(crate) fn assembly_context_change(&self, before: &Value, after: &mut Value) {
        let joint = before.get("joint") != after.get("joint");
        let index = before.get("index") != after.get("index");
        let kind = before.get("type") != after.get("type");
        let mut keys = Vec::new();
        if joint {
            keys.extend(["type", "name", "offset", "angle", "flip", "value", "min", "max", "index", "a_ref", "b_ref"].map(str::to_string));
        }
        if index {
            keys.extend(["min", "max"].map(str::to_string));
        }
        if joint || kind {
            for i in 0..3 {
                keys.extend([format!("q_{i}"), format!("lo_{i}"), format!("hi_{i}")]);
            }
        }
        for key in keys {
            if before.get(&key) == after.get(&key)
                && let Some(map) = after.as_object_mut()
            {
                map.remove(&key);
            }
        }
    }
    pub(crate) fn assembly_browser(&self) -> Result<(Vec<Value>, Value), AdapterError> {
        let placements = self.assembly_placements()?;
        let mut rows = Vec::new();
        for placement in &placements {
            let scope = self.session.occurrence_scope_count(&placement.path)?;
            let context = json!({"occurrence":placement.occurrence,"component":placement.component,"path":placement.path});
            let action = |label: &str, operation: &str, params: Value| json!({"label":format!("ui.ipados.assembly.{label}"),"operation":operation,"params":params});
            let mut actions = vec![
                json!({"label":"ui.ipados.assembly.activate","command":"component.activate","params":{"component":placement.component,"occurrence":placement.occurrence,"path":placement.path}}),
                action("move", "occurrence.move", json!({"placement":context})),
                action("linked_copy", "occurrence.copy", json!({"placement":context})),
                action("independent_copy", "component.paste_new", json!({"placement":context})),
                action("ground", "occurrence.ground", json!({"placement":context,"grounded":!placement.grounded})),
                json!({"label":"ui.ipados.assembly.visibility","command":"browser.visibility","params":{"paths":[placement.path],"visible":!self.session.occurrence_visible(&placement.path)}}),
                json!({"label":"ui.ipados.assembly.isolate","command":"browser.visibility","params":{"paths":[placement.path],"visible":true,"isolate":true}}),
            ];
            if self.session.pending_moves.contains_key(&placement.occurrence) {
                actions.extend([
                json!({"label":"ui.ipados.assembly.capture","command":"component.capture_position","params":{"occurrences":[placement.occurrence]}}),
                json!({"label":"ui.ipados.assembly.revert","command":"component.revert_position","params":{"occurrences":[placement.occurrence]}}),
            ]);
            }
            let diagnostic = if scope > 1 {
                Some(format!("This nested definition is shared by {scope} placements. Placement changes affect all of them."))
            } else {
                None
            };
            let parent = if placement.path.len() == 1 { "component:0".to_string() } else { path_id(&placement.path[..placement.path.len() - 1]) };
            rows.push(json!({"id":path_id(&placement.path),"parent_id":parent,"label":placement.name,"kind":"occurrence","entity":Sel::Occurrence{path:placement.path.clone()},"selected":false,
                "assembly":{"component":placement.component,"path":placement.path,"active":self.session.active_occurrence_path.as_ref()==Some(&placement.path),"grounded":placement.grounded,"visible":self.session.occurrence_visible(&placement.path),"pending":self.session.pending_moves.contains_key(&placement.occurrence),"diagnostic":diagnostic,"actions":actions}}));
        }
        let mut scratch = self.session.scratch();
        let diagnostics = if self.session.doc.assembly.joints.is_empty() {
            json!({"joints":[],"dof":[],"fixed":[],"conflicts":[],"errors":[],"floating":[],"links":[]})
        } else {
            scratch.execute("joint.list", &json!({}))?
        };
        let inspections =
            if self.session.doc.assembly.joints.is_empty() { json!({"joints":[]}) } else { scratch.execute("joint.inspect", &json!({}))? };
        for joint in inspections["joints"].as_array().into_iter().flatten() {
            let id = joint["id"].clone();
            let name = joint["name"].as_str().unwrap_or("Joint");
            let links = joint["links"].as_array().map_or(0, Vec::len);
            let mut actions = vec![
                json!({"label":"ui.ipados.assembly.edit_joint","operation":"joint.edit","params":{"joint":id}}),
                json!({"label":"ui.ipados.assembly.drive_joint","operation":"joint.drive","params":{"joint":id}}),
                json!({"label":"ui.ipados.assembly.limits","operation":"joint.limits","params":{"joint":id}}),
                json!({"label":"ui.ipados.assembly.suppress","operation":"joint.edit","params":{"joint":id,"suppressed":joint["suppressed"]!=true}}),
                json!({"label":"ui.ipados.assembly.delete_joint","command":"joint.delete","params":{"joint":id},"confirm":format!("Delete {name}? The native command also removes {links} connected motion links. Existing motion studies are preserved and may need repair.")}),
            ];
            let endpoint_paths: Vec<_> = ["a", "b"]
                .into_iter()
                .filter_map(|side| joint[side]["occurrence"].as_u64())
                .filter_map(|id| {
                    placements
                        .iter()
                        .filter(|p| self.session.occurrence_id_scope_count(p.occurrence).is_ok_and(|n| n == 1))
                        .find(|p| p.occurrence == id)
                        .map(|p| json!(Sel::Occurrence { path: p.path.clone() }))
                })
                .collect();
            if !endpoint_paths.is_empty() {
                actions.push(json!({"label":"ui.ipados.assembly.highlight","command":"select.entities","params":{"items":endpoint_paths}}));
            }
            let errors = joint["errors"]
                .as_array()
                .into_iter()
                .flatten()
                .chain(joint["conflicts"].as_array().into_iter().flatten())
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join("; ");
            rows.push(json!({"id":format!("joint:{id}"),"parent_id":"component:0","label":name,"kind":"joint","entity":null,"selected":false,"assembly":{"joint":id,"joint_type":joint["type"],"suppressed":joint["suppressed"],"visible":joint["visible"],"diagnostic":errors,"actions":actions}}));
        }
        let mut messages = Vec::new();
        for key in ["errors", "conflicts"] {
            for item in diagnostics[key].as_array().into_iter().flatten() {
                messages.push(format!(
                    "{}: {}",
                    item["name"].as_str().unwrap_or("Joint"),
                    item["why"].as_str().unwrap_or("Assembly solve needs attention.")
                ));
            }
        }
        for chain in diagnostics["floating"].as_array().into_iter().flatten() {
            messages.push(format!("Floating mechanism: {}", chain["occurrences"]));
        }
        for dof in diagnostics["dof"].as_array().into_iter().flatten() {
            messages.push(format!("Occurrence {}: {} degrees of freedom", dof["occurrence"], dof["dof"]));
        }
        let active_name = if self.session.active_component == 0 {
            self.session.doc.name.clone()
        } else {
            self.session.doc.components.iter().find(|c| c.id == self.session.active_component).map(|c| c.name.clone()).unwrap_or_default()
        };
        let state = json!({"active_component":self.session.active_component,"active_name":active_name,"active_path":self.session.active_occurrence_path.clone().unwrap_or_default(),"tools":TOOLS.iter().map(|id|json!({"id":id,"label":format!("cmd.{id}")})).collect::<Vec<_>>(),"diagnostics":messages,
            "actions":[{"label":"ui.ipados.assembly.root","command":"component.activate","params":{"component":0}},{"label":"ui.ipados.assembly.show_all","command":"browser.visibility","params":{"folder":"bodies","visible":true}}]});
        Ok((rows, state))
    }
    pub(crate) fn assembly_placements(&self) -> Result<Vec<OccurrencePlacement>, AdapterError> {
        let mut result = Vec::new();
        let mut pending = self.session.occurrence_children(&[])?;
        while let Some(placement) = pending.pop() {
            if result.len() >= 100_000 {
                return Err(error("assembly_limit", "Assembly exceeds the native placement budget."));
            }
            pending.extend(self.session.occurrence_children(&placement.path)?);
            result.push(placement);
        }
        Ok(result)
    }
    pub(crate) fn assembly_body_paths(&self) -> Result<BTreeMap<String, Vec<u64>>, AdapterError> {
        Ok(self.assembly_placements()?.into_iter().flat_map(|p| p.bodies.into_iter().map(move |b| (b.world, p.path.clone()))).collect())
    }
    pub(crate) fn assembly_form(&self, edit: &mut ModelEdit, selection: &[Sel]) -> Result<(), AdapterError> {
        let placements = self.assembly_placements()?;
        let placement_options: Vec<_> = placements
            .iter()
            .map(|p| option(format!("{} · {:?}", p.name, p.path), json!({"occurrence":p.occurrence,"component":p.component,"path":p.path})))
            .collect();
        let mut components = vec![option(self.session.doc.name.clone(), json!({"component":0,"path":[]}))];
        components.extend(placements.iter().map(|p| option(format!("{} · {:?}", p.name, p.path), json!({"component":p.component,"path":p.path}))));
        let bodies: Vec<_> = self.session.world_state()?.bodies.iter().map(|b| option(b.name.clone(), json!(b.name))).collect();
        let selected = selection.iter().find_map(|s| match s {
            Sel::Occurrence { path } => self.session.occurrence_placement(path).ok(),
            Sel::Body { name } | Sel::Face { body: name, .. } | Sel::Edge { body: name, .. } => {
                placements.iter().find(|p| p.bodies.iter().any(|b| &b.world == name)).cloned()
            }
            _ => None,
        });
        let f = &mut edit.fields;
        let p = &mut edit.params;
        match edit.operation.as_str() {
            "component.create" => {
                p["name"] = json!("Component");
                let active_path = self.session.active_occurrence_path.clone().or_else(|| {
                    let matching: Vec<_> = placements.iter().filter(|p| p.component == self.session.active_component).collect();
                    if matching.len() == 1 { matching.first().map(|p| p.path.clone()) } else { None }
                });
                if self.session.active_component == 0 {
                    p["parent_context"] = json!({"component":0,"path":[]});
                } else if let Some(path) = active_path {
                    p["parent_context"] = json!({"component":self.session.active_component,"path":path});
                }
                p["activate"] = json!(true);
                f.push(field("name", "text", Vec::new()));
                f.push(field("parent_context", "reference", components));
                f.push(field("activate", "choice", vec![option("Activate", json!(true)), option("Keep context", json!(false))]));
            }
            "component.rename" => {
                f.push(field("component", "reference", self.session.doc.components.iter().map(|c| option(c.name.clone(), json!(c.id))).collect()));
                f.push(field("name", "text", Vec::new()));
            }
            "component.from_bodies" | "component.move_bodies" => {
                p["bodies"] = json!(
                    selection
                        .iter()
                        .filter_map(|s| match s {
                            Sel::Body { name } => Some(name),
                            _ => None,
                        })
                        .collect::<Vec<_>>()
                );
                f.push(field("bodies", "references", bodies));
                if edit.operation == "component.move_bodies" {
                    f.push(field("destination", "reference", components));
                }
            }
            "occurrence.copy" | "component.paste_new" | "occurrence.move" | "occurrence.drag" | "occurrence.ground" => {
                f.push(field("placement", "reference", placement_options));
                if let Some(selected) = &selected {
                    p["placement"] = json!({"occurrence":selected.occurrence,"component":selected.component,"path":selected.path});
                }
                if edit.operation == "occurrence.ground" {
                    p["grounded"] = json!(selected.as_ref().is_none_or(|s| !s.grounded));
                    f.push(field("grounded", "choice", vec![option("Ground", json!(true)), option("Unground", json!(false))]));
                } else if edit.operation == "occurrence.drag" {
                    let origin = selected.as_ref().map(|s| [s.world[3][0], s.world[3][1], s.world[3][2]]).unwrap_or([0.; 3]);
                    for (i, axis) in ["x", "y", "z"].into_iter().enumerate() {
                        p[format!("at_{axis}")] = json!(format!("{} mm", origin[i]));
                        p[format!("point_{axis}")] = json!(format!("{} mm", origin[i]));
                        f.push(field(&format!("at_{axis}"), "expression", Vec::new()));
                        f.push(field(&format!("point_{axis}"), "expression", Vec::new()));
                    }
                } else {
                    for axis in ["x", "y", "z"] {
                        let key = format!("translate_{axis}");
                        p[&key] = json!("0 mm");
                        f.push(field(&key, "expression", Vec::new()));
                    }
                    if edit.operation == "occurrence.move" {
                        p["space"] = json!("world");
                        p["axis"] = json!([0, 0, 1]);
                        p["angle"] = json!("0 deg");
                        p["capture"] = json!(true);
                        f.push(field("space", "choice", choices(&["world", "local"])));
                        f.push(field(
                            "axis",
                            "choice",
                            vec![option("X", json!([1, 0, 0])), option("Y", json!([0, 1, 0])), option("Z", json!([0, 0, 1]))],
                        ));
                        f.push(field("angle", "expression", Vec::new()));
                        for axis in ["x", "y", "z"] {
                            let key = format!("pivot_{axis}");
                            p[&key] = json!("0 mm");
                            f.push(field(&key, "expression", Vec::new()));
                        }
                        f.push(field(
                            "capture",
                            "choice",
                            vec![option("Capture this placement", json!(true)), option("Leave pending", json!(false))],
                        ));
                    } else {
                        p["in_place"] = json!(true);
                    }
                }
                if matches!(edit.operation.as_str(), "occurrence.move" | "occurrence.drag") {
                    p["handle_axis"] = json!([1, 0, 0]);
                    f.push(field(
                        "handle_axis",
                        "choice",
                        vec![option("X", json!([1, 0, 0])), option("Y", json!([0, 1, 0])), option("Z", json!([0, 0, 1]))],
                    ));
                    if edit.operation == "occurrence.move" {
                        p["motion"] = json!("translation");
                        f.push(field("motion", "choice", choices(&["translation", "rotation"])));
                    }
                }
            }
            "component.capture_position" | "component.revert_position" => {
                let pending: Vec<_> = placements
                    .iter()
                    .filter(|v| self.session.pending_moves.contains_key(&v.occurrence))
                    .map(|v| option(format!("{} · {:?}", v.name, v.path), json!(v.occurrence)))
                    .collect();
                p["occurrences"] = json!([]);
                f.push(field("occurrences", "references", pending));
            }
            id if id.starts_with("joint.") => self.joint_form(edit, selection, &placements)?,
            _ => {}
        }
        Ok(())
    }
    fn assembly_params(&self, id: &str, params: &Value) -> Result<Value, AdapterError> {
        let mut p = params.clone();
        let map = p.as_object_mut().ok_or_else(|| error("bad_params", "Assembly parameters must be an object."))?;
        if let Some(value) = map.remove("placement") {
            let path: Vec<u64> = serde_json::from_value(value["path"].clone()).map_err(|e| error("selection", e.to_string()))?;
            let placement = self.session.occurrence_placement(&path)?;
            if value["occurrence"].as_u64() != Some(placement.occurrence) {
                return Err(error("selection", "The occurrence context changed."));
            }
            map.insert("occurrence".into(), json!(placement.occurrence));
            map.insert("path".into(), json!(path));
            if id == "component.paste_new" {
                map.insert("component".into(), json!(placement.component));
            }
        }
        for (key, parent_key, path_key) in [("parent_context", "parent", "parent_path"), ("destination", "component", "component_path")] {
            if let Some(value) = p.as_object_mut().and_then(|m| m.remove(key)) {
                p[parent_key] = value["component"].clone();
                p[path_key] = value["path"].clone();
            }
        }
        for (prefix, target) in [("translate", "translate"), ("pivot", "origin"), ("at", "at"), ("point", "point")] {
            if p.get(format!("{prefix}_x")).is_some() {
                let mut vector = Vec::new();
                for axis in ["x", "y", "z"] {
                    let key = format!("{prefix}_{axis}");
                    let value =
                        p.as_object_mut().and_then(|m| m.remove(&key)).ok_or_else(|| error("bad_params", "A complete vector is required."))?;
                    let expr = match value {
                        Value::String(s) => s,
                        Value::Number(n) => n.to_string(),
                        _ => return Err(error("bad_params", "Vector coordinates need length expressions.")),
                    };
                    vector.push(
                        self.session.doc.eval(&expr, solidcraft_engine::doc::expr::Kind::Length).map_err(|e| error("expression", e.to_string()))?,
                    );
                }
                p[target] = json!(vector);
            }
        }
        if id == "occurrence.move" && p["move_grounded"] == true {
            return Err(error("grounded", "Unground the occurrence explicitly before moving it."));
        }
        if id == "occurrence.move"
            && p["occurrence"]
                .as_u64()
                .is_some_and(|id| self.session.doc.assembly.joints.iter().any(|j| !j.suppressed && (j.a.occurrence == id || j.b.occurrence == id)))
        {
            return Err(error("constrained", "This occurrence is jointed. Use Drive joint, constrained reach, or Drag for a floating chain."));
        }
        if let Some(m) = p.as_object_mut() {
            m.remove("handle_axis");
            m.remove("motion");
        }
        if id.starts_with("joint.") {
            self.joint_params(id, &mut p)?;
        }
        Ok(p)
    }
    fn origin_options(&self, selection: &[Sel], placements: &[OccurrencePlacement]) -> Vec<Value> {
        let mut options = vec![option("World origin · fixed frame", json!({"occurrence":0,"frame":{"origin":[0,0,0],"z":[0,0,1],"x":[1,0,0]}}))];
        for placement in placements.iter().filter(|p| self.session.occurrence_scope_count(&p.path).is_ok_and(|n| n == 1)) {
            options.push(option(format!("{} · component origin · {:?}",placement.name,placement.path),json!({"occurrence":placement.occurrence,"path":placement.path,"frame":{"origin":placement.world[3][..3],"z":placement.world[2][..3],"x":placement.world[0][..3]}})));
            for selected in selection {
                let (body, kind, point) = match selected {
                    Sel::Face { body, point, .. } => (body, "face", point),
                    Sel::Edge { body, point, .. } => (body, "edge", point),
                    Sel::Vertex { body, point } => (body, "point", point),
                    _ => continue,
                };
                if !placement.bodies.iter().any(|b| &b.world == body) {
                    continue;
                }
                let mut value = json!({"occurrence":placement.occurrence,"path":placement.path});
                value[kind] = json!(point);
                options.push(option(format!("{} · selected {kind}", placement.name), value.clone()));
                if kind == "face" {
                    value.as_object_mut().map(|m| m.remove("face"));
                    value["circle"] = json!(point);
                    options.push(option(format!("{} · selected circular face centre", placement.name), value));
                }
            }
        }
        options.extend(
            self.session
                .doc
                .assembly
                .origins
                .iter()
                .filter(|o| o.origin.occurrence == 0 || self.session.occurrence_id_scope_count(o.origin.occurrence).is_ok_and(|n| n == 1))
                .map(|o| option(format!("Named origin · {}", o.name), json!(o.name))),
        );
        options
    }
    fn joint_form(&self, edit: &mut ModelEdit, selection: &[Sel], placements: &[OccurrencePlacement]) -> Result<(), AdapterError> {
        let origins = self.origin_options(selection, placements);
        let placement_options: Vec<_> = placements
            .iter()
            .filter(|p| self.session.occurrence_scope_count(&p.path).is_ok_and(|n| n == 1))
            .map(|p| option(format!("{} · {:?}", p.name, p.path), json!({"occurrence":p.occurrence,"path":p.path,"component":p.component})))
            .collect();
        let joint_options: Vec<_> = self.session.doc.assembly.joints.iter().map(|j| option(j.name.clone(), json!(j.id))).collect();
        let joint = edit.params["joint"].as_u64().and_then(|id| self.session.doc.assembly.joints.iter().find(|j| j.id == id));
        let f = &mut edit.fields;
        let p = &mut edit.params;
        if matches!(edit.operation.as_str(), "joint.create" | "joint.as_built" | "joint.edit") {
            if p.get("type").is_none() {
                p["type"] = joint.map(|j| json!(j.kind)).unwrap_or(json!("rigid"));
            }
            f.push(field("type", "choice", choices(&["rigid", "revolute", "slider", "cylindrical", "pin_slot", "planar", "ball"])));
            if p.get("name").is_none() {
                p["name"] = json!(joint.map(|j| j.name.as_str()).unwrap_or("Joint"));
            }
            f.push(field("name", "text", Vec::new()));
        }
        if edit.operation == "joint.create" {
            let kind = p["type"]
                .as_str()
                .and_then(solidcraft_engine::doc::joints::JointKind::parse)
                .ok_or_else(|| error("joint_type", "Choose a native joint type."))?;
            p["initial_values"] = json!(true);
            if p.get("limits_enabled").is_none() {
                p["limits_enabled"] = json!(false);
            }
            f.push(field("limits_enabled", "choice", vec![option("No limits", json!(false)), option("Use limits", json!(true))]));
            for (i, name) in kind.dofs().iter().enumerate() {
                let units = if kind.is_angle(i) { "deg" } else { "mm" };
                for (key, value) in [
                    (format!("q_{i}"), 0.),
                    (format!("lo_{i}"), if kind.is_angle(i) { -180. } else { 0. }),
                    (format!("hi_{i}"), if kind.is_angle(i) { 180. } else { 50. }),
                ] {
                    if p.get(&key).is_none() {
                        p[&key] = json!(format!("{value} {units}"));
                    }
                    let mut control = field(&key, "expression", Vec::new());
                    control["units"] = json!(units);
                    f.push(control);
                }
                edit.feedback.push(format!("Joint value {i}: {name} ({units})."));
            }
        }
        match edit.operation.as_str() {
            "joint.origin" => {
                if p.get("name").is_none() {
                    p["name"] = json!("Origin");
                }
                f.push(field("name", "text", Vec::new()));
                f.push(field("origin_ref", "reference", origins));
            }
            "joint.create" | "joint.edit" => {
                if edit.operation == "joint.edit" {
                    f.push(field("joint", "reference", joint_options));
                    if p.get("suppressed").is_none() {
                        p["suppressed"] = json!(joint.is_some_and(|j| j.suppressed));
                    }
                    f.push(field("suppressed", "choice", vec![option("Enabled", json!(false)), option("Suppressed", json!(true))]));
                }
                for side in ["a", "b"] {
                    let key = format!("{side}_mode");
                    if p.get(&key).is_none() {
                        p[&key] = json!(if edit.operation == "joint.edit" { "keep" } else { "reference" });
                    }
                    f.push(field(
                        &key,
                        "choice",
                        choices(if edit.operation == "joint.edit" { &["keep", "reference", "frame"] } else { &["reference", "frame"] }),
                    ));
                    f.push(field(&format!("{side}_ref"), "reference", origins.clone()));
                    f.push(field(&format!("{side}_placement"), "reference", placement_options.clone()));
                    for axis in ["x", "y", "z"] {
                        let key = format!("{side}_point_{axis}");
                        if p.get(&key).is_none() {
                            p[&key] = json!("0 mm");
                        }
                        f.push(field(&key, "expression", Vec::new()));
                    }
                    let key = format!("{side}_z");
                    if p.get(&key).is_none() {
                        p[&key] = json!([0, 0, 1]);
                    }
                    f.push(field(
                        &key,
                        "choice",
                        vec![
                            option("+X", json!([1, 0, 0])),
                            option("+Y", json!([0, 1, 0])),
                            option("+Z", json!([0, 0, 1])),
                            option("−X", json!([-1, 0, 0])),
                            option("−Y", json!([0, -1, 0])),
                            option("−Z", json!([0, 0, -1])),
                        ],
                    ));
                }
                if p.get("flip").is_none() {
                    p["flip"] = json!(joint.is_none_or(|j| j.flip));
                }
                f.push(field("flip", "choice", vec![option("Along the same axis", json!(true)), option("Mate opposing axes", json!(false))]));
                for (key, units, value) in
                    [("offset", "mm", joint.map_or(0., |j| j.offset)), ("angle", "deg", joint.map_or(0., |j| j.angle.to_degrees()))]
                {
                    if p.get(key).is_none() {
                        p[key] = json!(
                            joint
                                .and_then(|j| if key == "offset" { j.exprs.offset.clone() } else { j.exprs.angle.clone() })
                                .unwrap_or_else(|| format!("{value} {units}"))
                        );
                    }
                    f.push(field(key, "expression", Vec::new()));
                }
            }
            "joint.as_built" => {
                f.push(field("a_placement", "reference", placement_options.clone()));
                f.push(field("b_placement", "reference", placement_options));
                f.push(field("at_ref", "reference", origins));
            }
            "joint.drive" | "joint.limits" => {
                f.push(field("joint", "reference", joint_options));
                if let Some(joint) = joint {
                    if edit.operation == "joint.drive" {
                        let multiple = joint.kind.dofs().len() > 1;
                        if multiple {
                            p["drive_values"] = json!(true);
                        }
                        for (index, name) in joint.kind.dofs().iter().enumerate() {
                            let angle = joint.kind.is_angle(index);
                            let units = if angle { "deg" } else { "mm" };
                            let value = joint.values.get(index).copied().unwrap_or(0.) * if angle { 180. / std::f64::consts::PI } else { 1. };
                            let key = if multiple { format!("q_{index}") } else { "value".into() };
                            if p.get(&key).is_none() {
                                p[&key] = json!(format!("{value} {units}"));
                            }
                            let limits = joint
                                .limits
                                .get(index)
                                .copied()
                                .flatten()
                                .map(|(a, b)| if angle { (a.to_degrees(), b.to_degrees()) } else { (a, b) })
                                .unwrap_or((value - if angle { 180. } else { 100. }, value + if angle { 180. } else { 100. }));
                            let mut control = field(&key, "slider", Vec::new());
                            control["label"] = json!(format!("{} ({units})", name));
                            control["min"] = json!(limits.0);
                            control["max"] = json!(limits.1);
                            control["value"] = json!(value);
                            control["units"] = json!(units);
                            f.push(control);
                        }
                    } else {
                        if p.get("index").is_none() {
                            p["index"] = json!(0);
                        }
                        f.push(field("index", "choice", joint.kind.dofs().iter().enumerate().map(|(i, name)| option(*name, json!(i))).collect()));
                        if p.get("clear").is_none() {
                            p["clear"] = json!(false);
                        }
                        f.push(field("clear", "choice", vec![option("Set limits", json!(false)), option("Clear limits", json!(true))]));
                        let index = p["index"].as_u64().unwrap_or(0) as usize;
                        let angle = joint.kind.is_angle(index);
                        let units = if angle { "deg" } else { "mm" };
                        let limits = joint.limits.get(index).copied().flatten().unwrap_or((0., if angle { std::f64::consts::PI } else { 50. }));
                        for (key, value) in [("min", limits.0), ("max", limits.1)] {
                            if p.get(key).is_none() {
                                p[key] = json!(format!("{} {units}", if angle { value.to_degrees() } else { value }));
                            }
                            f.push(field(key, "expression", Vec::new()));
                        }
                    }
                }
            }
            "joint.reach" => {
                f.push(field("placement", "reference", placement_options));
                if let Some(path) = selection.iter().find_map(|s| match s {
                    Sel::Occurrence { path } => Some(path),
                    _ => None,
                }) {
                    let v = self.session.occurrence_placement(path)?;
                    p["placement"] = json!({"occurrence":v.occurrence,"path":v.path});
                    for (i, axis) in ["x", "y", "z"].iter().enumerate() {
                        if p.get(format!("at_{axis}")).is_none() {
                            p[format!("at_{axis}")] = json!(format!("{} mm", v.world[3][i]));
                            p[format!("point_{axis}")] = p[format!("at_{axis}")].clone();
                        }
                    }
                }
                for prefix in ["at", "point"] {
                    for axis in ["x", "y", "z"] {
                        f.push(field(&format!("{prefix}_{axis}"), "expression", Vec::new()));
                    }
                }
            }
            _ => {}
        }
        if let Some(joint) = joint {
            for (name, origin) in [("A", &joint.a), ("B", &joint.b)] {
                match self.session.doc.origin_world(&self.session.model.state(), origin) {
                    Ok(frame) => edit.feedback.push(format!(
                        "{name}: occurrence {}, origin {:?}, axis {:?}",
                        origin.occurrence,
                        &frame[3][..3],
                        &frame[2][..3]
                    )),
                    Err(e) => edit.feedback.push(format!("{name}: {e}")),
                }
            }
        }
        Ok(())
    }
    fn joint_params(&self, id: &str, p: &mut Value) -> Result<(), AdapterError> {
        if id == "joint.origin"
            && let Some(origin) = p.as_object_mut().and_then(|m| m.remove("origin_ref"))
        {
            let fields = origin.as_object().ok_or_else(|| error("joint_reference", "Choose a geometric or explicit origin frame."))?;
            for (key, value) in fields {
                p[key] = value.clone();
            }
        }
        for side in ["a", "b"] {
            let mode = p.as_object_mut().and_then(|m| m.remove(&format!("{side}_mode")));
            let reference = p.as_object_mut().and_then(|m| m.remove(&format!("{side}_ref")));
            let placement = p.as_object_mut().and_then(|m| m.remove(&format!("{side}_placement")));
            let mut point = Vec::new();
            for axis in ["x", "y", "z"] {
                if let Some(value) = p.as_object_mut().and_then(|m| m.remove(&format!("{side}_point_{axis}"))) {
                    let expression = match value {
                        Value::String(v) => v,
                        Value::Number(v) => v.to_string(),
                        _ => return Err(error("expression", "Enter a world coordinate.")),
                    };
                    point.push(
                        self.session
                            .doc
                            .eval(&expression, solidcraft_engine::doc::expr::Kind::Length)
                            .map_err(|e| error("expression", e.to_string()))?,
                    );
                }
            }
            let z = p.as_object_mut().and_then(|m| m.remove(&format!("{side}_z")));
            if id == "joint.as_built" {
                if let Some(placement) = placement {
                    p[side] = placement["occurrence"].clone();
                    self.validate_joint_occurrence(&placement)?;
                }
                continue;
            }
            match mode.as_ref().and_then(Value::as_str) {
                Some("keep") => {}
                Some("reference") => {
                    p[side] = reference.ok_or_else(|| error("joint_reference", format!("Choose the {side} reference.")))?;
                }
                Some("frame") => {
                    let placement = placement.ok_or_else(|| error("joint_reference", format!("Choose the {side} occurrence.")))?;
                    if point.len() != 3 {
                        return Err(error("joint_reference", "A joint frame needs three origin coordinates."));
                    }
                    p[side] = json!({"occurrence":placement["occurrence"],"path":placement["path"],"point":point,"z":z.unwrap_or(json!([0,0,1]))});
                }
                None => {}
                _ => return Err(error("joint_reference", "Unsupported origin mode.")),
            }
        }
        if let Some(at) = p.as_object_mut().and_then(|m| m.remove("at_ref")) {
            p["at"] = at;
        }
        if p.as_object_mut().and_then(|m| m.remove("drive_values")) == Some(json!(true)) {
            let joint = self.find_joint(&p["joint"])?;
            let mut values = Vec::new();
            for i in 0..joint.kind.dofs().len() {
                values.push(
                    p.as_object_mut().and_then(|m| m.remove(&format!("q_{i}"))).ok_or_else(|| error("joint_value", "Enter each joint value."))?,
                );
            }
            p["values"] = json!(values);
        }
        if p.as_object_mut().and_then(|m| m.remove("initial_values")) == Some(json!(true)) {
            let kind = p["type"]
                .as_str()
                .and_then(solidcraft_engine::doc::joints::JointKind::parse)
                .ok_or_else(|| error("joint_type", "Choose a native joint type."))?;
            let enabled = p.as_object_mut().and_then(|m| m.remove("limits_enabled")) == Some(json!(true));
            let mut values = Vec::new();
            let mut limits = Vec::new();
            for i in 0..kind.dofs().len() {
                values.push(
                    p.as_object_mut()
                        .and_then(|m| m.remove(&format!("q_{i}")))
                        .ok_or_else(|| error("joint_value", "Enter the initial joint value."))?,
                );
                let lo = p.as_object_mut().and_then(|m| m.remove(&format!("lo_{i}")));
                let hi = p.as_object_mut().and_then(|m| m.remove(&format!("hi_{i}")));
                if enabled {
                    let lo = lo.ok_or_else(|| error("joint_limits", "Enter both limit bounds."))?;
                    let hi = hi.ok_or_else(|| error("joint_limits", "Enter both limit bounds."))?;
                    if self.joint_number(&lo, kind.is_angle(i))? > self.joint_number(&hi, kind.is_angle(i))? {
                        return Err(error("joint_limits", "The minimum limit exceeds the maximum."));
                    }
                    limits.push(json!([lo, hi]));
                }
            }
            p["values"] = json!(values);
            if enabled {
                p["limits"] = json!(limits);
            }
        }
        if p.as_object_mut().and_then(|m| m.remove("clear")) == Some(json!(true))
            && let Some(m) = p.as_object_mut()
        {
            m.remove("min");
            m.remove("max");
        }
        if matches!(id, "joint.create" | "joint.as_built" | "joint.origin" | "joint.edit" | "joint.reach") {
            for key in if matches!(id, "joint.origin" | "joint.reach") { vec![""] } else { vec!["a", "b", "at"] } {
                let value = if key.is_empty() { &*p } else { &p[key] };
                if value.is_null() {
                    continue;
                }
                self.validate_joint_occurrence(value)?;
            }
        }
        if matches!(id, "joint.drive" | "joint.edit" | "joint.limits") {
            let joint = self.find_joint(&p["joint"])?;
            self.validate_mechanism_pending(&[joint.a.occurrence, joint.b.occurrence], Some(joint.id))?;
        }
        if id == "joint.reach"
            && let Some(id) = p["occurrence"].as_u64()
        {
            self.validate_mechanism_pending(&[id], None)?;
        }
        if id == "joint.limits" && p.get("min").is_some() && p.get("max").is_some() {
            let joint = self.find_joint(&p["joint"])?;
            let index = p["index"].as_u64().unwrap_or(0) as usize;
            let angle = joint.kind.is_angle(index);
            let evaluate = |v: &Value| -> Result<f64, AdapterError> {
                match v {
                    Value::String(s) => self
                        .session
                        .doc
                        .eval(s, if angle { solidcraft_engine::doc::expr::Kind::Angle } else { solidcraft_engine::doc::expr::Kind::Length })
                        .map_err(|e| error("expression", e.to_string())),
                    Value::Number(n) => self
                        .session
                        .doc
                        .eval(
                            &n.to_string(),
                            if angle { solidcraft_engine::doc::expr::Kind::Angle } else { solidcraft_engine::doc::expr::Kind::Length },
                        )
                        .map_err(|e| error("joint_limits", e.to_string())),
                    _ => Err(error("joint_limits", "Finite limits are required.")),
                }
            };
            if evaluate(&p["min"])? > evaluate(&p["max"])? {
                return Err(error("joint_limits", "The minimum limit exceeds the maximum."));
            }
        }
        Ok(())
    }
    fn find_joint(&self, key: &Value) -> Result<&solidcraft_engine::doc::joints::Joint, AdapterError> {
        self.session
            .doc
            .assembly
            .joints
            .iter()
            .find(|j| key.as_u64() == Some(j.id) || key.as_str() == Some(j.name.as_str()))
            .ok_or_else(|| error("joint", "Select an existing joint."))
    }
    fn joint_number(&self, value: &Value, angle: bool) -> Result<f64, AdapterError> {
        let expression = match value {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            _ => return Err(error("joint_value", "A finite numeric value or expression is required.")),
        };
        self.session
            .doc
            .eval(&expression, if angle { solidcraft_engine::doc::expr::Kind::Angle } else { solidcraft_engine::doc::expr::Kind::Length })
            .map_err(|e| error("joint_value", e.to_string()))
    }
    fn validate_mechanism_pending(&self, seed: &[u64], joint: Option<u64>) -> Result<(), AdapterError> {
        if self.session.pending_moves.is_empty() {
            return Ok(());
        }
        // A bounded dependency walk over the saved native joints/links, solely to
        // validate involved pending poses. Placement and DOF still use the solver.
        let solution = solidcraft_engine::doc::joints::solve(&self.session.doc, &self.session.model.state());
        let mut occurrences: std::collections::BTreeSet<u64> = seed.iter().copied().collect();
        let mut joints = std::collections::BTreeSet::new();
        if let Some(id) = joint {
            joints.insert(id);
        }
        loop {
            let before = (occurrences.len(), joints.len());
            for joint in self.session.doc.assembly.joints.iter().filter(|j| !j.suppressed) {
                if joints.contains(&joint.id)
                    || [joint.a.occurrence, joint.b.occurrence].iter().any(|id| occurrences.contains(id) && !solution.fixed.contains(id))
                {
                    joints.insert(joint.id);
                    occurrences.extend([joint.a.occurrence, joint.b.occurrence]);
                }
            }
            for link in &self.session.doc.assembly.links {
                if joints.contains(&link.a) || joints.contains(&link.b) {
                    joints.extend([link.a, link.b]);
                }
            }
            if before == (occurrences.len(), joints.len()) {
                break;
            }
        }
        for id in occurrences {
            self.validate_joint_occurrence(&json!(id))?;
        }
        Ok(())
    }
    fn validate_joint_occurrence(&self, value: &Value) -> Result<(), AdapterError> {
        let id = if let Some(name) = value.as_str() {
            self.session
                .doc
                .assembly
                .origins
                .iter()
                .find(|o| o.name == name)
                .map(|o| o.origin.occurrence)
                .ok_or_else(|| error("joint_reference", "The named origin no longer exists."))?
        } else if let Some(id) = value.as_u64() {
            id
        } else {
            value["occurrence"].as_u64().ok_or_else(|| error("joint_reference", "Choose an explicit occurrence for every joint origin."))?
        };
        if id == 0 {
            return Ok(());
        }
        if self.session.occurrence_id_scope_count(id)? != 1 {
            return Err(error(
                "ambiguous_joint",
                "This native joint cannot distinguish a nested definition repeated under multiple parents. Use an independent component copy for this joint.",
            ));
        }
        let placement = if let Some(path) = value.get("path") {
            let path: Vec<u64> = serde_json::from_value(path.clone()).map_err(|e| error("joint_reference", e.to_string()))?;
            self.session.occurrence_placement(&path)?
        } else {
            self.assembly_placements()?
                .into_iter()
                .find(|p| p.occurrence == id)
                .ok_or_else(|| error("joint_reference", "The occurrence no longer exists."))?
        };
        if placement.occurrence != id {
            return Err(error("joint_reference", "The occurrence and its full path disagree."));
        }
        if placement.path.iter().any(|id| self.session.pending_moves.contains_key(id)) {
            return Err(error(
                "pending_position",
                "Capture or revert the selected occurrence and its pending parents before defining a joint. Other pending positions can stay pending.",
            ));
        }
        Ok(())
    }
    pub(crate) fn prepare_assembly(&self, edit: &ModelEdit, params: Value, commit: bool) -> Result<PreparedModel, AdapterError> {
        if params.to_string().len() > 65536 {
            return Err(error("bad_params", "Assembly proposal exceeds the input budget."));
        }
        let p = self.assembly_params(&edit.operation, &params)?;
        let mut scratch = Self { session: self.session.scratch(), geometry_revision: self.geometry_revision, workspace_cache: None };
        let reply = scratch.execute(&edit.operation, p.clone())?.value;
        for key in ["errors", "conflicts"] {
            if reply[key].as_array().is_some_and(|v| !v.is_empty()) {
                return Err(error("assembly_solve", format!("{key}: {}", reply[key])));
            }
        }
        scratch.validate_joint_references()?;
        let model = scratch.snapshot()?;
        let transaction = if commit { Some(self.session.scratch().compute_commands(&[(edit.operation.clone(), p.clone())], None)?) } else { None };
        let mut feedback = Vec::new();
        if reply["moved"] == false {
            feedback.push(reply["reason"].as_str().unwrap_or("The requested value is already current; no placement moved.").to_string());
        }
        if let Some(values) = reply["values"].as_array() {
            for value in values {
                feedback.push(format!("{}: {} {}", value["name"].as_str().unwrap_or("Value"), value["value"], value["unit"].as_str().unwrap_or("")));
            }
        }
        if let Some(reached) = reply["reached"].as_bool() {
            feedback.push(format!(
                "{}; residual {} mm.",
                if reached { "Target reached" } else { "Target unreachable; nearest permitted pose shown" },
                reply["error_mm"]
            ));
        }
        for (key, label) in [
            ("stopped_by_loop", "Motion stopped by a closed-loop constraint"),
            ("stopped_by_limit", "A joint limit stopped motion"),
            ("stopped_by_contact", "Contact stopped motion"),
        ] {
            if !reply[key].is_null() && reply[key].as_array().is_none_or(|a| !a.is_empty()) {
                feedback.push(label.into());
            }
        }
        if let Some(warnings) = reply["warnings"].as_array() {
            feedback.extend(warnings.iter().filter_map(Value::as_str).map(str::to_string));
        }
        if let Some(notices) = reply["notices"].as_array() {
            feedback.extend(notices.iter().filter_map(Value::as_str).map(str::to_string));
        }
        if let Some(frozen) = reply["frozen_bodies"].as_array().filter(|v| !v.is_empty()) {
            feedback
                .push(format!("Native independent-copy base geometry: {}", frozen.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(", ")));
        }
        if let Some(id) = reply["joint"].as_u64().or_else(|| p["joint"].as_u64())
            && let Some(joint) = scratch.session.doc.assembly.joints.iter().find(|j| j.id == id)
        {
            for (side, origin) in [("A", &joint.a), ("B", &joint.b)] {
                match scratch.session.doc.origin_world(&scratch.session.model.state(), origin) {
                    Ok(frame) => feedback.push(format!("{side} frame: origin {:?} mm, axis {:?}", &frame[3][..3], &frame[2][..3])),
                    Err(e) => feedback.push(format!("{side} origin: {e}")),
                }
            }
        }
        if let Some(id) = reply["occurrence"].as_u64().or_else(|| p["occurrence"].as_u64())
            && let Ok(scope) = scratch.session.occurrence_id_scope_count(id)
            && scope > 1
        {
            feedback.push(format!("This shared nested occurrence definition affects {scope} placements; the selected path supplies its coordinate context, not independent storage."));
        }
        for placement in self.assembly_placements()? {
            if let Ok(after) = scratch.session.occurrence_placement(&placement.path)
                && placement.world.iter().flatten().zip(after.world.iter().flatten()).any(|(a, b)| (a - b).abs() > 1e-8)
            {
                feedback.push(format!("Repositioned {} · {:?}", placement.name, placement.path));
            }
        }
        let mut next = edit.clone();
        next.params = params.clone();
        next.reply = reply.clone();
        let handle = self.assembly_handle(&next);
        Ok(PreparedModel { model, params, version: edit.version, handle, transaction, reply, feedback })
    }
    pub(crate) fn assembly_handle(&self, edit: &ModelEdit) -> Option<crate::modelling::ModelHandle> {
        let p = &edit.params;
        let expression = |key: &str, angular: bool| -> Option<f64> {
            let value = p.get(key)?;
            let expression = match value {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                _ => return None,
            };
            let n = self
                .session
                .doc
                .eval(&expression, if angular { solidcraft_engine::doc::expr::Kind::Angle } else { solidcraft_engine::doc::expr::Kind::Length })
                .ok()?;
            Some(if angular { n.to_degrees() } else { n })
        };
        let vector = |prefix: &str| -> Option<Vec3> {
            Some(Vec3::new(
                expression(&format!("{prefix}_x"), false)?,
                expression(&format!("{prefix}_y"), false)?,
                expression(&format!("{prefix}_z"), false)?,
            ))
        };
        let mut angular = false;
        let mut origin;
        let mut normal;
        let distance;
        let parameter;
        let mut size = 25.;
        match edit.operation.as_str() {
            "occurrence.move" => {
                let path: Vec<u64> =
                    serde_json::from_value(p.get("placement").and_then(|v| v.get("path")).or_else(|| p.get("path"))?.clone()).ok()?;
                let placement = self.session.occurrence_placement(&path).ok()?;
                if placement.grounded {
                    return None;
                }
                angular = p["motion"] == "rotation";
                if angular {
                    origin = vector("pivot")?;
                    normal = serde_json::from_value::<Vec3>(p["axis"].clone()).ok()?;
                    distance = expression("angle", true)?;
                    parameter = "angle".into();
                } else {
                    let axis = p.get("handle_axis").cloned().unwrap_or(json!([1, 0, 0]));
                    normal = serde_json::from_value::<Vec3>(axis).ok()?;
                    let key = if normal.x.abs() > 0.5 {
                        "translate_x"
                    } else if normal.y.abs() > 0.5 {
                        "translate_y"
                    } else {
                        "translate_z"
                    };
                    distance = expression(key, false)?;
                    parameter = key.into();
                    origin = Vec3::new(placement.world[3][0], placement.world[3][1], placement.world[3][2]);
                }
                if p["space"] == "local" {
                    normal = solidcraft_engine::doc::apply_vector(&placement.parent_world, normal);
                    if angular {
                        origin = solidcraft_engine::doc::apply_point(&placement.parent_world, origin);
                    }
                }
            }
            "joint.drive" => {
                let joint = self.find_joint(&p["joint"]).ok()?;
                if joint.kind.dofs().len() != 1 || joint.suppressed {
                    return None;
                }
                let frame = self.session.doc.origin_world(&self.session.model.state(), &joint.a).ok()?;
                origin = Vec3::new(frame[3][0], frame[3][1], frame[3][2]);
                normal = Vec3::new(frame[2][0], frame[2][1], frame[2][2]);
                angular = joint.kind.is_angle(0);
                parameter = "value".into();
                distance = edit.reply["values"][0]["value"].as_f64().or_else(|| expression("value", angular))?;
                size = 40.;
            }
            "joint.reach" | "occurrence.drag" => {
                let target = edit.reply.get("point").and_then(|p| serde_json::from_value::<Vec3>(p.clone()).ok()).or_else(|| vector("point"))?;
                normal = serde_json::from_value::<Vec3>(p.get("handle_axis").cloned().unwrap_or(json!([1, 0, 0]))).ok()?;
                let key = if normal.x.abs() > 0.5 {
                    "point_x"
                } else if normal.y.abs() > 0.5 {
                    "point_y"
                } else {
                    "point_z"
                };
                distance = if key == "point_x" {
                    target.x
                } else if key == "point_y" {
                    target.y
                } else {
                    target.z
                };
                parameter = key.into();
                origin = target - normal * distance;
            }
            _ => return None,
        }
        normal = normal.normalized()?;
        let arm = normal.cross(if normal.z.abs() > 0.9 { Vec3::Y } else { Vec3::Z }).normalized()?;
        Some(crate::modelling::ModelHandle {
            origin,
            normal,
            distance,
            factor: 1.,
            parameter,
            units: if angular { "deg".into() } else { "mm".into() },
            angular,
            arm,
            size,
        })
    }
}
