//! Native browser presentation over the shared document groups and visibility.
//! The host sends these command arguments back to the existing engine unchanged.
use std::collections::BTreeMap;

use serde_json::{Value, json};
use solidcraft_engine::{Session, doc::FeatureKind, view};

use crate::SessionAdapter;

#[derive(Clone)]
struct Scope {
    parent: String,
    component: u64,
    key: String,
    visible: bool,
}

fn folder_id(scope: &Scope, folder: &str) -> String {
    format!("folder:{}:{folder}", scope.parent)
}

fn folder_row(scope: &Scope, folder: &str, group: Option<&solidcraft_engine::doc::BrowserGroup>, session: &Session) -> Value {
    let suffix = group.map_or_else(|| folder.to_string(), |g| format!("g{}", g.id));
    let key = format!("{}/{suffix}", scope.key);
    let visible = !session.folder_hidden(&key);
    let ancestors_visible = group.is_none_or(|group| {
        std::iter::successors(group.parent, |id| session.doc.browser_group(*id).and_then(|g| g.parent))
            .take(solidcraft_engine::doc::MAX_GROUP_NESTING + 1)
            .all(|id| !session.folder_hidden(&format!("{}/g{id}", scope.key)))
    });
    let parent =
        group.map_or_else(|| scope.parent.clone(), |g| folder_id(scope, &g.parent.map_or_else(|| folder.to_string(), |id| format!("g{id}"))));
    json!({"id":folder_id(scope, &suffix),"parent_id":parent,
        "label":group.map_or_else(||format!("ui.ipados.browser.{folder}"),|g|g.name.clone()),"kind":"folder","entity":null,"selected":false,
        "browser":{"icon":"folder","localised":group.is_none(),"container":true,"component":scope.component,"folder":folder,
            "group":group.map(|g|g.id),"visible":visible,"drawn":visible && scope.visible && ancestors_visible && (group.is_none() || !session.folder_hidden(&format!("{}/{folder}",scope.key))),
            "visibility":{"tree_folders":[key]},"rename":group.map(|g|json!({"command":"browser.rename_group","params":{"group":g.id}}))}})
}

