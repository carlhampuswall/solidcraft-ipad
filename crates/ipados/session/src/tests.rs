use super::*;
use crate::picking::project;
use solidcraft_engine::geom::Vec3;

fn metrics(w: f64, h: f64, scale: f64) -> ViewportMetrics {
    ViewportMetrics { width_points: w, height_points: h, scale, origin_points: [0.0; 2] }
}
fn proof() -> (SessionAdapter, Viewport) {
    let mut adapter = SessionAdapter::new();
    let model = adapter.proof().unwrap();
    let mut viewport = Viewport::default();
    viewport.set_metrics(metrics(800.0, 600.0, 2.0)).unwrap();
    viewport.begin_update(1);
    assert!(viewport.install(1, model));
    (adapter, viewport)
}
fn point(view: &Viewport, world: Vec3) -> [f64; 2] {
    let p = project(view.vp(), world, view.metrics.pixels()).unwrap();
    [p[0] / view.metrics.scale + view.metrics.origin_points[0], p[1] / view.metrics.scale + view.metrics.origin_points[1]]
}
fn event(source: InputSource, phase: InputPhase, id: u64, p: [f64; 2], time: f64) -> InputEvent {
    InputEvent {
        source,
        phase,
        id,
        x_points: p[0],
        y_points: p[1],
        x_pixels: p[0] * 2.0,
        y_pixels: p[1] * 2.0,
        scale: 2.0,
        timestamp: time,
        pressure: None,
        altitude: None,
        geometry_revision: Some(2),
        consumed: false,
        shift: false,
        button: 0,
        scroll: 0.0,
    }
}

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
    let reply = adapter.execute("solid.box", json!({"length":4,"width":3,"height":2})).unwrap();
    assert!(reply.changed);
    assert!(reply.value.get("feature").and_then(Value::as_u64).is_some());
    assert_eq!(adapter.session.doc.features.len(), 1);
    let snapshot = adapter.snapshot().unwrap();
    assert!((snapshot.volume_mm3 - 24.0).abs() < 1e-8);
    assert!(snapshot.triangle_count >= 12);
    assert_eq!(snapshot.picks.bodies.len(), snapshot.body_count);
    assert!(!snapshot.scene.lines.is_empty());
    let mesh = &snapshot.scene.meshes[0].mesh;
    assert!(mesh.edges.len() < mesh.triangles.len() * 3, "model edges, not triangle boundaries");
}

#[test]
fn invalid_input_is_controlled_and_preserves_document() {
    let mut adapter = SessionAdapter::new();
    let before = adapter.session.revision;
    assert_eq!(adapter.execute("solid.box", json!({"length":true,"width":3,"height":2})).unwrap_err().code, "bad_params");
    assert_eq!(adapter.session.revision, before);
    assert!(adapter.session.doc.features.is_empty());
    assert_eq!(adapter.execute("solid.not_a_command", json!({})).unwrap_err().code, "unknown_command");
    let mut view = Viewport::default();
    assert!(view.command("view.navigate", &json!({"motion":"zoom","factor":0})).is_err());
    assert!(view.command("view.perspective", &json!({"enabled":"yes"})).is_err());
}

#[test]
fn proof_snapshot_outlives_session() {
    let (adapter, view) = proof();
    let snapshot = view.snapshot().unwrap();
    assert_eq!(snapshot.model.body_count, 1);
    assert!((snapshot.model.volume_mm3 - 12_000.0).abs() < 1e-6);
    let mesh = snapshot.model.scene.meshes[0].mesh.clone();
    assert_eq!(mesh.bounds().min, Vec3::ZERO);
    assert_eq!(mesh.bounds().max, Vec3::new(40.0, 30.0, 10.0));
    drop(adapter);
    drop(view);
    assert!((mesh.measure().volume.abs() - 12_000.0).abs() < 1e-6);
}

#[test]
fn fixture_is_repeatable_real_commands_and_cannot_overwrite_a_design() {
    let mut adapter = SessionAdapter::new();
    let snapshot = adapter.fixture().unwrap();
    assert_eq!(snapshot.body_count, 2);
    assert!(snapshot.triangle_count > 24);
    assert!(snapshot.scene.lines.len() > snapshot.picks.bodies.iter().map(|b| b.mesh.edges.len()).sum::<usize>());
    assert!(adapter.proof().is_err());
    assert!(adapter.fixture().is_err());
}

