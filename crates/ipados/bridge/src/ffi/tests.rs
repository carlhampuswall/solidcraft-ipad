use super::*;

// The production registry deliberately caps eight live sessions. Cargo can run
// more ABI tests concurrently in one process, unlike the repository test runner.
static ABI_TESTS: Mutex<()> = Mutex::new(());

fn isolated_registry() -> std::sync::MutexGuard<'static, ()> {
    ABI_TESTS.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
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