impl SessionAdapter {
    pub(crate) fn organise_browser(&self, rows: &mut Vec<Value>) {
        let session = &self.session;
        let placements = self.assembly_placements().unwrap_or_default();
        let component_scope = |component| {
            let path = if component == 0 {
                Vec::new()
            } else {
                session
                    .active_occurrence_path
                    .as_ref()
                    .filter(|_| session.active_component == component)
                    .cloned()
                    .unwrap_or_else(|| session.doc.first_path(component))
            };
            Scope {
                parent: format!("component:{component}"),
                component,
                key: Session::folder_scope(&path),
                visible: path.is_empty() || session.occurrence_visible(&path),
            }
        };
        let mut body_context = BTreeMap::new();
        for p in &placements {
            let scope = Scope {
                parent: crate::assembly::path_id(&p.path),
                component: p.component,
                key: Session::folder_scope(&p.path),
                visible: session.occurrence_visible(&p.path),
            };
            for body in &p.bodies {
                body_context.insert(body.world.as_str(), (scope.clone(), body.source.as_str()));
            }
        }
        let mut folders: BTreeMap<(String, String), Scope> = BTreeMap::new();
        let state = session.world_state();
        let bodies: BTreeMap<_, _> = state.bodies.iter().map(|body| (body.name.as_str(), body)).collect();
        let features: BTreeMap<_, _> = session.doc.features.iter().map(|feature| (feature.id, feature)).collect();
        let mut feature_visibility = BTreeMap::new();
        for body in &state.bodies {
            let visible = feature_visibility.entry(body.feature).or_insert(false);
            *visible |= session.body_visible(&body.name);
        }
        let mut groups = BTreeMap::new();
        for group in &session.doc.browser_groups {
            for key in group.item_keys() {
                groups.entry((group.component, group.folder.as_str(), key)).or_insert(group.id);
            }
        }
        folders.insert(("component:0".into(), "sketches".into()), component_scope(0));
        for row in rows.iter_mut() {
            let kind = row["kind"].as_str().unwrap_or("").to_string();
            let mut browser = json!({"icon":"component"});
            let mut item = None;
            match kind.as_str() {
                "body" => {
                    let name = row["label"].as_str().unwrap_or("");
                    if let Some(body) = bodies.get(name) {
                        let (scope, source) = body_context.get(name).cloned().unwrap_or_else(|| (component_scope(body.component), name));
                        browser = json!({"icon":if body.is_surface(){"surface"}else{"body"},"visible":!session.visibility.hidden_bodies.iter().any(|n|n==name),
                            "drawn":session.body_visible(name),"visibility":{"items":[name]},"rename":{"command":"body.rename","params":{"body":source}}});
                        item = Some((scope, "bodies", source.to_string()));
                    }
                }
                "sketch" | "feature" => {
                    if let Some(id) = row["entity"]["id"].as_u64()
                        && let Some(feature) = features.get(&id)
                    {
                        let (icon, folder) = match feature.kind {
                            FeatureKind::Sketch { .. } => ("sketch", Some("sketches")),
                            FeatureKind::ConstructionPlane { .. } => ("plane", Some("construction")),
                            FeatureKind::ConstructionAxis { .. } => ("axis", Some("construction")),
                            FeatureKind::ConstructionPoint { .. } => ("point", Some("construction")),
                            FeatureKind::Extrude { .. } => ("extrude", None),
                            FeatureKind::Revolve { .. } => ("revolve", None),
                            FeatureKind::Fillet { .. } => ("fillet", None),
                            FeatureKind::Chamfer { .. } => ("chamfer", None),
                            FeatureKind::Cylinder { .. } => ("cylinder", None),
                            FeatureKind::Box { .. } => ("box", None),
                            _ => ("component", None),
                        };
                        browser = json!({"icon":icon,"rename":{"command":"timeline.rename","params":{"feature":id}}});
                        if let Some(folder) = folder {
                            let visible = if folder == "sketches" {
                                !session.visibility.hidden_sketches.contains(&id)
                                    && (session.visibility.shown_sketches.contains(&id) || !view::sketch_consumed(session, id))
                            } else {
                                !session.visibility.hidden_origin.contains(&feature.name)
                            };
                            browser["visible"] = json!(visible);
                            browser["drawn"] = json!(if folder == "sketches" {
                                view::sketch_shown(session, id)
                            } else {
                                session.construction_visible(&feature.name)
                            });
                            browser["visibility"] = json!({"items":[id]});
                            item = Some((component_scope(feature.component), folder, id.to_string()));
                        } else if let Some(visible) = feature_visibility.get(&id) {
                            browser["visible"] = json!(visible);
                            browser["drawn"] = json!(visible);
                            browser["visibility"] = json!({"features":[id]});
                        }
                    }
                }
                "component" | "occurrence" => {
                    let component = row["assembly"]["component"].as_u64().unwrap_or(0);
                    browser["container"] = json!(true);
                    browser["rename"] = json!({"command":"component.rename","params":{"component":component}});
                    if kind == "occurrence" {
                        browser["visible"] = json!(!session.visibility.hidden_occurrences.iter().any(|p| json!(p) == row["assembly"]["path"]));
                        browser["drawn"] = row["assembly"]["visible"].clone();
                        browser["visibility"] = json!({"paths":[row["assembly"]["path"]]});
                    }
                }
                "joint" => {
                    let id = row["assembly"]["joint"].clone();
                    browser = json!({"icon":"joint","visible":row["assembly"]["visible"],"drawn":row["assembly"]["visible"],"visibility":{"joints":[id]},
                        "rename":{"command":"joint.edit","params":{"joint":id}}});
                }
                _ => {}
            }
            if let Some((scope, folder, key)) = item {
                let group = groups.get(&(scope.component, folder, key.as_str()));
                row["parent_id"] = json!(folder_id(&scope, &group.map_or_else(|| folder.to_string(), |id| format!("g{id}"))));
                browser["component"] = json!(scope.component);
                browser["folder"] = json!(folder);
                browser["key"] = json!(key);
                browser["group"] = json!(group);
                folders.insert((scope.parent.clone(), folder.to_string()), scope);
            }
            row["browser"] = browser;
        }
        // Retain empty user folders, including in empty component definitions.
        for group in &session.doc.browser_groups {
            let scopes: Vec<_> = if group.folder == "bodies" && group.component != 0 {
                placements
                    .iter()
                    .filter(|p| p.component == group.component)
                    .map(|p| Scope {
                        parent: crate::assembly::path_id(&p.path),
                        component: p.component,
                        key: Session::folder_scope(&p.path),
                        visible: session.occurrence_visible(&p.path),
                    })
                    .collect()
            } else {
                Vec::new()
            };
            for scope in if scopes.is_empty() { vec![component_scope(group.component)] } else { scopes } {
                folders.insert((scope.parent.clone(), group.folder.clone()), scope);
            }
        }
        for ((_, folder), scope) in folders {
            rows.push(folder_row(&scope, &folder, None, session));
            for group in session.doc.browser_groups.iter().filter(|g| g.component == scope.component && g.folder == folder) {
                rows.push(folder_row(&scope, &folder, Some(group), session));
            }
        }
        // Present containers ahead of their loose items, retaining the feature
        // timeline's relative order.
        rows.sort_by_key(|row| row["kind"] != "folder");
    }
}