#[test]
fn camera_and_selection_keep_geometry_immutable_and_out_of_modelling_history() {
    let (mut adapter, mut view) = proof();
    let before = adapter.session.doc.clone();
    let undo = adapter.session.undo.len();
    let scene = view.snapshot().unwrap().model;
    let camera = view.camera;
    for (motion, extra) in
        [("orbit", json!({"delta":[30,20]})), ("pan", json!({"delta":[20,10],"viewport":[800,600]})), ("zoom", json!({"factor":0.8}))]
    {
        let mut p = extra;
        p["motion"] = json!(motion);
        view.command("view.navigate", &p).unwrap();
    }
    assert_ne!(view.camera, camera);
    for name in ["front", "back", "left", "right", "top", "bottom", "isometric"] {
        view.command(&format!("view.{name}"), &json!({})).unwrap();
        let mut expected = view.camera;
        expected.set_view(solidcraft_engine::standard_view(&format!("view.{name}")).unwrap());
        assert_eq!(expected, view.camera);
    }
    view.command("view.fit", &json!({})).unwrap();
    view.select(Some(Sel::Body { name: "Foundation".into() }), false);
    adapter.selection(view.selection()).unwrap();
    let reply = adapter.execute("engine.describe", json!({"command":"solid.box"})).unwrap();
    assert!(!reply.changed);
    assert_eq!(reply.geometry_revision, scene.geometry_revision);
    assert!(Arc::ptr_eq(&before, &adapter.session.doc));
    assert_eq!(adapter.session.undo.len(), undo);
    assert!(Arc::ptr_eq(&scene, &view.snapshot().unwrap().model));
}

#[test]
fn points_pixels_offsets_rotation_and_projection_pick_the_same_topology() {
    let (_, mut view) = proof();
    for m in [metrics(800.0, 600.0, 1.0), metrics(300.0, 900.0, 2.0), ViewportMetrics { origin_points: [17.0, 83.0], ..metrics(1000.0, 400.0, 3.0) }]
    {
        view.set_metrics(m).unwrap();
        for perspective in [false, true] {
            view.command("view.perspective", &json!({"enabled":perspective})).unwrap();
            view.command("view.top", &json!({})).unwrap();
            view.command("view.fit", &json!({})).unwrap();
            for (filter, p, kind) in [
                (PickFilter::Body, Vec3::new(20.0, 15.0, 10.0), "body"),
                (PickFilter::Face, Vec3::new(20.0, 15.0, 10.0), "face"),
                (PickFilter::Edge, Vec3::new(20.0, 0.0, 10.0), "edge"),
                (PickFilter::Vertex, Vec3::new(0.0, 0.0, 10.0), "vertex"),
            ] {
                view.filter = filter;
                let sel = view.pick(point(&view, p), None).unwrap_or_else(|| panic!("missing {kind} {m:?}, perspective={perspective}"));
                assert_eq!(serde_json::to_value(&sel).unwrap()["type"], kind);
                match sel {
                    Sel::Face { point, .. } | Sel::Edge { point, .. } | Sel::Vertex { point, .. } => {
                        assert!((point.z - 10.0).abs() < 1e-5, "must hit visible top, not occluded underside: {point:?}")
                    }
                    _ => {}
                }
            }
        }
    }
}

#[test]
fn fit_handles_empty_tiny_large_off_origin_and_portrait_scenes() {
    let mut view = Viewport::default();
    view.set_metrics(metrics(250.0, 900.0, 2.0)).unwrap();
    view.command("view.fit", &json!({})).unwrap();
    assert!(view.camera.is_valid());
    assert!(view.snapshot().is_none());
    for (size, origin) in [(0.001, [100.0, 200.0, -300.0]), (10000.0, [50000.0, -30000.0, 10000.0])] {
        let mut adapter = SessionAdapter::new();
        adapter.execute("solid.box", json!({"length":size,"width":size*0.7,"height":size*0.4,"corner":origin})).unwrap();
        view.begin_update(2);
        view.install(2, adapter.snapshot().unwrap());
        for perspective in [false, true] {
            view.command("view.perspective", &json!({"enabled":perspective})).unwrap();
            view.command("view.fit", &json!({})).unwrap();
            assert!(view.camera.is_valid());
            let b = view.model.as_ref().unwrap().bounds;
            let clip = view.camera.clip_bounds(&b);
            assert!(clip.0 < clip.1);
            for x in [b.min.x, b.max.x] {
                for y in [b.min.y, b.max.y] {
                    for z in [b.min.z, b.max.z] {
                        let p = project(view.vp(), Vec3::new(x, y, z), view.metrics.pixels()).unwrap();
                        assert!(p[0] > 0.0 && p[0] < 500.0 && p[1] > 0.0 && p[1] < 1800.0, "{p:?}");
                    }
                }
            }
        }
    }
    view.set_metrics(metrics(0.0, 0.0, 2.0)).unwrap();
    assert!(view.pick([0.0, 0.0], None).is_none());
    assert!(metrics(f64::NAN, 300.0, 2.0).validate().is_err());
}

