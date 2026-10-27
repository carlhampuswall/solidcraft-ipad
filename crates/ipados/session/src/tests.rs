use super::*;
use std::sync::Arc;

#[test]
fn session_owns_and_releases_the_real_document() {
    let adapter = SessionAdapter::new();
    let document = Arc::downgrade(&adapter.session.doc);
    assert!(document.upgrade().is_some());
    assert!(adapter.session.model.state().bodies.is_empty());
    drop(adapter);
    assert!(document.upgrade().is_none());
}

#[test]
fn registry_command_evaluates_a_real_timeline_feature() {
    let mut adapter = SessionAdapter::new();
    let reply = adapter.execute("solid.box", json!({"length": 4, "width": 3, "height": 2})).unwrap();
    assert!(reply.changed);
    assert!(reply.value.get("feature").and_then(Value::as_u64).is_some());
    assert_eq!(adapter.session.doc.features.len(), 1);
    assert_eq!(adapter.session.model.state().bodies.len(), 1);
    let snapshot = adapter.snapshot().unwrap();
    assert!((snapshot.volume_mm3 - 24.0).abs() < 1e-8);
    assert!(snapshot.triangle_count >= 12);
}

#[test]
fn invalid_input_is_controlled_and_preserves_document() {
    let mut adapter = SessionAdapter::new();
    let before = adapter.session.revision;
    let error = adapter.execute("solid.box", json!({"length": true, "width": 3, "height": 2})).unwrap_err();
    assert_eq!(error.code, "bad_params");
    assert_eq!(adapter.session.revision, before);
    assert!(adapter.session.doc.features.is_empty());
    let error = adapter.execute("solid.not_a_command", json!({})).unwrap_err();
    assert_eq!(error.code, "unknown_command");
}

#[test]
fn proof_rebuilds_engine_geometry_and_snapshot_outlives_session() {
    let mut adapter = SessionAdapter::new();
    let snapshot = adapter.proof().unwrap();
    assert_eq!(adapter.session.model.last_recomputed, 1);
    assert_eq!(snapshot.body_count, 1);
    assert!(snapshot.triangle_count >= 12);
    assert!((snapshot.volume_mm3 - 12_000.0).abs() < 1e-6);
    let mesh = &snapshot.scene.meshes.first().unwrap().mesh;
    let bounds = mesh.bounds();
    assert_eq!(bounds.min, solidcraft_engine::geom::Vec3::ZERO);
    assert_eq!(bounds.max, solidcraft_engine::geom::Vec3::new(40.0, 30.0, 10.0));
    drop(adapter);
    assert!((mesh.measure().volume.abs() - 12_000.0).abs() < 1e-6);
}

#[test]
fn proof_cannot_overwrite_an_existing_design() {
    let mut adapter = SessionAdapter::new();
    adapter.proof().unwrap();
    let error = adapter.proof().err().unwrap();
    assert_eq!(error.code, "proof_requires_empty_document");
    assert_eq!(adapter.session.doc.features.len(), 1);
}

#[test]
fn read_only_command_does_not_emit_document_change() {
    let mut adapter = SessionAdapter::new();
    let reply = adapter.execute("engine.describe", json!({"command": "solid.box"})).unwrap();
    assert!(!reply.changed);
    assert_eq!(reply.value.get("id").and_then(Value::as_str), Some("solid.box"));
}

#[test]
fn view_commands_advance_scene_without_changing_document() {
    let mut adapter = SessionAdapter::new();
    let before = adapter.proof().unwrap();
    let reply = adapter.execute("view.top", json!({})).unwrap();
    let after = adapter.snapshot().unwrap();
    assert!(!reply.changed);
    assert_eq!(before.document_id, after.document_id);
    assert_eq!(before.revision, after.revision);
    assert_eq!(before.geometry_revision, after.geometry_revision);
    assert!(after.scene_revision > before.scene_revision);
    assert_ne!(before.camera, after.camera);
    assert_eq!(before.camera.distance, after.camera.distance);
    assert_eq!(before.camera.target, after.camera.target);
    // Shared Camera::set_view deliberately stays 1e-6 radians short of the pole.
    assert!(after.camera.back().dist(solidcraft_engine::geom::Vec3::Z) < 2e-6);
    let command_camera: Camera = serde_json::from_value(reply.value["camera"].clone()).unwrap();
    assert_eq!(after.camera, command_camera);
}

#[test]
fn failed_commands_invalidate_scene_without_changing_document() {
    let mut adapter = SessionAdapter::new();
    let before = adapter.proof().unwrap();
    assert!(adapter.execute("missing.command", json!({})).is_err());
    let after = adapter.snapshot().unwrap();
    assert_eq!(before.revision, after.revision);
    assert_eq!(before.geometry_revision, after.geometry_revision);
    assert!(after.scene_revision > before.scene_revision);
    assert_eq!(before.volume_mm3, after.volume_mm3);
    adapter.scene_revision = u64::MAX;
    assert_eq!(adapter.execute("view.top", json!({})).unwrap_err().code, "scene_revision_limit");
}
