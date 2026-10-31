use super::*;

// The production registry deliberately caps eight live sessions. Cargo can run
// more ABI tests concurrently in one process, unlike the repository test runner.
static ABI_TESTS: Mutex<()> = Mutex::new(());

fn isolated_registry() -> std::sync::MutexGuard<'static, ()> {
    ABI_TESTS.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[test]
fn project_save_preserves_numeric_preview_and_replacement_publishes_only_complete_documents() {
    let _serial = isolated_registry();
    let directory = std::env::temp_dir().join(format!("solidcraft-project-abi-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let saved = directory.join("Fixture.solidcraft");
    let blank = directory.join("Blank.solidcraft");
    let id = sc_session_create();
    assert_eq!(request(id, r#"{"kind":"fixture"}"#), 0);
    let entry = lookup(id).unwrap();
    let committed_volume = entry.adapter.lock().unwrap().snapshot().unwrap().volume_mm3;
    view_request(id, json!({"kind":"command","command":"select.entity","params":{"entity":{"type":"body","name":"Post"}}}));
    assert_eq!(request(id, r#"{"kind":"numeric","action":"begin"}"#), 0);
    let token = response(id)["result"]["token"].as_u64().unwrap();
    assert_eq!(request(id, &json!({"kind":"numeric","action":"preview","token":token,"expression":"10 mm"}).to_string()), 0);
    let preview = entry.viewport.lock().unwrap().snapshot().unwrap();
    let generation = entry.generation.load(Ordering::Acquire);
    assert_eq!(request(id, &json!({"kind":"save_document","path":saved}).to_string()), 0, "{}", response(id));
    assert!(entry.viewport.lock().unwrap().numeric_active());
    assert_eq!(entry.generation.load(Ordering::Acquire), generation);
    let after = entry.viewport.lock().unwrap().snapshot().unwrap();
    assert!(Arc::ptr_eq(&preview.model, &after.model));

    assert_eq!(request(id, &json!({"kind":"document","name":"Blank","destination":blank}).to_string()), -1, "replacement is refused during an edit");
    assert_eq!(
        request(id, &json!({"kind":"numeric","action":"commit","token":token,"expression":"10 mm"}).to_string()),
        0,
        "autosave must not invalidate Apply"
    );
    assert_eq!(request(id, &json!({"kind":"document","name":"Blank","destination":blank}).to_string()), 0, "{}", response(id));
    assert_eq!(entry.adapter.lock().unwrap().snapshot().unwrap().body_count, 0);
    assert_eq!(request(id, &json!({"kind":"document","name":"Fixture","source":saved,"destination":saved}).to_string()), 0);
    assert_eq!(entry.adapter.lock().unwrap().snapshot().unwrap().body_count, 2);
    assert!(
        (entry.adapter.lock().unwrap().snapshot().unwrap().volume_mm3 - committed_volume).abs() < 1e-6,
        "autosave must not serialize provisional geometry"
    );
    let before = entry.viewport.lock().unwrap().snapshot().unwrap();
    assert_eq!(request(id, &json!({"kind":"document","name":"Missing","source":directory.join("missing.step"),"destination":blank}).to_string()), -1);
    let after = entry.viewport.lock().unwrap().snapshot().unwrap();
    assert_eq!(before.model.document_id, after.model.document_id);
    assert_eq!(before.model.volume_mm3, after.model.volume_mm3);
    assert_eq!(sc_session_destroy(id), 0);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn finished_profile_pencil_selection_reaches_modelling_and_the_drawn_highlight_lane() {
    let _serial = isolated_registry();
    let id = sc_session_create();
    for (command, params) in [
        ("sketch.create", json!({"plane":"XY"})),
        ("sketch.rectangle.two_point", json!({"p0":[-30,-10],"p1":[-10,10]})),
        ("sketch.rectangle.two_point", json!({"p0":[10,-5],"p1":[30,5]})),
        ("sketch.finish", json!({})),
    ] {
        assert_eq!(request(id, &json!({"kind":"command","command":command,"params":params}).to_string()), 0);
    }
    view_request(id, json!({"kind":"viewport","metrics":{"width_points":800,"height_points":600,"scale":2}}));
    view_request(id, json!({"kind":"command","command":"view.top","params":{}}));
    let projected = view_request(id, json!({"kind":"command","command":"view.project","params":{"point":[20,0,0]}}));
    for phase in ["began", "ended"] {
        view_request(
            id,
            json!({"kind":"input","event":{"source":"pencil","phase":phase,"id":1,
            "x_points":projected["result"]["points"][0],"y_points":projected["result"]["points"][1],
            "x_pixels":projected["result"]["pixels"][0],"y_pixels":projected["result"]["pixels"][1],
            "scale":2,"timestamp":1,"geometry_revision":projected["result"]["geometry_revision"]}}),
        );
    }
    let entry = lookup(id).unwrap();
    let snapshot = entry.viewport.lock().unwrap().snapshot().unwrap();
    let (_, bodies) = crate::renderer::model_geometry(&snapshot, 1).unwrap();
    let highlight = crate::renderer::highlight_geometry(&snapshot, &bodies);
    assert!(!highlight.tris.is_empty(), "profile fill must use the triangle lane drawn by the highlight pass");
    assert!(highlight.glass.is_empty() && highlight.trans.is_empty());
    assert_eq!(request(id, r#"{"kind":"model","action":"begin","operation":"solid.extrude"}"#), 0, "{}", response(id));
    let result = response(id)["result"].clone();
    assert!(!result["params"]["profile"].is_null(), "Pencil selection must reach the native form");
    assert_eq!(
        request(id, &json!({"kind":"model","action":"commit","token":result["token"],"params":result["params"]}).to_string()),
        0,
        "{}",
        response(id)
    );
    assert!((entry.adapter.lock().unwrap().snapshot().unwrap().volume_mm3 - 2000.).abs() < 1e-6);
    assert_eq!(sc_session_destroy(id), 0);
}

#[test]
fn native_origin_fills_and_dot_use_the_surface_lane_drawn_by_the_guide_slot() {
    let mut adapter = SessionAdapter::new();
    let mut view = Viewport::default();
    view.set_metrics(solidcraft_ipados_session::ViewportMetrics { width_points: 800., height_points: 600., scale: 1., origin_points: [0.; 2] })
        .unwrap();
    view.begin_update(1);
    view.install(1, adapter.snapshot().unwrap());
    view.command("view.origin", &json!({"visible":true})).unwrap();
    let frame = view.snapshot().unwrap();
    assert_eq!(frame.guides.meshes.len(), 4, "three planes and a filled origin dot");
    let geometry = crate::renderer::guide_geometry(&frame.guides);
    assert!(!geometry.trans.is_empty(), "the shared guide slot draws only its trans surface lane");
    assert!(geometry.tris.is_empty() && geometry.glass.is_empty(), "unsupported lanes would silently drop reference surfaces");
    assert!(!geometry.guide_lines.is_empty() && !geometry.under.is_empty());
    assert!(geometry.lines.is_empty(), "reference axes must precede sketch strokes");
    assert!(frame.model.scene.meshes.is_empty(), "reference geometry must not become CAD bodies");
    view.command("view.origin", &json!({"visible":false})).unwrap();
    assert!(crate::renderer::guide_geometry(&view.snapshot().unwrap().guides).trans.is_empty());
}

fn request(id: u64, text: &str) -> i32 {
    // SAFETY: text is a valid Rust-owned buffer retained for the call.
    unsafe { sc_request(id, text.as_ptr(), text.len()) }
}

fn response(id: u64) -> Value {
    let mut bytes = vec![0; sc_response_len(id)];
    // SAFETY: allocated writable buffer of exactly the response length.
    assert_eq!(unsafe { sc_response_copy(id, bytes.as_mut_ptr(), bytes.len()) }, 0);
    serde_json::from_slice(&bytes).unwrap()
}

#[test]
fn abi_owns_real_session_and_copies_engine_proof() {
    let _serial = isolated_registry();
    let id = sc_session_create();
    assert_ne!(id, 0);
    assert_eq!(request(id, r#"{"kind":"proof"}"#), 0);
    let reply = response(id);
    assert_eq!(reply["ok"], true);
    assert_eq!(reply["result"]["command"]["body_count"], 1);
    assert_eq!(reply["result"]["command"]["volume_mm3"], 12_000.0);
    assert!(reply["result"]["command"]["triangle_count"].as_u64().unwrap() >= 12);
    assert_eq!(sc_session_destroy(id), 0);
    assert_eq!(sc_session_destroy(id), -2);
    assert_eq!(request(id, r#"{"kind":"snapshot"}"#), -2);
}

#[test]
fn abi_rejects_bad_json_buffers_handles_and_commands() {
    let _serial = isolated_registry();
    let id = sc_session_create();
    assert_ne!(id, 0);
    assert_eq!(unsafe { sc_request(id, std::ptr::null(), 10) }, -1);
    assert_eq!(response(id)["error"]["code"], "request");
    assert_eq!(request(id, "{"), -1);
    assert_eq!(request(id, r#"{"kind":"command","command":"missing.command","params":{}}"#), -1);
    assert_eq!(response(id)["error"]["code"], "unknown_command");
    assert_eq!(request(id, r#"{"kind":"command","command":"solid.box","params":{"length":-1}}"#), -1);
    assert_eq!(response(id)["ok"], false);
    let mut short = [42u8; 1];
    assert_eq!(unsafe { sc_response_copy(id, short.as_mut_ptr(), short.len()) }, -1);
    assert_eq!(short, [42]);
    assert_eq!(sc_surface_resize(id, 0, 0), -1);
    let entry = lookup(id).unwrap();
    let bytes = entry.responses[2].lock().unwrap();
    let surface: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(surface["error"]["code"], "surface");
    drop(bytes);
    assert_eq!(sc_session_destroy(id), 0);
}

#[test]
fn input_transports_pencil_and_finger_without_camera_mutation() {
    let _serial = isolated_registry();
    let id = sc_session_create();
    assert_ne!(id, 0);
    for source in ["pencil", "finger"] {
        let value = json!({"kind":"input","event":{"source":source,"phase":"began","id":1,
            "x_points":2.0,"y_points":3.0,"x_pixels":4.0,"y_pixels":6.0,"scale":2.0,"timestamp":1.0,"pressure":0.5,"altitude":1.0}});
        assert_eq!(request(id, &value.to_string()), 0);
        assert_eq!(response(id)["result"]["source"], source);
        assert_eq!(response(id)["result"]["handled"], false);
    }
    assert_eq!(sc_session_destroy(id), 0);
}

#[test]
fn input_rejects_invalid_coordinates_and_device_values() {
    let _serial = isolated_registry();
    let id = sc_session_create();
    assert_ne!(id, 0);
    assert_eq!(request(id, r#"{"kind":"snapshot"}"#), 0);
    let before = response(id)["result"]["command"]["revision"].clone();
    let event = json!({"source":"pencil","phase":"began","id":1,
        "x_points":2.0,"y_points":3.0,"x_pixels":4.0,"y_pixels":6.0,
        "scale":2.0,"timestamp":1.0,"pressure":0.5,"altitude":1.0});
    for (field, invalid) in [
        ("scale", json!(0.0)),
        ("scale", json!(9.0)),
        ("x_pixels", json!(5.0)),
        ("timestamp", json!(-1.0)),
        ("pressure", json!(1.1)),
        ("altitude", json!(-0.1)),
        ("x_points", json!(f64::MAX)),
    ] {
        let mut invalid_event = event.clone();
        invalid_event[field] = invalid;
        let payload = json!({"kind":"input","event":invalid_event});
        assert_eq!(request(id, &payload.to_string()), -1, "{field}");
        assert_eq!(response(id)["error"]["code"], "input", "{field}");
    }
    let mut invalid_event = event;
    invalid_event["source"] = json!("unrecognized_device");
    assert_eq!(request(id, &json!({"kind":"input","event":invalid_event}).to_string()), -1);
    assert_eq!(response(id)["error"]["code"], "request");
    assert_eq!(request(id, r#"{"kind":"snapshot"}"#), 0);
    assert_eq!(response(id)["result"]["command"]["revision"], before);
    assert_eq!(sc_session_destroy(id), 0);
}

#[test]
fn oversized_request_is_rejected_before_json_parsing() {
    let _serial = isolated_registry();
    let id = sc_session_create();
    assert_ne!(id, 0);
    let oversized = vec![b' '; MAX_REQUEST + 1];
    // SAFETY: the allocation is valid and retained for its complete stated length.
    assert_eq!(unsafe { sc_request(id, oversized.as_ptr(), oversized.len()) }, -1);
    assert_eq!(response(id)["error"]["code"], "request");
    assert_eq!(sc_session_destroy(id), 0);
}

fn view_request(id: u64, value: Value) -> Value {
    let bytes = serde_json::to_vec(&value).unwrap();
    // SAFETY: both request and response buffers are valid, owned allocations.
    assert_eq!(unsafe { sc_view_request(id, bytes.as_ptr(), bytes.len()) }, 0);
    let mut bytes = vec![0; sc_view_response_len(id)];
    assert_eq!(unsafe { sc_view_response_copy(id, bytes.as_mut_ptr(), bytes.len()) }, 0);
    serde_json::from_slice(&bytes).unwrap()
}

#[test]
fn engine_evaluation_lock_does_not_block_navigation_suspension_or_response_copy() {
    let _serial = isolated_registry();
    let id = sc_session_create();
    assert_eq!(request(id, r#"{"kind":"proof"}"#), 0);
    let entry = lookup(id).unwrap();
    let engine_reply = response(id);
    let engine = entry.adapter.lock().unwrap();
    // Holding the actual evaluation mutex is a deterministic stand-in for an in-flight
    // kernel call. These paths must complete without acquiring it; no timing assertions.
    view_request(id, json!({"kind":"viewport","metrics":{"width_points":800,"height_points":600,"scale":2}}));
    let nav = view_request(id, json!({"kind":"command","command":"view.navigate","params":{"motion":"orbit","delta":[30,20]}}));
    assert!(nav["result"]["camera"].is_object());
    assert_eq!(sc_surface_suspend(id), 0);
    assert_eq!(response(id), engine_reply, "surface response cannot overwrite engine response");
    drop(engine);
    assert_eq!(sc_session_destroy(id), 0);
}

#[test]
fn suspension_retains_dispatched_command_results_but_rejects_cancelled_preview_commits() {
    let _serial = isolated_registry();
    let id = sc_session_create();
    assert_eq!(request(id, r#"{"kind":"fixture"}"#), 0);
    let entry = lookup(id).unwrap();
    let before = entry.adapter.lock().unwrap().snapshot().unwrap();
    view_request(id, json!({"kind":"command","command":"select.entity","params":{"entity":{"type":"body","name":"Post"}}}));
    assert_eq!(request(id, r#"{"kind":"numeric","action":"begin"}"#), 0);
    let token = response(id)["result"]["token"].as_u64().unwrap();
    assert_eq!(request(id, &json!({"kind":"numeric","action":"preview","token":token,"expression":"10 mm"}).to_string()), 0);
    assert!(entry.viewport.lock().unwrap().diagnostic()["preview_volume_mm3"].as_f64().unwrap() > before.volume_mm3);
    assert_eq!(sc_surface_suspend(id), 0);
    assert!(!entry.viewport.lock().unwrap().numeric_active());

    // Block the actual engine lane after ordinary-command dispatch. Observe its
    // publication state instead of sleeping or making wall-clock assertions.
    let engine = entry.adapter.lock().unwrap();
    let command = std::thread::spawn(move || {
        request(id, r#"{"kind":"command","command":"timeline.edit","params":{"feature":"Viewport base","set":{"length":"50 mm"}}}"#)
    });
    while !entry.viewport.lock().unwrap().is_updating() {
        assert!(!command.is_finished(), "command exited before reaching the engine lane");
        std::thread::yield_now();
    }
    assert_eq!(sc_surface_suspend(id), 0, "suspension must not wait for engine evaluation");
    drop(engine);
    assert_eq!(command.join().unwrap(), 0, "{}", response(id));

    let completed = entry.adapter.lock().unwrap().snapshot().unwrap();
    // Only the existing box length changes: (50 - 40) * 30 * 10 mm³.
    assert!((completed.volume_mm3 - before.volume_mm3 - 3000.0).abs() < 1e-6);
    assert!(completed.geometry_revision > before.geometry_revision);
    {
        let viewport = entry.viewport.lock().unwrap();
        let frame = viewport.snapshot().unwrap();
        assert!(!viewport.is_updating());
        assert!(viewport.selection().is_empty());
        assert_eq!(frame.model.volume_mm3, completed.volume_mm3);
        assert_eq!(frame.geometry_revision, completed.geometry_revision);
        assert_eq!(frame.model.document_id, completed.document_id);
    }
    assert_eq!(request(id, &json!({"kind":"numeric","action":"commit","token":token,"expression":"10 mm"}).to_string()), -1);
    assert_eq!(response(id)["error"]["code"], "stale_edit");
    for (command, expected) in [("edit.undo", before.volume_mm3), ("edit.redo", completed.volume_mm3)] {
        assert_eq!(request(id, &json!({"kind":"command","command":command,"params":{}}).to_string()), 0);
        assert!((entry.adapter.lock().unwrap().snapshot().unwrap().volume_mm3 - expected).abs() < 1e-6);
    }
    assert_eq!(sc_session_destroy(id), 0);
}

#[test]
fn abi_fixture_updates_geometry_and_preserves_camera_and_renderable_snapshot() {
    let _serial = isolated_registry();
    let id = sc_session_create();
    assert_eq!(request(id, r#"{"kind":"fixture"}"#), 0);
    let before = response(id)["result"]["command"].clone();
    assert_eq!(before["body_count"], 2);
    view_request(id, json!({"kind":"viewport","metrics":{"width_points":800,"height_points":600,"scale":2}}));
    let top = view_request(id, json!({"kind":"command","command":"view.top","params":{}}));
    assert_eq!(request(id, r#"{"kind":"command","command":"timeline.edit","params":{"feature":"Viewport post","set":{"height":"40"}}}"#), 0);
    let entry = lookup(id).unwrap();
    let viewport = entry.viewport.lock().unwrap();
    let frame = viewport.snapshot().unwrap();
    assert!(frame.geometry_revision > before["geometry_revision"].as_u64().unwrap());
    assert!(frame.model.volume_mm3 > before["volume_mm3"].as_f64().unwrap());
    assert_eq!(serde_json::to_value(frame.camera).unwrap(), top["result"]["camera"]);
    drop(viewport);
    assert_eq!(sc_session_destroy(id), 0);
}

#[test]
fn whole_body_feedback_references_the_original_gpu_block_without_copying_triangles() {
    let _serial = isolated_registry();
    let id = sc_session_create();
    assert_eq!(request(id, r#"{"kind":"command","command":"solid.sphere","params":{"radius":10,"body_name":"Sphere"}}"#), 0);
    view_request(id, json!({"kind":"viewport","metrics":{"width_points":800,"height_points":600,"scale":2}}));
    view_request(id, json!({"kind":"command","command":"view.top","params":{}}));
    view_request(id, json!({"kind":"command","command":"select.filter","params":{"filter":"body"}}));
    let projected = view_request(id, json!({"kind":"command","command":"view.project","params":{"point":[0,0,10]}}));
    let point = &projected["result"]["points"];
    for phase in ["began", "ended"] {
        view_request(
            id,
            json!({"kind":"input","event":{"source":"pencil","phase":phase,"id":1,
            "x_points":point[0],"y_points":point[1],"x_pixels":projected["result"]["pixels"][0],"y_pixels":projected["result"]["pixels"][1],
            "scale":2,"timestamp":1,"geometry_revision":projected["result"]["geometry_revision"]}}),
        );
    }
    let entry = lookup(id).unwrap();
    let viewport = entry.viewport.lock().unwrap();
    let snapshot = viewport.snapshot().unwrap();
    let (model, bodies) = crate::renderer::model_geometry(&snapshot, 42).unwrap();
    let highlight = crate::renderer::highlight_geometry(&snapshot, &bodies);
    assert_eq!(model.blocks.len(), 1);
    assert!(!model.blocks[0].bytes.vertices.is_empty());
    assert!(model.tris.is_empty());
    assert_eq!(highlight.selected_blocks.len(), 1);
    assert_eq!(highlight.selected_blocks[0].block, model.blocks[0].key);
    assert_eq!(highlight.selected_blocks[0].block, 42);
    assert_eq!(highlight.selected_blocks[0].vertices.len() * solidcraft_gpu::view::TRI_SIZE, model.blocks[0].bytes.vertices.len());
    assert!(highlight.tris.is_empty(), "body selection must not duplicate triangle bytes");
    drop(viewport);
    assert_eq!(sc_session_destroy(id), 0);
}

#[test]
fn malformed_engine_command_reports_error_without_stranding_a_valid_viewport() {
    let _serial = isolated_registry();
    let id = sc_session_create();
    assert_eq!(request(id, r#"{"kind":"proof"}"#), 0);
    assert_eq!(request(id, r#"{"kind":"command","command":"solid.box","params":{"length":true}}"#), -1);
    assert_eq!(response(id)["error"]["code"], "bad_params");
    let entry = lookup(id).unwrap();
    let viewport = entry.viewport.lock().unwrap();
    assert_eq!(viewport.diagnostic()["updating"], false);
    assert_eq!(viewport.snapshot().unwrap().model.volume_mm3, 12000.0);
    drop(viewport);
    assert_eq!(sc_session_destroy(id), 0);
}

#[test]
fn abi_batches_real_tool_preview_history_and_suspension_cleanup_without_a_surface() {
    let _serial = isolated_registry();
    let id = sc_session_create();
    let entry = lookup(id).unwrap();
    assert_eq!(request(id, r#"{"kind":"fixture"}"#), 0);
    entry
        .display_request(Request::Viewport {
            metrics: ViewportMetrics { width_points: 800.0, height_points: 600.0, scale: 2.0, origin_points: [0.0; 2] },
        })
        .unwrap();
    for (command, params) in [("view.top", json!({})), ("tool.set", json!({"tool":"post_radius"}))] {
        entry.display_request(Request::Command { command: command.into(), params }).unwrap();
    }
    let project = |radius: f64| -> Value {
        entry.display_request(Request::Command { command: "view.project".into(), params: json!({"point":[10.0+radius,4.0,35.0]}) }).unwrap()
    };
    let sample = |phase: &str, p: &Value, time: f64| -> Value {
        json!({"source":"pencil","phase":phase,"id":42,"x_points":p["points"][0],"y_points":p["points"][1],
            "x_pixels":p["pixels"][0],"y_pixels":p["pixels"][1],"scale":2.0,"timestamp":time,
            "geometry_revision":p["geometry_revision"],"predicted":false})
    };
    let begin = project(7.0);
    let next = project(12.0);
    let payload = json!({"kind":"input_batch","events":[sample("began",&begin,1.0),sample("moved",&next,2.0)]}).to_string();
    assert_eq!(unsafe { sc_view_request(id, payload.as_ptr(), payload.len()) }, 0);
    let before = entry.adapter.lock().unwrap().snapshot().unwrap().volume_mm3;
    assert_eq!(request(id, r#"{"kind":"tool_work"}"#), 0);
    assert_eq!(entry.adapter.lock().unwrap().snapshot().unwrap().volume_mm3, before);
    assert!(entry.viewport.lock().unwrap().snapshot().unwrap().model.volume_mm3 > before);
    let end = json!({"kind":"input","event":sample("ended",&next,3.0)}).to_string();
    assert_eq!(unsafe { sc_view_request(id, end.as_ptr(), end.len()) }, 0);
    assert_eq!(request(id, r#"{"kind":"tool_work"}"#), 0);
    let after = entry.adapter.lock().unwrap().snapshot().unwrap().volume_mm3;
    assert!(after > before);
    assert_eq!(request(id, r#"{"kind":"command","command":"edit.undo","params":{}}"#), 0);
    assert_eq!(entry.adapter.lock().unwrap().snapshot().unwrap().volume_mm3, before);
    assert_eq!(request(id, r#"{"kind":"command","command":"edit.redo","params":{}}"#), 0);
    assert_eq!(entry.adapter.lock().unwrap().snapshot().unwrap().volume_mm3, after);
    // Background cancellation is independent of whether a GPU surface was attached.
    let begin = project(12.0);
    let next = project(16.0);
    let payload = json!({"kind":"input_batch","events":[sample("began",&begin,4.0),sample("moved",&next,5.0)]}).to_string();
    assert_eq!(unsafe { sc_view_request(id, payload.as_ptr(), payload.len()) }, 0);
    assert_eq!(sc_surface_suspend(id), 0);
    assert_eq!(request(id, r#"{"kind":"tool_work"}"#), 0);
    assert_eq!(response(id)["result"]["worked"], false);
    assert_eq!(entry.adapter.lock().unwrap().snapshot().unwrap().volume_mm3, after);
    assert_eq!(entry.viewport.lock().unwrap().diagnostic()["input"]["contacts"], 0);
    // A corrupt input packet also discards a visible provisional edit.
    assert_eq!(unsafe { sc_view_request(id, payload.as_ptr(), payload.len()) }, 0);
    assert_eq!(request(id, r#"{"kind":"tool_work"}"#), 0);
    assert!(entry.viewport.lock().unwrap().diagnostic()["preview"].as_bool().unwrap());
    assert_eq!(unsafe { sc_view_request(id, b"{".as_ptr(), 1) }, -1);
    assert_eq!(entry.viewport.lock().unwrap().diagnostic()["input"]["contacts"], 0);
    assert_eq!(entry.viewport.lock().unwrap().snapshot().unwrap().model.volume_mm3, after);
    assert_eq!(sc_session_destroy(id), 0);
}

#[test]
fn workspace_numeric_boundary_validates_real_preview_commit_cancel_and_history() {
    let _serial = isolated_registry();
    let id = sc_session_create();
    assert_eq!(request(id, r#"{"kind":"fixture"}"#), 0);
    assert_eq!(request(id, r#"{"kind":"workspace"}"#), 0);
    assert_eq!(response(id)["result"]["tools"][1]["enabled"], false);
    view_request(id, json!({"kind":"command","command":"select.entity","params":{"entity":{"type":"body","name":"Post"}}}));
    assert_eq!(request(id, r#"{"kind":"workspace"}"#), 0);
    let workspace = response(id)["result"].clone();
    assert_eq!(workspace["radius"]["value"], 7.0);
    assert_eq!(workspace["tools"][1]["enabled"], true);
    assert_eq!(request(id, r#"{"kind":"numeric","action":"begin"}"#), 0);
    let token = response(id)["result"]["token"].as_u64().unwrap();
    assert_eq!(response(id)["result"]["workspace"]["can_undo"], false);
    let entry = lookup(id).unwrap();
    let before = entry.adapter.lock().unwrap().snapshot().unwrap().volume_mm3;
    let numeric = |action: &str, expression: &str| json!({"kind":"numeric","action":action,"token":token,"expression":expression}).to_string();
    assert_eq!(request(id, &numeric("preview", "12 mm")), 0);
    let preview = response(id)["result"]["viewport"]["preview_volume_mm3"].as_f64().unwrap();
    assert!(preview > before);
    assert_eq!(entry.adapter.lock().unwrap().snapshot().unwrap().volume_mm3, before);
    assert_eq!(request(id, &numeric("preview", "-")), -1);
    assert_eq!(response(id)["error"]["code"], "numeric_validation");
    assert_eq!(entry.viewport.lock().unwrap().diagnostic()["preview_volume_mm3"], preview);
    assert_eq!(request(id, r#"{"kind":"command","command":"edit.undo","params":{}}"#), -1);
    assert_eq!(response(id)["error"]["code"], "disabled");
    assert_eq!(request(id, &numeric("commit", "6 + 6")), 0);
    assert_eq!(response(id)["result"]["committed"], true);
    assert_eq!(response(id)["result"]["token"], Value::Null);
    assert_eq!(entry.adapter.lock().unwrap().snapshot().unwrap().volume_mm3, preview);
    assert_eq!(request(id, &numeric("commit", "15")), -1, "stale token cannot commit twice");
    for (command, expected) in [("edit.undo", before), ("edit.redo", preview)] {
        assert_eq!(request(id, &json!({"kind":"command","command":command,"params":{}}).to_string()), 0);
        assert_eq!(entry.adapter.lock().unwrap().snapshot().unwrap().volume_mm3, expected);
    }
    view_request(id, json!({"kind":"command","command":"select.entity","params":{"entity":{"type":"body","name":"Post"}}}));
    assert_eq!(request(id, r#"{"kind":"numeric","action":"begin"}"#), 0);
    let next = response(id)["result"]["token"].as_u64().unwrap();
    assert_ne!(next, token);
    assert_eq!(request(id, &json!({"kind":"numeric","action":"preview","token":next,"expression":"15"}).to_string()), 0);
    assert_eq!(request(id, &json!({"kind":"numeric","action":"cancel","token":next}).to_string()), 0);
    assert_eq!(entry.viewport.lock().unwrap().snapshot().unwrap().model.volume_mm3, preview);
    assert_eq!(entry.adapter.lock().unwrap().snapshot().unwrap().volume_mm3, preview);
    assert_eq!(sc_session_destroy(id), 0);
}

#[test]
fn numeric_lifecycle_suspend_invalidates_prepared_commit_even_without_a_surface() {
    let _serial = isolated_registry();
    let id = sc_session_create();
    assert_eq!(request(id, r#"{"kind":"fixture"}"#), 0);
    view_request(id, json!({"kind":"command","command":"select.entity","params":{"entity":{"type":"body","name":"Post"}}}));
    assert_eq!(request(id, r#"{"kind":"numeric","action":"begin"}"#), 0);
    let token = response(id)["result"]["token"].as_u64().unwrap();
    let entry = lookup(id).unwrap();
    let edit = entry.viewport.lock().unwrap().numeric_edit(token).unwrap();
    // Deterministic stand-in for the evaluation stage: suspend occurs before final
    // ticket validation, so the actual prepared command may never reach history.
    let prepared = entry.adapter.lock().unwrap().prepare_numeric(&edit, "10", true).unwrap();
    assert_eq!(sc_surface_suspend(id), 0);
    assert!(entry.viewport.lock().unwrap().numeric_edit(token).is_err());
    drop(prepared);
    assert_eq!(request(id, &json!({"kind":"numeric","action":"commit","token":token,"expression":"10"}).to_string()), -1);
    assert_eq!(response(id)["error"]["code"], "stale_edit");
    assert_eq!(
        entry.adapter.lock().unwrap().workspace(&[serde_json::from_value(json!({"type":"body","name":"Post"})).unwrap()], false, false)["radius"]["value"],
        7.0
    );
    assert_eq!(sc_session_destroy(id), 0);
}

fn engine_command(id: u64, command: &str, params: Value) -> Value {
    assert_eq!(request(id, &json!({"kind":"command","command":command,"params":params}).to_string()), 0, "{command}: {}", response(id));
    response(id)["result"]["command"]["value"].clone()
}

#[test]
fn dark_sketch_strokes_reach_gpu_for_active_finished_and_preview_scenes() {
    let _serial = isolated_registry();
    let id = sc_session_create();
    engine_command(id, "sketch.create", json!({"plane":"XY"}));
    engine_command(id, "sketch.line", json!({"points":[[10,10],[30,20]],"ids":["free"],"infer":false}));
    let entry = lookup(id).unwrap();
    let light = entry.viewport.lock().unwrap().snapshot().unwrap();
    let original = light.model.scene.lines[0].color;
    assert_eq!(light.model_line_color(original), original);
    let geometry_revision = light.geometry_revision;
    view_request(id, json!({"kind":"command","command":"view.background","params":{"color":[28,28,30],"sketch":[100,210,255]}}));
    let dark = entry.viewport.lock().unwrap().snapshot().unwrap();
    assert!(Arc::ptr_eq(&light.model, &dark.model));
    assert_eq!(dark.geometry_revision, geometry_revision);
    assert_eq!(dark.model.scene.lines[0].color, original, "the engine scene keeps its original palette");
    let (gpu, _) = crate::renderer::model_geometry(&dark, 1).unwrap();
    assert!(gpu.palette.entries().contains(&[100, 210, 255, 255]));
    assert!(!gpu.palette.entries().contains(&[original.0, original.1, original.2, 255]));
    assert!(dark.guides.grid.iter().all(|line| dark.model_line_color(line.color) == line.color), "axis and grid colours stay distinct");

    // Numeric previews use a different immutable display scene, but the native
    // appearance must persist through preview, cancel and document publication.
    view_request(id, json!({"kind":"command","command":"select.entity","params":{"entity":{"type":"sketch_curve","id":"free"}}}));
    assert_eq!(request(id, r#"{"kind":"numeric","action":"begin","dimension":"length"}"#), 0);
    let token = response(id)["result"]["token"].as_u64().unwrap();
    assert_eq!(request(id, &json!({"kind":"numeric","action":"preview","token":token,"expression":"40 mm"}).to_string()), 0);
    let preview = entry.viewport.lock().unwrap().snapshot().unwrap();
    assert!(!Arc::ptr_eq(&dark.model, &preview.model));
    assert!(crate::renderer::model_geometry(&preview, 2).unwrap().0.palette.entries().contains(&[100, 210, 255, 255]));
    assert_eq!(request(id, &json!({"kind":"numeric","action":"cancel","token":token}).to_string()), 0);
    engine_command(id, "sketch.line", json!({"points":[[10,30],[30,40]],"ids":["guide"],"infer":false,"construction":true}));
    engine_command(id, "sketch.finish", json!({}));
    let finished = entry.viewport.lock().unwrap().snapshot().unwrap();
    let (gpu, _) = crate::renderer::model_geometry(&finished, 3).unwrap();
    assert!(gpu.palette.entries().contains(&[100, 210, 255, 255]), "finished black strokes must also become blue");
    assert!(gpu.palette.entries().contains(&[230, 130, 40, 255]), "construction strokes keep their own colour");
    view_request(id, json!({"kind":"command","command":"view.background","params":{"color":null}}));
    let restored = entry.viewport.lock().unwrap().snapshot().unwrap();
    assert_eq!(restored.sketch_color, None);
    assert!(Arc::ptr_eq(&finished.model, &restored.model));
    assert!(crate::renderer::model_geometry(&restored, 4).unwrap().0.palette.entries().contains(&[20, 20, 20, 255]));
    assert_eq!(sc_session_destroy(id), 0);
}

#[test]
fn sketch_plane_preview_resolves_native_planes_without_document_or_history_changes() {
    let _serial = isolated_registry();
    let id = sc_session_create();
    view_request(id, json!({"kind":"viewport","metrics":{"width_points":800,"height_points":600,"scale":2}}));
    engine_command(id, "sketch.create", json!({"plane":"XY","name":"Existing"}));
    engine_command(id, "sketch.finish", json!({}));
    let before = lookup(id).unwrap().adapter.lock().unwrap().workspace(&[], false, false);
    assert_eq!(request(id, r#"{"kind":"sketch_plane","plane":{"origin":[4,5,6],"x_dir":[0,1,0],"y_dir":[0,0,1]}}"#), 0);
    assert_eq!(response(id)["result"]["planar"], true);
    let after = lookup(id).unwrap().adapter.lock().unwrap().workspace(&[], false, false);
    assert_eq!(after, before, "preview must not create a sketch or history entry");
    view_request(id, json!({"kind":"command","command":"view.cancel_plane","params":{}}));
    assert_eq!(request(id, r#"{"kind":"sketch_plane","plane":"missing plane"}"#), -1);
    assert_eq!(lookup(id).unwrap().adapter.lock().unwrap().workspace(&[], false, false), before);
    assert_eq!(sc_session_destroy(id), 0);
}

#[test]
fn sketch_dimension_editor_creates_and_changes_real_parameters_once_per_confirmation() {
    let _serial = isolated_registry();
    let id = sc_session_create();
    let sketch = engine_command(id, "sketch.create", json!({"plane":"XY"}))["sketch"].as_u64().unwrap();
    let rectangle = engine_command(id, "sketch.rectangle.two_point", json!({"p0":[0,0],"p1":[40,30]}));
    let line = rectangle["sides"]["bottom"].as_str().unwrap();
    view_request(id, json!({"kind":"command","command":"select.entity","params":{"entity":{"type":"sketch_curve","id":line}}}));
    let entry = lookup(id).unwrap();
    assert_eq!(request(id, r#"{"kind":"numeric","action":"begin","dimension":"length"}"#), 0, "{}", response(id));
    let draft = response(id)["result"].clone();
    let token = draft["token"].as_u64().unwrap();
    assert_eq!(draft["units"], "mm");
    assert_eq!(request(id, &json!({"kind":"numeric","action":"preview","token":token,"expression":"60 mm"}).to_string()), 0);
    assert_eq!(request(id, &json!({"kind":"numeric","action":"preview","token":token,"expression":"-"}).to_string()), -1);
    assert_eq!(request(id, &json!({"kind":"numeric","action":"commit","token":token,"expression":"60 mm"}).to_string()), 0);
    let inspected = engine_command(id, "sketch.inspect", json!({"sketch":sketch}));
    assert!(inspected.to_string().contains("60"));
    engine_command(id, "edit.undo", json!({}));
    // One Undo removes the new dimension, retaining the completed rectangle.
    let state = entry.adapter.lock().unwrap().workspace(&[], false, false);
    assert_eq!(state["sketch"]["curves"].as_array().unwrap().len(), 4);
    assert!(!state["sketch"]["constraints"].as_array().unwrap().iter().any(|c| c["dimension"] == true));
    engine_command(id, "edit.redo", json!({}));
    let state = entry.adapter.lock().unwrap().workspace(&[], false, false);
    let dimension = state["sketch"]["constraints"].as_array().unwrap().iter().find(|c| c["dimension"] == true).unwrap();
    let constraint = dimension["id"].as_str().unwrap();
    view_request(id, json!({"kind":"command","command":"select.entity","params":{"entity":{"type":"sketch_constraint","id":constraint}}}));
    assert_eq!(request(id, r#"{"kind":"numeric","action":"begin"}"#), 0);
    let edit = response(id)["result"].clone();
    assert_eq!(edit["expression"], "60 mm", "opening preserves the stored expression");
    assert_eq!(request(id, &json!({"kind":"numeric","action":"cancel","token":edit["token"]}).to_string()), 0);
    assert_eq!(sc_session_destroy(id), 0);
}

#[test]
fn sketch_snap_menu_can_disable_and_reenable_the_native_saved_preference() {
    let _serial = isolated_registry();
    let id = sc_session_create();
    engine_command(id, "sketch.create", json!({"plane":"XY"}));
    for enabled in [false, true] {
        view_request(id, json!({"kind":"command","command":"tool.snapping","params":{"enabled":enabled}}));
        assert_eq!(request(id, r#"{"kind":"tool_work"}"#), 0);
        let committed = response(id);
        assert_eq!(committed["result"]["committed"], true);
        assert_eq!(committed["result"]["viewport"]["input"]["sketch"]["snapping"], enabled);
        assert_eq!(request(id, r#"{"kind":"workspace"}"#), 0);
        assert_eq!(response(id)["result"]["input"]["sketch"]["snapping"], enabled);
        assert_eq!(engine_command(id, "sketch.options", json!({}))["options"]["snap"], enabled);
    }
    assert_eq!(sc_session_destroy(id), 0);
}

fn modelling_sketch(id: u64) -> u64 {
    let sketch = engine_command(id, "sketch.create", json!({"plane":"XY","name":"Profile"}))["sketch"].as_u64().unwrap();
    engine_command(id, "sketch.rectangle.two_point", json!({"p0":[0,0],"p1":[40,30]}));
    engine_command(id, "sketch.finish", json!({}));
    view_request(id, json!({"kind":"command","command":"select.entity","params":{"entity":{"type":"feature","id":sketch}}}));
    sketch
}

#[test]
fn modelling_extrusion_drag_survives_preview_publication_and_form_inspection() {
    let _serial = isolated_registry();
    for creating in [false, true] {
        let id = sc_session_create();
        let sketch = modelling_sketch(id);
        if !creating {
            engine_command(id, "solid.extrude", json!({"sketch":sketch,"distance":"10 mm","body_name":"Block"}));
        }
        let design = engine_command(id, "document.inspect", json!({}));
        let feature = design["timeline"]
            .as_array()
            .unwrap()
            .iter()
            .find(|feature| feature["type"] == "ExtrudeFeature")
            .and_then(|feature| feature["id"].as_u64());
        let history = design["undo"].clone();
        view_request(id, json!({"kind":"viewport","metrics":{"width_points":800,"height_points":600,"scale":2}}));
        view_request(id, json!({"kind":"command","command":"view.exit_planar","params":{}}));
        view_request(id, json!({"kind":"command","command":"view.home","params":{}}));
        view_request(id, json!({"kind":"command","command":"view.fit","params":{}}));
        assert_eq!(
            request(id, &json!({"kind":"model","action":"begin","feature":feature,"operation":creating.then_some("solid.extrude")}).to_string()),
            0,
            "{}",
            response(id)
        );
        let token = response(id)["result"]["token"].as_u64().unwrap();
        if creating {
            let params = response(id)["result"]["params"].clone();
            assert_eq!(request(id, &json!({"kind":"model","action":"preview","token":token,"params":params}).to_string()), 0, "{}", response(id));
            view_request(id, json!({"kind":"command","command":"view.fit","params":{}}));
        }
        let start = view_request(id, json!({"kind":"snapshot"}))["result"]["handle_point"].as_array().unwrap().clone();
        let input = |phase: &str, depth: f64, time: f64| {
            let mut world = start.clone();
            world[2] = json!(depth);
            let projected = view_request(id, json!({"kind":"command","command":"view.project","params":{"point":world}}));
            let viewport = view_request(id, json!({"kind":"snapshot"}))["result"].clone();
            view_request(
                id,
                json!({"kind":"input","event":{
                    "source":"pencil","phase":phase,"id":91,"timestamp":time,
                    "x_points":projected["result"]["points"][0],"y_points":projected["result"]["points"][1],
                    "x_pixels":projected["result"]["pixels"][0],"y_pixels":projected["result"]["pixels"][1],"scale":2,
                    "geometry_revision":viewport["geometry_revision"],"document_id":viewport["document_id"],
                    "viewport_epoch":viewport["viewport_epoch"],"camera_epoch":viewport["camera_epoch"],"display_preview":viewport["preview"]
                }}),
            )
        };
        assert_eq!(input("began", 10.0, 1.0)["result"]["handled"], true);
        for (index, depth) in [15.0, 20.0, 12.0].into_iter().enumerate() {
            input("moved", depth, 2.0 + index as f64);
            assert_eq!(request(id, r#"{"kind":"tool_work"}"#), 0, "{}", response(id));
            assert_eq!(response(id)["result"]["worked"], true, "the same contact must keep producing previews");
            assert_eq!(request(id, &json!({"kind":"model","action":"inspect","token":token}).to_string()), 0, "{}", response(id));
            let result = response(id)["result"].clone();
            assert_eq!(result["viewport"]["input"]["edit"], "dragging", "inspection must preserve the active handle");
            assert!((result["viewport"]["preview_volume_mm3"].as_f64().unwrap() - 1200.0 * depth).abs() < 1e-5);
        }
        input("ended", 12.0, 6.0);
        assert_eq!(request(id, r#"{"kind":"tool_work"}"#), 0, "{}", response(id));
        let entry = lookup(id).unwrap();
        let source_volume = if creating { 0. } else { 12_000. };
        assert!((entry.adapter.lock().unwrap().snapshot().unwrap().volume_mm3 - source_volume).abs() < 1e-6);
        let params = entry.viewport.lock().unwrap().model_edit(token).unwrap().params;
        assert_eq!(request(id, &json!({"kind":"model","action":"commit","token":token,"params":params}).to_string()), 0, "{}", response(id));
        assert!((entry.adapter.lock().unwrap().snapshot().unwrap().volume_mm3 - 14_400.0).abs() < 1e-5);
        assert_eq!(engine_command(id, "document.inspect", json!({}))["undo"], history.as_u64().unwrap() + 1);
        assert_eq!(sc_session_destroy(id), 0);
    }
}

#[test]
fn modelling_boundary_previews_real_sketch_then_commits_once_and_cancels_cleanly() {
    let _serial = isolated_registry();
    let id = sc_session_create();
    let sketch = modelling_sketch(id);
    let entry = lookup(id).unwrap();
    assert_eq!(request(id, r#"{"kind":"model","action":"begin","operation":"solid.extrude"}"#), 0, "{}", response(id));
    let token = response(id)["result"]["token"].as_u64().unwrap();
    let proposal = |action: &str, distance: Value| {
        json!({"kind":"model","action":action,"token":token,"params":{
            "sketch":sketch,"profiles":[0],"distance":distance,"operation":"new","body_name":"Block"
        }})
        .to_string()
    };
    assert_eq!(request(id, &proposal("preview", json!("10 mm"))), 0, "{}", response(id));
    assert_eq!(response(id)["result"]["valid"], true);
    assert!((entry.viewport.lock().unwrap().diagnostic()["preview_volume_mm3"].as_f64().unwrap() - 12_000.0).abs() < 1e-6);
    assert_eq!(entry.adapter.lock().unwrap().snapshot().unwrap().volume_mm3, 0.0);
    let last_good = entry.viewport.lock().unwrap().snapshot().unwrap().model.clone();
    assert_eq!(request(id, &proposal("preview", json!("-"))), -1);
    assert_eq!(request(id, &json!({"kind":"model","action":"inspect","token":token}).to_string()), 0);
    assert_eq!(response(id)["result"]["valid"], false, "an invalid numeric proposal cannot confirm the last good result");
    assert_eq!(response(id)["result"]["viewport"]["preview_stale"], true);
    assert_eq!(response(id)["result"]["viewport"]["preview"], true, "the retained scene remains provisional for source-aware input");
    assert!(
        Arc::ptr_eq(&entry.viewport.lock().unwrap().snapshot().unwrap().model, &last_good),
        "the actual invalid Preview request keeps the last good geometry visible"
    );
    assert_eq!(entry.adapter.lock().unwrap().snapshot().unwrap().volume_mm3, 0.0);
    assert_eq!(request(id, r#"{"kind":"command","command":"edit.undo","params":{}}"#), -1);
    assert_eq!(request(id, &proposal("commit", json!("10 mm"))), 0, "{}", response(id));
    assert_eq!(response(id)["result"]["committed"], true);
    assert!((entry.adapter.lock().unwrap().snapshot().unwrap().volume_mm3 - 12_000.0).abs() < 1e-6);
    assert_eq!(request(id, &proposal("commit", json!("10 mm"))), -1, "double confirmation cannot duplicate a feature");
    for (command, expected) in [("edit.undo", 0.0), ("edit.redo", 12_000.0)] {
        engine_command(id, command, json!({}));
        assert!((entry.adapter.lock().unwrap().snapshot().unwrap().volume_mm3 - expected).abs() < 1e-6);
    }
    let design = engine_command(id, "document.inspect", json!({}));
    let feature = design["timeline"].as_array().unwrap().iter().find(|feature| feature["type"] == "ExtrudeFeature").unwrap()["id"].as_u64().unwrap();
    assert_eq!(request(id, &json!({"kind":"model","action":"begin","feature":feature}).to_string()), 0, "{}", response(id));
    let next = response(id)["result"]["token"].as_u64().unwrap();
    assert_eq!(
        request(id, &json!({"kind":"model","action":"preview","token":next,"params":{"extent":{"distance":"15 mm"}}}).to_string()),
        0,
        "{}",
        response(id)
    );
    assert_eq!(request(id, &json!({"kind":"model","action":"cancel","token":next}).to_string()), 0);
    assert!((entry.adapter.lock().unwrap().snapshot().unwrap().volume_mm3 - 12_000.0).abs() < 1e-6);
    assert_eq!(sc_session_destroy(id), 0);
}

#[test]
fn modelling_lifecycle_suspend_invalidates_a_prepared_commit_without_mutation() {
    let _serial = isolated_registry();
    let id = sc_session_create();
    let sketch = modelling_sketch(id);
    assert_eq!(request(id, r#"{"kind":"model","action":"begin","operation":"solid.extrude"}"#), 0, "{}", response(id));
    let token = response(id)["result"]["token"].as_u64().unwrap();
    let entry = lookup(id).unwrap();
    let edit = entry.viewport.lock().unwrap().model_edit(token).unwrap();
    let params = json!({"sketch":sketch,"profiles":[0],"distance":"10 mm","operation":"new"});
    let prepared = entry.adapter.lock().unwrap().prepare_model(&edit, params.clone(), true).unwrap();
    assert_eq!(sc_surface_suspend(id), 0);
    assert!(entry.viewport.lock().unwrap().model_edit(token).is_err());
    drop(prepared);
    assert_eq!(request(id, &json!({"kind":"model","action":"commit","token":token,"params":params}).to_string()), -1);
    assert_eq!(entry.adapter.lock().unwrap().snapshot().unwrap().volume_mm3, 0.0);
    assert_eq!(sc_session_destroy(id), 0);
}

#[test]
fn modelling_preview_order_and_document_change_discard_superseded_results() {
    let _serial = isolated_registry();
    let id = sc_session_create();
    let sketch = modelling_sketch(id);
    let initial = json!({"sketch":sketch,"profiles":[0],"distance":"10 mm","operation":"new"});
    assert_eq!(
        request(id, &json!({"kind":"model","action":"begin","operation":"solid.extrude","params":initial}).to_string()),
        0,
        "{}",
        response(id)
    );
    let token = response(id)["result"]["token"].as_u64().unwrap();
    assert_eq!(response(id)["result"]["params"]["distance"], "10 mm");
    let entry = lookup(id).unwrap();
    let edit = entry.viewport.lock().unwrap().model_edit(token).unwrap();
    let old = entry.adapter.lock().unwrap().prepare_model(&edit, edit.params.clone(), false).unwrap();
    let mut next_params = edit.params.clone();
    next_params["distance"] = json!("15 mm");
    let updated = entry.adapter.lock().unwrap().refresh_model(&edit, &[], Some(next_params)).unwrap();
    entry.viewport.lock().unwrap().update_model(updated.clone()).unwrap();
    let newer = entry.adapter.lock().unwrap().prepare_model(&updated, updated.params.clone(), false).unwrap();
    // Force the newer evaluation to complete first without sleeps or scheduling
    // assumptions. The older result must never replace its displayed geometry.
    entry.viewport.lock().unwrap().publish_model(token, &newer).unwrap();
    assert!(entry.viewport.lock().unwrap().publish_model(token, &old).is_err());
    assert!((entry.viewport.lock().unwrap().diagnostic()["preview_volume_mm3"].as_f64().unwrap() - 18_000.0).abs() < 1e-6);
    assert_eq!(entry.adapter.lock().unwrap().snapshot().unwrap().volume_mm3, 0.0);
    let prepared = entry.adapter.lock().unwrap().prepare_model(&updated, updated.params.clone(), true).unwrap();
    // An independent authoring change invalidates the computed transaction even
    // when it is deliberately held before acceptance.
    entry.adapter.lock().unwrap().execute("parameters.add", json!({"name":"changed","expression":"2 mm"})).unwrap();
    assert!(entry.adapter.lock().unwrap().accept_model(&updated, prepared).is_err());
    assert_eq!(entry.adapter.lock().unwrap().snapshot().unwrap().volume_mm3, 0.0);
    assert_eq!(request(id, r#"{"kind":"model","action":"cancel"}"#), 0);
    assert_eq!(request(id, r#"{"kind":"model","action":"cancel"}"#), 0, "dismissal cancellation is idempotent");
    assert_eq!(sc_session_destroy(id), 0);
}

#[test]
fn modelling_inspect_refreshes_parameters_and_invalidates_previous_preview() {
    let _serial = isolated_registry();
    let id = sc_session_create();
    let sketch = modelling_sketch(id);
    assert_eq!(request(id, r#"{"kind":"model","action":"begin","operation":"solid.extrude"}"#), 0);
    let token = response(id)["result"]["token"].as_u64().unwrap();
    let params = json!({"sketch":sketch,"profiles":[0],"distance":"10 mm","operation":"new"});
    assert_eq!(request(id, &json!({"kind":"model","action":"preview","token":token,"params":params}).to_string()), 0);
    assert_eq!(request(id, &json!({"kind":"model","action":"inspect","token":token}).to_string()), 0);
    assert_eq!(response(id)["result"]["valid"], true);
    assert_ne!(request(id, &json!({"kind":"model","action":"selection","token":token,"params":{"field":"distance"}}).to_string()), 0);
    assert_eq!(request(id, &json!({"kind":"model","action":"selection","token":token,"params":{"field":"targets"}}).to_string()), 0);
    assert_eq!(response(id)["result"]["valid"], false);
    assert_eq!(response(id)["result"]["viewport"]["preview"], false, "selection reveals the same source scene used by picking");
    assert!(response(id)["result"]["handle"].is_null(), "selection must not be intercepted by the distance handle");
    assert_eq!(request(id, &json!({"kind":"model","action":"preview","token":token,"params":params}).to_string()), 0);
    assert_eq!(request(id, &json!({"kind":"model","action":"inspect","token":token}).to_string()), 0);
    let mut changed = response(id)["result"]["params"].clone();
    changed["distance"] = json!("20 mm");
    assert_eq!(request(id, &json!({"kind":"model","action":"inspect","token":token,"params":changed}).to_string()), 0);
    assert_eq!(response(id)["result"]["valid"], false);
    assert_eq!(response(id)["result"]["params"]["distance"], "20 mm");
    assert_eq!(request(id, r#"{"kind":"model","action":"cancel"}"#), 0);
    assert_eq!(sc_session_destroy(id), 0);
}

#[test]
fn assembly_completion_retains_native_clamped_drive_feedback_after_consuming_the_draft() {
    let _serial = isolated_registry();
    let id = sc_session_create();
    let base = engine_command(id, "component.create", json!({"name":"Base","grounded":true}))["occurrence"].as_u64().unwrap();
    engine_command(id, "solid.box", json!({"length":2,"width":2,"height":2}));
    engine_command(id, "component.activate", json!({"component":0}));
    let moving = engine_command(id, "component.create", json!({"name":"Moving","grounded":false}))["occurrence"].as_u64().unwrap();
    engine_command(id, "solid.box", json!({"length":2,"width":2,"height":2,"corner":[10,0,0]}));
    let joint = engine_command(
        id,
        "joint.as_built",
        json!({"name":"Slide","type":"slider","a":base,"b":moving,
        "at":{"occurrence":base,"point":[0,0,0],"z":[1,0,0]},"limits":[[0,5]]}),
    )["joint"]
        .as_u64()
        .unwrap();
    let undo = engine_command(id, "document.inspect", json!({}))["undo"].as_u64().unwrap();
    let params = json!({"joint":joint,"value":"20 mm"});
    assert_eq!(request(id, &json!({"kind":"model","action":"begin","operation":"joint.drive","params":params}).to_string()), 0, "{}", response(id));
    let token = response(id)["result"]["token"].as_u64().unwrap();
    assert_eq!(request(id, &json!({"kind":"model","action":"commit","token":token,"params":params}).to_string()), 0, "{}", response(id));
    let committed = response(id)["result"].clone();
    assert_eq!(committed["token"], Value::Null);
    assert_eq!(committed["reply"]["values"][0]["value"], 5.0);
    assert_eq!(committed["reply"]["moved"], true);
    assert!(!committed["feedback"].as_array().unwrap().is_empty());
    assert_eq!(engine_command(id, "document.inspect", json!({}))["undo"], undo + 1);
    assert_eq!(request(id, &json!({"kind":"model","action":"begin","operation":"joint.drive"}).to_string()), 0);
    let token = response(id)["result"]["token"].as_u64().unwrap();
    assert_eq!(request(id, &json!({"kind":"model","action":"preview","token":token,"params":{"joint":joint,"value":"0 mm"}}).to_string()), 0);
    assert_eq!(request(id, r#"{"kind":"model","action":"cancel"}"#), 0);
    let cancelled = response(id)["result"].clone();
    assert_eq!(cancelled["token"], Value::Null);
    assert_eq!(cancelled["reply"]["values"][0]["value"], 0.0);
    assert!(!cancelled["feedback"].as_array().unwrap().is_empty());
    assert_eq!(engine_command(id, "joint.list", json!({}))["joints"][0]["values"][0]["value"], 5.0);
    assert_eq!(sc_session_destroy(id), 0);
}

#[test]
fn assembly_renderer_retains_shared_triangles_across_moves_and_highlights_only_one_placement() {
    let _serial = isolated_registry();
    let id = sc_session_create();
    let first = engine_command(id, "component.create", json!({"name":"SharedPart","grounded":false}))["occurrence"].as_u64().unwrap();
    engine_command(id, "solid.box", json!({"length":2,"width":3,"height":4,"body_name":"SharedBlock"}));
    engine_command(id, "component.activate", json!({"component":0}));
    let second =
        engine_command(id, "occurrence.copy", json!({"occurrence":first,"path":[first],"translate":[10,0,0]}))["occurrence"].as_u64().unwrap();
    view_request(id, json!({"kind":"viewport","metrics":{"width_points":800,"height_points":600,"scale":1}}));
    view_request(id, json!({"kind":"command","command":"view.top","params":{}}));
    let entry = lookup(id).unwrap();
    let before = entry.viewport.lock().unwrap().snapshot().unwrap();
    assert_eq!(before.model.instances.len(), 2);
    assert!(Arc::ptr_eq(&before.model.instances[0].mesh, &before.model.instances[1].mesh));
    let mut cache = crate::renderer::NativeMeshCache::default();
    let (initial_gpu, _) = crate::renderer::model_geometry_cached(&before, 100, &mut cache).unwrap();
    assert_eq!(initial_gpu.instanced.len(), 1, "two occurrences draw from one shared triangle set");
    assert_eq!(initial_gpu.instanced[0].count(), 2);
    assert!(initial_gpu.tris.is_empty() && initial_gpu.blocks.is_empty());
    assert_eq!(cache.len(), 1);
    let triangles = initial_gpu.instanced[0].tris.clone();
    let key = initial_gpu.instanced[0].key;
    let old_placements = initial_gpu.instanced[0].instances.clone();
    engine_command(id, "occurrence.move", json!({"occurrence":second,"path":[second],"space":"world","translate":[5,0,0]}));
    view_request(id, json!({"kind":"command","command":"view.fit","params":{}}));
    let moved = entry.viewport.lock().unwrap().snapshot().unwrap();
    let (moved_gpu, _) = crate::renderer::model_geometry_cached(&moved, 101, &mut cache).unwrap();
    assert!(moved.geometry_revision > before.geometry_revision);
    assert!((before.model.volume_mm3 - 48.).abs() < 1e-6);
    assert!((moved.model.volume_mm3 - 48.).abs() < 1e-6);
    assert_eq!(moved_gpu.instanced.len(), 1);
    assert_eq!(moved_gpu.instanced[0].key, key);
    assert!(Arc::ptr_eq(&moved_gpu.instanced[0].tris, &triangles), "pose updates must retain exactly the uploaded triangle/index data");
    assert_ne!(moved_gpu.instanced[0].instances, old_placements, "the native placement changed");
    assert_eq!(cache.len(), 1);
    let current_name =
        moved.model.instances.iter().find(|body| moved.model.occurrence_paths.get(&body.body) == Some(&vec![second])).unwrap().body.clone();
    let metrics = ViewportMetrics { width_points: 800., height_points: 600., scale: 1., origin_points: [0., 0.] };
    for (point, expected) in [([11., 1.5, 4.], None), ([16., 1.5, 4.], Some(current_name.as_str()))] {
        let projected = view_request(id, json!({"kind":"command","command":"view.project","params":{"point":point}}));
        let pixel = [projected["result"]["pixels"][0].as_f64().unwrap(), projected["result"]["pixels"][1].as_f64().unwrap()];
        let selected = moved.model.picks.pick(&moved.camera, moved.view_proj, metrics, pixel, solidcraft_ipados_session::PickFilter::Body);
        assert_eq!(
            selected.map(|value| serde_json::to_value(value).unwrap()["name"].as_str().unwrap().to_string()).as_deref(),
            expected,
            "world-space picking must match the current placement"
        );
    }
    view_request(id, json!({"kind":"command","command":"select.entity","params":{"entity":{"type":"occurrence","path":[second]}}}));
    let selected = entry.viewport.lock().unwrap().snapshot().unwrap();
    assert_eq!(selected.selected_bodies, vec![current_name]);
    let (selected_gpu, bodies) = crate::renderer::model_geometry_cached(&selected, 102, &mut cache).unwrap();
    let highlight = crate::renderer::highlight_geometry(&selected, &bodies);
    assert_eq!(highlight.selected_instanced.len(), 1);
    assert_eq!(highlight.selected_instanced[0].count(), 1, "one exact occurrence, not every linked instance, is selected");
    assert!(Arc::ptr_eq(&highlight.selected_instanced[0].tris, &selected_gpu.instanced[0].tris));
    assert_eq!(highlight.selected_instanced[0].key, key);
    assert!(highlight.selected_blocks.is_empty() && highlight.tris.is_empty());
    assert_eq!(sc_session_destroy(id), 0);
}

#[test]
fn native_renderer_mesh_cache_is_bounded_and_never_reuses_an_evicted_key() {
    let _serial = isolated_registry();
    let id = sc_session_create();
    assert_eq!(request(id, r#"{"kind":"proof"}"#), 0);
    let entry = lookup(id).unwrap();
    let snapshot = entry.viewport.lock().unwrap().snapshot().unwrap();
    let source = snapshot.model.instances[0].clone();
    let mut cache = crate::renderer::NativeMeshCache::default();
    let (original_key, original_tris) = cache.shared(&source).unwrap().unwrap();
    let mut last_key = original_key;
    for _ in 0..256 {
        let copy = solidcraft_ipados_session::NativeMeshInstance { mesh: Arc::new(source.mesh.as_ref().clone()), ..source.clone() };
        let (key, tris) = cache.shared(&copy).unwrap().unwrap();
        assert!(key > last_key);
        assert!(!Arc::ptr_eq(&tris, &original_tris));
        last_key = key;
    }
    assert_eq!(cache.len(), 256);
    let (reinserted_key, reinserted_tris) = cache.shared(&source).unwrap().unwrap();
    assert!(reinserted_key > last_key);
    assert!(!Arc::ptr_eq(&reinserted_tris, &original_tris));
    assert_eq!(cache.len(), 256);
    assert_eq!(sc_session_destroy(id), 0);
}

#[test]
fn assembly_instancing_preserves_glass_and_face_appearance_fallbacks() {
    let _serial = isolated_registry();
    let id = sc_session_create();
    engine_command(id, "component.create", json!({"name":"Styled","grounded":false}));
    for (name, x) in [("Plain", 0), ("Glass", 5), ("Faces", 10)] {
        engine_command(id, "solid.box", json!({"length":2,"width":2,"height":2,"corner":[x,0,0],"body_name":name,"operation":"new"}));
    }
    engine_command(id, "component.activate", json!({"component":0}));
    engine_command(id, "appearance.assign", json!({"bodies":["Glass"],"color":[200,150,120],"opacity":0.4}));
    engine_command(id, "appearance.assign", json!({"body":"Faces","faces":[[11,1,2]],"color":[40,170,80]}));
    let entry = lookup(id).unwrap();
    let snapshot = entry.viewport.lock().unwrap().snapshot().unwrap();
    assert_eq!(snapshot.model.scene.meshes.iter().filter(|mesh| mesh.glass()).count(), 1);
    assert_eq!(snapshot.model.scene.meshes.iter().filter(|mesh| !mesh.face_colors.is_empty()).count(), 1);
    let (gpu, _) = crate::renderer::model_geometry(&snapshot, 100).unwrap();
    assert_eq!(gpu.instanced.len(), 1, "only the ordinary opaque body uses uniform-colour instancing");
    assert_eq!(gpu.instanced[0].count(), 1);
    assert_eq!(gpu.blocks.len(), 1);
    assert!(!gpu.blocks[0].bytes.opaque.is_empty() && !gpu.blocks[0].bytes.glass.is_empty());
    assert!(gpu.palette.entries().contains(&[40, 170, 80, 255]), "face appearance stays in the shared fallback palette");
    assert!(gpu.palette.entries().contains(&[200, 150, 120, 102]), "glass colour and opacity remain intact");
    assert_eq!(sc_session_destroy(id), 0);
}