#[test]
fn stale_async_results_and_stale_taps_are_discarded() {
    let (mut adapter, mut view) = proof();
    let old = view.model.clone().unwrap();
    view.select(Some(Sel::Body { name: "Foundation".into() }), false);
    view.begin_update(2);
    // Old model remains renderable while navigation works without locking the engine.
    view.command("view.navigate", &json!({"motion":"orbit","delta":[20,10]})).unwrap();
    assert!(Arc::ptr_eq(&old, &view.snapshot().unwrap().model));
    assert!(view.pick([400.0, 300.0], None).is_none());
    adapter.execute("timeline.edit", json!({"feature":"Foundation","set":{"height":"20"}})).unwrap();
    let new = adapter.snapshot().unwrap();
    assert!(!view.install(1, old.clone()));
    assert!(view.install(2, new));
    assert!(view.selection.is_empty());
    assert!(view.hover.is_none());
    assert!(view.pick([400.0, 300.0], Some(old.geometry_revision)).is_none());
    assert!(!view.install(1, old));
}

#[test]
fn edit_undo_redo_deletion_and_highlight_use_current_engine_entities() {
    let (mut adapter, mut view) = proof();
    view.command("view.top", &json!({})).unwrap();
    view.filter = PickFilter::Face;
    let hit = view.pick(point(&view, Vec3::new(20.0, 15.0, 10.0)), None).unwrap();
    view.select(Some(hit), false);
    let snap = view.snapshot().unwrap();
    assert!(!snap.highlight.meshes.is_empty());
    let selected_face = match view.selection[0] {
        Sel::Face { index, .. } => index,
        _ => panic!("face"),
    };
    assert!(snap.highlight.meshes[0].mesh.tri_face.iter().all(|f| *f as usize == selected_face));
    let old_geometry = snap.geometry_revision;
    for (generation, id, params, height) in [
        (2, "timeline.edit", json!({"feature":"Foundation","set":{"height":"20"}}), 20.0),
        (3, "edit.undo", json!({}), 10.0),
        (4, "edit.redo", json!({}), 20.0),
    ] {
        view.begin_update(generation);
        adapter.selection(view.selection()).unwrap();
        adapter.execute(id, params).unwrap();
        let next = adapter.snapshot().unwrap();
        assert!(next.geometry_revision > old_geometry);
        assert_eq!(next.bounds.max.z, height);
        assert!(view.install(generation, next));
        assert!(view.selection.is_empty());
    }
    adapter.execute("timeline.delete", json!({"feature":"Foundation"})).unwrap();
    view.begin_update(5);
    assert!(view.install(5, adapter.snapshot().unwrap()));
    assert_eq!(view.model.as_ref().unwrap().body_count, 0);
    assert!(view.pick([400.0, 300.0], None).is_none());
}

#[test]
fn hidden_geometry_cannot_be_picked() {
    let (mut adapter, mut view) = proof();
    adapter.execute("browser.visibility", json!({"items":["Foundation"],"visible":false})).unwrap();
    view.begin_update(2);
    view.install(2, adapter.snapshot().unwrap());
    assert_eq!(view.model.as_ref().unwrap().body_count, 0);
    assert!(view.pick([400.0, 300.0], None).is_none());
}

#[test]
fn basic_source_routing_consumption_multiselect_and_hover() {
    let (adapter, mut view) = proof();
    let before = adapter.session.doc.clone();
    let camera = view.camera;
    view.input(event(InputSource::Finger, InputPhase::Began, 1, [400.0, 300.0], 1.0)).unwrap();
    view.input(event(InputSource::Finger, InputPhase::Moved, 1, [430.0, 310.0], 2.0)).unwrap();
    view.input(event(InputSource::Finger, InputPhase::Ended, 1, [430.0, 310.0], 3.0)).unwrap();
    assert_ne!(camera, view.camera);
    assert!(view.selection.is_empty());
    view.command("view.top", &json!({})).unwrap();
    let p = point(&view, Vec3::new(20.0, 15.0, 10.0));
    for phase in [InputPhase::Began, InputPhase::Ended] {
        view.input(event(InputSource::Pencil, phase, 2, p, 4.0)).unwrap();
    }
    assert_eq!(view.selection.len(), 1);
    let cam = view.camera;
    let mut consumed = event(InputSource::Finger, InputPhase::Began, 4, p, 6.0);
    consumed.consumed = true;
    assert_eq!(view.input(consumed).unwrap()["handled"], false);
    assert_eq!(cam, view.camera);
    view.input(event(InputSource::Indirect, InputPhase::Hover, 0, p, 7.0)).unwrap();
    assert!(view.hover.is_some());
    view.command("select.multiple", &json!({"enabled":true})).unwrap();
    view.filter = PickFilter::Body;
    for phase in [InputPhase::Began, InputPhase::Ended] {
        view.input(event(InputSource::Indirect, phase, 3, p, 8.0)).unwrap();
    }
    assert_eq!(view.selection.len(), 2);
    view.command("select.clear", &json!({})).unwrap();
    assert!(view.selection.is_empty());
    assert!(Arc::ptr_eq(&before, &adapter.session.doc));
    let mut invalid = event(InputSource::Pencil, InputPhase::Moved, 2, p, 9.0);
    invalid.x_pixels += 1.0;
    assert_eq!(view.input(invalid).unwrap_err().code, "input");
}

