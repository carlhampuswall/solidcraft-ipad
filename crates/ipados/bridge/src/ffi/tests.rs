use super::*;

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
    let id = sc_session_create();
    assert_ne!(id, 0);
    assert_eq!(request(id, r#"{"kind":"proof"}"#), 0);
    let reply = response(id);
    assert_eq!(reply["ok"], true);
    assert_eq!(reply["result"]["body_count"], 1);
    assert_eq!(reply["result"]["volume_mm3"], 12_000.0);
    assert!(reply["result"]["triangle_count"].as_u64().unwrap() >= 12);
    assert_eq!(sc_session_destroy(id), 0);
    assert_eq!(sc_session_destroy(id), -2);
    assert_eq!(request(id, r#"{"kind":"snapshot"}"#), -2);
}

#[test]
fn abi_rejects_bad_json_buffers_handles_and_commands() {
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
    assert_eq!(response(id)["error"]["code"], "surface");
    assert_eq!(sc_session_destroy(id), 0);
}

#[test]
fn input_transports_pencil_and_finger_without_camera_mutation() {
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
    let id = sc_session_create();
    assert_ne!(id, 0);
    assert_eq!(request(id, r#"{"kind":"snapshot"}"#), 0);
    let before = response(id)["result"]["revision"].clone();
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
    assert_eq!(response(id)["result"]["revision"], before);
    assert_eq!(sc_session_destroy(id), 0);
}

#[test]
fn oversized_request_is_rejected_before_json_parsing() {
    let id = sc_session_create();
    assert_ne!(id, 0);
    let oversized = vec![b' '; MAX_REQUEST + 1];
    // SAFETY: the allocation is valid and retained for its complete stated length.
    assert_eq!(unsafe { sc_request(id, oversized.as_ptr(), oversized.len()) }, -1);
    assert_eq!(response(id)["error"]["code"], "request");
    assert_eq!(sc_session_destroy(id), 0);
}