#[test]
fn two_finger_pan_pinch_and_pointer_navigation_never_select_or_edit() {
    let (adapter, mut view) = proof();
    let before = adapter.session.doc.clone();
    for (id, p) in [(1, [300.0, 300.0]), (2, [500.0, 300.0])] {
        view.input(event(InputSource::Finger, InputPhase::Began, id, p, 1.0)).unwrap();
    }
    let cam = view.camera;
    view.input(event(InputSource::Finger, InputPhase::Moved, 2, [550.0, 330.0], 2.0)).unwrap();
    assert_ne!(cam.target, view.camera.target);
    assert_ne!(cam.distance, view.camera.distance);
    assert_eq!((cam.yaw, cam.pitch), (view.camera.yaw, view.camera.pitch));
    assert!(view.selection.is_empty());
    for (id, p) in [(1, [300.0, 300.0]), (2, [550.0, 330.0])] {
        view.input(event(InputSource::Finger, InputPhase::Cancelled, id, p, 3.0)).unwrap();
    }
    let camera = view.camera;
    for (phase, p, t) in
        [(InputPhase::Began, [400.0, 300.0], 4.0), (InputPhase::Moved, [420.0, 310.0], 5.0), (InputPhase::Ended, [420.0, 310.0], 6.0)]
    {
        let mut e = event(InputSource::Indirect, phase, 3, p, t);
        e.button = 1;
        view.input(e).unwrap();
    }
    assert_ne!(camera.yaw, view.camera.yaw);
    assert!(view.selection.is_empty());
    assert!(Arc::ptr_eq(&before, &adapter.session.doc));
}

#[test]
fn nearer_body_occludes_faces_edges_and_vertices_and_hidden_body_does_not() {
    let mut adapter = SessionAdapter::new();
    for (name, z) in [("Rear", 0), ("Front", 15)] {
        adapter.execute("solid.box", json!({"length":20,"width":20,"height":5,"corner":[0,0,z],"operation":"new","body_name":name})).unwrap();
    }
    let mut view = Viewport::default();
    view.set_metrics(metrics(800.0, 600.0, 2.0)).unwrap();
    view.begin_update(1);
    view.install(1, adapter.snapshot().unwrap());
    view.command("view.top", &json!({})).unwrap();
    for (filter, p) in [
        (PickFilter::Face, Vec3::new(10.0, 10.0, 20.0)),
        (PickFilter::Edge, Vec3::new(10.0, 0.0, 20.0)),
        (PickFilter::Vertex, Vec3::new(0.0, 0.0, 20.0)),
    ] {
        view.filter = filter;
        let selected = view.pick(point(&view, p), None).unwrap();
        match selected {
            Sel::Face { body, .. } | Sel::Edge { body, .. } | Sel::Vertex { body, .. } => assert_eq!(body, "Front"),
            _ => panic!("wrong type"),
        }
    }
    adapter.execute("browser.visibility", json!({"items":["Front"],"visible":false})).unwrap();
    view.begin_update(2);
    view.install(2, adapter.snapshot().unwrap());
    view.filter = PickFilter::Face;
    match view.pick(point(&view, Vec3::new(10.0, 10.0, 5.0)), None).unwrap() {
        Sel::Face { body, .. } => assert_eq!(body, "Rear"),
        _ => panic!("face"),
    }
}

#[test]
fn occurrence_world_transform_is_shared_by_render_bounds_and_picker() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("component.create", json!({"name":"Part","grounded":false})).unwrap();
    adapter.execute("solid.box", json!({"length":40,"width":30,"height":10})).unwrap();
    adapter.execute("occurrence.move", json!({"occurrence":"Part:1","translate":[50,20,10]})).unwrap();
    let scene = adapter.snapshot().unwrap();
    assert_eq!(scene.bounds.min, Vec3::new(50.0, 20.0, 10.0));
    assert_eq!(scene.scene.meshes[0].mesh.bounds().min, scene.bounds.min);
    let mut view = Viewport::default();
    view.set_metrics(metrics(800.0, 600.0, 2.0)).unwrap();
    view.begin_update(1);
    view.install(1, scene);
    view.command("view.top", &json!({})).unwrap();
    match view.pick(point(&view, Vec3::new(70.0, 35.0, 20.0)), None).unwrap() {
        Sel::Face { point, .. } => assert!((point.z - 20.0).abs() < 1e-5),
        _ => panic!("face"),
    }
}

#[test]
fn projection_readout_and_presented_revision_gate_synthetic_input() {
    let (_, mut view) = proof();
    view.command("view.top", &json!({})).unwrap();
    let camera = view.camera;
    let revision = view.diagnostic()["scene_revision"].clone();
    let projected = view.command("view.project", &json!({"point":[20,15,10]})).unwrap();
    assert_eq!(view.camera, camera);
    assert_eq!(view.diagnostic()["scene_revision"], revision);
    let p = [projected["points"][0].as_f64().unwrap(), projected["points"][1].as_f64().unwrap()];
    assert_eq!(projected["pixels"][0].as_f64().unwrap(), p[0] * 2.0);
    assert_eq!(projected["geometry_revision"], 2);
    for phase in [InputPhase::Began, InputPhase::Ended] {
        let mut sample = event(InputSource::Pencil, phase, 10, p, 1.0);
        sample.geometry_revision = None;
        view.input(sample).unwrap();
    }
    assert!(view.selection.is_empty(), "no selection before any scene has been presented");
    for phase in [InputPhase::Began, InputPhase::Ended] {
        view.input(event(InputSource::Pencil, phase, 10, p, 2.0)).unwrap();
    }
    assert_eq!(view.selection.len(), 1);
    assert!(view.command("view.project", &json!({"point":["bad",1,2]})).is_err());
}

#[test]
fn orthographic_close_zoom_picks_the_visible_front_surface() {
    let (_, mut view) = proof();
    view.command("view.top", &json!({})).unwrap();
    for _ in 0..3 {
        view.command("view.navigate", &json!({"motion":"zoom","factor":0.05})).unwrap();
    }
    let p = Vec3::new(20.0, 15.0, 10.0);
    match view.pick(point(&view, p), None).unwrap() {
        Sel::Face { point, .. } => assert!((point.z - 10.0).abs() < 1e-6, "pick must start at the render clip plane: {point:?}"),
        _ => panic!("face"),
    }
}

#[test]
fn whole_sphere_body_feedback_uses_the_real_body_even_without_display_edges() {
    let mut engine = SessionAdapter::new();
    engine.execute("solid.sphere", json!({"radius":10,"body_name":"Sphere"})).unwrap();
    let mut view = Viewport::default();
    view.set_metrics(metrics(800.0, 600.0, 2.0)).unwrap();
    view.begin_update(1);
    view.install(1, engine.snapshot().unwrap());
    view.command("view.top", &json!({})).unwrap();
    view.filter = PickFilter::Body;
    let hit = view.pick(point(&view, Vec3::new(0.0, 0.0, 10.0)), None).unwrap();
    view.select(Some(hit), false);
    let snapshot = view.snapshot().unwrap();
    assert_eq!(snapshot.selected_bodies, vec!["Sphere"]);
    assert!(snapshot.selection_tint.iter().any(|v| *v > 0.0));
    assert!(snapshot.highlight.meshes.is_empty(), "body feedback references existing GPU vertices");
}

#[test]
fn camera_motion_reuses_face_highlight_geometry_and_body_upload() {
    let (_, mut view) = proof();
    view.command("view.top", &json!({})).unwrap();
    let hit = view.pick(point(&view, Vec3::new(20.0, 15.0, 10.0)), None).unwrap();
    view.select(Some(hit), false);
    let before = view.snapshot().unwrap();
    view.command("view.navigate", &json!({"motion":"orbit","delta":[10,20]})).unwrap();
    let after = view.snapshot().unwrap();
    assert!(Arc::ptr_eq(&before.model, &after.model));
    assert!(Arc::ptr_eq(&before.highlight, &after.highlight));
    assert_eq!(before.highlight_revision, after.highlight_revision);
    assert_ne!(before.scene_revision, after.scene_revision);
}
