use super::*;
use crate::picking::project;
use solidcore_record::Vec3;
use solidcraft_engine::doc::FeatureKind;

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

#[test]
fn viewport_background_follows_appearance_without_rebuilding_model_or_history() {
    let (mut adapter, mut viewport) = proof();
    let before = viewport.snapshot().unwrap();
    let camera = viewport.diagnostic()["camera"].clone();
    let history = adapter.session.undo.len();
    assert_eq!(before.background, before.model.scene.background);

    let dark = solidcraft_render::Rgb(28, 28, 30);
    let reply = viewport.command("view.background", &json!({"color":[28,28,30]})).unwrap();
    assert_eq!(reply["background"], json!([28, 28, 30]));
    let after = viewport.snapshot().unwrap();
    assert_eq!(after.background, Some((dark, dark)));
    assert!(std::sync::Arc::ptr_eq(&before.model, &after.model));
    assert_eq!(after.scene_revision, before.scene_revision, "appearance must not trigger geometry uploads");
    assert_eq!(viewport.diagnostic()["camera"], camera);
    assert_eq!(after.model.scene.background, before.background, "the immutable model retains its original palette");

    viewport.begin_update(2);
    assert!(viewport.install(2, adapter.snapshot().unwrap()));
    assert_eq!(viewport.snapshot().unwrap().background, Some((dark, dark)), "scene publication must retain the native appearance");
    viewport.command("view.background", &json!({"color":null})).unwrap();
    assert_eq!(viewport.snapshot().unwrap().background, before.background, "Light restores the original background");

    let before_grid = viewport.snapshot().unwrap();
    let light_grid: Vec<_> = before_grid.guides.grid.iter().map(|line| line.color).collect();
    let colors = [solidcraft_render::Rgb(45, 45, 47), solidcraft_render::Rgb(62, 62, 65)];
    viewport.command("view.background", &json!({"color":[28,28,30],"grid":[[45,45,47],[62,62,65]]})).unwrap();
    let themed = viewport.snapshot().unwrap();
    assert!(!themed.guides.grid.is_empty());
    assert!(themed.guides.grid.iter().all(|line| colors.contains(&line.color)));
    assert!(std::sync::Arc::ptr_eq(&themed.model, &before_grid.model));
    assert_eq!(viewport.diagnostic()["camera"], camera);
    let revision = themed.guide_revision;
    viewport.command("view.background", &json!({"color":[28,28,30],"grid":[[45,45,47],[62,62,65]]})).unwrap();
    assert_eq!(viewport.snapshot().unwrap().guide_revision, revision, "reapplying appearance must reuse the guides");
    viewport.command("view.background", &json!({"color":null})).unwrap();
    let restored: Vec<_> = viewport.snapshot().unwrap().guides.grid.iter().map(|line| line.color).collect();
    assert_eq!(restored, light_grid, "Light restores the original grid palette");
    assert_eq!(adapter.session.undo.len(), history);
}

#[test]
fn viewport_background_rejects_invalid_channels_without_changing_appearance() {
    let mut viewport = Viewport::default();
    viewport.command("view.background", &json!({"color":[28,28,30]})).unwrap();
    for color in [json!([-1, 0, 0]), json!([256, 0, 0]), json!([1, 2]), json!([1, 2, 3, 4]), json!([1.5, 2, 3]), json!("dark"), json!(true)] {
        assert!(viewport.command("view.background", &json!({"color":color})).is_err());
        assert_eq!(viewport.diagnostic()["background"], json!([28, 28, 30]));
    }
    for grid in [json!([[45, 45, 47]]), json!([[45, 45, 47], [256, 62, 65]]), json!([[45, 45, 47], [62, -1, 65]]), json!(true)] {
        assert!(viewport.command("view.background", &json!({"color":[99,99,99],"grid":grid})).is_err());
        assert_eq!(viewport.diagnostic()["background"], json!([28, 28, 30]), "invalid grid colours must not change the background either");
    }
    viewport.command("view.background", &json!({"color":[28,28,30],"sketch":[100,210,255]})).unwrap();
    for sketch in [json!([100, 210]), json!([100, 210, 256]), json!([-1, 210, 255]), json!([100.5, 210, 255]), json!(true)] {
        assert!(viewport.command("view.background", &json!({"color":[99,99,99],"sketch":sketch})).is_err());
        assert_eq!(viewport.diagnostic()["background"], json!([28, 28, 30]));
        assert_eq!(viewport.diagnostic()["sketch_color"], json!([100, 210, 255]));
    }
}

#[test]
fn viewport_grid_toggle_preserves_references_camera_model_and_history_in_3d_and_sketch_views() {
    let (mut adapter, mut viewport) = proof();
    viewport.command("view.grid", &json!({"visible":true,"integrated_axes":false})).unwrap();
    viewport.command("view.origin", &json!({"visible":true})).unwrap();
    let before = viewport.snapshot().unwrap();
    let diagnostic = viewport.diagnostic();
    let history = adapter.session.undo.len();
    assert!(!before.guides.grid.is_empty(), "grid starts visible");
    let reply = viewport.command("view.grid", &json!({"visible":false})).unwrap();
    assert_eq!(reply["grid"]["visible"], false);
    let hidden = viewport.snapshot().unwrap();
    assert!(hidden.guides.grid.is_empty());
    assert_eq!(hidden.guides.lines.len(), before.guides.lines.len(), "axes and triad remain visible");
    assert_eq!(hidden.guides.meshes.len(), before.guides.meshes.len(), "planes and origin point remain visible");
    assert!(std::sync::Arc::ptr_eq(&hidden.model, &before.model));
    assert_eq!(reply["camera"], diagnostic["camera"]);
    assert_eq!(reply["camera_epoch"], diagnostic["camera_epoch"]);
    assert_eq!(reply["origin"], diagnostic["origin"]);

    viewport.command("view.background", &json!({"color":[28,28,30],"grid":[[45,45,47],[62,62,65]]})).unwrap();
    viewport.begin_update(2);
    assert!(viewport.install(2, adapter.snapshot().unwrap()));
    assert!(viewport.snapshot().unwrap().guides.grid.is_empty(), "hidden survives theme and scene changes");
    viewport.command("view.sketch_plane", &json!({"plane":solidcore_record::Plane::XY})).unwrap();
    assert!(viewport.snapshot().unwrap().guides.grid.is_empty(), "the same toggle applies to planar sketch views");
    viewport.command("view.grid", &json!({"visible":true})).unwrap();
    assert!(!viewport.snapshot().unwrap().guides.grid.is_empty());
    for visible in [json!(null), json!("false"), json!(0), json!([])] {
        assert!(viewport.command("view.grid", &json!({"visible":visible})).is_err());
        assert_eq!(viewport.diagnostic()["grid"]["visible"], true);
    }
    assert_eq!(adapter.session.undo.len(), history);
}

#[test]
fn viewport_integrated_axes_follow_the_visible_grid_plane_and_world_up_axis() {
    let mut engine = SessionAdapter::new();
    let mut view = Viewport::default();
    view.set_metrics(metrics(800., 600., 2.)).unwrap();
    view.begin_update(1);
    view.install(1, engine.snapshot().unwrap());
    let model = view.snapshot().unwrap().model.clone();
    assert_eq!(view.diagnostic()["grid"]["integrated_axes"], true);
    for (up, standard, expected) in [
        (solidcraft_render::UpAxis::Z, "view.isometric", [true, true, false]),
        (solidcraft_render::UpAxis::Z, "view.top", [true, true, false]),
        (solidcraft_render::UpAxis::Z, "view.front", [true, false, true]),
        (solidcraft_render::UpAxis::Z, "view.right", [false, true, true]),
        (solidcraft_render::UpAxis::Y, "view.top", [true, false, true]),
        (solidcraft_render::UpAxis::Y, "view.front", [true, true, false]),
        (solidcraft_render::UpAxis::Y, "view.right", [false, true, true]),
    ] {
        view.camera.up = up;
        view.command(standard, &json!({})).unwrap();
        assert_eq!(
            view.diagnostic()["origin"]["visible"],
            json!([false, expected[0], expected[1], expected[2], false, false, false]),
            "{up:?} {standard}"
        );
        let snapshot = view.snapshot().unwrap();
        for ((axis, color), visible) in [Vec3::X, Vec3::Y, Vec3::Z].into_iter().zip(solidcraft_engine::view::Palette::DEFAULT.axes).zip(expected) {
            let lines: Vec<_> = snapshot.guides.lines.iter().filter(|line| !line.on_top && line.color == color).collect();
            assert_eq!(!lines.is_empty(), visible, "{up:?} {standard} {axis:?}");
            if visible {
                assert!(lines.iter().flat_map(|line| &line.points).all(|point| point.cross(axis).len() < 1e-8));
                assert!(lines.iter().flat_map(|line| &line.points).any(|point| point.dot(axis) < -1.));
                assert!(lines.iter().flat_map(|line| &line.points).any(|point| point.dot(axis) > 1.));
                assert!(lines.iter().any(|line| line.alpha < 1.), "axes fade with the grid");
                assert!(
                    !snapshot.guides.grid.iter().any(|line| line.points.iter().all(|point| point.cross(axis).len() < 1e-8)),
                    "neutral grid must not overdraw the coloured axis"
                );
            }
        }
        assert!(view.diagnostic()["reference_labels"].as_array().unwrap().is_empty(), "automatic grid lines have no reference labels");
        assert!(Arc::ptr_eq(&model, &snapshot.model));
    }
    assert!(engine.session.undo.is_empty());
}

#[test]
fn viewport_integrated_axis_overrides_survive_plane_changes_and_grid_visibility() {
    let mut engine = SessionAdapter::new();
    let mut view = Viewport::default();
    view.set_metrics(metrics(800., 600., 2.)).unwrap();
    view.begin_update(1);
    view.install(1, engine.snapshot().unwrap());
    let before = view.snapshot().unwrap();
    view.command("view.top", &json!({})).unwrap();
    assert!(
        view.pick(point(&view, Vec3::new(-10., 0., 0.)), Some(engine.geometry_revision)).is_none(),
        "automatic grid guides must not capture empty-space taps"
    );
    view.command("view.origin", &json!({"item":"X","visible":false})).unwrap();
    view.command("view.origin", &json!({"item":"Z","visible":true})).unwrap();
    assert_eq!(view.diagnostic()["origin"]["visible"], json!([false, false, true, true, false, false, false]));
    view.command("view.front", &json!({})).unwrap();
    assert_eq!(view.diagnostic()["origin"]["visible"], json!([false, false, false, true, false, false, false]));
    view.command("view.grid", &json!({"visible":false})).unwrap();
    assert_eq!(view.diagnostic()["origin"]["visible"], json!([false, false, false, true, false, false, false]), "explicit Show survives hidden grid");
    assert!(view.snapshot().unwrap().guides.grid.is_empty());
    view.command("view.origin", &json!({"item":"Z","visible":null})).unwrap();
    assert_eq!(view.diagnostic()["origin"]["visible"], json!(vec![false; 7]), "automatic axes follow the hidden grid");
    view.command("view.grid", &json!({"visible":true})).unwrap();
    assert_eq!(view.diagnostic()["origin"]["visible"], json!([false, false, false, true, false, false, false]));
    view.command("view.origin", &json!({"item":"X","visible":null})).unwrap();
    assert_eq!(view.diagnostic()["origin"]["visible"], json!([false, true, false, true, false, false, false]));
    assert!(Arc::ptr_eq(&before.model, &view.snapshot().unwrap().model));
    assert!(engine.session.undo.is_empty());
}

#[test]
fn viewport_integrated_axes_preference_retains_legacy_defaults_and_validates_atomically() {
    let mut engine = SessionAdapter::new();
    let mut view = Viewport::default();
    view.set_metrics(metrics(800., 600., 2.)).unwrap();
    view.begin_update(1);
    view.install(1, engine.snapshot().unwrap());
    view.command("view.grid", &json!({"visible":true,"integrated_axes":false})).unwrap();
    assert_eq!(view.diagnostic()["origin"]["visible"], json!(vec![false; 7]));
    assert!(view.snapshot().unwrap().guides.lines.iter().all(|line| line.on_top));
    view.command("view.origin", &json!({"item":"X","visible":true})).unwrap();
    let snapshot = view.snapshot().unwrap();
    let axes: Vec<_> = snapshot.guides.lines.iter().filter(|line| !line.on_top).collect();
    assert_eq!(axes.len(), 1, "legacy explicit axes stay single short positive segments");
    assert_eq!(axes[0].points, vec![Vec3::ZERO, Vec3::X * 30.]);
    view.command("view.grid", &json!({"visible":false})).unwrap();
    assert_eq!(view.diagnostic()["grid"]["integrated_axes"], false, "the existing Grid command preserves the mode");
    for invalid in [json!(null), json!("true"), json!(0), json!([])] {
        let before = view.diagnostic();
        assert!(view.command("view.grid", &json!({"visible":true,"integrated_axes":invalid})).is_err());
        assert_eq!(view.diagnostic()["grid"], before["grid"], "invalid mode must not change Grid visibility either");
        assert_eq!(view.diagnostic()["origin"], before["origin"]);
    }
    view.command("view.origin", &json!({"visible":null})).unwrap();
    view.command("view.grid", &json!({"visible":true,"integrated_axes":true})).unwrap();
    assert_eq!(view.diagnostic()["origin"]["visible"], json!([false, true, true, false, false, false, false]));
    assert!(engine.session.undo.is_empty());
}

#[test]
fn viewport_integrated_axes_use_sketch_plane_world_colours_and_keep_offset_local_guides() {
    use solidcore_record::Plane;
    let offset = Plane::new(Vec3::new(5., 6., 7.), Vec3::Y, Vec3::Z).unwrap();
    let tilted = Plane::new(Vec3::new(5., 6., 7.), Vec3::new(1., 1., 0.), Vec3::Z).unwrap();
    for (plane, expected) in [(Plane::XZ, [true, false, true]), (Plane::YZ, [false, true, true]), (offset, [false; 3]), (tilted, [false; 3])] {
        let mut engine = SessionAdapter::new();
        engine.execute("sketch.create", json!({"plane":plane})).unwrap();
        let mut view = Viewport::default();
        view.set_metrics(metrics(800., 600., 2.)).unwrap();
        view.begin_update(1);
        view.install(1, engine.snapshot().unwrap());
        assert_eq!(view.diagnostic()["origin"]["visible"], json!([false, expected[0], expected[1], expected[2], false, false, false]));
        let snapshot = view.snapshot().unwrap();
        let lines: Vec<_> = snapshot.guides.lines.iter().filter(|line| !line.on_top).collect();
        assert!(!lines.is_empty(), "offset/tilted sketches retain their local grid axes");
        assert!(lines.iter().flat_map(|line| &line.points).all(|point| plane.height(*point).abs() < 1e-8));
        for direction in [plane.x, plane.y] {
            let axis = [Vec3::X, Vec3::Y, Vec3::Z].into_iter().position(|axis| direction.dot(axis).abs() > 1. - 1e-8);
            if let Some(index) = axis {
                let matching: Vec<_> =
                    lines.iter().filter(|line| line.points.iter().all(|point| (*point - plane.origin).cross(direction).len() < 1e-8)).collect();
                assert!(!matching.is_empty());
                assert!(
                    matching.iter().all(|line| line.color == solidcraft_engine::view::Palette::DEFAULT.axes[index]),
                    "principal sketch axes retain world XYZ colours"
                );
            }
        }
        assert!(view.diagnostic()["reference_labels"].as_array().unwrap().is_empty());
        let history = engine.session.undo.len();
        view.command("view.origin", &json!({"group":"axes","visible":false})).unwrap();
        assert!(view.snapshot().unwrap().guides.lines.iter().all(|line| line.on_top), "axis toggles also hide local sketch guides");
        assert_eq!(engine.session.undo.len(), history);
    }
}

#[test]
fn viewport_explicit_integrated_axes_pick_the_negative_and_panned_visible_span() {
    let mut engine = SessionAdapter::new();
    let mut view = Viewport::default();
    view.set_metrics(metrics(800., 600., 2.)).unwrap();
    view.begin_update(1);
    view.install(1, engine.snapshot().unwrap());
    view.command("view.top", &json!({})).unwrap();
    view.command("view.origin", &json!({"item":"X","visible":true})).unwrap();
    assert_eq!(view.pick(point(&view, Vec3::new(-10., 0., 0.)), Some(engine.geometry_revision)), Some(Sel::Axis { name: "X".into() }));
    view.camera.target = Vec3::new(-50., 0., 0.);
    view.changed();
    assert_eq!(
        view.pick(point(&view, Vec3::new(-50., 0., 0.)), Some(engine.geometry_revision)),
        Some(Sel::Axis { name: "X".into() }),
        "panned visible axes remain pickable beyond the old positive segment"
    );
    view.command("view.origin", &json!({"item":"X","visible":false})).unwrap();
    assert!(view.pick(point(&view, Vec3::new(-50., 0., 0.)), Some(engine.geometry_revision)).is_none());
}

#[test]
fn viewport_origin_axes_stay_short_outside_the_current_visible_grid() {
    fn assert_short_axis(view: &Viewport, revision: u64, index: usize, name: &str, axis: Vec3) {
        let snapshot = view.snapshot().unwrap();
        let lines: Vec<_> =
            snapshot.guides.lines.iter().filter(|line| !line.on_top && line.color == solidcraft_engine::view::Palette::DEFAULT.axes[index]).collect();
        assert_eq!(lines.len(), 1, "{name} stays a single origin reference marker");
        assert_eq!(lines[0].points, vec![Vec3::ZERO, axis * 30.], "{name} retains its short positive origin extent");
        for (distance, expected) in [(20., true), (-20., false), (60., false)] {
            assert_eq!(
                view.pick(point(view, axis * distance), Some(revision)),
                expected.then(|| Sel::Axis { name: name.into() }),
                "{name} at {distance} mm must follow its displayed short segment"
            );
        }
    }

    let mut engine = SessionAdapter::new();
    let mut view = Viewport::default();
    view.set_metrics(metrics(800., 600., 2.)).unwrap();
    view.begin_update(1);
    view.install(1, engine.snapshot().unwrap());
    let before = view.snapshot().unwrap();
    view.command("view.origin", &json!({"group":"axes","visible":false})).unwrap();
    view.command("view.origin", &json!({"item":"Z","visible":true})).unwrap();
    assert_short_axis(&view, engine.geometry_revision, 2, "Z", Vec3::Z);

    view.command("view.front", &json!({})).unwrap();
    let in_plane = view.snapshot().unwrap();
    assert!(in_plane.guides.lines.iter().filter(|line| !line.on_top).any(|line| line.points.iter().any(|point| point.z < -30.)));
    for distance in [-20., 60.] {
        assert_eq!(view.pick(point(&view, Vec3::Z * distance), Some(engine.geometry_revision)), Some(Sel::Axis { name: "Z".into() }));
    }
    view.command("view.grid", &json!({"visible":false})).unwrap();
    assert!(view.snapshot().unwrap().guides.grid.is_empty());
    assert_short_axis(&view, engine.geometry_revision, 2, "Z", Vec3::Z);
    view.command("view.grid", &json!({"visible":true})).unwrap();
    assert_eq!(view.pick(point(&view, Vec3::Z * -20.), Some(engine.geometry_revision)), Some(Sel::Axis { name: "Z".into() }));
    view.command("view.isometric", &json!({})).unwrap();
    assert_short_axis(&view, engine.geometry_revision, 2, "Z", Vec3::Z);

    view.command("view.origin", &json!({"group":"axes","visible":true})).unwrap();
    view.command("view.grid", &json!({"visible":false})).unwrap();
    for (index, (name, axis)) in [("X", Vec3::X), ("Y", Vec3::Y), ("Z", Vec3::Z)].into_iter().enumerate() {
        assert_short_axis(&view, engine.geometry_revision, index, name, axis);
    }
    assert!(Arc::ptr_eq(&before.model, &view.snapshot().unwrap().model));
    assert!(engine.session.undo.is_empty());
}

#[test]
fn viewport_integrated_axis_picking_keeps_visible_middles_with_clipped_endpoints() {
    let mut engine = SessionAdapter::new();
    let mut view = Viewport::default();
    view.set_metrics(metrics(800., 600., 2.)).unwrap();
    view.begin_update(1);
    view.install(1, engine.snapshot().unwrap());
    for fov in [0., std::f64::consts::FRAC_PI_4] {
        view.camera.target = Vec3::ZERO;
        view.camera.yaw = 0.;
        view.camera.pitch = 0.21;
        view.camera.fov = fov;
        view.command("view.origin", &json!({"item":"Y","visible":true})).unwrap();
        view.changed();
        assert_eq!(
            view.pick(point(&view, Vec3::new(0., -10., 0.)), Some(engine.geometry_revision)),
            Some(Sel::Axis { name: "Y".into() }),
            "visible axis middles remain pickable in orthographic and perspective views"
        );
    }
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
        azimuth: None,
        predicted: false,
        display_preview: false,
        sequence: 0,
        viewport_epoch: None,
        camera_epoch: None,
        document_id: None,
        pressure: None,
        altitude: None,
        geometry_revision: Some(2),
        consumed: false,
        sketch_annotation: None,
        shift: false,
        tap_count: 0,
        button: 0,
        scroll: 0.0,
    }
}

#[test]
fn projects_round_trip_native_step_and_failed_replacements_preserve_the_live_design() {
    let directory = std::env::temp_dir().join(format!(
        "solidcraft-projects-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let first = directory.join("First.solidcraft");
    let second = directory.join("Second.solidcraft");
    let step = directory.join("Block.step");
    let imported = directory.join("Imported.solidcraft");
    let corrupt = directory.join("Corrupt.solidcraft");
    let mut adapter = SessionAdapter::new();
    adapter.execute("file.new", json!({"name":"First"})).unwrap();
    adapter.execute("solid.box", json!({"length":10,"width":20,"height":30})).unwrap();
    adapter.execute("file.save_as", json!({"path":first})).unwrap();
    adapter.execute("file.export", json!({"path":step,"format":"step"})).unwrap();
    let original_id = adapter.session.document_id.clone();
    let original_revision = adapter.geometry_revision;
    let (_, blank) = adapter.replace_document(None, "Second", second.to_str().unwrap()).unwrap();
    assert_eq!(blank.body_count, 0);
    assert_ne!(adapter.session.document_id, original_id);
    assert!(blank.geometry_revision > original_revision);
    adapter.execute("solid.box", json!({"length":2,"width":3,"height":4})).unwrap();
    adapter.execute("file.save", json!({"path":second})).unwrap();
    let (_, reopened) = adapter.replace_document(Some(first.to_str().unwrap()), "First", first.to_str().unwrap()).unwrap();
    assert!((reopened.volume_mm3 - 6000.).abs() < 1e-6);
    assert_eq!(adapter.session.doc.features.len(), 1);
    let (_, reopened) = adapter.replace_document(Some(second.to_str().unwrap()), "Second", second.to_str().unwrap()).unwrap();
    assert!((reopened.volume_mm3 - 24.).abs() < 1e-6);
    let duplicate = directory.join("First copy.solidcraft");
    let (_, duplicated) = adapter.replace_document(Some(first.to_str().unwrap()), "First copy", duplicate.to_str().unwrap()).unwrap();
    assert_eq!(adapter.session.doc.name, "First copy");
    assert_eq!(duplicated.geometry_revision, adapter.geometry_revision);
    assert!((duplicated.volume_mm3 - 6000.).abs() < 1e-6);

    let (_, snapshot) = adapter.replace_document(Some(step.to_str().unwrap()), "Imported", imported.to_str().unwrap()).unwrap();
    assert!((snapshot.volume_mm3 - 6000.).abs() < 1e-3);
    assert_eq!(snapshot.body_count, 1);
    assert!(!adapter.session.doc.components.is_empty(), "STEP bodies belong to an import component");
    // The staged source can go away: the native container retains its import.
    std::fs::remove_file(&step).unwrap();
    adapter.replace_document(Some(imported.to_str().unwrap()), "Imported", imported.to_str().unwrap()).unwrap();
    assert!((adapter.snapshot().unwrap().volume_mm3 - 6000.).abs() < 1e-3);
    adapter.execute("solid.box", json!({"length":1,"width":1,"height":1,"operation":"new","body_name":"Keep"})).unwrap();
    let document = adapter.session.doc.clone();
    let undo = adapter.session.undo.len();
    let revision = adapter.geometry_revision;
    let identity = adapter.session.document_id.clone();
    std::fs::write(&corrupt, b"not a design").unwrap();
    assert!(adapter.replace_document(Some(corrupt.to_str().unwrap()), "Bad", first.to_str().unwrap()).is_err());
    assert!(adapter.replace_document(None, "Unwritable", corrupt.join("child.solidcraft").to_str().unwrap()).is_err());
    assert!(adapter.replace_document(None, "", first.to_str().unwrap()).is_err());
    assert!(Arc::ptr_eq(&document, &adapter.session.doc));
    assert_eq!(adapter.session.undo.len(), undo);
    assert_eq!(adapter.geometry_revision, revision);
    assert_eq!(adapter.session.document_id, identity);
    assert!((adapter.snapshot().unwrap().volume_mm3 - 6001.).abs() < 1e-3);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!((adapter.snapshot().unwrap().volume_mm3 - 6000.).abs() < 1e-3);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn projects_insert_native_snapshots_into_root_and_active_components_without_source_dependencies() {
    let directory = std::env::temp_dir().join(format!("solidcraft-project-insertion-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    for (ext, nested) in [("sc", false), ("solidcraft", true)] {
        let path = directory.join(format!("Source.{ext}"));
        let destination = directory.join(format!("Destination-{ext}.solidcraft"));
        let mut source = SessionAdapter::new();
        source.execute("solid.box", json!({"length":2,"width":3,"height":4,"body_name":"Source body"})).unwrap();
        source.execute("material.assign", json!({"bodies":["Source body"],"material":"Aluminum"})).unwrap();
        source.session.doc_mut().appearances.bodies.insert("Source body".into(), solidcraft_engine::doc::appearance::Look::custom([12, 34, 56], 0.4));
        source.execute("file.save_as", json!({"path":path})).unwrap();
        // Existing callers still get a live link when they omit the option.
        let mut linked = SessionAdapter::new();
        linked.execute("component.insert_design", json!({"path":path})).unwrap();
        assert_eq!(linked.session.doc.links.len(), 1);

        let mut adapter = SessionAdapter::new();
        let keep =
            adapter.execute("solid.box", json!({"length":1,"width":1,"height":1,"corner":[-20,0,0],"body_name":"Keep"})).unwrap().value["feature"]
                .as_u64()
                .unwrap();
        let (parent, parent_path) = if nested {
            let created = adapter.execute("component.create", json!({"name":"Destination object","activate":true,"grounded":false})).unwrap().value;
            let occurrence = created["occurrence"].as_u64().unwrap();
            adapter.execute("occurrence.move", json!({"occurrence":occurrence,"translate":[50,0,0],"capture":true})).unwrap();
            (created["component"].as_u64().unwrap(), vec![occurrence])
        } else {
            (0, vec![])
        };
        let identity = adapter.session.document_id.clone();
        let active_path = adapter.session.active_occurrence_path.clone();
        let history = adapter.session.undo.len();
        let inserted = adapter.execute("component.insert_design", json!({"path":path,"link":false})).unwrap().value;
        let component = inserted["component"].as_u64().unwrap();
        let occurrence = inserted["occurrence"].as_u64().unwrap();
        assert_eq!(adapter.session.undo.len(), history + 1);
        assert_eq!(adapter.session.document_id, identity);
        assert_eq!(adapter.session.active_component, parent);
        assert_eq!(adapter.session.active_occurrence_path, active_path);
        assert!(adapter.session.doc.feature(keep).is_some());
        assert!(adapter.session.doc.links.is_empty());
        assert_eq!(adapter.session.doc.occurrences.iter().find(|o| o.id == occurrence).unwrap().parent, parent);
        let body = adapter.session.model.state().bodies.iter().find(|b| b.component == component).unwrap().clone();
        assert_eq!(adapter.session.doc.body_material(&body.name), "Aluminum");
        assert_eq!(adapter.session.doc.model_body_look(&body).unwrap().color, [12, 34, 56]);
        assert!((adapter.session.doc.model_body_opacity(&body) - 0.4).abs() < 1e-9);
        let mut inserted_path = parent_path;
        inserted_path.push(occurrence);
        let marker = assembly_marker(&adapter, &inserted_path, Vec3::new(1., 1.5, 2.));
        assert!((marker - Vec3::new(if nested { 51. } else { 1. }, 1.5, 2.)).len() < 1e-9);
        assert!((assembly_volume(&adapter, &inserted_path) - 24.).abs() < 1e-6);

        std::fs::remove_file(&path).unwrap();
        adapter.execute("edit.undo", json!({})).unwrap();
        assert_eq!(adapter.snapshot().unwrap().body_count, 1);
        assert!(adapter.session.doc.links.is_empty());
        adapter.execute("edit.redo", json!({})).unwrap();
        assert_eq!(adapter.snapshot().unwrap().body_count, 2);
        assert!(adapter.session.doc.links.is_empty());
        adapter.execute("file.save_as", json!({"path":destination})).unwrap();
        let mut reopened = SessionAdapter::new();
        reopened.execute("doc.open", json!({"path":destination})).unwrap();
        assert!(reopened.session.doc.links.is_empty());
        assert_eq!(reopened.snapshot().unwrap().body_count, 2);
        assert!((assembly_volume(&reopened, &inserted_path) - 24.).abs() < 1e-6);
        assert_eq!(reopened.session.doc.body_material(&body.name), "Aluminum");
    }
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn projects_failed_native_insertions_preserve_objects_active_component_and_history() {
    let directory = std::env::temp_dir().join(format!("solidcraft-project-insertion-invalid-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let empty = directory.join("Empty.solidcraft");
    SessionAdapter::new().execute("file.save_as", json!({"path":empty})).unwrap();
    let corrupt = directory.join("Corrupt.sc");
    std::fs::write(&corrupt, b"not a design").unwrap();
    let mut adapter = SessionAdapter::new();
    adapter.execute("component.create", json!({"name":"Keep active","activate":true})).unwrap();
    adapter.execute("solid.box", json!({"length":2,"width":3,"height":4})).unwrap();
    adapter.execute("edit.undo", json!({})).unwrap();
    let document = adapter.session.doc.clone();
    let history = (adapter.session.undo.len(), adapter.session.redo.len());
    let active = (adapter.session.active_component, adapter.session.active_occurrence_path.clone());
    for params in [
        json!({"path":corrupt,"link":false}),
        json!({"path":empty,"link":false}),
        json!({"path":empty,"link":"false"}),
        json!({"path":empty,"link":false,"reuse":true}),
        json!({"path":directory.join("Missing.sc"),"link":false}),
    ] {
        assert!(adapter.execute("component.insert_design", params).is_err());
        assert_eq!(*adapter.session.doc, *document);
        assert_eq!((adapter.session.undo.len(), adapter.session.redo.len()), history);
        assert_eq!((adapter.session.active_component, adapter.session.active_occurrence_path.clone()), active);
    }
    adapter.execute("edit.redo", json!({})).unwrap();
    assert!((adapter.snapshot().unwrap().volume_mm3 - 24.).abs() < 1e-6);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn projects_recovery_keeps_the_last_cold_reopenable_save_during_sketch_edits() {
    let directory = std::env::temp_dir().join(format!("solidcraft-project-recovery-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("Plate.solidcraft");
    let recovery = directory.join("Plate.solidcraft.recovery");
    let mut adapter = SessionAdapter::new();
    adapter.execute("sketch.create", json!({"plane":"XY","name":"Outline"})).unwrap();
    adapter.execute("sketch.rectangle.two_point", json!({"p0":[0,0],"p1":[40,20]})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    adapter.execute("solid.extrude", json!({"distance":10,"profiles":[{"point":[10,10]}]})).unwrap();
    let saved = adapter.execute("file.save_as", json!({"path":path})).unwrap();
    assert_eq!(saved.value["recovery_updated"], true);
    let working = std::fs::read(&path).unwrap();
    assert_eq!(std::fs::read(&recovery).unwrap(), working, "the first save already has recovery");

    adapter.execute("sketch.edit", json!({"sketch":"Outline"})).unwrap();
    adapter.execute("sketch.line", json!({"points":[[20,0],[20,20]]})).unwrap();
    let history = adapter.session.undo.len();
    let active = adapter.session.active_sketch;
    let saved = adapter.execute("file.save", json!({"path":path})).unwrap();
    assert_eq!(saved.value["recovery_updated"], false, "cold reopen must detect a warning hidden by the live sketch editor");
    assert_eq!(adapter.session.active_sketch, active, "autosave must leave authoring alone");
    assert_eq!(adapter.session.undo.len(), history);
    let unfinished = std::fs::read(&path).unwrap();
    assert_ne!(unfinished, working, "unfinished edits still reach the ordinary save");
    adapter.execute("file.save", json!({"path":path})).unwrap();
    assert_eq!(std::fs::read(directory.join("Plate.solidcraft.bak")).unwrap(), unfinished);
    assert_eq!(std::fs::read(&recovery).unwrap(), working, "repeated saves cannot rotate away the working copy");

    let mut restored = SessionAdapter::new();
    restored.replace_document(Some(recovery.to_str().unwrap()), "Restored", directory.join("Restored.solidcraft").to_str().unwrap()).unwrap();
    assert!((restored.snapshot().unwrap().volume_mm3 - 8000.).abs() < 1e-5);
    adapter.execute("edit.undo", json!({})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    adapter.execute("parameters.add", json!({"name":"Revision","expression":"2"})).unwrap();
    let saved = adapter.execute("file.save", json!({"path":path})).unwrap();
    assert_eq!(saved.value["recovery_updated"], true);
    assert_eq!(std::fs::read(&recovery).unwrap(), std::fs::read(&path).unwrap());
    assert_ne!(std::fs::read(&recovery).unwrap(), working);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn projects_with_deleted_face_dependencies_remain_editable_and_preserve_working_recovery() {
    let directory = std::env::temp_dir().join(format!("solidcraft-deleted-dependencies-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("Part.solidcraft");
    let mut adapter = SessionAdapter::new();
    let base =
        adapter.execute("solid.box", json!({"length":10,"width":10,"height":10,"body_name":"Stock"})).unwrap().value["feature"].as_u64().unwrap();
    let face = modelling_face(&adapter, "Stock", Vec3::new(2., 2., 10.));
    let Sel::Face { index, point, .. } = face else { panic!("face") };
    let sketch = adapter.execute("sketch.create", json!({"plane":{"face":point,"body":"Stock","index":index},"project_edges":false})).unwrap().value
        ["sketch"]
        .as_u64()
        .unwrap();
    adapter.execute("sketch.rectangle.two_point", json!({"p0":[1,1],"p1":[3,3]})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    adapter.execute("solid.extrude", json!({"distance":5,"operation":"new"})).unwrap();
    adapter.execute("solid.box", json!({"corner":[30,0,0],"length":2,"width":3,"height":4,"body_name":"Independent","operation":"new"})).unwrap();
    adapter.execute("file.save_as", json!({"path":path})).unwrap();
    let recovery = directory.join("Part.solidcraft.recovery");
    let working = std::fs::read(&recovery).unwrap();
    let deleted = adapter.execute("timeline.delete", json!({"feature":base})).unwrap();
    assert!(!deleted.value["failing"].as_array().unwrap().is_empty());
    assert!(adapter.snapshot().is_err(), "modelling and recovery validation remain strict");
    let scene = adapter.workspace_snapshot().unwrap();
    assert_eq!(scene.body_count, 1, "failed features must not retain bodies from before deletion");
    assert_eq!(scene.picks.bodies[0].name, "Independent");
    assert!((scene.volume_mm3 - 24.).abs() < 1e-6);
    let workspace = adapter.workspace(&[], false, false);
    assert!(workspace["modelling"]["timeline"].as_array().unwrap().iter().any(|f| f["id"] == sketch && f["error"].is_string()));
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!((adapter.snapshot().unwrap().volume_mm3 - 1044.).abs() < 1e-6);
    adapter.execute("edit.redo", json!({})).unwrap();
    let saved = adapter.execute("file.save", json!({"path":path})).unwrap();
    assert_eq!(saved.value["recovery_updated"], false);
    assert_eq!(std::fs::read(&recovery).unwrap(), working);

    let mut reopened = SessionAdapter::new();
    let (_, scene) = reopened.replace_document(Some(path.to_str().unwrap()), "Part", path.to_str().unwrap()).unwrap();
    assert_eq!(scene.body_count, 1);
    assert!((scene.volume_mm3 - 24.).abs() < 1e-6);
    assert_eq!(std::fs::read(&recovery).unwrap(), working, "opening a partial model must not replace its working recovery");
    assert!(reopened.session.model.result(sketch).unwrap().error.is_some());
    // Existing saves can hold a failed helper whose consumer was deleted by an
    // older build. It must be reachable through the ordinary inspector/timeline.
    Arc::make_mut(&mut reopened.session.doc).feature_mut(sketch).unwrap().internal_profile = true;
    reopened.workspace_cache = None;
    let workspace = reopened.workspace(&[], false, false);
    assert!(workspace["rows"].as_array().unwrap().iter().any(|f| f["entity"]["id"] == sketch));
    assert!(workspace["modelling"]["timeline"].as_array().unwrap().iter().any(|f| f["id"] == sketch));
    reopened.execute("timeline.delete", json!({"feature":sketch})).unwrap();
    assert!((reopened.snapshot().unwrap().volume_mm3 - 24.).abs() < 1e-6);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn projects_recovery_upgrades_existing_saves_and_reports_recovery_write_failures() {
    let directory = std::env::temp_dir().join(format!("solidcraft-project-recovery-upgrade-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("Existing.solidcraft");
    let recovery = directory.join("Existing.solidcraft.recovery");
    let mut adapter = SessionAdapter::new();
    adapter.execute("solid.box", json!({"length":2,"width":3,"height":4})).unwrap();
    // An older build has an ordinary file and rolling backup, but no recovery yet.
    adapter.session.execute("file.save_as", &json!({"path":path})).unwrap();
    let old = std::fs::read(&path).unwrap();
    adapter.execute("parameters.add", json!({"name":"Revision","expression":"1"})).unwrap();
    assert!(adapter.execute("file.save", json!({"path":path,"format":"invalid"})).is_err());
    assert_eq!(std::fs::read(&recovery).unwrap(), old, "upgrade protects the existing working file before replacement");
    assert_eq!(std::fs::read(&path).unwrap(), old);
    adapter.execute("file.save", json!({"path":path})).unwrap();
    assert_ne!(std::fs::read(&recovery).unwrap(), old);

    std::fs::remove_file(&recovery).unwrap();
    std::fs::create_dir(&recovery).unwrap();
    adapter.execute("parameters.change", json!({"name":"Revision","expression":"2"})).unwrap();
    assert_eq!(adapter.execute("file.save", json!({"path":path})).unwrap_err().code, "recovery_write");
    assert!(adapter.session.is_dirty(), "recovery failure must not be reported as fully saved");
    std::fs::remove_dir(&recovery).unwrap();
    adapter.execute("file.save", json!({"path":path})).unwrap();
    assert_eq!(std::fs::read(&recovery).unwrap(), std::fs::read(&path).unwrap());
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn projects_recovery_seeds_only_rebuildable_files_after_primary_corruption() {
    let directory = std::env::temp_dir().join(format!("solidcraft-project-recovery-corruption-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("Existing.solidcraft");
    let backup = directory.join("Existing.solidcraft.bak");
    let recovery = directory.join("Existing.solidcraft.recovery");
    let mut adapter = SessionAdapter::new();
    adapter.execute("solid.box", json!({"length":2,"width":3,"height":4})).unwrap();
    adapter.session.execute("file.save_as", &json!({"path":path})).unwrap();
    let working = std::fs::read(&path).unwrap();
    std::fs::write(&backup, &working).unwrap();
    std::fs::write(&path, b"damaged primary").unwrap();
    assert!(adapter.execute("file.save", json!({"path":path,"format":"invalid","overwrite":true})).is_err());
    assert_eq!(std::fs::read(&recovery).unwrap(), working, "a validated rolling backup can seed recovery");
    assert_eq!(std::fs::read(&path).unwrap(), b"damaged primary");
    std::fs::remove_file(&recovery).unwrap();
    std::fs::write(&backup, b"damaged backup").unwrap();
    assert!(adapter.execute("file.save", json!({"path":path,"format":"invalid","overwrite":true})).is_err());
    assert!(!recovery.exists(), "corrupt bytes must never become a working recovery file");
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn projects_preserve_upstream_reference_data_from_sc_through_legacy_native_containers() {
    let directory = std::env::temp_dir().join(format!(
        "solidcraft-ipados-upstream-roundtrip-{}-{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let scan = directory.join("reference.xyz");
    let desktop = directory.join("desktop.sc");
    let native = directory.join("native.solidcraft");
    let reopened = directory.join("reopened.solidcraft");
    std::fs::write(&scan, "0 0 0\n1 2 3\n4 5 6\n").unwrap();

    // Create data supported by upstream but not exposed by the native forms.
    let mut upstream = solidcraft_engine::Session::default();
    upstream.execute("file.new", &json!({"name":"Portable reference fixture"})).unwrap();
    upstream.execute("solid.box", &json!({"length":20,"width":15,"height":10,"body_name":"Stock"})).unwrap();
    let cloud = upstream.execute("pointcloud.insert", &json!({"path":scan,"name":"Reference"})).unwrap()["cloud"].as_u64().unwrap();
    upstream.execute("parameters.add", &json!({"name":"radius","expression":"2 mm"})).unwrap();
    let sketch = upstream.execute("sketch.create", &json!({"plane":"XY","name":"Equation reference"})).unwrap()["sketch"].as_u64().unwrap();
    let curve = upstream
        .execute("sketch.equation_curve", &json!({"x":"radius * cos(t)","y":"radius * sin(t)","start":"0","end":"pi","construction":true}))
        .unwrap()["curve"]
        .as_str()
        .unwrap()
        .to_string();
    upstream.execute("sketch.finish", &json!({})).unwrap();
    upstream.execute("file.save_as", &json!({"path":desktop})).unwrap();
    let expected = upstream.doc.clone();
    let feature_ids: Vec<_> = expected.features.iter().map(|feature| feature.id).collect();
    assert_eq!(expected.point_clouds[0].id, cloud);
    assert_eq!(expected.point_clouds[0].points.len(), 3);
    assert_eq!(expected.point_clouds[0].world_point(1), Some(Vec3::new(1.0, 2.0, 3.0)));
    // Both native opens must use the embedded points, without the original scan.
    std::fs::remove_file(&scan).unwrap();

    let mut adapter = SessionAdapter::new();
    for (source, destination, name) in [(&desktop, &native, "native"), (&native, &reopened, "reopened")] {
        let (_, snapshot) =
            adapter.replace_document(Some(source.to_str().unwrap()), "Portable reference fixture", destination.to_str().unwrap()).unwrap();
        // Upstream Save As names an imported copy after its destination. All
        // geometry and reference data must remain exactly the original's.
        let mut expected = (*expected).clone();
        expected.name = name.into();
        assert_eq!(*adapter.session.doc, expected, "only upstream's destination-based name changes in the native copy");
        assert_eq!(adapter.session.doc.features.iter().map(|feature| feature.id).collect::<Vec<_>>(), feature_ids);
        assert_eq!(adapter.session.doc.point_clouds, expected.point_clouds);
        let equation = adapter.session.doc.sketch(sketch).unwrap().equation_of(&curve).unwrap();
        assert_eq!(equation.x.as_deref(), Some("radius * cos(t)"));
        assert_eq!((equation.y.as_str(), equation.start.as_str(), equation.end.as_str()), ("radius * sin(t)", "0", "pi"));
        assert_eq!(snapshot.body_count, 1);
        assert!((snapshot.volume_mm3 - 3000.0).abs() < 1e-6, "reference data must not change the 20 × 15 × 10 mm box");
        let saved = solidcraft_engine::io::read_design(&std::fs::read(destination).unwrap()).unwrap();
        assert_eq!(saved, expected, "the shared reader preserves the native .solidcraft output too");
    }
    std::fs::remove_dir_all(directory).unwrap();
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
fn finished_profile_pencil_selection_seeds_the_exact_extrusion_region() {
    let mut adapter = SessionAdapter::new();
    let sketch = adapter.execute("sketch.create", json!({"plane":"XY"})).unwrap().value["sketch"].as_u64().unwrap();
    adapter.execute("sketch.rectangle.two_point", json!({"p0":[-30,-10],"p1":[-10,10]})).unwrap();
    adapter.execute("sketch.rectangle.two_point", json!({"p0":[10,-5],"p1":[30,5]})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    let index = adapter
        .session
        .world_state()
        .unwrap()
        .sketch(sketch)
        .unwrap()
        .profiles
        .iter()
        .position(|p| p.region.contains(solidcore_record::Vec2::new(20., 0.)))
        .unwrap();
    let mut view = Viewport::default();
    view.set_metrics(metrics(800., 600., 2.)).unwrap();
    view.begin_update(1);
    view.install(1, adapter.snapshot().unwrap());
    view.command("view.top", &json!({})).unwrap();
    let camera = view.camera;
    let tap = point(&view, Vec3::new(20., 0., 0.));
    feed(&mut view, InputSource::Pencil, InputPhase::Began, 1, tap, 1.);
    feed(&mut view, InputSource::Pencil, InputPhase::Ended, 1, tap, 1.1);
    assert_eq!(view.selection(), &[Sel::Profile { sketch, index }]);
    assert_eq!(view.camera, camera);
    let highlighted = view.snapshot().unwrap();
    assert!(!highlighted.highlight.meshes.is_empty());
    let edit = adapter.begin_model(7000, Some("solid.extrude"), None, view.selection()).unwrap();
    assert_eq!(edit.params["profile"]["sketch"], sketch);
    assert!(!edit.params["profile"]["profiles"].is_null());
    let prepared = adapter.prepare_model(&edit, edit.params.clone(), true).unwrap();
    adapter.accept_model(&edit, prepared).unwrap();
    assert!((adapter.snapshot().unwrap().volume_mm3 - 2000.).abs() < 1e-6);
}

#[test]
fn finished_profile_projected_partition_wins_over_the_overlapping_source_sketch() {
    let mut adapter = SessionAdapter::new();
    let source = adapter.execute("sketch.create", json!({"plane":"XY","name":"Source"})).unwrap().value["sketch"].as_u64().unwrap();
    adapter.execute("sketch.rectangle.two_point", json!({"p0":[-20,-10],"p1":[20,10]})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    let sketch = adapter.execute("sketch.create", json!({"plane":"XY","name":"Partition"})).unwrap().value["sketch"].as_u64().unwrap();
    let mut request = projection_request(&adapter, source, true);
    request.construction = false;
    adapter.project_sketch(request).unwrap();
    adapter.execute("sketch.line", json!({"points":[[-10,-10],[-10,10]],"infer":false})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    let world = adapter.session.world_state().unwrap();
    let profiles = &world.sketch(sketch).unwrap().profiles;
    assert_eq!(profiles.len(), 2, "ordinary projected lines must partition the sketch");
    let index = profiles.iter().position(|p| p.region.contains(solidcore_record::Vec2::new(-15., 0.))).unwrap();
    assert!((profiles[index].area - 200.).abs() < 1e-6);
    // Feature recency alone must not decide which overlapping region wins.
    adapter.execute("sketch.create", json!({"plane":"XY","name":"Later outline"})).unwrap();
    adapter.execute("sketch.rectangle.two_point", json!({"p0":[-20,-10],"p1":[20,10]})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    let mut view = Viewport::default();
    view.set_metrics(metrics(800., 600., 2.)).unwrap();
    view.begin_update(1);
    view.install(1, adapter.snapshot().unwrap());
    assert!(view.model.as_ref().unwrap().picks.profiles.iter().any(|p| p.sketch == source));
    for perspective in [false, true] {
        view.command("view.perspective", &json!({"enabled":perspective})).unwrap();
        for direction in ["view.top", "view.isometric"] {
            view.command(direction, &json!({})).unwrap();
            for filter in [PickFilter::Auto, PickFilter::Face] {
                view.filter = filter;
                let tap = point(&view, Vec3::new(-15., 0., 0.));
                assert_eq!(view.pick(tap, None), Some(Sel::Profile { sketch, index }));
            }
        }
    }
    let tap = point(&view, Vec3::new(-15., 0., 0.));
    feed(&mut view, InputSource::Pencil, InputPhase::Began, 1, tap, 1.);
    feed(&mut view, InputSource::Pencil, InputPhase::Ended, 1, tap, 1.1);
    assert_eq!(view.selection(), &[Sel::Profile { sketch, index }]);
    let highlight = view.snapshot().unwrap();
    let fill = &highlight.highlight.meshes[0].mesh;
    assert!(fill.positions.iter().all(|p| p.x >= -20. - 1e-6 && p.x <= -10. + 1e-6));
    let edit = adapter.begin_model(7003, Some("solid.extrude"), None, view.selection()).unwrap();
    let prepared = adapter.prepare_model(&edit, edit.params.clone(), true).unwrap();
    adapter.accept_model(&edit, prepared).unwrap();
    assert!((adapter.snapshot().unwrap().volume_mm3 - 2000.).abs() < 1e-6);
}

#[test]
fn finished_profile_picking_keeps_nearer_planes_ahead_of_smaller_regions() {
    let mut adapter = SessionAdapter::new();
    let small = adapter.execute("sketch.create", json!({"plane":"XY"})).unwrap().value["sketch"].as_u64().unwrap();
    adapter.execute("sketch.rectangle.two_point", json!({"p0":[-5,-5],"p1":[5,5]})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    let large = adapter.execute("sketch.create", json!({"plane":"XY","offset":10})).unwrap().value["sketch"].as_u64().unwrap();
    adapter.execute("sketch.rectangle.two_point", json!({"p0":[-20,-10],"p1":[20,10]})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    let mut view = Viewport::default();
    view.set_metrics(metrics(800., 600., 2.)).unwrap();
    view.begin_update(1);
    view.install(1, adapter.snapshot().unwrap());
    for perspective in [false, true] {
        view.command("view.perspective", &json!({"enabled":perspective})).unwrap();
        for (direction, z, sketch) in [("view.top", 10., large), ("view.bottom", 0., small)] {
            view.command(direction, &json!({})).unwrap();
            assert_eq!(view.pick(point(&view, Vec3::new(0., 0., z)), None), Some(Sel::Profile { sketch, index: 0 }));
        }
    }
}

#[test]
fn finished_profile_selection_refresh_keeps_distance_and_replaces_the_region() {
    let mut adapter = SessionAdapter::new();
    let sketch = adapter.execute("sketch.create", json!({"plane":"XY"})).unwrap().value["sketch"].as_u64().unwrap();
    adapter.execute("sketch.rectangle.two_point", json!({"p0":[-30,-10],"p1":[-10,10]})).unwrap();
    adapter.execute("sketch.rectangle.two_point", json!({"p0":[10,-5],"p1":[30,5]})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    let edit = adapter.begin_model(7001, Some("solid.extrude"), None, &[]).unwrap();
    let mut params = edit.params.clone();
    params["distance"] = json!("7 mm");
    let edit = adapter.refresh_model(&edit, &[], Some(params)).unwrap();
    let index = adapter
        .session
        .world_state()
        .unwrap()
        .sketch(sketch)
        .unwrap()
        .profiles
        .iter()
        .position(|p| p.region.contains(solidcore_record::Vec2::new(20., 0.)))
        .unwrap();
    let edit = adapter.refresh_model(&edit, &[Sel::Profile { sketch, index }], None).unwrap();
    assert_eq!(edit.params["distance"], "7 mm");
    assert!(!edit.params["profile"].is_null(), "canvas reselection must populate the form");
    let prepared = adapter.prepare_model(&edit, edit.params.clone(), true).unwrap();
    adapter.accept_model(&edit, prepared).unwrap();
    assert!((adapter.snapshot().unwrap().volume_mm3 - 1400.).abs() < 1e-6);
}

#[test]
fn finished_profile_picking_respects_holes_visibility_depth_filters_and_stale_scenes() {
    let mut adapter = SessionAdapter::new();
    let sketch = adapter.execute("sketch.create", json!({"plane":"XZ","offset":"12 mm"})).unwrap().value["sketch"].as_u64().unwrap();
    adapter.execute("sketch.rectangle.two_point", json!({"p0":[-20,-15],"p1":[20,15]})).unwrap();
    adapter.execute("sketch.rectangle.two_point", json!({"p0":[-5,-5],"p1":[5,5]})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    let world = adapter.session.world_state().unwrap();
    let ss = world.sketch(sketch).unwrap();
    let mut view = Viewport::default();
    view.set_metrics(ViewportMetrics { origin_points: [17., 83.], ..metrics(800., 600., 3.) }).unwrap();
    view.begin_update(1);
    view.install(1, adapter.snapshot().unwrap());
    view.command("view.front", &json!({})).unwrap();
    view.command("view.fit", &json!({})).unwrap();
    for perspective in [false, true] {
        view.command("view.perspective", &json!({"enabled":perspective})).unwrap();
        for local in [[0., 0.], [10., 0.]] {
            let local = solidcore_record::Vec2::new(local[0], local[1]);
            let expected = ss.profiles.iter().position(|p| p.region.contains(local)).unwrap();
            assert_eq!(view.pick(point(&view, ss.plane.to_world(local)), None), Some(Sel::Profile { sketch, index: expected }));
        }
    }
    let tap = point(&view, ss.plane.origin);
    for filter in [PickFilter::Body, PickFilter::Edge, PickFilter::Vertex, PickFilter::Occurrence] {
        view.filter = filter;
        assert!(view.pick(tap, None).is_none());
    }
    view.filter = PickFilter::Auto;
    assert!(matches!(view.pick(tap, None), Some(Sel::Profile { .. })));
    let revision = adapter.geometry_revision;
    assert!(view.pick(tap, Some(revision + 1)).is_none());
    view.begin_update(2);
    assert!(view.pick(tap, None).is_none());
    view.install(2, adapter.snapshot().unwrap());
    adapter.execute("browser.visibility", json!({"items":[sketch],"visible":false})).unwrap();
    view.begin_update(3);
    view.install(3, adapter.snapshot().unwrap());
    assert!(view.pick(tap, None).is_none());
    adapter.execute("browser.visibility", json!({"items":[sketch],"visible":true})).unwrap();
    adapter.session.visibility.hidden_profiles.push(sketch);
    view.begin_update(4);
    view.install(4, adapter.snapshot().unwrap());
    assert!(view.pick(tap, None).is_none());

    // A region drawn on a body face wins; a region behind the solid does not.
    let mut adapter = SessionAdapter::new();
    adapter.execute("solid.box", json!({"length":40,"width":30,"height":10,"corner":[-20,-15,0]})).unwrap();
    let sketch = adapter.execute("sketch.create", json!({"plane":"XY","offset":"10 mm"})).unwrap().value["sketch"].as_u64().unwrap();
    adapter.execute("sketch.rectangle.two_point", json!({"p0":[-10,-10],"p1":[10,10]})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    let mut view = Viewport::default();
    view.set_metrics(metrics(800., 600., 2.)).unwrap();
    view.begin_update(1);
    view.install(1, adapter.snapshot().unwrap());
    view.command("view.top", &json!({})).unwrap();
    assert_eq!(view.pick(point(&view, Vec3::new(0., 0., 10.)), None), Some(Sel::Profile { sketch, index: 0 }));
    view.command("view.bottom", &json!({})).unwrap();
    assert!(matches!(view.pick(point(&view, Vec3::new(0., 0., 0.)), None), Some(Sel::Face { .. })));
}

#[test]
fn finished_profile_multiple_selection_extrudes_only_selected_regions() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("sketch.create", json!({"plane":"XY"})).unwrap();
    for (x, width) in [(-30., 10.), (-10., 5.), (10., 15.)] {
        adapter.execute("sketch.rectangle.two_point", json!({"p0":[x,-5],"p1":[x+width,5]})).unwrap();
    }
    adapter.execute("sketch.finish", json!({})).unwrap();
    let mut view = Viewport::default();
    view.set_metrics(metrics(800., 600., 2.)).unwrap();
    view.begin_update(1);
    view.install(1, adapter.snapshot().unwrap());
    view.command("view.top", &json!({})).unwrap();
    view.multiple = true;
    for (id, x) in [(1, -25.), (2, 17.5)] {
        let tap = point(&view, Vec3::new(x, 0., 0.));
        feed(&mut view, InputSource::Pencil, InputPhase::Began, id, tap, id as f64);
        feed(&mut view, InputSource::Pencil, InputPhase::Ended, id, tap, id as f64 + 0.1);
    }
    assert_eq!(view.selection().len(), 2);
    let edit = adapter.begin_model(7002, Some("solid.extrude"), None, view.selection()).unwrap();
    let prepared = adapter.prepare_model(&edit, edit.params.clone(), true).unwrap();
    adapter.accept_model(&edit, prepared).unwrap();
    let snapshot = adapter.snapshot().unwrap();
    assert_eq!(snapshot.body_count, 2);
    assert!((snapshot.volume_mm3 - 2500.).abs() < 1e-6);
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
fn default_pencil_selection_picks_edges_vertices_and_faces_without_changing_filters() {
    let (adapter, mut view) = proof();
    let document = adapter.session.doc.clone();
    let history = adapter.session.undo.len();
    let model = view.snapshot().unwrap().model;
    for scale in [1., 2., 3.] {
        view.set_metrics(metrics(800., 600., scale)).unwrap();
        for perspective in [false, true] {
            view.command("view.perspective", &json!({"enabled":perspective})).unwrap();
            for orientation in ["view.top", "view.isometric"] {
                view.command(orientation, &json!({})).unwrap();
                view.command("view.fit", &json!({})).unwrap();
                let camera = view.camera;
                for (world, kind) in [(Vec3::new(20., 0., 10.), "edge"), (Vec3::new(0., 0., 10.), "vertex"), (Vec3::new(20., 15., 10.), "face")] {
                    view.command("select.clear", &json!({})).unwrap();
                    let p = point(&view, world);
                    let frame = view.diagnostic();
                    for phase in [InputPhase::Began, InputPhase::Ended] {
                        let mut sample = event(InputSource::Pencil, phase, 1, p, 1.);
                        sample.scale = scale;
                        sample.x_pixels = p[0] * scale;
                        sample.y_pixels = p[1] * scale;
                        sample.geometry_revision = Some(model.geometry_revision);
                        sample.viewport_epoch = frame["viewport_epoch"].as_u64();
                        sample.camera_epoch = frame["camera_epoch"].as_u64();
                        sample.document_id = frame["document_id"].as_str().map(str::to_string);
                        view.input(sample).unwrap();
                    }
                    assert_eq!(view.diagnostic()["selection"][0]["type"], kind, "{orientation}, perspective={perspective}, scale={scale}");
                    assert_eq!(view.selection.len(), 1);
                    assert_eq!(view.camera, camera);
                }
            }
        }
    }
    assert_eq!(view.diagnostic()["filter"], "auto");
    assert!(Arc::ptr_eq(&view.snapshot().unwrap().model, &model));
    assert!(Arc::ptr_eq(&adapter.session.doc, &document));
    assert_eq!(adapter.session.undo.len(), history);

    // The previous face-only behaviour stays reachable through the saved filter.
    view.command("view.top", &json!({})).unwrap();
    let edge = point(&view, Vec3::new(20., 0., 10.));
    for (filter, kind) in [("face", "face"), ("edge", "edge"), ("auto", "edge")] {
        view.command("select.filter", &json!({"filter":filter})).unwrap();
        assert_eq!(serde_json::to_value(view.pick(edge, Some(model.geometry_revision)).unwrap()).unwrap()["type"], kind);
    }
}

#[test]
fn home_is_fixed_across_model_bounds_and_navigation_while_fit_still_frames_the_model() {
    use solidcraft_render::{Camera, StandardView, UpAxis};

    let mut expected = Camera::default();
    expected.set_view(StandardView::Iso);
    assert_eq!(expected.target, Vec3::ZERO);
    assert_eq!(expected.distance, 200.0);
    assert_eq!(expected.fov, 0.0);
    assert_eq!(expected.roll, 0.0);
    assert_eq!(expected.up, UpAxis::Z);

    for (width, height) in [(800.0, 600.0), (300.0, 900.0)] {
        for model in [None, Some((0.01, [1000.0, -2000.0, 500.0])), Some((10000.0, [-50000.0, 20000.0, -1000.0]))] {
            let mut adapter = SessionAdapter::new();
            if let Some((size, corner)) = model {
                adapter.execute("solid.box", json!({"length":size,"width":size,"height":size,"corner":corner})).unwrap();
            }
            let mut view = Viewport::default();
            view.set_metrics(metrics(width, height, 2.0)).unwrap();
            view.begin_update(1);
            assert!(view.install(1, adapter.snapshot().unwrap()));
            let scene = view.model.clone().unwrap();
            let document = adapter.session.doc.clone();
            let history = adapter.session.undo.len();

            for _ in 0..2 {
                view.command("view.perspective", &json!({"enabled":true})).unwrap();
                view.command("view.navigate", &json!({"motion":"pan","delta":[100,50],"viewport":[width,height]})).unwrap();
                view.command("view.navigate", &json!({"motion":"orbit","delta":[50,30]})).unwrap();
                view.command("view.navigate", &json!({"motion":"roll","angle":0.5})).unwrap();
                view.command("view.navigate", &json!({"motion":"zoom","factor":2.0})).unwrap();
                let epoch = view.camera_epoch;
                view.command("view.home", &json!({})).unwrap();
                assert_eq!(view.camera, expected, "Home must not depend on model bounds, navigation or aspect");
                assert!(view.camera_epoch > epoch, "Home invalidates input stamped with the previous camera");
                assert!(Arc::ptr_eq(&scene, &view.snapshot().unwrap().model));
            }

            view.command("view.fit", &json!({})).unwrap();
            if model.is_some() {
                assert_eq!(view.camera.target, scene.bounds.center());
                assert_ne!(view.camera.distance, expected.distance);
                assert_eq!(view.camera.yaw, expected.yaw);
                assert_eq!(view.camera.pitch, expected.pitch);
            }
            assert!(Arc::ptr_eq(&document, &adapter.session.doc));
            assert_eq!(adapter.session.undo.len(), history);
            view.planar = true;
            let planar_camera = view.camera;
            assert!(view.command("view.home", &json!({})).is_err());
            assert_eq!(view.camera, planar_camera, "Home must not turn an active planar view");
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
fn history_feature_selection_highlights_current_created_and_modified_bodies_without_document_edits() {
    let (mut adapter, mut view) = proof();
    let base = adapter.session.doc.features[0].id;
    let other =
        adapter.execute("solid.sphere", json!({"radius":5,"center":[70,0,0],"body_name":"Other"})).unwrap().value["feature"].as_u64().unwrap();
    let fillet =
        adapter.execute("solid.fillet", json!({"edges":[[40,0,5]],"radius":2,"body":"Foundation"})).unwrap().value["feature"].as_u64().unwrap();
    view.begin_update(2);
    assert!(view.install(2, adapter.snapshot().unwrap()));
    let before = view.snapshot().unwrap();
    let doc = adapter.session.doc.clone();
    let history = adapter.session.undo.len();
    for (feature, body) in [(base, "Foundation"), (fillet, "Foundation"), (other, "Other")] {
        view.command("select.entity", &json!({"entity":{"type":"feature","id":feature},"additive":false})).unwrap();
        assert_eq!(view.selection(), &[Sel::Feature { id: feature }]);
        let selected = view.snapshot().unwrap();
        assert_eq!(selected.selected_bodies, [body]);
        assert!(Arc::ptr_eq(&before.model, &selected.model), "selection must reuse the current model upload");
        assert_eq!(selected.camera, before.camera);
        let workspace = adapter.workspace(view.selection(), false, false);
        assert!(
            workspace["rows"].as_array().unwrap().iter().any(|row| row["entity"] == json!(Sel::Feature { id: feature }) && row["selected"] == true)
        );
    }
    view.command("select.entity", &json!({"entity":{"type":"feature","id":base},"additive":true})).unwrap();
    assert_eq!(view.snapshot().unwrap().selected_bodies, ["Foundation", "Other"]);
    view.command("select.clear", &json!({})).unwrap();
    assert!(view.snapshot().unwrap().selected_bodies.is_empty());
    assert!(Arc::ptr_eq(&adapter.session.doc, &doc));
    assert_eq!(adapter.session.undo.len(), history);
    assert!(view.command("select.entity", &json!({"entity":{"type":"feature","id":u64::MAX}})).is_err());
    // Rebuilding after hide/suppress/undo never resurrects geometry from history.
    for (generation, command, params) in [
        (3, "browser.visibility", json!({"items":["Foundation"],"visible":false})),
        (4, "browser.visibility", json!({"items":["Foundation"],"visible":true})),
        (5, "timeline.suppress", json!({"feature":fillet,"suppressed":true})),
        (6, "edit.undo", json!({})),
    ] {
        adapter.execute(command, params).unwrap();
        view.begin_update(generation);
        assert!(view.install(generation, adapter.snapshot().unwrap()));
        assert!(view.selection().is_empty());
        view.command("select.entity", &json!({"entity":{"type":"feature","id":fillet}})).unwrap();
        let names = view.snapshot().unwrap().selected_bodies;
        assert_eq!(names.is_empty(), generation == 3 || generation == 5);
    }
}

#[test]
fn history_geometry_selection_identifies_features_without_editing_the_document() {
    use solidcraft_engine::doc::{naming, persistent_name::FaceName};
    let (mut adapter, _) = proof();
    let base = adapter.session.doc.features[0].id;
    let fillet =
        adapter.execute("solid.fillet", json!({"edges":[[40,0,5]],"radius":2,"body":"Foundation"})).unwrap().value["feature"].as_u64().unwrap();
    let world = adapter.session.world_state().unwrap();
    let body = world.bodies.iter().find(|body| body.name == "Foundation").unwrap();
    let names = naming::face_names(body);
    let doc = adapter.session.doc.clone();
    let undo = adapter.session.undo.len();
    let revision = adapter.geometry_revision;
    for feature in [base, fillet] {
        let index = names
            .iter()
            .enumerate()
            .find(|(index, name)| {
                naming::face_name_exact(body, *index) && FaceName::parse(name).ok().and_then(|name| name.made_by()).map(|id| id.0) == Some(feature)
            })
            .unwrap()
            .0;
        let selection = [Sel::Face { body: body.name.clone(), index, point: Vec3::ZERO }];
        let workspace = adapter.workspace(&selection, false, false);
        assert_eq!(workspace["selection_features"], json!([feature]), "face provenance must distinguish a fillet from its base");
        assert_eq!(workspace["selection"], json!(selection), "history emphasis must retain the face selection");
    }
    let mesh = body.mesh();
    let (index, expected) = mesh
        .edge_faces
        .iter()
        .enumerate()
        .find_map(|(index, faces)| {
            let origins: std::collections::BTreeSet<_> = faces
                .iter()
                .filter_map(|face| {
                    let index = *face as usize;
                    naming::face_name_exact(body, index).then(|| FaceName::parse(names.get(index)?).ok()?.made_by().map(|id| id.0)).flatten()
                })
                .collect();
            (origins.contains(&fillet) && origins.contains(&base)).then_some((index, origins))
        })
        .unwrap();
    assert_eq!(
        adapter.workspace(&[Sel::Edge { body: body.name.clone(), index, point: Vec3::ZERO }], false, false)["selection_features"],
        json!(expected)
    );
    for pick in [Sel::Body { name: body.name.clone() }, Sel::Vertex { body: body.name.clone(), point: Vec3::ZERO }] {
        assert_eq!(adapter.workspace(&[pick], false, false)["selection_features"], json!([base]));
    }
    let selection = [Sel::Body { name: body.name.clone() }, Sel::Feature { id: fillet }, Sel::Feature { id: base }];
    assert_eq!(adapter.workspace(&selection, false, false)["selection_features"], json!([base, fillet]), "multiple picks are deduplicated");
    assert_eq!(adapter.workspace(&[], false, false)["selection_features"], json!([]), "deselection removes all history cues");
    assert_eq!(
        adapter.workspace(&[Sel::Feature { id: u64::MAX }, Sel::Body { name: "missing".into() }], false, false)["selection_features"],
        json!([])
    );
    assert!(Arc::ptr_eq(&doc, &adapter.session.doc));
    assert_eq!(adapter.session.undo.len(), undo);
    assert_eq!(adapter.geometry_revision, revision);
}

#[test]
fn history_feature_selection_highlights_visible_sketches_construction_and_exact_occurrence_geometry() {
    let mut adapter = SessionAdapter::new();
    let model = adapter.fixture().unwrap();
    let sketch = adapter.session.doc.features.last().unwrap().id;
    let plane =
        adapter.execute("construct.plane.offset", json!({"base":"XY","offset":20,"name":"Shelf"})).unwrap().value["feature"].as_u64().unwrap();
    let mut view = Viewport::default();
    view.set_metrics(metrics(800., 600., 2.)).unwrap();
    view.begin_update(1);
    assert!(view.install(1, adapter.snapshot().unwrap()));
    for feature in [sketch, plane] {
        view.command("select.entity", &json!({"entity":{"type":"feature","id":feature},"additive":false})).unwrap();
        let snapshot = view.snapshot().unwrap();
        assert!(snapshot.selected_bodies.is_empty());
        assert!(!snapshot.highlight.lines.is_empty(), "non-solid feature must have visible feedback");
        assert_eq!(adapter.workspace(view.selection(), false, false)["selection_features"], json!([feature]));
    }
    assert_eq!(adapter.workspace(&[Sel::Profile { sketch, index: 0 }], false, false)["selection_features"], json!([sketch]));
    assert_eq!(adapter.workspace(&[Sel::Plane { name: "Shelf".into() }], false, false)["selection_features"], json!([plane]));
    adapter.execute("browser.visibility", json!({"items":[sketch,plane],"visible":false})).unwrap();
    view.begin_update(2);
    assert!(view.install(2, adapter.snapshot().unwrap()));
    for feature in [sketch, plane] {
        view.command("select.entity", &json!({"entity":{"type":"feature","id":feature}})).unwrap();
        assert!(view.snapshot().unwrap().highlight.lines.is_empty(), "selection must respect explicit visibility");
    }
    assert_eq!(model.body_count, 2);
    let mut consumed = SessionAdapter::new();
    consumed.fixture().unwrap();
    let sketch = consumed.session.doc.features.last().unwrap().id;
    consumed.execute("solid.extrude", json!({"sketch":sketch,"distance":3,"operation":"new","body_name":"From sketch"})).unwrap();
    assert!(!solidcraft_engine::view::sketch_shown(&consumed.session, sketch), "the engine normally hides a consumed sketch");
    view.begin_update(3);
    assert!(view.install(3, consumed.snapshot().unwrap()));
    view.command("select.entity", &json!({"entity":{"type":"feature","id":sketch}})).unwrap();
    assert!(!view.snapshot().unwrap().highlight.lines.is_empty(), "consumed sketch selection still outlines its source geometry");

    let (mut adapter, _, arm) = assembly_pair();
    let world = adapter.session.world_state().unwrap();
    let placements = adapter.assembly_body_paths().unwrap();
    let body = world.bodies.iter().find(|b| placements.get(&b.name) == Some(&vec![arm])).unwrap();
    let feature = body.feature;
    let name = body.name.clone();
    assert_eq!(adapter.workspace(&[Sel::Body { name: name.clone() }], false, false)["selection_features"], json!([feature]));
    assert_eq!(adapter.workspace(&[Sel::Occurrence { path: vec![arm] }], false, false)["selection_features"], json!([feature]));
    view.begin_update(4);
    assert!(view.install(4, adapter.snapshot().unwrap()));
    view.command("select.entity", &json!({"entity":{"type":"feature","id":feature}})).unwrap();
    assert_eq!(view.snapshot().unwrap().selected_bodies, [name]);
}

#[test]
fn history_navigation_preserves_undo_and_redo_while_publishing_model_stages() {
    let mut adapter = SessionAdapter::new();
    for size in [10, 20, 30] {
        adapter.execute("solid.box", json!({"length":size,"width":size,"height":size,"corner":[size * 3,0,0],"operation":"new"})).unwrap();
    }
    adapter.execute("edit.undo", json!({})).unwrap();
    let undo = adapter.session.undo.clone();
    let redo = adapter.session.redo.clone();
    assert!(!undo.is_empty() && !redo.is_empty());
    for command in ["timeline.roll_to", "timeline.rollback"] {
        for (position, volume) in [(0, 0.), (1, 1000.), (2, 9000.), (1, 1000.), (0, 0.), (2, 9000.)] {
            let revision = adapter.geometry_revision;
            let reply = adapter.execute(command, json!({"position":position})).unwrap();
            assert!(reply.changed && reply.geometry_revision > revision);
            assert_eq!(reply.value["marker"], position);
            assert!((adapter.snapshot().unwrap().volume_mm3 - volume).abs() < 1e-6);
            assert_eq!(adapter.workspace(&[], false, false)["modelling"]["position"], position);
            for (actual, expected) in [(&adapter.session.undo, &undo), (&adapter.session.redo, &redo)] {
                assert_eq!(actual.len(), expected.len(), "{command} must preserve both history stacks");
                for (actual, expected) in actual.iter().zip(expected) {
                    assert!(Arc::ptr_eq(&actual.doc, &expected.doc));
                    assert_eq!(actual.label, expected.label);
                    assert_eq!(actual.active_sketch, expected.active_sketch);
                    assert_eq!(actual.pending_moves, expected.pending_moves);
                    assert_eq!(actual.visibility, expected.visibility);
                }
            }
        }
        let doc = adapter.session.doc.clone();
        assert!(adapter.execute(command, json!({"position":-1})).is_err());
        assert!(Arc::ptr_eq(&adapter.session.doc, &doc));
        assert_eq!((adapter.session.undo.len(), adapter.session.redo.len()), (undo.len(), redo.len()));
    }
    adapter.execute("edit.redo", json!({})).unwrap();
    assert_eq!(adapter.session.doc.features.len(), 3, "redo must restore the actual model edit");
    assert!((modelling_volume(&adapter) - 36_000.).abs() < 1e-6);
    adapter.execute("timeline.roll_to", json!({"position":0})).unwrap();
    adapter.execute("edit.undo", json!({})).unwrap();
    assert_eq!(adapter.session.doc.features.len(), 2, "undo must remove the model edit instead of moving the marker");
}

#[test]
fn history_navigation_keeps_native_edits_undoable_and_new_edits_clear_redo() {
    let (mut adapter, _, _, extrude) = modelling_plate(40., "10 mm");
    let undo = adapter.session.undo.len();
    adapter.execute("timeline.roll_to", json!({"position":1})).unwrap();
    // The legacy preference preserves the manually chosen history position.
    let edit = adapter.begin_model_with_rollback(819, None, Some(extrude), &[], false).unwrap();
    let mut params = edit.params.clone();
    params["extent"]["distance"] = json!("20 mm");
    modelling_commit(&mut adapter, &edit, params);
    assert_eq!(adapter.session.undo.len(), undo + 1);
    adapter.execute("timeline.roll_to", json!({"position":"end"})).unwrap();
    assert!((modelling_volume(&adapter) - 24_000.).abs() < 1e-6);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert_eq!(adapter.session.doc.marker, Some(1));
    adapter.execute("timeline.roll_to", json!({"feature":extrude})).unwrap();
    assert!((modelling_volume(&adapter) - 12_000.).abs() < 1e-6);
    adapter.execute("edit.redo", json!({})).unwrap();
    adapter.execute("timeline.roll_to", json!({})).unwrap();
    assert!((modelling_volume(&adapter) - 24_000.).abs() < 1e-6);
    adapter.execute("edit.undo", json!({})).unwrap();
    adapter.execute("timeline.roll_to", json!({"position":"end"})).unwrap();
    adapter.execute("timeline.rename", json!({"feature":extrude,"name":"Edited plate"})).unwrap();
    assert!(adapter.session.redo.is_empty(), "a real edit must still discard the redo branch");
    assert_eq!(adapter.session.undo.len(), undo + 1);
}

#[test]
fn history_rollback_displays_each_stage_and_rebuilds_all_downstream_features_after_native_edit() {
    let (mut adapter, _, _, extrude) = modelling_plate(40., "10 mm");
    let body = adapter.session.world_state().unwrap().bodies[0].name.clone();
    adapter.execute("solid.fillet", json!({"edges":[[40,0,5]],"radius":2,"body":body})).unwrap();
    let pattern = adapter.begin_model(817, Some("solid.pattern.rectangular"), None, &[Sel::Body { name: body }]).unwrap();
    let mut params = pattern.params.clone();
    params["count1"] = json!(2);
    params["count2"] = json!(1);
    params["spacing1"] = json!("60 mm");
    modelling_commit(&mut adapter, &pattern, params);
    let ids: Vec<_> = adapter.session.doc.features.iter().map(|feature| feature.id).collect();
    assert_eq!(ids.len(), 4);
    let corner_loss = 4. - std::f64::consts::PI;
    let original = adapter.snapshot().unwrap();
    // RenderSnapshot measures tessellated curved faces. Use the established
    // curved-body relative tolerance against the analytic rounded-box volume.
    let expected_original = 2. * (12_000. - corner_loss * 10.);
    assert!((original.volume_mm3 - expected_original).abs() < expected_original * 5e-4);
    adapter.execute("timeline.roll_to", json!({"position":2})).unwrap();
    assert_eq!(adapter.workspace(&[], false, false)["modelling"]["position"], 2);
    assert!((adapter.snapshot().unwrap().volume_mm3 - 12_000.).abs() < 1e-6);
    // The inspector temporarily shows only the feature being edited, then
    // rebuilds the fillet and pattern through the latest feature on Apply.
    let edit = adapter.begin_model(818, None, Some(extrude), &[]).unwrap();
    let mut viewport = Viewport::default();
    viewport.set_metrics(metrics(800., 600., 2.)).unwrap();
    viewport.begin_update(1);
    assert!(viewport.install(1, adapter.snapshot().unwrap()));
    viewport.begin_model(edit.clone()).unwrap();
    assert!((viewport.snapshot().unwrap().model.volume_mm3 - 12_000.).abs() < 1e-6);
    assert_eq!(adapter.workspace_with_model(&[], false, true, Some(&edit))["modelling"]["position"], 2);
    assert_eq!(adapter.session.doc.marker, Some(2), "opening the form never moves the authoritative marker");
    let mut params = edit.params.clone();
    params["extent"]["distance"] = json!("20 mm");
    let preview = adapter.prepare_model(&edit, params.clone(), false).unwrap();
    assert!((preview.model.volume_mm3 - 24_000.).abs() < 1e-6, "the preview must exclude the fillet and pattern");
    viewport.publish_model(818, &preview).unwrap();
    viewport.finish_model(818, None).unwrap();
    assert!((viewport.snapshot().unwrap().model.volume_mm3 - 12_000.).abs() < 1e-6, "Cancel restores the original view");
    // The preference preserves the previous preview and marker behavior.
    let legacy = adapter.begin_model_with_rollback(819, None, Some(extrude), &[], false).unwrap();
    assert!(legacy.repair_source.is_none());
    assert!((adapter.prepare_model(&legacy, params.clone(), true).unwrap().model.volume_mm3 - 24_000.).abs() < 1e-6);
    let history = adapter.session.undo.len();
    let edited = modelling_commit(&mut adapter, &edit, params);
    let expected_edited = 2. * (24_000. - corner_loss * 20.);
    assert_eq!(adapter.session.doc.marker, None);
    assert_eq!(adapter.session.undo.len(), history + 1, "Apply creates one undo step");
    assert!((edited.volume_mm3 - expected_edited).abs() < expected_edited * 5e-4);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert_eq!(adapter.session.doc.marker, None, "Undo must not leave the temporary marker");
    assert!((adapter.snapshot().unwrap().volume_mm3 - expected_original).abs() < expected_original * 5e-4);
    adapter.execute("edit.redo", json!({})).unwrap();
    assert!((adapter.snapshot().unwrap().volume_mm3 - expected_edited).abs() < expected_edited * 5e-4);
    let mut loaded = modelling_roundtrip(&adapter);
    assert_eq!(loaded.session.doc.marker, None, "native save/reopen retains the completed timeline");
    for engine in [&mut adapter, &mut loaded] {
        let mut view = Viewport::default();
        view.set_metrics(metrics(800., 600., 2.)).unwrap();
        for (ticket, position, bodies, expected) in [
            (1, 0, 0, 0.),
            (2, 1, 0, 0.),
            (3, 2, 1, 24_000.),
            (4, 3, 1, 24_000. - corner_loss * 20.),
            (5, 4, 2, 2. * (24_000. - corner_loss * 20.)),
            (6, 2, 1, 24_000.),
            (7, 4, 2, 2. * (24_000. - corner_loss * 20.)),
        ] {
            engine.execute("timeline.roll_to", json!({"position":position})).unwrap();
            view.begin_update(ticket);
            let snapshot = engine.snapshot().unwrap();
            assert!(view.install(ticket, snapshot));
            let displayed = view.snapshot().unwrap();
            assert_eq!(displayed.model.body_count, bodies);
            let tolerance = if position < 3 { 1e-6 } else { expected * 5e-4 };
            assert!((displayed.model.volume_mm3 - expected).abs() < tolerance, "stage {position}: {} != {expected}", displayed.model.volume_mm3);
            assert!(view.selection().is_empty());
            assert_eq!(engine.workspace(&[], false, false)["modelling"]["position"], position);
            assert_eq!(engine.session.doc.features.iter().map(|feature| feature.id).collect::<Vec<_>>(), ids);
        }
    }
}

#[test]
fn history_sketch_edit_temporarily_rolls_back_and_finishes_at_latest_with_one_undo_step() {
    let (mut adapter, sketch, width, extrude) = modelling_plate(40., "10 mm");
    adapter.execute("solid.box", json!({"corner":[80,0,0],"length":2,"width":3,"height":4,"body_name":"Later","operation":"new"})).unwrap();
    let ids: Vec<_> = adapter.session.doc.features.iter().map(|feature| feature.id).collect();
    let history = adapter.session.undo.len();
    adapter.execute("sketch.edit", json!({"sketch":sketch,"restore_marker":null})).unwrap();
    assert_eq!(adapter.session.doc.marker, Some(1));
    assert_eq!(adapter.snapshot().unwrap().body_count, 0, "downstream solids must disappear while editing their sketch");
    numeric_apply(&mut adapter, &[Sel::SketchConstraint { id: width.clone() }], None, "60 mm");
    adapter.execute("sketch.finish", json!({})).unwrap();
    assert_eq!(adapter.session.doc.marker, None);
    assert!((adapter.snapshot().unwrap().volume_mm3 - 18_024.).abs() < 1e-6);
    assert_eq!(adapter.session.undo.len(), history + 1);
    assert_eq!(adapter.session.doc.features.iter().map(|feature| feature.id).collect::<Vec<_>>(), ids);
    assert!(adapter.session.doc.feature(extrude).is_some());
    adapter.execute("edit.undo", json!({})).unwrap();
    assert_eq!(adapter.session.doc.marker, None);
    assert!((adapter.snapshot().unwrap().volume_mm3 - 12_024.).abs() < 1e-6);
    adapter.execute("edit.redo", json!({})).unwrap();
    adapter.execute("sketch.edit", json!({"sketch":sketch,"restore_marker":null})).unwrap();
    numeric_apply(&mut adapter, &[Sel::SketchConstraint { id: width }], None, "80 mm");
    adapter.execute("sketch.cancel", json!({})).unwrap();
    assert_eq!(adapter.session.doc.marker, None);
    assert!((adapter.snapshot().unwrap().volume_mm3 - 18_024.).abs() < 1e-6, "Cancel restores the latest unchanged model");
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
fn topology_taps_accumulate_by_default_toggle_individually_and_allow_single_selection() {
    for (source, scale) in [(InputSource::Pencil, 1.), (InputSource::Pencil, 2.), (InputSource::Indirect, 2.)] {
        let mut adapter = SessionAdapter::new();
        adapter.execute("solid.box", json!({"length":20,"width":20,"height":20,"body_name":"Block"})).unwrap();
        let mut view = Viewport::default();
        view.set_metrics(metrics(800., 600., scale)).unwrap();
        view.begin_update(1);
        assert!(view.install(1, adapter.snapshot().unwrap()));
        let doc = adapter.session.doc.clone();
        let history = adapter.session.undo.len();
        let model = view.model.clone().unwrap();
        for (filter, locations) in [
            (PickFilter::Auto, [Vec3::new(20., 0., 10.), Vec3::new(20., 20., 10.)]),
            (PickFilter::Edge, [Vec3::new(20., 0., 10.), Vec3::new(20., 20., 10.)]),
            (PickFilter::Face, [Vec3::new(20., 10., 10.), Vec3::new(10., 0., 10.)]),
        ] {
            view.filter = filter;
            view.command("select.clear", &json!({})).unwrap();
            let points = locations.map(|location| point(&view, location));
            let tap = |view: &mut Viewport, point, shift| {
                for phase in [InputPhase::Began, InputPhase::Ended] {
                    let mut input = sample(view, source, phase, 700, point, 1.);
                    input.shift = shift;
                    view.input(input).unwrap();
                }
            };
            let expected = points.map(|p| view.pick(p, Some(model.geometry_revision)).unwrap());
            assert!(expected.iter().all(|selection| match filter {
                PickFilter::Face => matches!(selection, Sel::Face { .. }),
                _ => matches!(selection, Sel::Edge { .. }),
            }));
            tap(&mut view, points[0], false);
            tap(&mut view, points[1], false);
            assert_eq!(view.selection().len(), 2, "a second {filter:?} tap must retain the first selection");
            assert_eq!(serde_json::to_value(view.selection()).unwrap(), json!(expected));
            assert!(!view.snapshot().unwrap().highlight.meshes.is_empty() || !view.snapshot().unwrap().highlight.lines.is_empty());
            if filter == PickFilter::Face {
                let shell = adapter.begin_model(700, Some("solid.shell"), None, view.selection()).unwrap();
                assert_eq!(shell.params["faces"].as_array().unwrap().len(), 2, "both faces must reach the operation");
            }
            tap(&mut view, points[0], false);
            assert_eq!(serde_json::to_value(view.selection()).unwrap(), json!([expected[1]]), "tapping a selected item removes only that item");
            view.command("select.multiple", &json!({"enabled":false})).unwrap();
            tap(&mut view, points[0], false);
            assert_eq!(
                serde_json::to_value(view.selection()).unwrap(),
                json!([expected[0]]),
                "the saved single-selection preference retains replacement"
            );
            tap(&mut view, points[1], true);
            assert_eq!(view.selection().len(), 2, "Shift still adds when Multiple is disabled");
            view.command("select.multiple", &json!({"enabled":true})).unwrap();
            tap(&mut view, [1., 1.], false);
            assert!(view.selection().is_empty(), "an empty-space tap clears the accumulated selection");
        }
        assert!(Arc::ptr_eq(&doc, &adapter.session.doc));
        assert!(Arc::ptr_eq(&model, view.model.as_ref().unwrap()));
        assert_eq!(adapter.session.undo.len(), history, "selection never creates history entries");
    }
}

#[test]
fn finger_empty_space_taps_clear_selection_on_release_without_editing_the_design() {
    let (mut adapter, mut view) = proof();
    let feature = adapter.session.doc.features[0].id;
    let doc = adapter.session.doc.clone();
    let history = (adapter.session.undo.len(), adapter.session.redo.len());
    for scale in [1., 2.] {
        view.set_metrics(metrics(800., 600., scale)).unwrap();
        view.command("view.top", &json!({})).unwrap();
        let face = view.pick(point(&view, Vec3::new(20., 15., 10.)), None).unwrap();
        for selection in [vec![Sel::Feature { id: feature }], vec![face.clone()], vec![Sel::Feature { id: feature }, face.clone()]] {
            view.selection = selection.clone();
            let before = view.snapshot().unwrap();
            feed(&mut view, InputSource::Finger, InputPhase::Began, 1, [5., 5.], 1.);
            assert_eq!(view.selection(), selection, "touch-down may still become navigation");
            feed(&mut view, InputSource::Finger, InputPhase::Ended, 1, [8., 7.], 1.1);
            assert!(view.selection().is_empty(), "a finger tap on empty space must clear selection");
            let after = view.snapshot().unwrap();
            assert!(after.selected_bodies.is_empty());
            assert!(after.highlight.meshes.is_empty() && after.highlight.lines.is_empty());
            assert_eq!(after.camera, before.camera);
            assert!(Arc::ptr_eq(&after.model, &before.model));
            let workspace = adapter.workspace(view.selection(), false, false);
            assert!(workspace["rows"].as_array().unwrap().iter().all(|row| row["selected"] != true));
        }
    }
    assert!(Arc::ptr_eq(&doc, &adapter.session.doc));
    assert_eq!(history, (adapter.session.undo.len(), adapter.session.redo.len()));
}

#[test]
fn finger_empty_space_clearing_preserves_navigation_geometry_and_invalid_contacts() {
    for case in [
        "geometry",
        "cancelled",
        "drag_return",
        "lift_motion",
        "two_fingers",
        "pencil_overlap",
        "consumed_begin",
        "consumed_end",
        "stale_camera_begin",
        "stale_camera_end",
        "camera_changed",
        "stale_geometry_begin",
        "stale_geometry_end",
        "updating",
        "old_preview",
    ] {
        let (adapter, mut view) = proof();
        view.command("view.top", &json!({})).unwrap();
        view.filter = PickFilter::Edge;
        let selected = Sel::Feature { id: adapter.session.doc.features[0].id };
        view.selection = vec![selected.clone()];
        let position = if case == "geometry" { point(&view, Vec3::new(20., 15., 10.)) } else { [5., 5.] };
        if case == "geometry" {
            assert!(view.pick(position, None).is_none(), "an Edge filter must miss the face interior");
        }
        let camera = view.camera;
        let mut begin = sample(&view, InputSource::Finger, InputPhase::Began, 1, position, 1.);
        begin.consumed = case == "consumed_begin";
        begin.display_preview = case == "old_preview";
        if case == "stale_camera_begin" {
            begin.camera_epoch = Some(view.camera_epoch + 1);
        }
        if case == "stale_geometry_begin" {
            begin.geometry_revision = Some(view.model.as_ref().unwrap().geometry_revision + 1);
        }
        view.input(begin).unwrap();
        match case {
            "drag_return" => {
                feed(&mut view, InputSource::Finger, InputPhase::Moved, 1, [45., 25.], 1.1);
                feed(&mut view, InputSource::Finger, InputPhase::Moved, 1, position, 1.2);
            }
            "two_fingers" | "pencil_overlap" => {
                let source = if case == "two_fingers" { InputSource::Finger } else { InputSource::Pencil };
                feed(&mut view, source, InputPhase::Began, 2, [60., 60.], 1.1);
                feed(&mut view, source, InputPhase::Cancelled, 2, [60., 60.], 1.2);
            }
            "camera_changed" => {
                view.command("view.navigate", &json!({"motion":"pan","delta":[20,0],"viewport":[800,600]})).unwrap();
            }
            "updating" => view.begin_update(2),
            _ => {}
        }
        let phase = if case == "cancelled" { InputPhase::Cancelled } else { InputPhase::Ended };
        let end = if case == "lift_motion" { [position[0] + 8., position[1]] } else { position };
        let mut ended = sample(&view, InputSource::Finger, phase, 1, end, 1.3);
        ended.consumed = case == "consumed_end";
        ended.display_preview = case == "old_preview";
        if case == "stale_camera_end" {
            ended.camera_epoch = Some(view.camera_epoch + 1);
        }
        if case == "stale_geometry_end" {
            ended.geometry_revision = Some(view.model.as_ref().unwrap().geometry_revision + 1);
        }
        view.input(ended).unwrap();
        assert_eq!(view.selection(), &[selected], "{case} must preserve selection");
        if case != "drag_return" && case != "camera_changed" {
            assert_eq!(view.camera, camera, "{case} unexpectedly navigated");
        }
    }
}

#[test]
fn finger_taps_preserve_finished_profiles_and_active_sketch_geometry() {
    let mut adapter = SessionAdapter::new();
    adapter.fixture().unwrap();
    let sketch = adapter.session.doc.features.last().unwrap().id;
    adapter.execute("sketch.edit", json!({"sketch":sketch})).unwrap();
    adapter.execute("sketch.line", json!({"points":[[-32,12],[-24,12]]})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    let mut view = Viewport::default();
    view.set_metrics(metrics(800., 600., 2.)).unwrap();
    for (generation, editing) in [(1, false), (2, true)] {
        if editing {
            adapter.execute("sketch.edit", json!({"sketch":sketch})).unwrap();
        }
        view.begin_update(generation);
        assert!(view.install(generation, adapter.snapshot().unwrap()));
        if !editing {
            view.command("view.top", &json!({})).unwrap();
        }
        view.command("view.tool_handles", &json!({"enabled":false})).unwrap();
        view.filter = PickFilter::Edge;
        let selected = Sel::Feature { id: sketch };
        view.selection = vec![selected.clone()];
        for at in [Vec3::new(-28., -3., 0.), Vec3::new(-28., -12., 0.), Vec3::new(-28., 12., 0.)] {
            let position = point(&view, at);
            feed(&mut view, InputSource::Finger, InputPhase::Began, 1, position, 1.);
            feed(&mut view, InputSource::Finger, InputPhase::Ended, 1, position, 1.1);
            assert!(view.selection().contains(&selected), "visible sketch geometry must retain the feature: editing={editing}, at={at:?}");
        }
        feed(&mut view, InputSource::Finger, InputPhase::Began, 2, [5., 5.], 2.);
        feed(&mut view, InputSource::Finger, InputPhase::Ended, 2, [5., 5.], 2.1);
        assert!(view.selection().is_empty(), "empty space also clears sketch selection");
    }
}

#[test]
fn pencil_selection_taps_keep_the_touch_down_target_through_lift_motion() {
    for scale in [1., 2.] {
        let mut adapter = SessionAdapter::new();
        adapter.execute("solid.box", json!({"length":20,"width":20,"height":20,"body_name":"Block"})).unwrap();
        let mut view = Viewport::default();
        view.set_metrics(metrics(800., 600., scale)).unwrap();
        view.begin_update(1);
        assert!(view.install(1, adapter.snapshot().unwrap()));
        let camera = view.camera;
        let model = view.model.clone().unwrap();
        let history = adapter.session.undo.len();
        for (filter, location) in [
            (PickFilter::Auto, Vec3::new(20., 0., 10.)),
            (PickFilter::Edge, Vec3::new(20., 0., 10.)),
            (PickFilter::Vertex, Vec3::new(20., 0., 20.)),
            (PickFilter::Face, Vec3::new(20., 10., 10.)),
            (PickFilter::Body, Vec3::new(20., 10., 10.)),
        ] {
            view.filter = filter;
            let start = point(&view, location);
            let expected = view.pick(start, None).unwrap();
            for (id, offset) in [(1, [8., 0.]), (2, [0., -8.]), (3, [6., 6.])] {
                // Selecting and toggling use the same down target even when
                // moving to the next target starts during the final sample.
                for count in [1, 0] {
                    feed(&mut view, InputSource::Pencil, InputPhase::Began, id, start, 1.);
                    feed(&mut view, InputSource::Pencil, InputPhase::Ended, id, [start[0] + offset[0], start[1] + offset[1]], 1.1);
                    assert_eq!(view.selection().len(), count, "{filter:?}, scale {scale}, lift {offset:?}");
                    if count == 1 {
                        assert_eq!(serde_json::to_value(view.selection()).unwrap(), json!([expected]));
                    }
                }
            }
        }
        assert_eq!(view.camera, camera);
        assert!(Arc::ptr_eq(&model, view.model.as_ref().unwrap()));
        assert_eq!(adapter.session.undo.len(), history);
    }
}

#[test]
fn pencil_face_and_empty_space_taps_alternate_without_losing_contact_intent() {
    let (_, mut view) = proof();
    view.filter = PickFilter::Face;
    view.command("view.top", &json!({})).unwrap();
    let face = point(&view, Vec3::new(20., 15., 10.));
    let expected = view.pick(face, None).unwrap();
    let empty = [30., 30.];
    assert!(view.pick(empty, None).is_none());
    for id in 1..=12 {
        let (start, count) = if id % 2 == 1 { (face, 1) } else { (empty, 0) };
        // Hover travels between targets without changing contact ownership.
        feed(&mut view, InputSource::Pencil, InputPhase::Hover, 0, start, id as f64);
        feed(&mut view, InputSource::Pencil, InputPhase::Cancelled, 0, start, id as f64 + 0.01);
        feed(&mut view, InputSource::Pencil, InputPhase::Began, id, start, id as f64 + 0.02);
        feed(&mut view, InputSource::Pencil, InputPhase::Moved, id, [start[0] + 6., start[1]], id as f64 + 0.04);
        feed(&mut view, InputSource::Pencil, InputPhase::Ended, id, [start[0] + 8., start[1]], id as f64 + 0.06);
        assert_eq!(view.selection().len(), count, "tap {id} must select/clear after moving between targets");
        if count == 1 {
            assert_eq!(serde_json::to_value(view.selection()).unwrap(), json!([expected]));
        }
    }
    // A real stroke, even one returning to its origin, must never become a tap.
    for (source, distance) in [(InputSource::Pencil, 12.), (InputSource::Indirect, 8.)] {
        feed(&mut view, source, InputPhase::Began, 20, face, 20.);
        feed(&mut view, source, InputPhase::Moved, 20, [face[0] + distance, face[1]], 20.1);
        feed(&mut view, source, InputPhase::Ended, 20, face, 20.2);
        assert!(view.selection().is_empty());
    }
}

#[test]
fn pencil_edge_tap_does_not_lose_its_target_at_the_pick_tolerance_boundary() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("solid.box", json!({"length":20,"width":20,"height":20,"body_name":"Block"})).unwrap();
    let mut view = Viewport::default();
    view.set_metrics(metrics(800., 600., 2.)).unwrap();
    view.begin_update(1);
    assert!(view.install(1, adapter.snapshot().unwrap()));
    view.command("view.top", &json!({})).unwrap();
    view.filter = PickFilter::Edge;
    let edge = point(&view, Vec3::new(20., 10., 20.));
    let start = [edge[0] + 6., edge[1]];
    let end = [edge[0] + 10., edge[1]];
    let expected = view.pick(start, None).unwrap();
    assert!(view.pick(end, None).is_none(), "lift coordinate is outside the edge's eight-point target");
    for count in [1, 0] {
        feed(&mut view, InputSource::Pencil, InputPhase::Began, 1, start, 1.);
        feed(&mut view, InputSource::Pencil, InputPhase::Ended, 1, end, 1.1);
        assert_eq!(view.selection().len(), count);
        if count == 1 {
            assert_eq!(serde_json::to_value(view.selection()).unwrap(), json!([expected]));
        }
    }
    feed(&mut view, InputSource::Pencil, InputPhase::Began, 2, end, 2.);
    feed(&mut view, InputSource::Pencil, InputPhase::Ended, 2, start, 2.1);
    assert!(view.selection().is_empty(), "a tap starting outside the edge target must not acquire it during lift");
}

#[test]
fn cylindrical_rim_hover_and_taps_select_complete_rims_and_keep_other_rims_separate() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("solid.cylinder", json!({"radius":10,"height":20,"body_name":"Sleeve"})).unwrap();
    adapter.execute("solid.cylinder", json!({"radius":6,"height":22,"base":[0,0,-1],"operation":"cut","targets":["Sleeve"]})).unwrap();
    let model = adapter.snapshot().unwrap();
    let mesh = &model.picks.bodies[0].mesh;
    let rims: std::collections::BTreeSet<_> = (0..mesh.edges.len()).filter(|&i| mesh.edge_pickable(i)).map(|i| mesh.circular_rim(i)).collect();
    assert_eq!(rims.len(), 4, "inner/outer and top/bottom are four independent circular rims");
    let doc = adapter.session.doc.clone();
    let history = adapter.session.undo.len();
    for (source, scale, filter) in [
        (InputSource::Pencil, 1., PickFilter::Auto),
        (InputSource::Pencil, 2., PickFilter::Edge),
        (InputSource::Indirect, 2., PickFilter::Auto),
        (InputSource::Indirect, 2., PickFilter::Edge),
    ] {
        let mut view = Viewport::default();
        view.set_metrics(metrics(800., 600., scale)).unwrap();
        view.begin_update(1);
        assert!(view.install(1, model.clone()));
        view.filter = filter;
        for perspective in [false, true] {
            view.command("view.perspective", &json!({"enabled":perspective})).unwrap();
            for rim in &rims {
                assert_eq!(rim.len(), 1, "solidcore keeps each analytic circle as one closed edge");
                assert!(mesh.edges[rim[0]].first().unwrap().dist(*mesh.edges[rim[0]].last().unwrap()) < 1e-6);
                for &edge in rim {
                    view.command("select.clear", &json!({})).unwrap();
                    let points = &mesh.edges[edge];
                    let location = points[points.len() / 2];
                    let radial = Vec3::new(location.x, location.y, 0.).normalized().unwrap();
                    view.camera.target = Vec3::Z * 10.;
                    view.camera = view.camera.looking_from(radial + Vec3::Z * if location.z > 10. { 2. } else { -2. });
                    view.changed();
                    let p = point(&view, location);
                    let picked = view.pick(p, Some(model.geometry_revision)).unwrap();
                    assert!(matches!(&picked, Sel::Edge { index, .. } if *index == edge), "{picked:?}, expected {edge}");
                    feed(&mut view, source, InputPhase::Hover, 0, p, 1.);
                    let frame = view.snapshot().unwrap();
                    assert_eq!(frame.highlight.lines.len(), rim.len(), "hover must cover the complete rim");
                    for &piece in rim {
                        assert_eq!(frame.highlight.lines.iter().filter(|line| line.points == mesh.edges[piece]).count(), 1);
                    }
                    assert!(view.selection().is_empty());
                    for phase in [InputPhase::Began, InputPhase::Ended] {
                        feed(&mut view, source, phase, 703, p, 2.);
                    }
                    assert_eq!(view.selection().len(), rim.len(), "one tap selects the complete rim");
                    assert_eq!(view.selection().first(), Some(&picked), "preserve the actual pick and its placement point");
                    feed(&mut view, source, InputPhase::Hover, 0, p, 3.);
                    assert_eq!(view.snapshot().unwrap().highlight.lines.len(), rim.len(), "selected hover must not draw the rim twice");
                    let other = *rim.last().unwrap();
                    view.select(Some(Sel::Edge { body: "Sleeve".into(), index: other, point: mesh.edges[other][0] }), true);
                    assert!(view.selection().is_empty(), "a second pick toggles the entire rim off");
                    view.command("select.multiple", &json!({"enabled":false})).unwrap();
                    view.select(Some(picked.clone()), false);
                    assert_eq!(view.selection().len(), rim.len(), "single selection still includes the complete rim");
                    let other_rim = rims.iter().find(|other| *other != rim).unwrap();
                    let other = Sel::Edge { body: "Sleeve".into(), index: other_rim[0], point: mesh.edges[other_rim[0]][0] };
                    view.select(Some(other.clone()), true);
                    assert_eq!(view.selection().len(), rim.len() + other_rim.len(), "another rim stays independently selectable");
                    view.select(Some(picked), true);
                    assert_eq!(view.selection().len(), rim.len(), "toggling one rim retains the other");
                    view.select(Some(other), true);
                    assert!(view.selection().is_empty());
                    view.command("select.multiple", &json!({"enabled":true})).unwrap();
                    assert!(Arc::ptr_eq(&view.snapshot().unwrap().model, &model));
                }
            }
        }
    }
    assert!(Arc::ptr_eq(&doc, &adapter.session.doc));
    assert_eq!(adapter.session.undo.len(), history, "selection must not edit the document");
}

#[test]
fn cylindrical_wall_hover_and_taps_group_hidden_seams_without_merging_inner_outer_or_caps() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("solid.cylinder", json!({"radius":10,"height":20,"body_name":"Sleeve"})).unwrap();
    adapter.execute("solid.cylinder", json!({"radius":6,"height":22,"base":[0,0,-1],"operation":"cut","targets":["Sleeve"]})).unwrap();
    let model = adapter.snapshot().unwrap();
    let mesh = &model.picks.bodies[0].mesh;
    let walls: std::collections::BTreeSet<Vec<u32>> = mesh
        .edge_faces
        .iter()
        .zip(&mesh.seams)
        .filter(|(_, seam)| **seam)
        .map(|(faces, _)| {
            let mut faces = faces.clone();
            faces.sort_unstable();
            faces
        })
        .collect();
    assert_eq!(walls.len(), 2, "the inner and outer walls each have their own hidden seam");
    let doc = adapter.session.doc.clone();
    let history = adapter.session.undo.len();
    for (source, scale, filter) in [
        (InputSource::Pencil, 1., PickFilter::Auto),
        (InputSource::Pencil, 2., PickFilter::Face),
        (InputSource::Indirect, 2., PickFilter::Auto),
        (InputSource::Indirect, 2., PickFilter::Face),
    ] {
        let mut view = Viewport::default();
        view.set_metrics(metrics(800., 600., scale)).unwrap();
        view.begin_update(1);
        assert!(view.install(1, model.clone()));
        view.filter = filter;
        for perspective in [false, true] {
            view.command("view.perspective", &json!({"enabled":perspective})).unwrap();
            for wall in &walls {
                assert_eq!(wall.len(), 1, "solidcore keeps each analytic cylinder as one face");
                let triangle_count = mesh.tri_face.iter().filter(|face| wall.contains(face)).count();
                for &face in wall {
                    view.command("select.clear", &json!({})).unwrap();
                    view.command("select.multiple", &json!({"enabled":true})).unwrap();
                    let tri = mesh.triangles.iter().zip(&mesh.tri_face).find(|(_, id)| **id == face).unwrap().0;
                    let [a, b, c] = mesh.tri(tri).unwrap();
                    let centroid = (a + b + c) / 3.;
                    let radial = Vec3::new(centroid.x, centroid.y, 0.).normalized().unwrap();
                    let radius = if centroid.x.hypot(centroid.y) > 8. { 10. } else { 6. };
                    let location = radial * radius + Vec3::Z * 10.;
                    view.camera.target = Vec3::Z * 10.;
                    view.camera = view.camera.looking_from(if radius == 10. { radial + Vec3::Z * 0.4 } else { -radial + Vec3::Z * 4. });
                    view.changed();
                    let p = point(&view, location);
                    let picked = view.pick(p, Some(model.geometry_revision)).unwrap();
                    assert!(matches!(&picked, Sel::Face { index, .. } if *index == face as usize), "{picked:?}, expected {face}");
                    feed(&mut view, source, InputPhase::Hover, 0, p, 1.);
                    let highlighted = || {
                        let snapshot = view.snapshot().unwrap();
                        let count: usize = snapshot.highlight.meshes.iter().map(|m| m.mesh.triangles.len()).sum();
                        let faces: std::collections::BTreeSet<_> = snapshot.highlight.meshes.iter().flat_map(|m| &m.mesh.tri_face).copied().collect();
                        (count, faces)
                    };
                    assert_eq!(highlighted(), (triangle_count, wall.iter().copied().collect()), "hover must cover exactly the whole wall");
                    assert!(view.selection().is_empty());
                    for phase in [InputPhase::Began, InputPhase::Ended] {
                        feed(&mut view, source, phase, 702, p, 2.);
                    }
                    assert_eq!(view.selection().len(), wall.len(), "one tap selects the complete cylinder");
                    assert_eq!(view.selection().first(), Some(&picked), "keep the picked tool placement point first");
                    feed(&mut view, source, InputPhase::Hover, 0, p, 3.);
                    let snapshot = view.snapshot().unwrap();
                    assert_eq!(
                        snapshot.highlight.meshes.iter().map(|m| m.mesh.triangles.len()).sum::<usize>(),
                        triangle_count,
                        "selected hover draws the wall once"
                    );
                    assert!(Arc::ptr_eq(&snapshot.model, &model));
                    let other = *wall.last().unwrap() as usize;
                    view.select(Some(Sel::Face { body: "Sleeve".into(), index: other, point: location }), true);
                    assert!(view.selection().is_empty(), "a second pick toggles the entire wall off");
                    view.command("select.multiple", &json!({"enabled":false})).unwrap();
                    view.select(Some(picked.clone()), false);
                    assert_eq!(view.selection().len(), wall.len(), "single selection still selects one whole surface");
                    view.command("view.top", &json!({})).unwrap();
                    let cap = view.pick(point(&view, Vec3::new(8., 0., 20.)), Some(model.geometry_revision)).unwrap();
                    view.select(Some(cap), false);
                    assert_eq!(view.selection().len(), 1, "the planar cap stays separate from both walls");
                    view.select(Some(picked.clone()), true);
                    assert_eq!(view.selection().len(), wall.len() + 1, "a wall retains the separate cap");
                    let other_wall = walls.iter().find(|other| *other != wall).unwrap();
                    let other = Sel::Face { body: "Sleeve".into(), index: other_wall[0] as usize, point: location };
                    view.select(Some(other.clone()), true);
                    assert_eq!(view.selection().len(), wall.len() + other_wall.len() + 1, "inner and outer walls remain independent surfaces");
                    view.select(Some(other), true);
                    assert_eq!(view.selection().len(), wall.len() + 1, "toggling the other wall keeps this wall and cap");
                    view.select(Some(picked), true);
                    assert_eq!(view.selection().len(), 1, "toggling this wall keeps the cap");
                }
            }
        }
    }
    assert!(Arc::ptr_eq(&doc, &adapter.session.doc));
    assert_eq!(adapter.session.undo.len(), history);
}

#[test]
fn multiple_pencil_edges_reach_fillet_and_chamfer_as_one_undoable_operation() {
    for operation in ["solid.fillet", "solid.chamfer"] {
        let mut adapter = SessionAdapter::new();
        adapter.execute("solid.box", json!({"length":20,"width":20,"height":20,"body_name":"Block"})).unwrap();
        let mut view = Viewport::default();
        view.set_metrics(metrics(800., 600., 2.)).unwrap();
        view.begin_update(1);
        view.install(1, adapter.snapshot().unwrap());
        for location in [Vec3::new(20., 0., 10.), Vec3::new(20., 20., 10.)] {
            let p = point(&view, location);
            for phase in [InputPhase::Began, InputPhase::Ended] {
                feed(&mut view, InputSource::Pencil, phase, 701, p, 1.);
            }
        }
        assert_eq!(view.selection().len(), 2);
        let edit = adapter.begin_model(701, Some(operation), None, view.selection()).unwrap();
        assert_eq!(edit.params["edges"].as_array().unwrap().len(), 2);
        let history = adapter.session.undo.len();
        modelling_commit(&mut adapter, &edit, edit.params.clone());
        let loss_per_edge = if operation == "solid.fillet" { (1. - std::f64::consts::PI / 4.) * 20. } else { 10. };
        assert!((modelling_volume(&adapter) - (8_000. - 2. * loss_per_edge)).abs() < 0.1, "{operation} must modify both selected edges");
        assert_eq!(adapter.session.undo.len(), history + 1);
        adapter.execute("edit.undo", json!({})).unwrap();
        assert!((modelling_volume(&adapter) - 8_000.).abs() < 1e-6);
        adapter.execute("edit.redo", json!({})).unwrap();
        assert!((modelling_volume(&adapter) - (8_000. - 2. * loss_per_edge)).abs() < 0.1);
    }
}

#[test]
fn two_finger_pan_pinch_twist_and_pointer_navigation_never_select_or_edit() {
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
    assert!((view.camera.roll - cam.roll - 30.0_f64.atan2(250.0)).abs() < 1e-10);
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
fn two_finger_twist_direction_wrap_planar_and_saved_legacy_preference() {
    for enabled in [false, true] {
        for planar in [false, true] {
            for (start, end) in [(0.0_f64, 0.4_f64), (0.0, -0.4), (3.1, -3.1), (-3.1, 3.1)] {
                let (adapter, mut view) = proof();
                let document = adapter.session.doc.clone();
                let history = (adapter.session.undo.len(), adapter.session.redo.len());
                view.planar = planar;
                assert!(view.diagnostic()["touch_twist"]["enabled"].as_bool().unwrap());
                view.command("view.touch_twist", &json!({"enabled":enabled})).unwrap();
                let p = |angle: f64| [400.0 + 150.0 * angle.cos(), 300.0 + 150.0 * angle.sin()];
                // Deliberately reverse contact-id order: either finger can move.
                view.input(event(InputSource::Finger, InputPhase::Began, 20, [400.0, 300.0], 1.0)).unwrap();
                view.input(event(InputSource::Finger, InputPhase::Began, 10, p(start), 1.1)).unwrap();
                let camera = view.camera;
                view.input(event(InputSource::Finger, InputPhase::Moved, 10, p(end), 2.0)).unwrap();
                let difference = end - start;
                let expected = if enabled { difference.sin().atan2(difference.cos()) } else { 0.0 };
                assert!((view.camera.roll - camera.roll - expected).abs() < 1e-10);
                assert_eq!((view.camera.yaw, view.camera.pitch), (camera.yaw, camera.pitch));
                assert!(view.selection.is_empty());
                assert!(Arc::ptr_eq(&document, &adapter.session.doc));
                assert_eq!(history, (adapter.session.undo.len(), adapter.session.redo.len()));
                let camera = view.camera;
                for (id, p) in [(20, [400.0, 300.0]), (10, p(end))] {
                    view.input(event(InputSource::Finger, InputPhase::Ended, id, p, 3.0)).unwrap();
                    assert_eq!(view.camera, camera, "lifting a contact must not rotate the camera");
                }
                assert_eq!(view.diagnostic()["input"]["contacts"], 0);
            }
        }
    }
}

#[test]
fn sketch_rotation_snap_uses_sketch_axes_in_both_directions_and_preserves_free_rotation() {
    for plane_name in ["XY", "XZ", "YZ", "rotated"] {
        let mut adapter = SessionAdapter::new();
        adapter.execute("sketch.create", json!({"plane":"XY"})).unwrap();
        adapter.execute("sketch.finish", json!({})).unwrap();
        let model = adapter.snapshot().unwrap();
        let document = adapter.session.doc.clone();
        let history = (adapter.session.undo.len(), adapter.session.redo.len());
        for enabled in [true, false] {
            for target in (-8..=8).map(|step| f64::from(step) * 45.0) {
                for error in [-2.0_f64, 2.0] {
                    // Preview an arbitrary face basis as well as the principal planes.
                    let plane = match plane_name {
                        "XZ" => solidcore_record::Plane::XZ,
                        "YZ" => solidcore_record::Plane::YZ,
                        "rotated" => solidcore_record::Plane::new(Vec3::new(12., 7., -3.), Vec3::new(1., 2., 3.), Vec3::new(2., -1., 0.)).unwrap(),
                        _ => solidcore_record::Plane::XY,
                    };
                    let mut view = Viewport::default();
                    view.set_metrics(metrics(800., 600., 2.)).unwrap();
                    view.begin_update(1);
                    view.install(1, model.clone());
                    view.command("view.sketch_plane", &json!({"plane":plane})).unwrap();
                    assert_eq!(view.diagnostic()["sketch_rotation_snap"]["enabled"], true);
                    view.command("view.sketch_rotation_snap", &json!({"enabled":enabled})).unwrap();
                    let camera = view.camera;
                    let p = |degrees: f64| {
                        let angle = degrees.to_radians();
                        [400. + 200. * angle.cos(), 300. + 200. * angle.sin()]
                    };
                    feed(&mut view, InputSource::Finger, InputPhase::Began, 20, [400., 300.], 1.);
                    feed(&mut view, InputSource::Finger, InputPhase::Began, 10, p(0.), 1.1);
                    feed(&mut view, InputSource::Finger, InputPhase::Moved, 10, p(target + error), 2.);
                    let expected = if enabled { target } else { target + error }.to_radians();
                    let (right, up, _) = view.camera.basis();
                    assert!((plane.x.dot(right) - expected.cos()).abs() < 1e-10, "{plane_name}, {target}, {error}, {enabled}");
                    assert!((plane.x.dot(up) + expected.sin()).abs() < 1e-10);
                    assert_eq!((view.camera.yaw, view.camera.pitch), (camera.yaw, camera.pitch));
                    let camera = view.camera;
                    for (id, p) in [(10, p(target + error)), (20, [400., 300.])] {
                        feed(&mut view, InputSource::Finger, InputPhase::Ended, id, p, 3.);
                        assert_eq!(view.camera, camera, "lifting must not apply the unsnapped remainder");
                    }
                    assert!(view.selection.is_empty());
                }
            }
        }
        assert!(Arc::ptr_eq(&document, &adapter.session.doc));
        assert_eq!(history, (adapter.session.undo.len(), adapter.session.redo.len()));
    }
}

#[test]
fn sketch_rotation_snap_accumulates_small_moves_releases_and_rebases_without_drift() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("sketch.create", json!({"plane":"XZ"})).unwrap();
    let mut view = Viewport::default();
    view.set_metrics(metrics(800., 600., 2.)).unwrap();
    view.begin_update(1);
    view.install(1, adapter.snapshot().unwrap());
    let plane = view.model.as_ref().unwrap().sketch_plane.unwrap();
    let document = adapter.session.doc.clone();
    let history = adapter.session.undo.len();
    let p = |degrees: f64| {
        let angle = degrees.to_radians();
        [400. + 200. * angle.cos(), 300. + 200. * angle.sin()]
    };
    feed(&mut view, InputSource::Finger, InputPhase::Began, 1, [400., 300.], 1.);
    feed(&mut view, InputSource::Finger, InputPhase::Began, 2, p(0.), 1.1);
    for (index, (raw, expected)) in [
        (10., 10.),
        (42.5, 45.),
        (43., 45.),
        (46., 45.),
        (49., 45.),
        (51., 51.),
        (47., 45.),
        (43., 45.),
        (39., 39.),
        (89., 90.),
        (91., 90.),
        (94., 90.),
        (96., 96.),
        (137., 135.),
        (179., 180.),
        (181., 180.),
        (226., 225.),
        (271., 270.),
        (359., 360.),
        (361., 360.),
    ]
    .into_iter()
    .enumerate()
    {
        feed(&mut view, InputSource::Finger, InputPhase::Moved, 2, p(raw), index as f64 + 2.);
        let expected = f64::to_radians(expected);
        let (right, up, _) = view.camera.basis();
        assert!((plane.x.dot(right) - expected.cos()).abs() < 1e-10, "raw {raw}");
        assert!((plane.x.dot(up) + expected.sin()).abs() < 1e-10, "raw {raw}");
    }
    let camera = view.camera;
    for (phase, id, position, time) in [
        (InputPhase::Began, 3, [200., 200.], 30.),
        (InputPhase::Moved, 2, p(370.), 31.),
        (InputPhase::Cancelled, 3, [200., 200.], 32.),
        (InputPhase::Moved, 2, p(370.), 33.),
    ] {
        feed(&mut view, InputSource::Finger, phase, id, position, time);
        assert!(view.camera.target.dist(camera.target) < 1e-10);
        assert_eq!(
            (view.camera.yaw, view.camera.pitch, view.camera.roll, view.camera.distance),
            (camera.yaw, camera.pitch, camera.roll, camera.distance)
        );
    }
    // The old held remainder must not leak through a finger-count transition.
    feed(&mut view, InputSource::Finger, InputPhase::Moved, 2, p(377.), 34.);
    assert!((view.camera.roll - camera.roll - 7.0_f64.to_radians()).abs() < 1e-10);
    let before = view.diagnostic();
    for params in [json!({}), json!({"enabled":null}), json!({"enabled":1}), json!({"enabled":"true"})] {
        assert!(view.command("view.sketch_rotation_snap", &params).is_err());
        assert_eq!(view.diagnostic(), before);
    }
    view.command("view.sketch_rotation_snap", &json!({"enabled":false})).unwrap();
    assert_eq!(view.diagnostic()["input"]["contacts"], 0);
    assert!(Arc::ptr_eq(&document, &adapter.session.doc));
    assert_eq!(history, adapter.session.undo.len());
}

#[test]
fn two_finger_twist_preserves_anchor_and_rejects_hostile_navigation() {
    let (_, mut view) = proof();
    let screen = [300.0, 220.0];
    let (x, y, _) = view.camera.basis();
    let pivot = view.plane_point(screen, solidcore_record::Plane { origin: view.camera.target, x, y }).unwrap();
    let camera = view.camera;
    view.command("view.navigate", &json!({"motion":"roll","angle":0.7,"anchor_points":screen})).unwrap();
    let after = point(&view, pivot);
    assert!((after[0] - screen[0]).hypot(after[1] - screen[1]) < 1e-6);
    assert_eq!((view.camera.yaw, view.camera.pitch, view.camera.distance), (camera.yaw, camera.pitch, camera.distance));
    let camera = view.camera;
    for invalid in [json!({}), json!({"enabled":null}), json!({"enabled":1}), json!({"enabled":"true"})] {
        assert!(view.command("view.touch_twist", &invalid).is_err());
        assert!(view.two_finger_twist);
        assert_eq!(view.camera, camera);
    }
    for invalid in [
        json!({"motion":"roll"}),
        json!({"motion":"roll","angle":null}),
        json!({"motion":"roll","angle":"1"}),
        json!({"motion":"roll","angle":4}),
        json!({"motion":"roll","angle":1e300}),
        json!({"motion":"roll","angle":0.1,"anchor_points":[1]}),
        json!({"motion":"roll","angle":0.1,"anchor_points":[0,1e300]}),
    ] {
        assert!(view.command("view.navigate", &invalid).is_err());
        assert_eq!(view.camera, camera);
    }
}

#[test]
fn two_finger_twist_intent_count_transitions_and_close_contacts_do_not_jump() {
    let (_, mut view) = proof();
    let camera = view.camera;
    for (phase, id, point, time) in [
        (InputPhase::Began, 1, [300.0, 300.0], 1.0),
        (InputPhase::Began, 2, [500.0, 300.0], 1.1),
        (InputPhase::Moved, 2, [500.0, 305.0], 1.2), // Below intent threshold.
        (InputPhase::Began, 3, [400.0, 400.0], 1.3),
        (InputPhase::Moved, 2, [480.0, 340.0], 1.4), // Three fingers suppress navigation.
        (InputPhase::Cancelled, 3, [400.0, 400.0], 1.5),
        (InputPhase::Moved, 2, [480.0, 340.0], 1.6), // Rebased, stationary.
    ] {
        view.input(event(InputSource::Finger, phase, id, point, time)).unwrap();
        assert_eq!(view.camera, camera);
    }
    view.input(event(InputSource::Finger, InputPhase::Moved, 2, [460.0, 370.0], 2.0)).unwrap();
    assert_ne!(view.camera.roll, camera.roll);
    view.command("input.cancel", &json!({})).unwrap();
    let camera = view.camera;
    for (phase, id, p, time) in
        [(InputPhase::Began, 1, [300.0, 300.0], 3.0), (InputPhase::Began, 2, [310.0, 300.0], 3.1), (InputPhase::Moved, 2, [300.0, 310.0], 3.2)]
    {
        view.input(event(InputSource::Finger, phase, id, p, time)).unwrap();
        assert_eq!(view.camera, camera);
    }
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

fn input_fixture() -> (SessionAdapter, Viewport) {
    let mut engine = SessionAdapter::new();
    let model = engine.fixture().unwrap();
    let mut view = Viewport::default();
    view.set_metrics(metrics(800.0, 600.0, 2.0)).unwrap();
    view.begin_update(1);
    view.install(1, model);
    view.command("view.top", &json!({})).unwrap();
    view.command("tool.set", &json!({"tool":"post_radius"})).unwrap();
    (engine, view)
}
fn sample(view: &Viewport, source: InputSource, phase: InputPhase, id: u64, p: [f64; 2], time: f64) -> InputEvent {
    let mut e = event(source, phase, id, p, time);
    e.scale = view.metrics.scale;
    e.x_pixels = p[0] * e.scale;
    e.y_pixels = p[1] * e.scale;
    e.geometry_revision = view.model.as_ref().map(|m| m.geometry_revision);
    e.document_id = view.model.as_ref().map(|m| m.document_id.clone());
    e.viewport_epoch = Some(view.metrics_epoch);
    e.camera_epoch = Some(view.camera_epoch);
    e
}
fn feed(view: &mut Viewport, source: InputSource, phase: InputPhase, id: u64, p: [f64; 2], time: f64) -> Value {
    view.input(sample(view, source, phase, id, p, time)).unwrap()
}
fn start_radius(view: &mut Viewport) -> [f64; 2] {
    let h = view.handle().unwrap();
    let p = point(view, h.point(h.radius));
    feed(view, InputSource::Pencil, InputPhase::Began, 91, p, 1.0);
    p
}
fn radius_move(view: &mut Viewport, radius: f64, time: f64) -> [f64; 2] {
    let h = view.handle().unwrap();
    let p = point(view, h.point(radius));
    feed(view, InputSource::Pencil, InputPhase::Moved, 91, p, time);
    p
}
fn run_tool(engine: &mut SessionAdapter, view: &mut Viewport) -> bool {
    let Some(work) = view.take_tool_work() else { return false };
    let prepared = engine.prepare_tool(work.clone()).unwrap();
    assert!(view.tool_is_current(&work));
    if work.commit {
        let model = engine.accept_tool(prepared).unwrap();
        assert!(view.complete_tool(&work, model));
    } else {
        assert!(view.publish_tool_preview(prepared));
    }
    work.commit
}

#[test]
fn pencil_radius_drag_previews_real_geometry_and_commits_exactly_one_history_step() {
    let (mut engine, mut view) = input_fixture();
    let before = engine.session.doc.clone();
    let undo = engine.session.undo.len();
    let original = engine.snapshot().unwrap();
    let camera = view.camera;
    start_radius(&mut view);
    assert_eq!(view.selection(), &[Sel::Body { name: "Post".into() }]);
    let p = radius_move(&mut view, 10.0, 2.0);
    assert!(!run_tool(&mut engine, &mut view));
    assert!(Arc::ptr_eq(&before, &engine.session.doc));
    assert_eq!(engine.session.undo.len(), undo);
    let preview = view.snapshot().unwrap();
    assert!(!Arc::ptr_eq(&preview.model, &original));
    // Same engine tessellation pipeline: post volume scales with radius squared.
    let base = 12000.0;
    assert!(((preview.model.volume_mm3 - base) / (original.volume_mm3 - base) - 100.0 / 49.0).abs() < 0.02);
    assert_eq!(preview.geometry_revision, original.geometry_revision, "preview is not an authoritative revision");
    // Body feedback follows the evaluated preview, rather than leaving the old rim.
    for p in preview.highlight.lines.iter().filter(|line| !line.on_top).flat_map(|line| &line.points) {
        assert!(((p.x - 10.0).hypot(p.y - 4.0) - 10.0).abs() < 1e-6);
    }
    // Many updates replace one pending work item; they do not record history.
    for r in [10.2, 10.5, 11.0, 12.0] {
        radius_move(&mut view, r, 3.0);
    }
    let p2 = point(&view, view.handle().unwrap().point(12.0));
    feed(&mut view, InputSource::Pencil, InputPhase::Ended, 91, p2, 4.0);
    assert!(run_tool(&mut engine, &mut view));
    assert_eq!(engine.session.undo.len(), undo + 1);
    let committed = engine.snapshot().unwrap();
    assert_eq!(committed.radius_handle.as_ref().unwrap().radius, 12.0);
    assert_eq!(camera, view.camera);
    assert!(((committed.volume_mm3 - base) / (original.volume_mm3 - base) - 144.0 / 49.0).abs() < 0.02);
    engine.execute("edit.undo", json!({})).unwrap();
    assert!((engine.snapshot().unwrap().volume_mm3 - original.volume_mm3).abs() < 1e-6);
    engine.execute("edit.redo", json!({})).unwrap();
    assert!((engine.snapshot().unwrap().volume_mm3 - committed.volume_mm3).abs() < 1e-6);
    assert_ne!(p, p2);
}

#[test]
fn ui_ownership_and_contact_capture_are_stable_until_completion() {
    let (_, mut view) = proof();
    let camera = view.camera;
    let mut e = sample(&view, InputSource::Finger, InputPhase::Began, 2, [400.0, 300.0], 1.0);
    e.consumed = true;
    view.input(e).unwrap();
    feed(&mut view, InputSource::Finger, InputPhase::Moved, 2, [600.0, 400.0], 2.0);
    feed(&mut view, InputSource::Finger, InputPhase::Ended, 2, [600.0, 400.0], 3.0);
    let mut e = sample(&view, InputSource::Pencil, InputPhase::Began, 3, [400.0, 300.0], 4.0);
    e.consumed = true;
    view.input(e).unwrap();
    feed(&mut view, InputSource::Pencil, InputPhase::Ended, 3, [400.0, 300.0], 5.0);
    assert_eq!(camera, view.camera);
    assert!(view.selection.is_empty());
    assert_eq!(view.diagnostic()["input"]["contacts"], 0);
    let (_, mut tool) = input_fixture();
    start_radius(&mut tool);
    radius_move(&mut tool, 10.0, 2.0);
    feed(&mut tool, InputSource::Pencil, InputPhase::Moved, 91, [-1.0, 300.0], 3.0);
    assert!(tool.take_tool_work().is_none());
    assert!(tool.contacts.preview.is_none());
}

#[test]
fn one_finger_turns_the_object_in_screen_direction_from_every_view() {
    use solidcraft_render::{Camera, StandardView, UpAxis};

    for up in [UpAxis::Z, UpAxis::Y] {
        for standard in [
            StandardView::Front,
            StandardView::Back,
            StandardView::Left,
            StandardView::Right,
            StandardView::Top,
            StandardView::Bottom,
            StandardView::Iso,
        ] {
            for roll in [0.0, 0.7, std::f64::consts::PI] {
                for fov in [0.0, 0.6] {
                    for scale in [1.0, 2.0] {
                        for delta in [[20.0, 0.0], [-20.0, 0.0], [0.0, 20.0], [0.0, -20.0], [15.0, 20.0]] {
                            let mut view = Viewport::default();
                            view.set_metrics(metrics(800.0, 600.0, scale)).unwrap();
                            view.camera = Camera { up, fov, ..Camera::default() };
                            view.camera.set_view(standard);
                            view.camera.roll = roll;
                            let before = view.camera;
                            let (right, screen_up, back) = before.basis();
                            let front = before.target + back * (before.distance * 0.1);
                            let project = |camera: Camera| camera.to_screen(front, 800.0, 600.0, 100.0).unwrap();
                            let start = project(before);
                            feed(&mut view, InputSource::Finger, InputPhase::Began, 1, [400.0, 300.0], 1.0);
                            let end = [400.0 + delta[0], 300.0 + delta[1]];
                            feed(&mut view, InputSource::Finger, InputPhase::Moved, 1, end, 2.0);
                            feed(&mut view, InputSource::Finger, InputPhase::Ended, 1, end, 3.0);
                            let moved = view.camera.eye() - before.eye();
                            let projected = project(view.camera);
                            let context = (up, standard, roll, fov, scale, delta);
                            if delta[0] != 0.0 {
                                assert!(moved.dot(right) * delta[0] < 0.0, "horizontal viewpoint: {context:?}");
                                assert!((projected.0 - start.0) * delta[0] > 0.0, "horizontal object follows finger: {context:?}");
                            } else {
                                assert!(moved.dot(right).abs() < 1e-8, "horizontal drift: {context:?}");
                            }
                            if delta[1] != 0.0 {
                                assert!(moved.dot(screen_up) * delta[1] > 0.0, "vertical viewpoint: {context:?}");
                                assert!((projected.1 - start.1) * delta[1] > 0.0, "vertical object follows finger: {context:?}");
                            } else {
                                assert!(moved.dot(screen_up).abs() < 1e-8, "vertical drift: {context:?}");
                            }
                            assert_eq!((view.camera.target, view.camera.distance, view.camera.fov), (before.target, before.distance, before.fov));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn one_finger_orbits_the_off_centre_object_without_screen_drift() {
    for fov in [0.0, 0.6] {
        for over_object in [true, false] {
            let (engine, mut view) = proof();
            let model = view.model.clone().unwrap();
            let design = engine.session.doc.clone();
            let undo = engine.session.undo.len();
            view.camera.fov = fov;
            view.command("view.navigate", &json!({"motion":"pan","delta":[100,50],"viewport":[800,600]})).unwrap();
            let pivot = model.bounds.center();
            assert!(view.camera.target.dist(pivot) > 1.0);
            let before = view.camera;
            let fixed = point(&view, pivot);
            let start = if over_object { fixed } else { [20.0, 20.0] };
            feed(&mut view, InputSource::Finger, InputPhase::Began, 1, start, 1.0);
            for step in 1..=20 {
                let end = [start[0] + f64::from(step) * 8.0, start[1] + f64::from(step) * 4.0];
                feed(&mut view, InputSource::Finger, InputPhase::Moved, 1, end, 1.0 + f64::from(step));
                let projected = point(&view, pivot);
                assert!((projected[0] - fixed[0]).hypot(projected[1] - fixed[1]) < 1e-6, "object centre drifted: {fov}, {over_object}");
                assert!((view.camera.eye().dist(pivot) - before.eye().dist(pivot)).abs() < 1e-6);
            }
            assert!(view.camera.back().dot(before.back()) < 0.8, "viewpoint must change around the object");
            assert!(Arc::ptr_eq(&model, view.model.as_ref().unwrap()));
            assert!(Arc::ptr_eq(&design, &engine.session.doc));
            assert_eq!(engine.session.undo.len(), undo);
            assert!(view.selection.is_empty());
        }
    }
}

#[test]
fn touch_orbit_preference_preserves_legacy_orbit_and_leaves_pan_and_pointer_alone() {
    use solidcraft_render::Camera;

    for follows_viewpoint in [false, true] {
        for planar in [false, true] {
            let mut view = Viewport::default();
            view.set_metrics(metrics(800.0, 600.0, 2.0)).unwrap();
            let reply = view.command("view.touch_orbit", &json!({"follows_viewpoint":follows_viewpoint})).unwrap();
            assert_eq!(reply["touch_orbit"]["follows_viewpoint"], follows_viewpoint);
            view.planar = planar;
            let before = view.camera;
            feed(&mut view, InputSource::Finger, InputPhase::Began, 1, [300.0, 300.0], 1.0);
            feed(&mut view, InputSource::Finger, InputPhase::Moved, 1, [325.0, 310.0], 2.0);
            let mut expected = before;
            if planar {
                expected.pan(25.0, 10.0, 600.0);
                assert_eq!(view.camera, expected);
            } else if !follows_viewpoint {
                expected.orbit(25.0, 10.0);
                assert_eq!(view.camera, expected);
            }
            let before = view.camera;
            view.command("view.touch_orbit", &json!({"follows_viewpoint":follows_viewpoint})).unwrap();
            assert_eq!(view.diagnostic()["input"]["contacts"], 0);
            assert_eq!(view.camera, before, "changing the preference must not move the camera");
            let mut begin = sample(&view, InputSource::Indirect, InputPhase::Began, 2, [300.0, 300.0], 3.0);
            begin.button = 1;
            view.input(begin).unwrap();
            feed(&mut view, InputSource::Indirect, InputPhase::Moved, 2, [325.0, 310.0], 4.0);
            let mut expected: Camera = before;
            if planar {
                expected.pan(25.0, 10.0, 600.0);
            } else {
                expected.orbit(25.0, 10.0);
            }
            assert_eq!(view.camera, expected);
            for invalid in [json!({}), json!({"follows_viewpoint":null}), json!({"follows_viewpoint":"true"}), json!({"follows_viewpoint":1})] {
                assert_eq!(view.command("view.touch_orbit", &invalid).unwrap_err().code, "bad_params");
                assert_eq!(view.touch_orbit_follows_viewpoint, follows_viewpoint);
            }
        }
    }
}

#[test]
fn one_finger_uses_the_touched_body_centre_and_keeps_it_through_pole_crossings() {
    let (mut engine, mut view) = input_fixture();
    view.command("tool.set", &json!({"tool":"select"})).unwrap();
    let model = engine.snapshot().unwrap();
    let pivot = model.picks.bodies.iter().find(|body| body.name == "Post").unwrap().mesh.bounds().center();
    assert!(pivot.dist(model.bounds.center()) > 1.0);
    let start = point(&view, pivot);
    let before = view.camera;
    feed(&mut view, InputSource::Finger, InputPhase::Began, 1, start, 1.0);
    for step in 1..=50 {
        let previous = view.camera;
        feed(&mut view, InputSource::Finger, InputPhase::Moved, 1, [start[0], start[1] + f64::from(step) * 10.0], 1.0 + f64::from(step));
        let projected = point(&view, pivot);
        assert!((projected[0] - start[0]).hypot(projected[1] - start[1]) < 1e-6);
        assert!((view.camera.back().dot(previous.back()) - 0.08_f64.cos()).abs() < 1e-8, "continuous turn through poles");
    }
    for step in (0..50).rev() {
        feed(&mut view, InputSource::Finger, InputPhase::Moved, 1, [start[0], start[1] + f64::from(step) * 10.0], 102.0 - f64::from(step));
    }
    let (right, up, back) = before.basis();
    let (turned_right, turned_up, turned_back) = view.camera.basis();
    assert!(turned_right.dist(right) < 1e-8 && turned_up.dist(up) < 1e-8 && turned_back.dist(back) < 1e-8);
    assert!(view.camera.eye().dist(before.eye()) < 1e-6, "reversing the drag restores the viewpoint");
}

#[test]
fn one_two_one_fingers_rebase_without_jumps_or_incidental_motion() {
    let (engine, mut view) = proof();
    let before = engine.session.doc.clone();
    let camera = view.camera;
    feed(&mut view, InputSource::Finger, InputPhase::Began, 1, [300.0, 300.0], 1.0);
    feed(&mut view, InputSource::Finger, InputPhase::Moved, 1, [301.0, 301.0], 2.0);
    assert_eq!(camera, view.camera);
    feed(&mut view, InputSource::Finger, InputPhase::Moved, 1, [330.0, 310.0], 3.0);
    let camera = view.camera;
    feed(&mut view, InputSource::Finger, InputPhase::Began, 2, [500.0, 300.0], 4.0);
    feed(&mut view, InputSource::Finger, InputPhase::Moved, 1, [330.0, 310.0], 5.0);
    assert_eq!(camera, view.camera);
    feed(&mut view, InputSource::Finger, InputPhase::Moved, 2, [550.0, 330.0], 6.0);
    let camera = view.camera;
    feed(&mut view, InputSource::Finger, InputPhase::Ended, 2, [550.0, 330.0], 7.0);
    feed(&mut view, InputSource::Finger, InputPhase::Moved, 1, [330.0, 310.0], 8.0);
    assert_eq!(camera, view.camera);
    feed(&mut view, InputSource::Finger, InputPhase::Moved, 1, [340.0, 315.0], 9.0);
    assert!((view.camera.yaw - camera.yaw).abs() < 0.2);
    assert!(Arc::ptr_eq(&before, &engine.session.doc));
    assert!(view.selection.is_empty());
}

#[test]
fn pencil_then_fingers_suspend_rebase_and_commit_only_actual_edit_motion() {
    let (mut engine, mut view) = input_fixture();
    let before = engine.session.doc.clone();
    start_radius(&mut view);
    radius_move(&mut view, 9.0, 2.0);
    run_tool(&mut engine, &mut view);
    feed(&mut view, InputSource::Finger, InputPhase::Began, 1, [200.0, 300.0], 3.0);
    feed(&mut view, InputSource::Finger, InputPhase::Began, 2, [500.0, 300.0], 3.0);
    feed(&mut view, InputSource::Finger, InputPhase::Moved, 2, [550.0, 330.0], 4.0);
    let camera = view.camera;
    let p = radius_move(&mut view, 20.0, 5.0);
    assert!(view.take_tool_work().is_none());
    assert!((view.diagnostic()["input"]["radius"].as_f64().unwrap() - 9.0).abs() < 1e-9);
    for id in [2, 1] {
        feed(&mut view, InputSource::Finger, InputPhase::Ended, id, [200.0, 300.0], 6.0);
    }
    // First sample after navigation only rebases, regardless of its world position.
    feed(&mut view, InputSource::Pencil, InputPhase::Moved, 91, p, 7.0);
    assert!(view.take_tool_work().is_none());
    assert!((view.diagnostic()["input"]["radius"].as_f64().unwrap() - 9.0).abs() < 1e-9);
    assert!(Arc::ptr_eq(&before, &engine.session.doc));
    feed(&mut view, InputSource::Pencil, InputPhase::Ended, 91, p, 8.0);
    assert!(run_tool(&mut engine, &mut view));
    assert!((engine.snapshot().unwrap().radius_handle.as_ref().unwrap().radius - 9.0).abs() < 1e-9);
    assert_eq!(camera, view.camera);
}

#[test]
fn fingers_then_pencil_and_lift_during_navigation_cancel_without_history() {
    for pencil_first in [false, true] {
        let (mut engine, mut view) = input_fixture();
        let before = engine.session.doc.clone();
        let undo = engine.session.undo.len();
        if pencil_first {
            start_radius(&mut view);
            radius_move(&mut view, 10.0, 2.0);
            run_tool(&mut engine, &mut view);
        }
        feed(&mut view, InputSource::Finger, InputPhase::Began, 1, [200.0, 200.0], 3.0);
        feed(&mut view, InputSource::Finger, InputPhase::Began, 2, [500.0, 200.0], 3.0);
        feed(&mut view, InputSource::Finger, InputPhase::Moved, 2, [530.0, 230.0], 4.0);
        if !pencil_first {
            let h = view.handle().unwrap();
            let p = point(&view, h.point(h.radius));
            feed(&mut view, InputSource::Pencil, InputPhase::Began, 91, p, 5.0);
        }
        assert_eq!(view.diagnostic()["input"]["edit"], "suspended");
        let p = point(&view, view.handle().unwrap().point(15.0));
        feed(&mut view, InputSource::Pencil, InputPhase::Ended, 91, p, 6.0);
        assert!(view.take_tool_work().is_none());
        assert!(view.contacts.preview.is_none());
        assert!(Arc::ptr_eq(&before, &engine.session.doc));
        assert_eq!(undo, engine.session.undo.len());
    }
}

#[test]
fn cancellation_discards_inflight_preview_and_commit_without_undoing_other_work() {
    for command in ["input.cancel", "tool.cancel", "tool.previous"] {
        let (engine, mut view) = input_fixture();
        let before = engine.session.doc.clone();
        let p = start_radius(&mut view);
        radius_move(&mut view, 10.0, 2.0);
        feed(&mut view, InputSource::Pencil, InputPhase::Ended, 91, [p[0] + 30.0, p[1]], 3.0);
        let work = view.take_tool_work().unwrap();
        assert!(work.commit);
        let prepared = engine.prepare_tool(work.clone()).unwrap();
        view.command(command, &json!({})).unwrap();
        assert!(!view.tool_is_current(&work));
        drop(prepared);
        assert!(Arc::ptr_eq(&before, &engine.session.doc));
        assert!(view.contacts.preview.is_none());
        assert_eq!(view.diagnostic()["input"]["contacts"], 0);
    }
}

#[test]
fn predictions_never_advance_taps_previews_or_commits_and_missing_pressure_is_pencil() {
    let (mut engine, mut view) = input_fixture();
    let before = engine.session.doc.clone();
    let undo = engine.session.undo.len();
    let p = start_radius(&mut view);
    for phase in [InputPhase::Moved, InputPhase::Ended] {
        let mut e = sample(&view, InputSource::Pencil, phase, 91, [p[0] + 100.0, p[1]], 2.0);
        e.predicted = true;
        e.pressure = None;
        assert_eq!(view.input(e).unwrap()["handled"], false);
    }
    assert!(view.take_tool_work().is_none());
    feed(&mut view, InputSource::Pencil, InputPhase::Ended, 91, p, 3.0);
    assert!(view.take_tool_work().is_none());
    assert_eq!(undo, engine.session.undo.len());
    assert!(Arc::ptr_eq(&before, &engine.session.doc));
    start_radius(&mut view);
    radius_move(&mut view, 10.0, 4.0);
    assert!(!run_tool(&mut engine, &mut view));
}

#[test]
fn stale_feature_document_resize_and_source_sequences_cannot_commit() {
    let (mut engine, mut view) = input_fixture();
    start_radius(&mut view);
    let p = radius_move(&mut view, 10.0, 2.0);
    let work = view.take_tool_work().unwrap();
    engine.execute("timeline.delete", json!({"feature":"Viewport post"})).unwrap();
    assert!(engine.prepare_tool(work.clone()).is_err());
    view.begin_update(2);
    view.install(2, engine.snapshot().unwrap());
    assert!(!view.tool_is_current(&work));
    feed(&mut view, InputSource::Pencil, InputPhase::Ended, 91, p, 3.0);
    assert!(view.take_tool_work().is_none());
    let (_, mut view) = input_fixture();
    start_radius(&mut view);
    radius_move(&mut view, 10.0, 2.0);
    let old = sample(&view, InputSource::Pencil, InputPhase::Ended, 91, p, 3.0);
    view.set_metrics(ViewportMetrics { origin_points: [23.0, 81.0], ..metrics(600.0, 800.0, 3.0) }).unwrap();
    assert_eq!(view.input(old).unwrap()["handled"], false);
    assert!(view.take_tool_work().is_none());
    let h = view.handle().unwrap();
    let p = point(&view, h.point(h.radius));
    let world = view.plane_point(p, h.plane).unwrap();
    assert!(world.dist(h.point(h.radius)) < 1e-6);
    let mut e = sample(&view, InputSource::Pencil, InputPhase::Began, 2, p, 4.0);
    e.sequence = 10;
    view.input(e.clone()).unwrap();
    assert_eq!(view.input(e).unwrap()["handled"], false);
    let mut reversed = sample(&view, InputSource::Pencil, InputPhase::Moved, 2, p, 3.0);
    reversed.sequence = 11;
    assert!(view.input(reversed).is_err());
    assert_eq!(view.diagnostic()["input"]["contacts"], 0);
}

#[test]
fn active_sketch_fingers_pan_in_plane_and_require_explicit_exit() {
    let mut engine = SessionAdapter::new();
    engine.execute("sketch.create", json!({"plane":"YZ"})).unwrap();
    let mut view = Viewport::default();
    view.set_metrics(metrics(800.0, 600.0, 2.0)).unwrap();
    view.begin_update(1);
    view.install(1, engine.snapshot().unwrap());
    assert!(view.planar);
    let normal = view.model.as_ref().unwrap().sketch_plane.unwrap().normal();
    assert!(view.camera.back().dot(normal) > 0.999);
    let camera = view.camera;
    feed(&mut view, InputSource::Finger, InputPhase::Began, 1, [300.0, 300.0], 1.0);
    feed(&mut view, InputSource::Finger, InputPhase::Moved, 1, [340.0, 320.0], 2.0);
    assert_ne!(camera.target, view.camera.target);
    assert_eq!(camera.back(), view.camera.back());
    assert!((view.camera.target - camera.target).dot(normal).abs() < 1e-9);
    assert!(view.command("view.isometric", &json!({})).is_err());
    assert!(view.command("view.exit_planar", &json!({})).is_err());
    engine.execute("sketch.finish", json!({})).unwrap();
    view.begin_update(2);
    view.install(2, engine.snapshot().unwrap());
    view.command("view.exit_planar", &json!({})).unwrap();
    view.command("view.isometric", &json!({})).unwrap();
    assert!(!view.planar);
}

#[test]
fn incidental_palm_contact_and_lift_before_rebase_cannot_commit_stale_radius() {
    let (mut engine, mut view) = input_fixture();
    start_radius(&mut view);
    radius_move(&mut view, 10.0, 2.0);
    run_tool(&mut engine, &mut view);
    let camera = view.camera;
    feed(&mut view, InputSource::Finger, InputPhase::Began, 1, [200.0, 200.0], 3.0);
    feed(&mut view, InputSource::Finger, InputPhase::Moved, 1, [202.0, 201.0], 3.01);
    feed(&mut view, InputSource::Finger, InputPhase::Ended, 1, [202.0, 201.0], 3.02);
    assert_eq!(camera, view.camera);
    assert_eq!(view.diagnostic()["input"]["edit"], "dragging");
    // One finger has a conservative movement/time threshold while Pencil is down.
    feed(&mut view, InputSource::Finger, InputPhase::Began, 2, [200.0, 200.0], 4.0);
    feed(&mut view, InputSource::Finger, InputPhase::Moved, 2, [220.0, 210.0], 4.01);
    assert_eq!(camera, view.camera);
    feed(&mut view, InputSource::Finger, InputPhase::Moved, 2, [240.0, 220.0], 4.2);
    assert_ne!(camera, view.camera);
    feed(&mut view, InputSource::Finger, InputPhase::Ended, 2, [240.0, 220.0], 5.0);
    assert_eq!(view.diagnostic()["input"]["edit"], "rebase");
    let p = point(&view, view.handle().unwrap().point(20.0));
    feed(&mut view, InputSource::Pencil, InputPhase::Ended, 91, p, 6.0);
    assert!(view.take_tool_work().is_none());
    assert!(view.contacts.preview.is_none());
    assert_eq!(engine.snapshot().unwrap().radius_handle.as_ref().unwrap().radius, 7.0);
}

#[test]
fn cancelled_preview_preserves_a_previous_commit_and_rejects_stale_publication() {
    let (mut engine, mut view) = input_fixture();
    start_radius(&mut view);
    let p = radius_move(&mut view, 12.0, 2.0);
    feed(&mut view, InputSource::Pencil, InputPhase::Ended, 91, p, 3.0);
    run_tool(&mut engine, &mut view);
    let committed = engine.session.doc.clone();
    let undo = engine.session.undo.len();
    start_radius(&mut view);
    let p = radius_move(&mut view, 16.0, 4.0);
    run_tool(&mut engine, &mut view);
    radius_move(&mut view, 18.0, 5.0);
    let work = view.take_tool_work().unwrap();
    let stale = engine.prepare_tool(work.clone()).unwrap();
    feed(&mut view, InputSource::Pencil, InputPhase::Cancelled, 91, p, 6.0);
    assert!(!view.publish_preview(&work, stale.model));
    assert!(Arc::ptr_eq(&committed, &engine.session.doc));
    assert_eq!(undo, engine.session.undo.len());
    assert_eq!(view.diagnostic()["input"]["contacts"], 0);
    assert!(view.contacts.preview.is_none());
}

#[test]
fn pencil_taps_keep_camera_and_navigation_invalidates_stationary_selection_candidates() {
    let (_, mut view) = proof();
    view.command("view.top", &json!({})).unwrap();
    let p = point(&view, Vec3::new(20.0, 15.0, 10.0));
    let camera = view.camera;
    feed(&mut view, InputSource::Pencil, InputPhase::Began, 1, p, 1.0);
    feed(&mut view, InputSource::Pencil, InputPhase::Ended, 1, p, 2.0);
    assert_eq!(camera, view.camera);
    assert_eq!(view.selection.len(), 1);
    view.command("select.clear", &json!({})).unwrap();
    feed(&mut view, InputSource::Pencil, InputPhase::Began, 1, p, 3.0);
    feed(&mut view, InputSource::Finger, InputPhase::Began, 2, [200.0, 200.0], 4.0);
    feed(&mut view, InputSource::Finger, InputPhase::Moved, 2, [240.0, 220.0], 5.0);
    feed(&mut view, InputSource::Finger, InputPhase::Ended, 2, [240.0, 220.0], 6.0);
    feed(&mut view, InputSource::Pencil, InputPhase::Ended, 1, p, 7.0);
    assert!(view.selection.is_empty());
}

#[test]
fn stale_presented_camera_cannot_begin_selection_or_commit_a_handle() {
    let (_, mut view) = proof();
    view.command("view.top", &json!({})).unwrap();
    let p = point(&view, Vec3::new(20.0, 15.0, 10.0));
    let mut begin = sample(&view, InputSource::Pencil, InputPhase::Began, 1, p, 1.0);
    view.command("view.navigate", &json!({"motion":"pan","delta":[20,0],"viewport":[800,600]})).unwrap();
    view.input(begin.clone()).unwrap();
    begin.phase = InputPhase::Ended;
    begin.timestamp = 2.0;
    view.input(begin).unwrap();
    assert!(view.selection.is_empty());
    let (_, mut view) = input_fixture();
    let p = start_radius(&mut view);
    radius_move(&mut view, 12.0, 2.0);
    let mut end = sample(&view, InputSource::Pencil, InputPhase::Ended, 91, p, 3.0);
    end.camera_epoch = Some(0);
    view.input(end).unwrap();
    assert!(view.take_tool_work().is_none());
    assert!(view.contacts.preview.is_none());
}

#[test]
fn cancelled_preview_still_on_screen_cannot_start_a_new_pick_or_edit() {
    let (mut engine, mut view) = input_fixture();
    let p = start_radius(&mut view);
    radius_move(&mut view, 12.0, 2.0);
    run_tool(&mut engine, &mut view);
    view.command("tool.cancel", &json!({})).unwrap();
    view.command("select.clear", &json!({})).unwrap();
    for source in [InputSource::Pencil, InputSource::Indirect] {
        for phase in [InputPhase::Began, InputPhase::Moved, InputPhase::Ended] {
            let mut e = sample(&view, source, phase, 9, p, 3.0);
            e.display_preview = true;
            view.input(e).unwrap();
        }
    }
    assert!(view.selection.is_empty());
    assert!(view.take_tool_work().is_none());
}

#[test]
fn workspace_edge_length_tracks_latest_pick_and_preserves_document_history() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("solid.box", json!({"length":40,"width":30,"height":10,"body_name":"Block"})).unwrap();
    let mut view = Viewport::default();
    view.begin_update(1);
    assert!(view.install(1, adapter.snapshot().unwrap()));
    let mesh = adapter.session.world_state().unwrap().body("Block").unwrap().mesh();
    let pick = |axis: usize, length: f64| {
        let (index, points) = mesh
            .edges
            .iter()
            .enumerate()
            .find(|(_, p)| {
                let delta = *p.last().unwrap() - *p.first().unwrap();
                (delta.len() - length).abs() < 1e-8 && [delta.x, delta.y, delta.z][axis].abs() > length - 1e-8
            })
            .unwrap();
        Sel::Edge { body: "Block".into(), index, point: (*points.first().unwrap() + *points.last().unwrap()) * 0.5 }
    };
    let before =
        (adapter.session.revision, adapter.geometry_revision, adapter.session.undo.len(), adapter.session.redo.len(), adapter.session.is_dirty());
    let doc = adapter.session.doc.clone();
    view.select(Some(pick(0, 40.0)), false);
    let straight = adapter.workspace(view.selection(), false, false);
    assert_eq!(straight["selection_edge_length_mm"], 40.0);
    assert!(straight["selection_edge_radius_mm"].is_null());
    view.select(Some(pick(1, 30.0)), true);
    assert_eq!(view.selection().len(), 2);
    assert_eq!(adapter.workspace(view.selection(), false, false)["selection_edge_length_mm"], 30.0);
    view.command("select.entity", &json!({"entity":{"type":"body","name":"Block"},"additive":true})).unwrap();
    assert!(adapter.workspace(view.selection(), false, false)["selection_edge_length_mm"].is_null());
    view.command("select.clear", &json!({})).unwrap();
    assert!(adapter.workspace(view.selection(), false, false)["selection_edge_length_mm"].is_null());
    let stale = Sel::Edge { body: "Missing".into(), index: 0, point: Vec3::ZERO };
    assert!(adapter.workspace(&[stale], false, false)["selection_edge_length_mm"].is_null());
    assert!(Arc::ptr_eq(&doc, &adapter.session.doc));
    assert_eq!(
        before,
        (adapter.session.revision, adapter.geometry_revision, adapter.session.undo.len(), adapter.session.redo.len(), adapter.session.is_dirty())
    );
}

#[test]
fn workspace_edge_length_uses_circular_rim_circumference() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("solid.cylinder", json!({"radius":7,"height":20,"body_name":"Post"})).unwrap();
    let mesh = adapter.session.world_state().unwrap().body("Post").unwrap().mesh();
    let (index, points) =
        mesh.edges.iter().enumerate().find(|(index, _)| !mesh.circular_rim(*index).is_empty() && mesh.edge_pickable(*index)).unwrap();
    let edge = Sel::Edge { body: "Post".into(), index, point: points[points.len() / 2] };
    let measured = adapter.workspace(&[edge], false, false);
    let length = measured["selection_edge_length_mm"].as_f64().unwrap();
    assert!((length - std::f64::consts::TAU * 7.0).abs() < 1e-6, "circular edge must measure its full rim rather than a chord: {length}");
    assert!((measured["selection_edge_radius_mm"].as_f64().unwrap() - 7.0).abs() < 1e-6);
}

#[test]
fn workspace_edge_length_and_radius_measure_a_semicircle_and_clear_together() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("sketch.create", json!({"plane":"XY"})).unwrap();
    adapter.execute("sketch.arc.center_point", json!({"center":[0,0],"start":[7,0],"sweep":180,"infer":false})).unwrap();
    adapter.execute("sketch.line", json!({"points":[[-7,0],[7,0]],"infer":false})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    adapter.execute("solid.extrude", json!({"distance":15,"operation":"new","body_name":"Half"})).unwrap();
    let mesh = adapter.session.world_state().unwrap().body("Half").unwrap().mesh();
    let (index, points) = mesh.edges.iter().enumerate().find(|(_, p)| p.len() > 2 && p.iter().all(|p| (p.z - 15.0).abs() < 1e-6)).unwrap();
    let edge = Sel::Edge { body: "Half".into(), index, point: points[points.len() / 2] };
    let document = adapter.session.doc.clone();
    let history = (adapter.session.revision, adapter.geometry_revision, adapter.session.undo.len(), adapter.session.redo.len());
    let measured = adapter.workspace(&[edge], false, false);
    assert!((measured["selection_edge_length_mm"].as_f64().unwrap() - std::f64::consts::PI * 7.0).abs() < 1e-6, "{measured}");
    assert!((measured["selection_edge_radius_mm"].as_f64().unwrap() - 7.0).abs() < 1e-6, "{measured}");
    for selection in [vec![], vec![Sel::Body { name: "Half".into() }], vec![Sel::Edge { body: "Missing".into(), index: 0, point: Vec3::ZERO }]] {
        let measured = adapter.workspace(&selection, false, false);
        assert!(measured["selection_edge_length_mm"].is_null() && measured["selection_edge_radius_mm"].is_null());
    }
    assert!(Arc::ptr_eq(&document, &adapter.session.doc));
    assert_eq!(history, (adapter.session.revision, adapter.geometry_revision, adapter.session.undo.len(), adapter.session.redo.len()));
}

#[test]
fn workspace_reads_real_hierarchy_stable_ids_and_contextual_tool_reasons() {
    let (mut adapter, mut view) = input_fixture();
    let first = adapter.workspace(view.selection(), view.radius_tool_active(), false);
    assert_eq!(first["title"], adapter.session.doc.name);
    assert_eq!(first["tools"][1]["enabled"], false);
    assert_eq!(first["tools"][1]["reason"], "ui.ipados.workspace.select_post");
    assert!(first["rows"].as_array().unwrap().iter().any(|r| r["kind"] == "sketch"));
    assert_eq!(first["rows"].as_array().unwrap().iter().filter(|r| r["kind"] == "body").count(), 2);
    view.command("select.entity", &json!({"entity":{"type":"body","name":"Post"}})).unwrap();
    let selected = adapter.workspace(view.selection(), view.radius_tool_active(), false);
    assert_eq!(selected["tools"][1]["enabled"], true);
    assert_eq!(selected["radius"]["value"], 7.0);
    let before_ids: Vec<_> = first["rows"].as_array().unwrap().iter().map(|r| r["id"].clone()).collect();
    let after_ids: Vec<_> = selected["rows"].as_array().unwrap().iter().map(|r| r["id"].clone()).collect();
    assert_eq!(before_ids, after_ids);
    assert_eq!(selected["rows"].as_array().unwrap().iter().find(|r| r["id"] == "body:Post").unwrap()["selected"], true);
}

#[test]
fn browser_folders_rename_visibility_and_history_use_native_document_state() {
    fn row(adapter: &mut SessionAdapter, id: &str) -> Value {
        adapter.workspace(&[], false, false)["rows"].as_array().unwrap().iter().find(|r| r["id"] == id).unwrap().clone()
    }
    let (mut adapter, _) = input_fixture();
    assert_eq!(row(&mut adapter, "body:Base")["browser"]["icon"], "body");
    let sketch = adapter.session.doc.features.iter().find(|f| matches!(f.kind, FeatureKind::Sketch { .. })).unwrap().id;
    assert_eq!(row(&mut adapter, &format!("feature:{sketch}"))["browser"]["icon"], "sketch");
    assert_eq!(row(&mut adapter, &format!("feature:{sketch}"))["parent_id"], "folder:component:0:sketches");
    let group =
        adapter.execute("browser.group", json!({"folder":"bodies","name":"Frame","items":["Base","Post"]})).unwrap().value["group"].as_u64().unwrap();
    let folder = format!("folder:component:0:g{group}");
    assert_eq!(row(&mut adapter, "body:Base")["parent_id"], folder);
    let mut rename = row(&mut adapter, "body:Base")["browser"]["rename"].clone();
    rename["params"]["name"] = json!("Foot");
    adapter.execute(rename["command"].as_str().unwrap(), rename["params"].clone()).unwrap();
    assert_eq!(row(&mut adapter, "body:Foot")["parent_id"], folder);
    assert!(adapter.session.doc.browser_groups[0].items.contains(&"Foot".into()));
    adapter.execute("browser.visibility", json!({"items":["Foot"],"visible":false})).unwrap();
    let mut params = row(&mut adapter, &folder)["browser"]["visibility"].clone();
    params["visible"] = json!(false);
    adapter.execute("browser.visibility", params.clone()).unwrap();
    assert_eq!(adapter.snapshot().unwrap().body_count, 0);
    assert_eq!(row(&mut adapter, "body:Post")["browser"]["visible"], true, "hiding a folder retains each item's own setting");
    assert_eq!(row(&mut adapter, "body:Post")["browser"]["drawn"], false);
    params["visible"] = json!(true);
    adapter.execute("browser.visibility", params).unwrap();
    assert_eq!(adapter.snapshot().unwrap().body_count, 1);
    assert_eq!(row(&mut adapter, "body:Foot")["browser"]["visible"], false);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert_eq!(adapter.snapshot().unwrap().body_count, 0);
    adapter.execute("edit.redo", json!({})).unwrap();
    assert_eq!(adapter.snapshot().unwrap().body_count, 1);
    adapter.execute("browser.rename_group", json!({"group":group,"name":"Bed frame"})).unwrap();
    assert_eq!(row(&mut adapter, &folder)["label"], "Bed frame");
    let path = std::env::temp_dir().join(format!("solidcraft-ipados-browser-{}.solidcraft", std::process::id()));
    adapter.execute("file.save_as", json!({"path":path})).unwrap();
    let mut loaded = SessionAdapter::new();
    loaded.execute("doc.open", json!({"path":path})).unwrap();
    assert_eq!(row(&mut loaded, "body:Foot")["parent_id"], folder);
    assert_eq!(row(&mut loaded, "body:Foot")["browser"]["visible"], false);
    assert_eq!(row(&mut loaded, &folder)["label"], "Bed frame");
    loaded.execute("browser.move", json!({"folder":"bodies","items":["Post"]})).unwrap();
    assert_eq!(row(&mut loaded, "body:Post")["parent_id"], "folder:component:0:bodies");
    loaded.execute("browser.ungroup", json!({"group":group})).unwrap();
    assert_eq!(row(&mut loaded, "body:Foot")["parent_id"], "folder:component:0:bodies");
    assert_eq!(loaded.snapshot().unwrap().body_count, 1, "removing a folder preserves its parts");
    std::fs::remove_file(path).unwrap();
}

#[test]
fn browser_nested_groups_preserve_parentage_and_inherited_visibility() {
    fn row(adapter: &mut SessionAdapter, id: &str) -> Value {
        adapter.workspace(&[], false, false)["rows"].as_array().unwrap().iter().find(|r| r["id"] == id).unwrap().clone()
    }
    let (mut adapter, _) = input_fixture();
    let outer = adapter.execute("browser.group", json!({"folder":"bodies","name":"Frame"})).unwrap().value["group"].as_u64().unwrap();
    let inner =
        adapter.execute("browser.group", json!({"parent":outer,"name":"Legs","items":["Base","Post"]})).unwrap().value["group"].as_u64().unwrap();
    let empty = adapter.execute("browser.group", json!({"parent":inner,"name":"Empty"})).unwrap().value["group"].as_u64().unwrap();
    let folder = |id| format!("folder:component:0:g{id}");
    assert_eq!(row(&mut adapter, &folder(outer))["parent_id"], "folder:component:0:bodies");
    assert_eq!(row(&mut adapter, &folder(inner))["parent_id"], folder(outer));
    assert_eq!(row(&mut adapter, &folder(empty))["parent_id"], folder(inner));
    assert_eq!(row(&mut adapter, "body:Base")["parent_id"], folder(inner));
    for visible in [false, true] {
        let mut params = row(&mut adapter, &folder(outer))["browser"]["visibility"].clone();
        params["visible"] = json!(visible);
        adapter.execute("browser.visibility", params).unwrap();
        for id in [folder(inner), folder(empty), "body:Base".into(), "body:Post".into()] {
            let r = row(&mut adapter, &id);
            assert_eq!(r["browser"]["visible"], true, "hiding an ancestor retains {id}'s own setting");
            assert_eq!(r["browser"]["drawn"], visible, "{id} inherits the ancestor's visibility");
        }
        assert_eq!(adapter.snapshot().unwrap().body_count, if visible { 2 } else { 0 });
    }
}

#[test]
fn browser_selection_and_pencil_selection_share_the_same_model_and_multiselection() {
    let (mut adapter, mut view) = input_fixture();
    view.command("tool.set", &json!({"tool":"select"})).unwrap();
    view.command("select.entity", &json!({"entity":{"type":"body","name":"Post"}})).unwrap();
    assert_eq!(view.snapshot().unwrap().selected_bodies, ["Post"]);
    view.command("select.entity", &json!({"entity":{"type":"body","name":"Base"},"additive":true})).unwrap();
    assert_eq!(view.selection().len(), 2);
    view.command("select.entity", &json!({"entity":{"type":"body","name":"Base"},"additive":true})).unwrap();
    assert_eq!(view.selection().len(), 1);
    view.command("select.filter", &json!({"filter":"face"})).unwrap();
    let p = point(&view, Vec3::new(-8.0, 0.0, 10.0));
    feed(&mut view, InputSource::Pencil, InputPhase::Began, 200, p, 1.0);
    feed(&mut view, InputSource::Pencil, InputPhase::Ended, 200, p, 2.0);
    let workspace = adapter.workspace(view.selection(), false, false);
    assert_eq!(workspace["rows"].as_array().unwrap().iter().find(|r| r["id"] == "body:Base").unwrap()["selected"], true);
    assert_eq!(workspace["rows"].as_array().unwrap().iter().find(|r| r["id"] == "body:Post").unwrap()["selected"], true);
    view.command("select.multiple", &json!({"enabled":false})).unwrap();
    feed(&mut view, InputSource::Pencil, InputPhase::Began, 201, p, 3.0);
    feed(&mut view, InputSource::Pencil, InputPhase::Ended, 201, p, 4.0);
    let single = adapter.workspace(view.selection(), false, false);
    assert_eq!(single["rows"].as_array().unwrap().iter().find(|r| r["id"] == "body:Base").unwrap()["selected"], true);
    assert_eq!(single["rows"].as_array().unwrap().iter().find(|r| r["id"] == "body:Post").unwrap()["selected"], false);
    assert!(view.command("select.entity", &json!({"entity":{"type":"body","name":"Missing"}})).is_err());
}

#[test]
fn numeric_expression_preview_commit_and_history_use_real_geometry_once() {
    let (mut adapter, mut view) = input_fixture();
    adapter.execute("parameters.add", json!({"name":"post_size","expression":"6 mm"})).unwrap();
    view.begin_update(2);
    view.install(2, adapter.snapshot().unwrap());
    view.command("select.entity", &json!({"entity":{"type":"body","name":"Post"}})).unwrap();
    let before = adapter.snapshot().unwrap();
    let history = adapter.session.undo.len();
    let edit = adapter.begin_numeric(10, view.selection()).unwrap();
    view.begin_numeric(edit.clone()).unwrap();
    let preview = adapter.prepare_numeric(&edit, "post_size * 2", false).unwrap();
    view.publish_numeric(10, &preview).unwrap();
    assert!(view.snapshot().unwrap().model.volume_mm3 > before.volume_mm3);
    assert_eq!(adapter.session.undo.len(), history);
    assert_eq!(adapter.snapshot().unwrap().volume_mm3, before.volume_mm3);
    drop(preview);
    let prepared = adapter.prepare_numeric(&edit, "post_size * 2", true).unwrap();
    let model = adapter.accept_numeric(&edit, prepared).unwrap();
    let volume = model.volume_mm3;
    view.finish_numeric(10, Some(model)).unwrap();
    assert_eq!(adapter.session.undo.len(), history + 1);
    assert!((adapter.radius_handle().unwrap().radius - 12.0).abs() < 1e-9);
    let FeatureKind::Cylinder { radius, .. } =
        &adapter.session.doc.features.iter().find(|f| f.id == adapter.radius_handle().unwrap().feature).unwrap().kind
    else {
        panic!("cylinder")
    };
    assert_eq!(radius.as_str(), "post_size * 2");
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!((adapter.radius_handle().unwrap().radius - 7.0).abs() < 1e-9);
    assert_eq!(adapter.snapshot().unwrap().volume_mm3, before.volume_mm3);
    adapter.execute("edit.redo", json!({})).unwrap();
    assert_eq!(adapter.snapshot().unwrap().volume_mm3, volume);
}

#[test]
fn numeric_validation_preserves_preview_and_cancel_restores_original_without_history() {
    let (mut adapter, mut view) = input_fixture();
    view.command("select.entity", &json!({"entity":{"type":"body","name":"Post"}})).unwrap();
    let before = adapter.snapshot().unwrap();
    let history = adapter.session.undo.len();
    let edit = adapter.begin_numeric(11, view.selection()).unwrap();
    view.begin_numeric(edit.clone()).unwrap();
    assert!((adapter.prepare_numeric(&edit, "1,5 mm", false).unwrap().value - 1.5).abs() < 1e-9);
    assert_eq!(adapter.prepare_numeric(&edit, "max(8,9)", false).unwrap().value, 9.0);
    let preview = adapter.prepare_numeric(&edit, "9", false).unwrap();
    view.publish_numeric(11, &preview).unwrap();
    let valid_volume = view.snapshot().unwrap().model.volume_mm3;
    for expression in ["", "-", "2 +", "1,2,3", "not_a_parameter", "0", "51", "-3"] {
        assert!(adapter.prepare_numeric(&edit, expression, false).is_err(), "{expression}");
        assert_eq!(view.snapshot().unwrap().model.volume_mm3, valid_volume);
        assert_eq!(adapter.session.undo.len(), history);
    }
    view.finish_numeric(11, None).unwrap();
    assert_eq!(view.snapshot().unwrap().model.volume_mm3, before.volume_mm3);
    assert_eq!(adapter.snapshot().unwrap().volume_mm3, before.volume_mm3);
    assert_eq!(adapter.session.undo.len(), history);
    assert!(view.numeric_edit(11).is_err());
}

#[test]
fn numeric_focus_locks_selection_and_tools_but_allows_finger_navigation_and_keyboard_resize() {
    let (mut adapter, mut view) = input_fixture();
    view.command("select.entity", &json!({"entity":{"type":"body","name":"Post"}})).unwrap();
    let edit = adapter.begin_numeric(12, view.selection()).unwrap();
    view.begin_numeric(edit.clone()).unwrap();
    let before_selection = view.selection().to_vec();
    assert!(view.command("select.clear", &json!({})).is_err());
    assert!(view.command("tool.set", &json!({"tool":"select"})).is_err());
    let workspace = adapter.workspace(view.selection(), true, true);
    assert_eq!(workspace["can_undo"], false);
    assert_eq!(workspace["tools"][0]["reason"], "ui.ipados.workspace.finish_edit");
    let p = point(&view, Vec3::new(-8.0, 0.0, 10.0));
    feed(&mut view, InputSource::Pencil, InputPhase::Began, 201, p, 1.0);
    feed(&mut view, InputSource::Pencil, InputPhase::Ended, 201, p, 2.0);
    assert_eq!(view.selection(), before_selection);
    let camera = view.camera;
    feed(&mut view, InputSource::Finger, InputPhase::Began, 202, [200.0, 200.0], 3.0);
    feed(&mut view, InputSource::Finger, InputPhase::Moved, 202, [250.0, 230.0], 4.0);
    feed(&mut view, InputSource::Finger, InputPhase::Ended, 202, [250.0, 230.0], 5.0);
    assert_ne!(view.camera, camera);
    view.set_metrics(metrics(420.0, 310.0, 2.0)).unwrap();
    assert!(view.numeric_edit(12).is_ok());
    assert_eq!(view.selection(), before_selection);
    view.cancel_numeric();
    assert!(view.numeric_edit(12).is_err());
}

#[test]
fn stale_numeric_tickets_cannot_commit_over_document_or_selection_changes() {
    let (mut adapter, mut view) = input_fixture();
    view.command("select.entity", &json!({"entity":{"type":"body","name":"Post"}})).unwrap();
    let edit = adapter.begin_numeric(13, view.selection()).unwrap();
    view.begin_numeric(edit.clone()).unwrap();
    let prepared = adapter.prepare_numeric(&edit, "10", true).unwrap();
    view.cancel_numeric();
    view.command("select.entity", &json!({"entity":{"type":"body","name":"Base"}})).unwrap();
    assert!(view.numeric_edit(13).is_err());
    drop(prepared);
    adapter.execute("timeline.edit", json!({"feature":"Viewport post","set":{"height":"30"}})).unwrap();
    assert!(adapter.prepare_numeric(&edit, "10", true).is_err());
    assert_eq!(adapter.radius_handle().unwrap().radius, 7.0);
}

#[test]
fn empty_workspace_and_invalidated_browser_selections_are_controlled() {
    let mut adapter = SessionAdapter::new();
    let mut view = Viewport::default();
    let workspace = adapter.workspace(&[], false, false);
    assert_eq!(workspace["rows"].as_array().unwrap().len(), 2);
    let sketches = workspace["rows"].as_array().unwrap().iter().find(|row| row["id"] == "folder:component:0:sketches").unwrap();
    assert_eq!(sketches["browser"]["icon"], "folder");
    assert_eq!(sketches["label"], "ui.ipados.browser.sketches");
    assert_eq!(workspace["can_undo"], false);
    assert_eq!(workspace["can_redo"], false);
    assert_eq!(workspace["tools"][1]["reason"], "ui.ipados.workspace.no_radius");
    assert!(adapter.begin_numeric(1, &[]).is_err());
    assert!(view.command("select.entity", &json!({"entity":{"type":"feature","id":1}})).is_err());
}

#[test]
fn panel_aspect_changes_keep_fitted_handle_visible_and_reverse_without_geometry_or_history_changes() {
    let (mut adapter, mut view) = input_fixture();
    view.command("view.isometric", &json!({})).unwrap();
    view.command("view.fit", &json!({})).unwrap();
    let model = view.snapshot().unwrap().model;
    let camera = view.camera;
    let history = adapter.session.undo.len();
    let document = adapter.session.doc.clone();
    let handle = view.handle_point().unwrap();
    for _ in 0..20 {
        view.set_metrics(metrics(320.0, 900.0, 2.0)).unwrap();
        let p = point(&view, handle);
        assert!(view.metrics.pixel(p).is_some(), "fitted handle remains in narrow portrait canvas");
        assert!(view.camera.half_height() > camera.half_height());
        assert!(Arc::ptr_eq(&view.snapshot().unwrap().model, &model));
        view.set_metrics(metrics(320.0, 360.0, 2.0)).unwrap();
        assert!(view.metrics.pixel(point(&view, handle)).is_some(), "keyboard resize keeps the handle visible");
        view.set_metrics(metrics(800.0, 600.0, 2.0)).unwrap();
        assert!((view.camera.distance - camera.distance).abs() < 1e-9);
        assert_eq!(view.camera.target, camera.target);
        assert_eq!(view.camera.yaw, camera.yaw);
        assert_eq!(view.camera.pitch, camera.pitch);
    }
    assert!(Arc::ptr_eq(&adapter.session.doc, &document));
    assert_eq!(adapter.session.undo.len(), history);
    assert_eq!(adapter.snapshot().unwrap().volume_mm3, model.volume_mm3);
}

fn sketch_commit(adapter: &mut SessionAdapter, commands: Vec<(String, Value)>) -> Arc<SceneSnapshot> {
    let id = adapter.session.active_sketch.unwrap();
    let work = ToolWork {
        interaction: 100,
        version: 1,
        document_id: adapter.session.document_id.clone(),
        geometry_revision: adapter.geometry_revision,
        operation: ToolOperation::Sketch { sketch: id, commands },
        commit: true,
    };
    let prepared = adapter.prepare_tool(work).unwrap();
    adapter.accept_tool(prepared).unwrap()
}
fn numeric_apply(adapter: &mut SessionAdapter, selection: &[Sel], mode: Option<&str>, expression: &str) -> NumericEdit {
    let edit = adapter.begin_numeric_for(100, selection, mode).unwrap();
    let before = adapter.session.undo.len();
    let prepared = adapter.prepare_numeric(&edit, expression, true).unwrap();
    adapter.accept_numeric(&edit, prepared).unwrap();
    assert_eq!(adapter.session.undo.len(), before + 1);
    edit
}
fn curve_selection(sketch: u64, id: &str) -> Vec<Sel> {
    vec![Sel::SketchCurve { id: id.into(), sketch: Some(sketch) }]
}

#[test]
fn sketch_dimensioned_rectangle_reopens_roundtrips_and_regenerates_native_prism() {
    let mut adapter = SessionAdapter::new();
    let id = adapter.execute("sketch.create", json!({"plane":"XY","name":"Plate sketch"})).unwrap().value["sketch"].as_u64().unwrap();
    let rectangle = adapter.execute("sketch.rectangle.two_point", json!({"p0":[0,0],"p1":[40,30]})).unwrap().value;
    let bottom = rectangle["sides"]["bottom"].as_str().unwrap();
    let right = rectangle["sides"]["right"].as_str().unwrap();
    numeric_apply(&mut adapter, &curve_selection(id, bottom), Some("length"), "40 mm");
    numeric_apply(&mut adapter, &curve_selection(id, right), Some("length"), "30 mm");
    let sketch = adapter.sketch_snapshot().unwrap();
    assert!(sketch.solved && sketch.dof == 0);
    assert_eq!(sketch.profiles, 1);
    assert!(sketch.sketch.points.iter().filter(|p| p.id != "origin").any(|p| p.pos.len() < 1e-7));
    let width = sketch
        .sketch
        .constraints
        .iter()
        .find(|c| c.kind.is_dimension() && c.kind.indices().1.iter().any(|i| sketch.sketch.curves[*i].id == bottom))
        .unwrap()
        .id
        .clone();
    adapter.execute("sketch.finish", json!({})).unwrap();
    adapter.execute("solid.extrude", json!({"sketch":id,"distance":"10 mm","body_name":"Plate"})).unwrap();
    assert!((adapter.snapshot().unwrap().volume_mm3 - 12000.0).abs() < 1e-6);
    adapter.execute("sketch.edit", json!({"sketch":id})).unwrap();
    let selection = [Sel::SketchConstraint { id: width.clone() }];
    let edit = numeric_apply(&mut adapter, &selection, None, "60 mm");
    assert_eq!(edit.expression, "40 mm");
    assert!((adapter.snapshot().unwrap().volume_mm3 - 18000.0).abs() < 1e-6);
    let sk = adapter.sketch_snapshot().unwrap();
    let line = sk.sketch.curve_index(right).unwrap();
    let pts = sk.sketch.curves[line].kind.point_ids();
    assert!((sk.sketch.point(pts[0]).unwrap().dist(sk.sketch.point(pts[1]).unwrap()) - 30.0).abs() < 1e-7);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!((adapter.snapshot().unwrap().volume_mm3 - 12000.0).abs() < 1e-6);
    adapter.execute("edit.redo", json!({})).unwrap();
    assert!((adapter.snapshot().unwrap().volume_mm3 - 18000.0).abs() < 1e-6);
    adapter.execute("sketch.finish", json!({})).unwrap();
    // The actual native container reader/writer, with no filesystem or platform browser.
    let bytes = solidcraft_engine::io::write_design_container(&adapter.session.doc).unwrap();
    let doc = solidcraft_engine::io::DesignContainerReader::new(std::io::Cursor::new(bytes)).unwrap().read_document().unwrap();
    let mut loaded = SessionAdapter { session: solidcraft_engine::Session::new(doc), geometry_revision: 1, workspace_cache: None };
    loaded.execute("sketch.edit", json!({"sketch":id})).unwrap();
    let edit = loaded.begin_numeric(101, &selection).unwrap();
    assert_eq!(edit.expression, "60 mm");
    let p = loaded.prepare_numeric(&edit, "40 mm", true).unwrap();
    loaded.accept_numeric(&edit, p).unwrap();
    assert!((loaded.snapshot().unwrap().volume_mm3 - 12000.0).abs() < 1e-6);
}

#[test]
fn sketch_resize_preserves_circle_cut_split_by_projected_face_edges() {
    let mut adapter = SessionAdapter::new();
    let sketch = adapter.execute("sketch.create", json!({"plane":"XY"})).unwrap().value["sketch"].as_u64().unwrap();
    let rectangle = adapter.execute("sketch.rectangle.two_point", json!({"p0":[0,0],"p1":[80,80]})).unwrap().value;
    let mut dimensions = Vec::new();
    for side in ["top", "right"] {
        let curve = rectangle["sides"][side].as_str().unwrap();
        numeric_apply(&mut adapter, &curve_selection(sketch, curve), Some("length"), "80 mm");
        dimensions.push(adapter.sketch_snapshot().unwrap().sketch.constraints.iter().filter_map(|c| c.param.clone()).next_back().unwrap());
    }
    adapter.execute("sketch.finish", json!({})).unwrap();
    adapter.execute("solid.extrude", json!({"sketch":sketch,"distance":4})).unwrap();
    let body = adapter.session.world_state().unwrap().bodies[0].name.clone();
    let face = modelling_face(&adapter, &body, Vec3::new(40., 40., 4.));
    let Sel::Face { index, point, .. } = face else { panic!("top face") };
    let top = adapter.execute("sketch.create", json!({"plane":{"face":point,"body":body,"index":index}})).unwrap().value["sketch"].as_u64().unwrap();
    let projected_corner = adapter
        .sketch_snapshot()
        .unwrap()
        .sketch
        .points
        .iter()
        .find(|point| point.link.is_some() && (point.pos.x - 80.).abs() < 1e-7 && (point.pos.y - 80.).abs() < 1e-7)
        .unwrap()
        .id
        .clone();
    adapter.execute("sketch.circle.center", json!({"center":projected_corner,"radius":45})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    let state = adapter.session.model.state();
    let selected: Vec<_> = state
        .sketch(top)
        .unwrap()
        .profiles
        .iter()
        .enumerate()
        .filter(|(_, profile)| (profile.region.interior_point() - solidcore_record::Vec2::new(80., 80.)).len() < 45.)
        .map(|(index, _)| Sel::Profile { sketch: top, index })
        .collect();
    assert_eq!(selected.len(), 2);
    for selection in &selected {
        let Sel::Profile { index, .. } = selection else { panic!("profile") };
        let solved = state.sketch(top).unwrap();
        let captured = solidcraft_engine::doc::capture_profile_selection(solved, &[*index]).unwrap();
        assert_eq!(solidcraft_engine::doc::profile_selection_indices(solved, &captured).unwrap(), [*index], "a partial pick must stay partial");
    }
    let edit = adapter.begin_model(804, Some("solid.extrude"), None, &selected).unwrap();
    let mut params = edit.params.clone();
    params["distance"] = json!("10 mm");
    params["direction"] = json!("negative");
    params["operation"] = json!("cut");
    params["targets"] = json!([body]);
    modelling_commit(&mut adapter, &edit, params);
    let cut = adapter.session.doc.features.last().unwrap().id;
    assert!(matches!(&adapter.session.doc.feature(cut).unwrap().kind,
        FeatureKind::Extrude { profiles: solidcraft_engine::doc::ProfileSel::WholeBoundaries { profiles }, .. }
        if profiles.len() == 1 && profiles[0].outer == ["c1"] && profiles[0].holes.is_empty()));
    let original = 4. * (80. * 80. - std::f64::consts::PI * 45_f64.powi(2) / 4.);
    assert!((modelling_volume(&adapter) - original).abs() < original * 5e-4);
    adapter.execute("timeline.roll_to", json!({"position":1})).unwrap();
    for parameter in dimensions {
        adapter.execute("parameters.change", json!({"name":parameter,"expression":"40 mm"})).unwrap();
    }
    let mut loaded = modelling_roundtrip(&adapter);
    let cap = (45_f64.powi(2) * (40_f64 / 45.).acos() - 40. * (45_f64.powi(2) - 40_f64.powi(2)).sqrt()) / 2.;
    let expected = 4. * (40. * 40. - std::f64::consts::PI * 45_f64.powi(2) / 4. + 2. * cap);
    for adapter in [&mut adapter, &mut loaded] {
        for position in [2, 3, 4, 2, 4] {
            adapter.execute("timeline.roll_to", json!({"position":position})).unwrap();
            let actual = modelling_volume(adapter);
            let expected = if position == 4 { expected } else { 6400. };
            assert!(
                (actual - expected).abs() < expected * 5e-4,
                "stage {position}: {actual} != {expected}; {:?}",
                adapter.session.doc.feature(cut).unwrap().kind
            );
        }
        let mut reopened = modelling_roundtrip(adapter);
        assert!((modelling_volume(&reopened) - expected).abs() < expected * 5e-4);
        assert!(reopened.snapshot().unwrap().body_count > 0);
    }
}

#[test]
fn sketch_constraints_drag_conflicts_and_previews_preserve_valid_document() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("sketch.create", json!({"plane":"XY"})).unwrap();
    let line =
        adapter.execute("sketch.line", json!({"points":[[0,0],[20,0]],"infer":true})).unwrap().value["curves"][0].as_str().unwrap().to_string();
    let before = adapter.session.undo.len();
    sketch_commit(
        &mut adapter,
        vec![
            ("sketch.drag".into(), json!({"entity":format!("{line}.end"),"from":[20,0],"to":[30,15],"live":true})),
            ("sketch.drag_end".into(), json!({})),
        ],
    );
    assert_eq!(adapter.session.undo.len(), before + 1);
    let sk = adapter.sketch_snapshot().unwrap();
    let i = sk.sketch.resolve_point(&format!("{line}.end")).unwrap();
    assert!(sk.sketch.point(i).unwrap().y.abs() < 1e-6);
    numeric_apply(&mut adapter, &curve_selection(sk.id, &line), Some("length"), "30 mm");
    let doc = adapter.session.doc.clone();
    let undo = adapter.session.undo.len();
    let bad = ToolWork {
        interaction: 101,
        version: 1,
        document_id: adapter.session.document_id.clone(),
        geometry_revision: adapter.geometry_revision,
        commit: true,
        operation: ToolOperation::Sketch {
            sketch: sk.id,
            commands: vec![("sketch.dimension".into(), json!({"entities":[line],"type":"length","value":"40 mm"}))],
        },
    };
    assert!(adapter.prepare_tool(bad).is_err());
    assert!(Arc::ptr_eq(&doc, &adapter.session.doc));
    assert_eq!(adapter.session.undo.len(), undo);
    // Duplicate relationships are rejected by the native command and cannot leave half an edit.
    let duplicate = adapter.execute("sketch.constraint.horizontal_vertical", json!({"line":line,"mode":"horizontal","if_redundant":"refuse"}));
    assert!(duplicate.is_err());
    assert!(Arc::ptr_eq(&doc, &adapter.session.doc));
    assert!(adapter.execute("sketch.line", json!({"points":[[1,1],[1,1]]})).is_err());
    assert!(adapter.execute("sketch.constraint.equal", json!({"a":line,"b":"missing"})).is_err());
    assert_eq!(adapter.session.undo.len(), undo);
}

#[test]
fn sketch_native_circle_arc_and_spline_handles_preview_cancel_and_undo() {
    for (command, params) in [
        ("sketch.circle.center", json!({"center":[3,4],"radius":5})),
        ("sketch.arc.three_point", json!({"start":[0,0],"end":[10,0],"through":[5,5]})),
        ("sketch.spline.fit_point", json!({"points":[[0,0],[5,5],[10,0]]})),
    ] {
        let mut adapter = SessionAdapter::new();
        adapter.execute("sketch.create", json!({"plane":"XZ"})).unwrap();
        let doc = adapter.session.doc.clone();
        let undo = adapter.session.undo.len();
        let sketch = adapter.session.active_sketch.unwrap();
        let mut work = ToolWork {
            interaction: 100,
            version: 1,
            document_id: adapter.session.document_id.clone(),
            geometry_revision: adapter.geometry_revision,
            commit: false,
            operation: ToolOperation::Sketch { sketch, commands: vec![(command.into(), params)] },
        };
        let preview = adapter.prepare_tool(work.clone()).unwrap();
        assert!(!preview.model.sketch.as_ref().unwrap().sketch.curves.is_empty());
        drop(preview);
        assert!(Arc::ptr_eq(&doc, &adapter.session.doc));
        assert_eq!(adapter.session.undo.len(), undo);
        work.commit = true;
        let p = adapter.prepare_tool(work).unwrap();
        adapter.accept_tool(p).unwrap();
        let sk = adapter.sketch_snapshot().unwrap();
        let c = sk.sketch.curves[0].id.clone();
        let handle = sk.sketch.curves[0].kind.point_ids()[0];
        let at = sk.sketch.point(handle).unwrap();
        let point = sk.sketch.points[handle].id.clone();
        sketch_commit(
            &mut adapter,
            vec![
                ("sketch.drag".into(), json!({"entity":point,"from":at,"to":[at.x+2.0,at.y+3.0],"live":true})),
                ("sketch.drag_end".into(), json!({})),
            ],
        );
        let moved = adapter.sketch_snapshot().unwrap();
        assert!(moved.sketch.point(handle).unwrap().dist(at) > 1.0);
        if command == "sketch.circle.center" {
            numeric_apply(&mut adapter, &curve_selection(sketch, &c), Some("radius"), "8 mm");
        }
        adapter.execute("edit.undo", json!({})).unwrap();
        adapter.execute("edit.redo", json!({})).unwrap();
        assert!(adapter.sketch_snapshot().unwrap().solved);
    }
}

#[test]
fn sketch_plane_preview_uses_upstream_transform_and_never_adds_history() {
    let mut adapter = SessionAdapter::new();
    let plane = json!({"origin":[5,6,7],"x_dir":[0,1,0],"y_dir":[0,0,1]});
    let before = adapter.session.doc.clone();
    let undo = adapter.session.undo.len();
    let resolved = adapter.preview_sketch_plane(plane.clone()).unwrap();
    assert_eq!(resolved.origin, Vec3::new(5., 6., 7.));
    assert_eq!(resolved.x, Vec3::Y);
    assert_eq!(resolved.y, Vec3::Z);
    assert!(Arc::ptr_eq(&before, &adapter.session.doc));
    assert_eq!(undo, adapter.session.undo.len());
    adapter.execute("sketch.create", json!({"plane":plane})).unwrap();
    adapter.execute("sketch.line", json!({"points":[[0,0],[10,0]]})).unwrap();
    let sk = adapter.sketch_snapshot().unwrap();
    assert_eq!(sk.plane.to_world(solidcore_record::Vec2::new(10., 0.)), Vec3::new(5., 16., 7.));
    let view = adapter.workspace(&[], false, false);
    assert_eq!(view["sketch"]["curves"][0]["type"], "line");
}

#[test]
fn origin_references_are_automatic_during_sketch_creation_without_changing_saved_visibility() {
    let mut engine = SessionAdapter::new();
    let before = engine.session.doc.clone();
    let mut view = Viewport::default();
    view.command("view.origin_style", &json!({"legacy":true})).unwrap();
    view.command("view.grid", &json!({"visible":true,"integrated_axes":false})).unwrap();
    view.set_metrics(metrics(800., 600., 2.)).unwrap();
    view.begin_update(1);
    view.install(1, engine.snapshot().unwrap());
    assert!(view.snapshot().unwrap().guides.meshes.is_empty());
    assert_eq!(view.diagnostic()["origin"]["visible"], json!(vec![false; 7]));

    view.command("view.choose_sketch_plane", &json!({})).unwrap();
    assert_eq!(view.diagnostic()["reference_labels"].as_array().unwrap().len(), 7);
    assert_eq!(view.snapshot().unwrap().guides.meshes.len(), 4);
    view.command("select.entity", &json!({"entity":{"type":"plane","name":"XY"}})).unwrap();
    view.command("view.cancel_plane", &json!({})).unwrap();
    assert!(view.selection().is_empty(), "an automatically hidden plane must not remain selected after cancellation");
    assert!(view.snapshot().unwrap().guides.meshes.is_empty());
    assert!(view.diagnostic()["reference_labels"].as_array().unwrap().is_empty());
    assert!(Arc::ptr_eq(&before, &engine.session.doc));
    assert!(engine.session.undo.is_empty());

    view.command("view.origin", &json!({"visible":false})).unwrap();
    view.command("view.choose_sketch_plane", &json!({"reveal_hidden":false})).unwrap();
    assert_eq!(view.diagnostic()["origin"]["visible"], json!(vec![false; 7]), "the preference retains explicit Hide");
    assert!(view.snapshot().unwrap().guides.meshes.is_empty());
    view.command("view.cancel_plane", &json!({})).unwrap();
    view.command("view.choose_sketch_plane", &json!({})).unwrap();
    assert_eq!(view.diagnostic()["origin"]["visible"], json!([false, false, false, false, true, true, true]));
    assert_eq!(view.snapshot().unwrap().guides.meshes.len(), 3, "Hide all must not leave the plane picker empty");
    for (name, direction, world) in
        [("XY", "view.top", Vec3::new(10., 10., 0.)), ("XZ", "view.front", Vec3::new(10., 0., 10.)), ("YZ", "view.right", Vec3::new(0., 10., 10.))]
    {
        view.command(direction, &json!({})).unwrap();
        assert_eq!(view.pick_reference(point(&view, world)), Some(Sel::Plane { name: name.into() }), "revealed {name} must be pickable");
    }
    view.command("select.entity", &json!({"entity":{"type":"plane","name":"XZ"}})).unwrap();
    view.command("view.cancel_plane", &json!({})).unwrap();
    assert!(view.selection().is_empty());
    assert_eq!(view.diagnostic()["origin"]["visible"], json!(vec![false; 7]), "cancel restores Hide all");
    let hidden = view.diagnostic();
    for invalid in [Value::Null, json!("true"), json!(1)] {
        assert!(view.command("view.choose_sketch_plane", &json!({"reveal_hidden":invalid})).is_err());
        assert_eq!(view.diagnostic(), hidden, "invalid chooser settings must not change the camera or visibility");
    }
    view.command("view.origin", &json!({"visible":null})).unwrap();

    view.command("view.origin", &json!({"item":"X","visible":true})).unwrap();
    view.command("view.origin", &json!({"item":"XZ","visible":false})).unwrap();
    view.command("view.choose_sketch_plane", &json!({})).unwrap();
    assert_eq!(view.diagnostic()["origin"]["visible"], json!(vec![true; 7]), "individual Hide must not hide a sketch plane");
    let plane = engine.preview_sketch_plane(json!("XY")).unwrap();
    view.command("view.sketch_plane", &json!({"plane":plane})).unwrap();
    assert!(view.diagnostic()["reference_labels"].as_array().unwrap().is_empty());
    engine.execute("sketch.create", json!({"plane":"XY"})).unwrap();
    view.begin_update(2);
    view.install(2, engine.snapshot().unwrap());
    assert!(view.snapshot().unwrap().guides.meshes.is_empty());
    assert!(!view.snapshot().unwrap().guides.lines.is_empty(), "active sketch keeps its planar axes");
    engine.execute("sketch.finish", json!({})).unwrap();
    view.begin_update(3);
    view.install(3, engine.snapshot().unwrap());
    assert_eq!(view.diagnostic()["origin"]["visible"], json!([false, true, false, false, false, false, false]));
    assert_eq!(view.diagnostic()["reference_labels"].as_array().unwrap().len(), 1, "explicit Show survives sketch completion");
    view.command("view.origin", &json!({"item":"X","visible":null})).unwrap();
    assert!(view.diagnostic()["reference_labels"].as_array().unwrap().is_empty());
    view.command("view.origin", &json!({"visible":null})).unwrap();
    view.command("view.choose_sketch_plane", &json!({})).unwrap();
    assert_eq!(view.diagnostic()["reference_labels"].as_array().unwrap().len(), 7, "reset restores automatic visibility for every reference");
    view.command("view.cancel_plane", &json!({})).unwrap();
    assert!(view.snapshot().unwrap().guides.meshes.is_empty());
    view.command("view.origin", &json!({"visible":true})).unwrap();
    view.command("view.choose_sketch_plane", &json!({})).unwrap();
    view.command("view.cancel_plane", &json!({})).unwrap();
    assert_eq!(view.diagnostic()["reference_labels"].as_array().unwrap().len(), 7, "Show all preserves the old always-visible preference");
}

#[test]
fn origin_plane_display_is_compact_neutral_spaced_and_restores_original_style() {
    use solidcore_record::Plane;
    let mut engine = SessionAdapter::new();
    let document = engine.session.doc.clone();
    let mut view = Viewport::default();
    view.set_metrics(metrics(800., 600., 2.)).unwrap();
    view.begin_update(1);
    view.install(1, engine.snapshot().unwrap());
    view.command("view.choose_sketch_plane", &json!({})).unwrap();
    assert_eq!(view.diagnostic()["origin"]["legacy_planes"], false);
    let labels = view.diagnostic()["reference_labels"].as_array().unwrap().clone();
    assert_eq!(labels.len(), 4);
    assert!(labels.iter().all(|label| ["O", "X", "Y", "Z"].contains(&label["name"].as_str().unwrap())));
    let camera = view.camera;
    for legacy in [false, true, false] {
        view.command("view.origin_style", &json!({"legacy":legacy})).unwrap();
        let size = if legacy { 25.0 } else { 12.5 };
        for (name, plane, y_sign) in [("XY", Plane::XY, 1.), ("XZ", Plane::XZ, -1.), ("YZ", Plane::YZ, 1.)] {
            view.command("view.origin", &json!({"visible":false})).unwrap();
            view.command("view.origin", &json!({"item":name,"visible":true})).unwrap();
            let frame = view.snapshot().unwrap();
            assert_eq!(frame.guides.meshes.len(), 1);
            let positions = &frame.guides.meshes[0].mesh.positions;
            let offset = if legacy { 0.0 } else { size * 0.1 };
            let origin = plane.origin + (plane.x + plane.y * y_sign) * offset;
            assert_eq!(
                positions,
                &vec![origin, origin + plane.x * size, origin + plane.x * size + plane.y * size * y_sign, origin + plane.y * size * y_sign]
            );
            let palette = solidcraft_engine::view::Palette::DEFAULT;
            let color = if legacy {
                match name {
                    "XY" => palette.axes[2],
                    "XZ" => palette.axes[1],
                    _ => palette.axes[0],
                }
            } else {
                palette.grid_major
            };
            assert_eq!(frame.guides.meshes[0].color, color);
            let outline = frame.guides.lines.iter().find(|line| line.points.len() == 5 && line.points[..4] == positions[..]).unwrap();
            assert_eq!(outline.color, color);
            let inside = plane.to_world(solidcore_record::Vec2::new(10., 10. * y_sign));
            let outside = plane.to_world(solidcore_record::Vec2::new(20., 20. * y_sign));
            assert_eq!(view.pick_reference(point(&view, inside)), Some(Sel::Plane { name: name.into() }));
            assert_eq!(view.pick_reference(point(&view, outside)).is_some(), legacy);
            for local in [(0.5, 10.), (10., 0.5)] {
                let gap = plane.to_world(solidcore_record::Vec2::new(local.0, local.1 * y_sign));
                assert_eq!(view.pick_reference(point(&view, gap)).is_some(), legacy, "the axis gap must not select {name}");
            }
            assert_eq!(view.diagnostic()["reference_labels"].as_array().unwrap().len(), usize::from(legacy));
        }
        assert_eq!(view.camera, camera, "style preference must preserve the camera");
    }
    let before = view.diagnostic();
    for invalid in [json!({}), json!({"legacy":null}), json!({"legacy":"true"}), json!({"legacy":1})] {
        assert!(view.command("view.origin_style", &invalid).is_err());
        assert_eq!(view.diagnostic(), before);
    }
    assert!(Arc::ptr_eq(&document, &engine.session.doc));
    assert!(engine.session.undo.is_empty());
}

#[test]
fn origin_planes_pick_with_finger_pencil_and_pointer_and_create_native_sketches() {
    use solidcore_record::Plane;
    for (name, plane, world) in
        [("XY", Plane::XY, Vec3::new(10., 10., 0.)), ("XZ", Plane::XZ, Vec3::new(10., 0., 10.)), ("YZ", Plane::YZ, Vec3::new(0., 10., 10.))]
    {
        for source in [InputSource::Finger, InputSource::Pencil, InputSource::Indirect] {
            let mut engine = SessionAdapter::new();
            let before = engine.session.doc.clone();
            let undo = engine.session.undo.len();
            let mut view = Viewport::default();
            view.set_metrics(metrics(800., 600., 2.)).unwrap();
            view.begin_update(1);
            view.install(1, engine.snapshot().unwrap());
            view.command("view.origin", &json!({"visible":false})).unwrap();
            view.command("view.origin", &json!({"item":name,"visible":true})).unwrap();
            view.command("view.choose_sketch_plane", &json!({"reveal_hidden":false})).unwrap();
            assert_eq!(view.snapshot().unwrap().guides.meshes.len(), 1);
            let screen = point(&view, world);
            for phase in [InputPhase::Began, InputPhase::Ended] {
                let mut sample = event(source, phase, 1, screen, 1.);
                sample.geometry_revision = Some(engine.geometry_revision);
                view.input(sample).unwrap();
            }
            assert_eq!(view.selection(), &[Sel::Plane { name: name.into() }], "{name} {source:?}");
            let resolved = engine.preview_sketch_plane(json!(name)).unwrap();
            view.command("view.sketch_plane", &json!({"plane":resolved})).unwrap();
            assert!(view.diagnostic()["reference_labels"].as_array().unwrap().is_empty());
            assert!(Arc::ptr_eq(&before, &engine.session.doc));
            assert_eq!(engine.session.undo.len(), undo);
            assert_eq!(resolved, plane);
            engine.execute("sketch.create", json!({"plane":name})).unwrap();
            view.begin_update(2);
            view.install(2, engine.snapshot().unwrap());
            assert_eq!(view.model.as_ref().unwrap().sketch_plane, Some(plane));
            assert_eq!(engine.session.undo.len(), undo + 1);
            assert!(view.snapshot().unwrap().guides.meshes.is_empty());
        }
    }
}

#[test]
fn origin_visibility_controls_rendering_picking_and_retains_legacy_empty_view() {
    let mut engine = SessionAdapter::new();
    let before = engine.session.doc.clone();
    let mut view = Viewport::default();
    view.set_metrics(metrics(800., 600., 2.)).unwrap();
    view.begin_update(1);
    view.install(1, engine.snapshot().unwrap());
    view.command("view.choose_sketch_plane", &json!({})).unwrap();
    for (name, world) in [("XY", Vec3::new(10., 10., 0.)), ("XZ", Vec3::new(10., 0., 10.)), ("YZ", Vec3::new(0., 10., 10.))] {
        view.command("view.origin", &json!({"visible":false})).unwrap();
        view.command("view.origin", &json!({"item":name,"visible":true})).unwrap();
        assert_eq!(view.pick(point(&view, world), Some(engine.geometry_revision)), Some(Sel::Plane { name: name.into() }));
        view.command("view.origin", &json!({"item":name,"visible":false})).unwrap();
        assert!(view.pick(point(&view, world), Some(engine.geometry_revision)).is_none());
        assert!(view.command("select.entity", &json!({"entity":{"type":"plane","name":name}})).is_err());
    }
    assert!(view.snapshot().unwrap().guides.meshes.is_empty());
    assert!(view.diagnostic()["reference_labels"].as_array().unwrap().is_empty());
    assert!(Arc::ptr_eq(&before, &engine.session.doc));
    assert!(engine.session.undo.is_empty());
    assert!(view.command("view.origin", &json!({"item":"unknown","visible":true})).is_err());
    assert!(view.command("view.origin", &json!({"visible":"true"})).is_err());
}

#[test]
fn origin_group_visibility_is_independent_atomic_and_preserves_automatic_choices() {
    let mut engine = SessionAdapter::new();
    let mut view = Viewport::default();
    view.set_metrics(metrics(800., 600., 2.)).unwrap();
    view.begin_update(1);
    view.install(1, engine.snapshot().unwrap());
    let before = view.snapshot().unwrap();
    view.command("view.origin", &json!({"item":"O","visible":true})).unwrap();
    view.command("view.origin", &json!({"item":"X","visible":true})).unwrap();
    view.command("view.origin", &json!({"item":"XZ","visible":true})).unwrap();
    view.command("view.origin", &json!({"group":"axes","visible":true})).unwrap();
    assert_eq!(view.diagnostic()["origin"]["visible"], json!([true, true, true, true, false, true, false]));
    view.command("view.origin", &json!({"group":"planes","visible":true})).unwrap();
    assert_eq!(view.snapshot().unwrap().guides.meshes.len(), 4);
    view.command("select.entity", &json!({"entity":{"type":"plane","name":"XY"}})).unwrap();
    view.command("view.origin", &json!({"group":"planes","visible":false})).unwrap();
    assert!(view.selection().is_empty(), "group hide clears a hidden reference selection");
    assert_eq!(view.snapshot().unwrap().guides.meshes.len(), 1, "the origin dot remains visible");
    assert_eq!(view.diagnostic()["origin"]["visible"], json!([true, true, true, true, false, false, false]));
    assert!(view.pick(point(&view, Vec3::new(15., 15., 0.)), Some(engine.geometry_revision)).is_none());
    view.command("view.origin", &json!({"group":"axes","visible":false})).unwrap();
    assert_eq!(view.diagnostic()["origin"]["visible"], json!([true, false, false, false, false, false, false]));
    view.command("view.origin", &json!({"group":"planes","visible":null})).unwrap();
    view.command("view.choose_sketch_plane", &json!({})).unwrap();
    assert_eq!(view.diagnostic()["origin"]["visible"], json!([true, false, false, false, true, true, true]));
    view.command("view.cancel_plane", &json!({})).unwrap();
    assert_eq!(view.diagnostic()["origin"]["visible"], json!([true, false, false, false, false, false, false]));

    let diagnostic = view.diagnostic();
    for params in [
        json!({"group":"unknown","visible":true}),
        json!({"group":null,"visible":true}),
        json!({"group":["axes"],"visible":true}),
        json!({"group":"axes","item":"XY","visible":true}),
        json!({"group":"planes","visible":"true"}),
    ] {
        assert!(view.command("view.origin", &params).is_err());
        assert_eq!(view.diagnostic()["origin"], diagnostic["origin"], "invalid group requests must apply nothing");
    }
    let after = view.snapshot().unwrap();
    assert!(Arc::ptr_eq(&before.model, &after.model));
    assert_eq!(before.camera, after.camera);
    assert_eq!(before.geometry_revision, after.geometry_revision);
    assert!(engine.session.undo.is_empty());
}

#[test]
fn origin_axes_and_point_have_existing_engine_identity_and_individual_visibility() {
    let mut engine = SessionAdapter::new();
    let mut view = Viewport::default();
    view.set_metrics(metrics(800., 600., 2.)).unwrap();
    view.begin_update(1);
    view.install(1, engine.snapshot().unwrap());
    view.command("view.origin", &json!({"visible":false})).unwrap();
    for (name, world, entity) in [
        ("O", Vec3::ZERO, Sel::Vertex { body: String::new(), point: Vec3::ZERO }),
        ("X", Vec3::new(20., 0., 0.), Sel::Axis { name: "X".into() }),
        ("Y", Vec3::new(0., 20., 0.), Sel::Axis { name: "Y".into() }),
        ("Z", Vec3::new(0., 0., 20.), Sel::Axis { name: "Z".into() }),
    ] {
        view.command("view.origin", &json!({"item":name,"visible":true})).unwrap();
        let screen = point(&view, world);
        assert_eq!(view.pick(screen, Some(engine.geometry_revision)), Some(entity.clone()));
        for phase in [InputPhase::Began, InputPhase::Ended] {
            let mut sample = event(InputSource::Finger, phase, 1, screen, 1.);
            sample.geometry_revision = Some(engine.geometry_revision);
            view.input(sample).unwrap();
        }
        assert_eq!(view.selection(), &[entity]);
        view.command("view.origin", &json!({"item":name,"visible":false})).unwrap();
        assert!(view.pick(screen, Some(engine.geometry_revision)).is_none());
        assert!(view.selection().is_empty());
    }
}

#[test]
fn plane_chooser_cancellation_restores_camera_and_rejects_stale_or_dragged_taps() {
    let mut engine = SessionAdapter::new();
    let mut view = Viewport::default();
    view.set_metrics(metrics(800., 600., 2.)).unwrap();
    view.begin_update(1);
    view.install(1, engine.snapshot().unwrap());
    view.command("view.origin", &json!({"visible":true})).unwrap();
    view.command("view.front", &json!({})).unwrap();
    let camera = view.camera;
    view.command("view.choose_sketch_plane", &json!({})).unwrap();
    let p = point(&view, Vec3::new(10., 10., 0.));
    assert!(view.pick(p, Some(engine.geometry_revision + 1)).is_none());
    for phase in [InputPhase::Began, InputPhase::Ended] {
        let mut sample = event(InputSource::Pencil, phase, 1, p, 1.);
        sample.geometry_revision = Some(engine.geometry_revision + 1);
        view.input(sample).unwrap();
    }
    assert!(view.selection().is_empty());
    for (phase, dx, time) in [(InputPhase::Began, 0., 2.), (InputPhase::Moved, 50., 3.), (InputPhase::Ended, 50., 4.)] {
        let mut sample = event(InputSource::Finger, phase, 2, [p[0] + dx, p[1]], time);
        sample.geometry_revision = Some(engine.geometry_revision);
        view.input(sample).unwrap();
    }
    assert!(view.selection().is_empty());
    // Two stationary fingers are a navigation/history gesture, never two taps.
    for (phase, id) in [(InputPhase::Began, 3), (InputPhase::Began, 4), (InputPhase::Ended, 3), (InputPhase::Ended, 4)] {
        let mut sample = event(InputSource::Finger, phase, id, p, 5.);
        sample.geometry_revision = Some(engine.geometry_revision);
        view.input(sample).unwrap();
    }
    assert!(view.selection().is_empty());
    view.command("view.cancel_plane", &json!({})).unwrap();
    assert_eq!(view.camera, camera);
    assert_eq!(view.diagnostic()["origin"]["choosing"], false);
    assert!(engine.session.undo.is_empty());
}

#[test]
fn origin_chooser_frames_plane_axis_and_point_targets_in_portrait_and_landscape() {
    for (width, height) in [(800., 600.), (390., 760.), (300., 900.)] {
        let mut engine = SessionAdapter::new();
        let mut view = Viewport::default();
        view.command("view.origin_style", &json!({"legacy":true})).unwrap();
        view.set_metrics(metrics(width, height, 2.)).unwrap();
        view.begin_update(1);
        view.install(1, engine.snapshot().unwrap());
        view.command("view.origin", &json!({"visible":true})).unwrap();
        view.command("view.choose_sketch_plane", &json!({})).unwrap();
        let labels = view.diagnostic()["reference_labels"].as_array().unwrap().clone();
        assert_eq!(labels.len(), 7);
        for label in labels {
            let x = label["position"][0].as_f64().unwrap();
            let y = label["position"][1].as_f64().unwrap();
            assert!((22. ..width - 22.).contains(&x) && (22. ..height - 22.).contains(&y), "{width}×{height}: {label}");
        }
    }
}

#[test]
fn origin_references_respect_explicit_topology_filters_and_native_construction_visibility() {
    let (mut engine, mut view) = proof();
    view.command("view.top", &json!({})).unwrap();
    for (filter, world) in [("face", Vec3::new(20., 15., 10.)), ("edge", Vec3::new(20., 0., 10.)), ("vertex", Vec3::new(0., 0., 10.))] {
        view.command("select.filter", &json!({"filter":filter})).unwrap();
        view.command("view.origin", &json!({"visible":false})).unwrap();
        let p = point(&view, world);
        let expected = view.pick(p, Some(engine.geometry_revision));
        assert!(expected.is_some());
        view.command("view.origin", &json!({"visible":true})).unwrap();
        assert_eq!(view.pick(p, Some(engine.geometry_revision)), expected, "{filter}");
    }
    engine.execute("construct.plane.at_angle", json!({"base":"XY","axis":"X","angle":"35 deg","name":"Tilted"})).unwrap();
    view.begin_update(2);
    view.install(2, engine.snapshot().unwrap());
    view.command("view.origin", &json!({"visible":false})).unwrap();
    view.command("view.choose_sketch_plane", &json!({"reveal_hidden":false})).unwrap();
    let plane = engine.preview_sketch_plane(json!("Tilted")).unwrap();
    let world = plane.to_world(solidcore_record::Vec2::new(15., 20.));
    let p = point(&view, world);
    for legacy in [false, true, false] {
        view.command("view.origin_style", &json!({"legacy":legacy})).unwrap();
        assert_eq!(view.pick(p, Some(engine.geometry_revision)), Some(Sel::Plane { name: "Tilted".into() }));
        let frame = view.snapshot().unwrap();
        assert_eq!(frame.guides.meshes.len(), 1);
        let patch = &frame.guides.meshes[0];
        assert_eq!(patch.color, solidcraft_engine::view::Palette::DEFAULT.sketch_construction);
        let far_corner = plane.to_local(patch.mesh.positions[2]);
        assert!(far_corner.x >= 25.0 && far_corner.y >= 25.0, "construction plane remains full size in either origin style");
    }
    engine.execute("browser.visibility", json!({"items":["Tilted"],"visible":false})).unwrap();
    view.begin_update(3);
    view.install(3, engine.snapshot().unwrap());
    assert!(view.snapshot().unwrap().guides.meshes.is_empty());
    assert!(view.command("select.entity", &json!({"entity":{"type":"plane","name":"Tilted"}})).is_err());
}

fn sketch_input_fixture(plane: Value, tool: &str) -> (SessionAdapter, Viewport) {
    let mut engine = SessionAdapter::new();
    engine.execute("sketch.create", json!({"plane":plane})).unwrap();
    let mut view = Viewport::default();
    view.set_metrics(metrics(800.0, 600.0, 2.0)).unwrap();
    view.begin_update(1);
    view.install(1, engine.snapshot().unwrap());
    view.command("tool.set", &json!({"tool":tool})).unwrap();
    (engine, view)
}

#[test]
fn constraint_first_taps_apply_repeatedly_without_dragging_and_are_undoable() {
    use solidcore_record::Vec2;
    for source in [InputSource::Pencil, InputSource::Finger, InputSource::Indirect] {
        let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "select");
        for (id, points) in [("first", [[-30., -20.], [30., -10.]]), ("second", [[-30., 15.], [30., 25.]])] {
            engine.execute("sketch.line", json!({"points":points,"ids":[id],"infer":false,"connect":false})).unwrap();
        }
        view.begin_update(2);
        view.install(2, engine.snapshot().unwrap());
        view.command("tool.constraint", &json!({"constraint":"horizontal"})).unwrap();
        let before = engine.sketch_snapshot().unwrap().sketch;
        let history = engine.session.undo.len();
        for (n, (id, xy)) in [("first", [0., -15.]), ("second", [0., 20.])].into_iter().enumerate() {
            let at = point(&view, Vec3::new(xy[0], xy[1], 0.));
            feed(&mut view, source, InputPhase::Began, n as u64 + 1, at, n as f64 + 1.);
            assert!(view.take_tool_work().is_none());
            feed(&mut view, source, InputPhase::Ended, n as u64 + 1, at, n as f64 + 1.1);
            assert!(run_tool(&mut engine, &mut view), "{source:?} must apply on release");
            let sk = engine.sketch_snapshot().unwrap().sketch;
            let a = sk.point(sk.resolve_point(&format!("{id}.start")).unwrap()).unwrap();
            let b = sk.point(sk.resolve_point(&format!("{id}.end")).unwrap()).unwrap();
            assert!((a.y - b.y).abs() < 1e-6);
            assert!(a.dist(b) > 50., "an edge remains an edge");
            assert_eq!(view.constraint_tool(), Some("horizontal"));
            assert!(view.selection().is_empty());
        }
        assert_eq!(engine.session.undo.len(), history + 2);
        let after = engine.sketch_snapshot().unwrap().sketch;
        assert_eq!(after.curves.len(), before.curves.len());
        engine.execute("edit.undo", json!({})).unwrap();
        engine.execute("edit.undo", json!({})).unwrap();
        assert_eq!(engine.sketch_snapshot().unwrap().sketch, before);
        engine.execute("edit.redo", json!({})).unwrap();
        engine.execute("edit.redo", json!({})).unwrap();
        assert_eq!(engine.sketch_snapshot().unwrap().sketch, after);
        assert!(after.point(after.resolve_point("first.start").unwrap()).unwrap().dist(Vec2::new(-30., -20.)) < 20.);
        view.command("tool.cancel", &json!({})).unwrap();
        assert_eq!(view.diagnostic()["input"]["tool"], "select");
    }
}

#[test]
fn constraint_first_pair_accumulates_without_multiple_selection_and_rejects_bad_picks() {
    use solidcore_record::Vec2;
    for source in [InputSource::Pencil, InputSource::Finger, InputSource::Indirect] {
        let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "select");
        engine.execute("sketch.line", json!({"points":[[-30,-20],[30,-10]],"ids":["first"],"infer":false,"connect":false})).unwrap();
        engine.execute("sketch.line", json!({"points":[[-30,15],[30,35]],"ids":["second"],"infer":false,"connect":false})).unwrap();
        engine.execute("sketch.circle.center", json!({"center":[60,0],"radius":8,"id":"round","infer":false,"connect":false})).unwrap();
        view.begin_update(2);
        view.install(2, engine.snapshot().unwrap());
        view.command("select.multiple", &json!({"enabled":false})).unwrap();
        view.command("tool.constraint", &json!({"constraint":"parallel"})).unwrap();
        let before = engine.sketch_snapshot().unwrap().sketch;
        let history = engine.session.undo.len();
        let tap = |view: &mut Viewport, n: u64, xy: [f64; 2]| {
            let at = point(view, Vec3::new(xy[0], xy[1], 0.));
            feed(view, source, InputPhase::Began, n, at, n as f64);
            feed(view, source, InputPhase::Ended, n, at, n as f64 + 0.1);
        };
        tap(&mut view, 1, [0., -15.]);
        assert_eq!(view.selection().len(), 1);
        assert!(view.take_tool_work().is_none());
        view.command("select.entity", &json!({"entity":{"type":"sketch_curve","id":"round"}})).unwrap();
        assert_eq!(view.selection().len(), 1);
        assert_eq!(engine.sketch_snapshot().unwrap().sketch, before);
        tap(&mut view, 2, [0., -15.]);
        assert!(view.selection().is_empty(), "tapping the first pick again deselects it");
        tap(&mut view, 3, [0., -15.]);
        tap(&mut view, 4, [0., 25.]);
        assert!(run_tool(&mut engine, &mut view));
        let sk = engine.sketch_snapshot().unwrap().sketch;
        let direction = |id: &str| -> Vec2 {
            (sk.point(sk.resolve_point(&format!("{id}.end")).unwrap()).unwrap()
                - sk.point(sk.resolve_point(&format!("{id}.start")).unwrap()).unwrap())
            .normalized()
            .unwrap()
        };
        assert!(direction("first").dot(direction("second")).abs() > 1. - 1e-8);
        assert_eq!(engine.session.undo.len(), history + 1);
        assert!(view.selection().is_empty());
        assert_eq!(view.constraint_tool(), Some("parallel"));
    }
}

#[test]
fn constraint_first_empty_taps_return_to_select_and_respect_dismissal_preference() {
    for source in [InputSource::Pencil, InputSource::Finger, InputSource::Indirect] {
        let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "select");
        engine.execute("sketch.line", json!({"points":[[-30,-20],[30,-10]],"ids":["first"],"infer":false,"connect":false})).unwrap();
        view.begin_update(2);
        view.install(2, engine.snapshot().unwrap());
        let before = engine.sketch_snapshot().unwrap().sketch;
        let history = engine.session.undo.len();
        let empty = [760., 560.];
        for cancel in [true, false] {
            view.command("view.tool_dismissal", &json!({"cancel_on_dismiss":cancel})).unwrap();
            for key in ["horizontal", "vertical", "parallel", "perpendicular", "coincident", "tangent", "equal", "concentric", "midpoint", "fix"] {
                view.command("tool.constraint", &json!({"constraint":key})).unwrap();
                assert!(view.pick_sketch_unfiltered(empty).is_none());
                feed(&mut view, source, InputPhase::Began, 1, empty, 1.);
                assert_eq!(view.constraint_tool(), Some(key), "touch-down does not dismiss");
                feed(&mut view, source, InputPhase::Ended, 1, empty, 1.1);
                assert_eq!(view.constraint_tool(), if cancel { None } else { Some(key) }, "{source:?}: {key}");
                assert!(view.selection().is_empty());
                assert!(view.take_tool_work().is_none());
                if cancel {
                    assert_eq!(view.diagnostic()["input"]["tool"], "select");
                    view.command("tool.previous", &json!({})).unwrap();
                    assert_eq!(view.constraint_tool(), Some(key), "the previous tool can be restored");
                }
            }
            view.command("tool.constraint", &json!({"constraint":"parallel"})).unwrap();
            let at = point(&view, Vec3::new(0., -15., 0.));
            feed(&mut view, source, InputPhase::Began, 2, at, 2.);
            feed(&mut view, source, InputPhase::Ended, 2, at, 2.1);
            assert_eq!(view.selection().len(), 1);
            feed(&mut view, source, InputPhase::Began, 3, empty, 3.);
            feed(&mut view, source, InputPhase::Ended, 3, empty, 3.1);
            assert!(view.selection().is_empty(), "empty tap clears the partial pair");
            assert_eq!(view.constraint_tool(), if cancel { None } else { Some("parallel") });
            assert!(view.take_tool_work().is_none());
        }
        assert_eq!(engine.sketch_snapshot().unwrap().sketch, before);
        assert_eq!(engine.session.undo.len(), history);
    }
}

#[test]
fn constraint_first_occupied_stale_cancelled_and_dragged_taps_do_not_dismiss() {
    for source in [InputSource::Pencil, InputSource::Finger, InputSource::Indirect] {
        let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "select");
        engine.execute("sketch.line", json!({"points":[[-30,-20],[30,-10]],"ids":["first"],"infer":false,"connect":false})).unwrap();
        engine.execute("sketch.circle.center", json!({"center":[35,0],"radius":8,"id":"round","infer":false,"connect":false})).unwrap();
        view.begin_update(2);
        view.install(2, engine.snapshot().unwrap());
        view.command("tool.constraint", &json!({"constraint":"parallel"})).unwrap();
        view.command("select.entity", &json!({"entity":{"type":"sketch_curve","id":"first"}})).unwrap();
        let selection = view.selection().to_vec();
        let circle = point(&view, Vec3::new(43., 0., 0.));
        assert!(view.pick_sketch(circle).is_none(), "circle is ineligible for Parallel");
        assert!(view.pick_sketch_unfiltered(circle).is_some());
        feed(&mut view, source, InputPhase::Began, 1, circle, 1.);
        feed(&mut view, source, InputPhase::Ended, 1, circle, 1.1);
        assert_eq!(view.constraint_tool(), Some("parallel"));
        assert_eq!(view.selection(), selection);

        let empty = [760., 560.];
        for mode in ["cancelled", "consumed", "stale", "dragged", "secondary"] {
            let mut begin = sample(&view, source, InputPhase::Began, 2, empty, 2.);
            begin.consumed = mode == "consumed";
            begin.button = u8::from(mode == "secondary");
            view.input(begin).unwrap();
            if mode == "dragged" {
                feed(&mut view, source, InputPhase::Moved, 2, [720., 520.], 2.1);
            }
            let phase = if mode == "cancelled" { InputPhase::Cancelled } else { InputPhase::Ended };
            let mut end = sample(&view, source, phase, 2, empty, 2.2);
            if mode == "stale" {
                end.geometry_revision = Some(engine.geometry_revision + 1);
            }
            view.input(end).unwrap();
            assert_eq!(view.constraint_tool(), Some("parallel"), "{source:?}: {mode}");
            assert_eq!(view.selection(), selection, "{source:?}: {mode}");
            assert!(view.take_tool_work().is_none());
        }
    }
}

#[test]
fn constraint_first_all_menu_choices_reach_the_existing_solver() {
    use solidcraft_engine::sketch::ConstraintKind;
    for key in ["horizontal", "vertical", "parallel", "perpendicular", "coincident", "tangent", "equal", "concentric", "midpoint", "fix"] {
        let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "select");
        engine.execute("sketch.line", json!({"points":[[-30,0],[30,5]],"ids":["first"],"infer":false,"connect":false})).unwrap();
        engine.execute("sketch.line", json!({"points":[[-20,25],[15,40]],"ids":["second"],"infer":false,"connect":false})).unwrap();
        engine.execute("sketch.circle.center", json!({"center":[0,18],"radius":10,"id":"round","connect":false})).unwrap();
        engine.execute("sketch.circle.center", json!({"center":[5,20],"radius":5,"id":"other_round","connect":false})).unwrap();
        view.begin_update(2);
        view.install(2, engine.snapshot().unwrap());
        view.command("tool.constraint", &json!({"constraint":key})).unwrap();
        let before = engine.sketch_snapshot().unwrap().sketch.constraints.len();
        let picks = match key {
            "horizontal" | "vertical" | "fix" => vec![("sketch_curve", "first")],
            "coincident" => vec![("sketch_point", "first.end"), ("sketch_point", "second.start")],
            "concentric" => vec![("sketch_curve", "round"), ("sketch_curve", "other_round")],
            "midpoint" => vec![("sketch_point", "second.start"), ("sketch_curve", "first")],
            "tangent" => vec![("sketch_curve", "first"), ("sketch_curve", "round")],
            _ => vec![("sketch_curve", "first"), ("sketch_curve", "second")],
        };
        for (kind, id) in picks {
            view.command("select.entity", &json!({"entity":{"type":kind,"id":id},"additive":false})).unwrap();
        }
        assert!(run_tool(&mut engine, &mut view), "{key}");
        let snapshot = engine.sketch_snapshot().unwrap();
        assert!(snapshot.solved, "{key}");
        if key == "fix" {
            let index = snapshot.sketch.curve_index("first").unwrap();
            assert!(snapshot.sketch.curves[index].fixed);
            view.command("select.entity", &json!({"entity":{"type":"sketch_curve","id":"first"}})).unwrap();
            assert!(run_tool(&mut engine, &mut view));
            assert!(engine.sketch_snapshot().unwrap().sketch.curves[index].fixed, "repeated Fix never unfixes geometry");
            continue;
        }
        assert!(snapshot.sketch.constraints.len() > before, "{key}");
        assert!(
            snapshot.sketch.constraints.iter().any(|c| match key {
                "horizontal" => matches!(c.kind, ConstraintKind::Horizontal { .. }),
                "vertical" => matches!(c.kind, ConstraintKind::Vertical { .. }),
                "parallel" => matches!(c.kind, ConstraintKind::Parallel { .. }),
                "perpendicular" => matches!(c.kind, ConstraintKind::Perpendicular { .. }),
                "coincident" => matches!(c.kind, ConstraintKind::Coincident { .. }),
                "tangent" => matches!(c.kind, ConstraintKind::Tangent { .. }),
                "equal" => matches!(c.kind, ConstraintKind::Equal { .. }),
                "concentric" => matches!(c.kind, ConstraintKind::Concentric { .. }),
                "midpoint" => matches!(c.kind, ConstraintKind::Midpoint { .. }),
                "fix" => matches!(c.kind, ConstraintKind::Fix { .. }),
                _ => false,
            }),
            "{key} must create its requested relation"
        );
    }
}

#[test]
fn constraint_first_preselection_cancellation_redundancy_and_lifecycle() {
    let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "select");
    engine.execute("sketch.line", json!({"points":[[-30,-10],[30,10]],"ids":["line"],"infer":false,"connect":false})).unwrap();
    view.begin_update(2);
    view.install(2, engine.snapshot().unwrap());
    assert!(view.command("tool.constraint", &json!({"constraint":"unknown"})).is_err());
    view.command("select.entity", &json!({"entity":{"type":"sketch_curve","id":"line"}})).unwrap();
    view.command("tool.constraint", &json!({"constraint":"vertical"})).unwrap();
    let work = view.take_tool_work().unwrap();
    view.command("tool.set", &json!({"tool":"select"})).unwrap();
    assert!(!view.tool_is_current(&work), "switching tools invalidates queued work");
    view.command("tool.constraint", &json!({"constraint":"vertical"})).unwrap();
    assert!(run_tool(&mut engine, &mut view), "preselected geometry still applies immediately");
    let before = engine.sketch_snapshot().unwrap().sketch;
    let history = engine.session.undo.len();
    view.command("select.entity", &json!({"entity":{"type":"sketch_curve","id":"line"}})).unwrap();
    let work = view.take_tool_work().unwrap();
    assert!(engine.prepare_tool(work.clone()).is_err(), "a duplicate is refused by the engine");
    view.fail_tool(&work);
    assert_eq!(engine.sketch_snapshot().unwrap().sketch, before);
    assert_eq!(engine.session.undo.len(), history);
    view.command("tool.cancel", &json!({})).unwrap();
    view.command("tool.constraint", &json!({"constraint":"parallel"})).unwrap();
    view.command("select.entity", &json!({"entity":{"type":"sketch_curve","id":"line"}})).unwrap();
    assert!(view.take_tool_work().is_none());
    view.command("input.cancel", &json!({})).unwrap();
    assert_eq!(view.constraint_tool(), Some("parallel"), "input cancellation preserves the chosen tool");
    engine.execute("sketch.finish", json!({})).unwrap();
    view.begin_update(3);
    view.install(3, engine.snapshot().unwrap());
    assert!(view.constraint_tool().is_none());
    assert!(view.command("tool.constraint", &json!({"constraint":"horizontal"})).is_err());
}

#[test]
fn sketch_select_drags_free_points_and_edges_with_one_undo_step() {
    use solidcore_record::Vec2;

    for source in [InputSource::Pencil, InputSource::Finger, InputSource::Indirect] {
        for (grab, delta) in [(Vec2::new(40., 20.), Vec2::new(10., 15.)), (Vec2::new(20., 20.), Vec2::new(0., 15.))] {
            let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "select");
            engine.execute("sketch.line", json!({"points":[[0,20],[40,20]],"ids":["free"],"infer":false,"connect":false})).unwrap();
            view.begin_update(2);
            view.install(2, engine.snapshot().unwrap());
            let original = engine.sketch_snapshot().unwrap().sketch;
            let history = engine.session.undo.len();
            let camera = view.camera;
            let start = point(&view, Vec3::new(grab.x, grab.y, 0.));
            let end = point(&view, Vec3::new(grab.x + delta.x, grab.y + delta.y, 0.));
            feed(&mut view, source, InputPhase::Began, 1, start, 1.);
            assert!(view.take_tool_work().is_none());
            feed(&mut view, source, InputPhase::Moved, 1, end, 1.1);
            let work = view.take_tool_work().expect("Select must preview a geometry drag");
            assert!(!work.commit);
            let prepared = engine.prepare_tool(work.clone()).unwrap();
            assert!(view.publish_preview(&work, prepared.model));
            assert_eq!(engine.sketch_snapshot().unwrap().sketch, original);
            assert_eq!(engine.session.undo.len(), history);
            feed(&mut view, source, InputPhase::Ended, 1, end, 1.2);
            assert!(run_tool(&mut engine, &mut view));
            let moved = engine.sketch_snapshot().unwrap().sketch;
            let endpoint = moved.point(moved.resolve_point("free.end").unwrap()).unwrap();
            assert!(endpoint.dist(Vec2::new(40., 20.) + delta) < 1e-6, "{source:?}: {endpoint:?}");
            assert_eq!(view.camera, camera);
            assert_eq!(view.diagnostic()["input"]["tool"], "select");
            assert_eq!(engine.session.undo.len(), history + 1);
            engine.execute("edit.undo", json!({})).unwrap();
            assert_eq!(engine.sketch_snapshot().unwrap().sketch, original);
            engine.execute("edit.redo", json!({})).unwrap();
            assert_eq!(engine.sketch_snapshot().unwrap().sketch, moved);
        }
    }
}

#[test]
fn sketch_select_drag_moves_the_selected_group_and_single_selection_excludes_other_geometry() {
    for multiple in [false, true] {
        let (mut engine, mut view) = sketch_input_fixture(json!("YZ"), "select");
        engine.execute("sketch.line", json!({"points":[[0,20],[40,20]],"ids":["first"],"infer":false})).unwrap();
        engine.execute("sketch.line", json!({"points":[[0,-20],[40,-20]],"ids":["second"],"infer":false})).unwrap();
        view.begin_update(2);
        view.install(2, engine.snapshot().unwrap());
        view.command("select.multiple", &json!({"enabled":multiple})).unwrap();
        view.command("select.entity", &json!({"entity":{"type":"sketch_curve","id":"second"}})).unwrap();
        let snapshot = engine.snapshot().unwrap();
        let plane = snapshot.sketch.as_ref().unwrap().plane;
        let screen = |xy| point(&view, plane.to_world(solidcore_record::Vec2::from(xy)));
        let start = screen([20., 20.]);
        let end = screen([20., 30.]);
        feed(&mut view, InputSource::Pencil, InputPhase::Began, 1, start, 1.);
        feed(&mut view, InputSource::Pencil, InputPhase::Ended, 1, end, 1.1);
        assert!(run_tool(&mut engine, &mut view));
        let sketch = engine.sketch_snapshot().unwrap().sketch;
        for (id, y) in [("first.start", 30.), ("first.end", 30.), ("second.start", if multiple { -10. } else { -20. })] {
            assert!((sketch.point(sketch.resolve_point(id).unwrap()).unwrap().y - y).abs() < 1e-6, "{id}, Multiple: {multiple}");
        }
    }
}

#[test]
fn sketch_select_drag_respects_partial_and_fixed_constraints() {
    use solidcore_record::Vec2;

    for locked in [false, true] {
        let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "select");
        engine.execute("sketch.line", json!({"points":[[0,20],[40,20]],"ids":["line"],"infer":false,"connect":false})).unwrap();
        engine.execute("sketch.constraint.horizontal_vertical", json!({"line":"line","mode":"horizontal"})).unwrap();
        engine.execute("sketch.constraint.fix", json!({"entity":if locked {"line"} else {"line.start"},"fixed":true})).unwrap();
        view.begin_update(2);
        view.install(2, engine.snapshot().unwrap());
        let original = engine.sketch_snapshot().unwrap().sketch;
        let history = engine.session.undo.len();
        let start = point(&view, Vec3::new(40., 20., 0.));
        let end = point(&view, Vec3::new(50., 35., 0.));
        feed(&mut view, InputSource::Pencil, InputPhase::Began, 1, start, 1.);
        feed(&mut view, InputSource::Pencil, InputPhase::Ended, 1, end, 1.1);
        let work = view.take_tool_work().expect("actual end coordinates must produce a drag");
        if locked {
            assert_eq!(engine.prepare_tool(work.clone()).err().unwrap().code, "fixed_geometry");
            view.fail_tool(&work);
            assert_eq!(engine.sketch_snapshot().unwrap().sketch, original);
            assert_eq!(engine.session.undo.len(), history);
        } else {
            let prepared = engine.prepare_tool(work.clone()).unwrap();
            assert!(view.complete_tool(&work, engine.accept_tool(prepared).unwrap()));
            let sketch = engine.sketch_snapshot().unwrap();
            assert!(sketch.solved);
            assert_eq!(sketch.sketch.constraints, original.constraints);
            for (id, expected) in [("line.start", Vec2::new(0., 20.)), ("line.end", Vec2::new(50., 20.))] {
                let actual = sketch.sketch.point(sketch.sketch.resolve_point(id).unwrap()).unwrap();
                assert!(actual.dist(expected) < 1e-6, "{id}: {actual:?}");
            }
            assert_eq!(engine.session.undo.len(), history + 1);
        }
    }
}

#[test]
fn sketch_select_geometry_taps_toggle_without_jitter_edits_and_legacy_navigation_remains_available() {
    for source in [InputSource::Pencil, InputSource::Finger, InputSource::Indirect] {
        let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "select");
        engine.execute("sketch.line", json!({"points":[[0,20],[40,20]],"ids":["line"],"infer":false})).unwrap();
        view.begin_update(2);
        view.install(2, engine.snapshot().unwrap());
        view.command("select.multiple", &json!({"enabled":true})).unwrap();
        let start = point(&view, Vec3::new(40., 20., 0.));
        for id in 1..=2 {
            feed(&mut view, source, InputPhase::Began, id, start, id as f64);
            let jitter = [start[0] + 2., start[1] + 2.];
            feed(&mut view, source, InputPhase::Moved, id, jitter, id as f64 + 0.1);
            feed(&mut view, source, InputPhase::Ended, id, jitter, id as f64 + 0.2);
            assert!(view.take_tool_work().is_none(), "tap jitter must not propose geometry edits");
            assert_eq!(view.selection().len(), if id == 1 { 1 } else { 0 }, "tap must toggle once");
        }
        view.command("view.tool_handles", &json!({"enabled":false})).unwrap();
        let camera = view.camera;
        feed(&mut view, source, InputPhase::Began, 3, start, 3.);
        feed(&mut view, source, InputPhase::Moved, 3, [start[0] + 40., start[1]], 3.1);
        feed(&mut view, source, InputPhase::Ended, 3, [start[0] + 40., start[1]], 3.2);
        assert!(view.take_tool_work().is_none(), "legacy Select does not move geometry");
        assert_eq!(view.camera == camera, source != InputSource::Finger);
    }
}

#[test]
fn sketch_select_geometry_cancel_and_two_finger_navigation_reject_late_previews() {
    for finish in ["cancel", "navigation", "return"] {
        let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "select");
        engine.execute("sketch.line", json!({"points":[[0,20],[40,20]],"ids":["line"],"infer":false})).unwrap();
        view.begin_update(2);
        view.install(2, engine.snapshot().unwrap());
        let original = engine.sketch_snapshot().unwrap().sketch;
        let history = engine.session.undo.len();
        let camera = view.camera;
        let start = point(&view, Vec3::new(40., 20., 0.));
        let end = [start[0] + 40., start[1] - 30.];
        feed(&mut view, InputSource::Finger, InputPhase::Began, 1, start, 1.);
        feed(&mut view, InputSource::Finger, InputPhase::Moved, 1, end, 1.1);
        let work = view.take_tool_work().unwrap();
        let prepared = engine.prepare_tool(work.clone()).unwrap();
        if finish == "cancel" {
            feed(&mut view, InputSource::Finger, InputPhase::Cancelled, 1, end, 1.2);
            assert_eq!(view.camera, camera);
        } else if finish == "return" {
            feed(&mut view, InputSource::Finger, InputPhase::Ended, 1, start, 1.2);
            assert_eq!(view.camera, camera);
        } else {
            feed(&mut view, InputSource::Finger, InputPhase::Began, 2, [100., 100.], 1.2);
            feed(&mut view, InputSource::Finger, InputPhase::Moved, 2, [160., 100.], 1.3);
            assert_ne!(view.camera, camera, "second finger must restore two-finger pan/zoom");
            feed(&mut view, InputSource::Finger, InputPhase::Ended, 2, [160., 100.], 1.4);
            feed(&mut view, InputSource::Finger, InputPhase::Ended, 1, end, 1.5);
        }
        assert!(!view.publish_preview(&work, prepared.model));
        assert!(view.take_tool_work().is_none());
        assert_eq!(engine.sketch_snapshot().unwrap().sketch, original);
        assert_eq!(engine.session.undo.len(), history);
    }
}

fn sketch_tap(view: &mut Viewport, id: u64, xy: [f64; 2]) {
    let plane = view.model.as_ref().unwrap().sketch.as_ref().unwrap().plane;
    let screen = point(view, plane.to_world(solidcore_record::Vec2::new(xy[0], xy[1])));
    feed(view, InputSource::Pencil, InputPhase::Began, id, screen, id as f64);
    feed(view, InputSource::Pencil, InputPhase::Ended, id, screen, id as f64 + 0.1);
}
#[test]
fn sketch_point_taps_repeat_render_pick_and_roundtrip_with_one_undo_each() {
    use solidcore_record::Vec2;
    for plane in ["XY", "XZ", "YZ"] {
        let (mut engine, mut view) = sketch_input_fixture(json!(plane), "sketch.point");
        assert!(engine.workspace(&[], false, false)["tools"].as_array().unwrap().iter().any(|t| t["id"] == "sketch.point" && t["enabled"] == true));
        assert_eq!(view.diagnostic()["input"]["sketch"]["prompt"], "point_place");
        let history = engine.session.undo.len();
        for (id, xy) in [(1, [20., 15.]), (2, [-25., 30.])] {
            sketch_tap(&mut view, id, xy);
            assert!(run_tool(&mut engine, &mut view), "one tap places one point");
            assert_eq!(engine.session.undo.len(), history + id as usize);
            assert_eq!(view.diagnostic()["input"]["tool"], "sketch.point");
            let snapshot = engine.sketch_snapshot().unwrap();
            let p = snapshot.sketch.points.iter().find(|p| p.pos.dist(Vec2::new(xy[0], xy[1])) < 1e-7).unwrap();
            assert!(snapshot.sketch.curves.is_empty());
            assert_eq!(snapshot.profiles, 0);
            assert!(matches!(view.pick_sketch(point(&view, snapshot.plane.to_world(p.pos))), Some(Sel::SketchPoint { id }) if id == p.id));
            let frame = view.snapshot().unwrap();
            let world = snapshot.plane.to_world(p.pos);
            assert!(frame.model.bounds.min.x <= world.x + 1e-7 && frame.model.bounds.max.x >= world.x - 1e-7);
            assert!(frame.model.bounds.min.y <= world.y + 1e-7 && frame.model.bounds.max.y >= world.y - 1e-7);
            assert!(frame.model.bounds.min.z <= world.z + 1e-7 && frame.model.bounds.max.z >= world.z - 1e-7);
            assert!(
                frame.highlight.lines.iter().any(|l| {
                    l.color == solidcraft_engine::view::Palette::DEFAULT.sketch
                        && (l.points[0] + l.points[1]).dist(snapshot.plane.to_world(p.pos) * 2.) < 1e-7
                }),
                "unselected standalone point remains visible"
            );
        }
        let original = engine.sketch_snapshot().unwrap().sketch;
        let mut loaded = modelling_roundtrip(&engine);
        loaded.execute("sketch.edit", json!({"sketch":engine.session.active_sketch.unwrap()})).unwrap();
        assert_eq!(loaded.sketch_snapshot().unwrap().sketch, original);
        engine.execute("edit.undo", json!({})).unwrap();
        assert_eq!(engine.sketch_snapshot().unwrap().sketch.points.len(), 2);
        engine.execute("edit.redo", json!({})).unwrap();
        assert_eq!(engine.sketch_snapshot().unwrap().sketch, original);
        view.begin_update(2);
        view.install(2, engine.snapshot().unwrap());
        view.command("tool.set", &json!({"tool":"sketch.delete"})).unwrap();
        sketch_tap(&mut view, 3, [20., 15.]);
        assert!(run_tool(&mut engine, &mut view));
        assert_eq!(engine.sketch_snapshot().unwrap().sketch.points.len(), 2);
    }
}

#[test]
fn sketch_point_drag_preview_cancel_and_final_lift_are_atomic() {
    let (mut engine, mut view) = sketch_input_fixture(json!("YZ"), "sketch.point");
    let original = engine.sketch_snapshot().unwrap().sketch;
    let history = engine.session.undo.len();
    let plane = engine.sketch_snapshot().unwrap().plane;
    for (phase, xy) in [(InputPhase::Began, [20., 15.]), (InputPhase::Moved, [35., 25.])] {
        let at = point(&view, plane.to_world(solidcore_record::Vec2::new(xy[0], xy[1])));
        feed(&mut view, InputSource::Pencil, phase, 1, at, 1.);
    }
    let work = view.take_tool_work().unwrap();
    let prepared = engine.prepare_tool(work.clone()).unwrap();
    assert!(!work.commit);
    assert!(view.publish_preview(&work, prepared.model));
    assert_eq!(engine.sketch_snapshot().unwrap().sketch, original);
    assert!(!view.snapshot().unwrap().highlight.lines.is_empty());
    view.command("tool.cancel", &json!({})).unwrap();
    assert!(!view.tool_is_current(&work));
    assert_eq!(engine.session.undo.len(), history);
    assert_eq!(engine.sketch_snapshot().unwrap().sketch, original);
    for (phase, xy) in [(InputPhase::Began, [20., 15.]), (InputPhase::Moved, [35., 25.]), (InputPhase::Ended, [35., 25.])] {
        let at = point(&view, plane.to_world(solidcore_record::Vec2::new(xy[0], xy[1])));
        feed(&mut view, InputSource::Pencil, phase, 2, at, 2.);
    }
    assert!(run_tool(&mut engine, &mut view));
    let sketch = engine.sketch_snapshot().unwrap().sketch;
    assert_eq!(sketch.points.len(), 2);
    assert!(sketch.points.iter().any(|p| p.pos.dist(solidcore_record::Vec2::new(35., 25.)) < 1e-7));
    assert_eq!(engine.session.undo.len(), history + 1);
}

#[test]
fn sketch_point_circle_coincident_works_in_both_pick_orders() {
    use solidcraft_engine::sketch::ConstraintKind;
    for reverse in [false, true] {
        let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "sketch.point");
        engine.execute("sketch.circle.center", json!({"center":[0,0],"radius":20,"id":"circle"})).unwrap();
        engine.execute("sketch.constraint.fix", json!({"entities":["circle"],"fixed":true})).unwrap();
        view.begin_update(2);
        view.install(2, engine.snapshot().unwrap());
        sketch_tap(&mut view, 1, [30., 15.]);
        assert!(run_tool(&mut engine, &mut view));
        let snapshot = engine.sketch_snapshot().unwrap();
        let id = snapshot.sketch.points.last().unwrap().id.clone();
        let before = snapshot.sketch.clone();
        let history = engine.session.undo.len();
        view.command("tool.constraint", &json!({"constraint":"coincident"})).unwrap();
        let picks = if reverse { [[0., 20.], [30., 15.]] } else { [[30., 15.], [0., 20.]] };
        for (i, xy) in picks.into_iter().enumerate() {
            sketch_tap(&mut view, i as u64 + 2, xy);
        }
        assert!(run_tool(&mut engine, &mut view));
        let snapshot = engine.sketch_snapshot().unwrap();
        let p = snapshot.sketch.resolve_point(&id).unwrap();
        let c = snapshot.sketch.curve_index("circle").unwrap();
        assert!(snapshot.solved);
        assert!((snapshot.sketch.point(p).unwrap().len() - 20.).abs() < 1e-6);
        assert!(
            snapshot
                .sketch
                .constraints
                .iter()
                .any(|constraint| matches!(constraint.kind, ConstraintKind::PointOnCurve { p: pi, c: ci } if pi == p && ci == c))
        );
        assert_eq!(engine.session.undo.len(), history + 1);
        engine.execute("edit.undo", json!({})).unwrap();
        assert_eq!(engine.sketch_snapshot().unwrap().sketch, before);
    }
}

#[test]
fn sketch_point_snaps_to_origin_endpoints_and_midpoints_without_reusing_them() {
    use solidcraft_engine::sketch::ConstraintKind;
    for (xy, midpoint) in [([0., 0.], false), ([20., 20.], false), ([40., 20.], true)] {
        let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "sketch.point");
        engine.execute("sketch.line", json!({"points":[[20,20],[60,20]],"ids":["line"],"infer":false,"connect":false})).unwrap();
        view.begin_update(2);
        view.install(2, engine.snapshot().unwrap());
        let before = engine.sketch_snapshot().unwrap().sketch.points.len();
        sketch_tap(&mut view, 1, xy);
        assert!(run_tool(&mut engine, &mut view));
        let sketch = engine.sketch_snapshot().unwrap().sketch;
        assert_eq!(sketch.points.len(), before + 1);
        assert!(sketch.constraints.iter().any(|c| if midpoint {
            matches!(c.kind, ConstraintKind::Midpoint { .. })
        } else {
            matches!(c.kind, ConstraintKind::Coincident { .. })
        }));
    }
}

#[test]
fn sketch_point_on_circle_can_pick_the_overlapping_edge_for_coincident() {
    let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "sketch.point");
    engine.execute("sketch.circle.center", json!({"center":[0,0],"radius":20,"id":"circle"})).unwrap();
    view.begin_update(2);
    view.install(2, engine.snapshot().unwrap());
    view.command("tool.auto_constraints", &json!({"enabled":false})).unwrap();
    sketch_tap(&mut view, 1, [0., 20.]);
    assert!(run_tool(&mut engine, &mut view));
    view.command("tool.constraint", &json!({"constraint":"coincident"})).unwrap();
    sketch_tap(&mut view, 2, [0., 20.]);
    assert!(matches!(view.selection(), [Sel::SketchPoint { .. }]));
    sketch_tap(&mut view, 3, [0., 20.]);
    assert!(run_tool(&mut engine, &mut view));
    assert!(engine.sketch_snapshot().unwrap().solved);
}

#[test]
fn sketch_point_circle_snap_creates_a_persistent_relation_only_when_enabled() {
    use solidcraft_engine::sketch::ConstraintKind;
    for automatic in [false, true] {
        let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "sketch.point");
        engine.execute("sketch.circle.center", json!({"center":[0,0],"radius":20,"id":"circle"})).unwrap();
        view.begin_update(2);
        view.install(2, engine.snapshot().unwrap());
        view.command("tool.auto_constraints", &json!({"enabled":automatic})).unwrap();
        sketch_tap(&mut view, 1, [0., 20.2]);
        assert_eq!(view.diagnostic()["input"]["sketch"]["snap"]["kind"], "on_curve");
        assert_eq!(view.diagnostic()["input"]["sketch"]["snap"]["persistent"], automatic);
        assert!(run_tool(&mut engine, &mut view));
        let snapshot = engine.sketch_snapshot().unwrap();
        assert!(snapshot.solved);
        assert_eq!(snapshot.sketch.constraints.iter().any(|c| matches!(c.kind, ConstraintKind::PointOnCurve { .. })), automatic);
        assert_eq!(snapshot.sketch.points.len(), 3);
        assert!((snapshot.sketch.points.last().unwrap().pos.len() - 20.).abs() < 1e-7);
    }
}

#[test]
fn sketch_pencil_rectangle_commits_once_and_cancel_never_removes_previous_geometry() {
    let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "sketch.rectangle.two_point");
    let undo = engine.session.undo.len();
    sketch_tap(&mut view, 1, [0.0, 0.0]);
    assert!(view.take_tool_work().is_none());
    sketch_tap(&mut view, 2, [40.0, 30.0]);
    assert!(run_tool(&mut engine, &mut view));
    assert_eq!(engine.session.undo.len(), undo + 1);
    assert_eq!(engine.sketch_snapshot().unwrap().profiles, 1);
    assert_eq!(engine.sketch_snapshot().unwrap().sketch.curves.len(), 4);
    assert_eq!(view.diagnostic()["input"]["tool"], "select", "a completed rectangle returns to selection");
    assert!(view.diagnostic()["numeric_token"].is_null(), "creation does not open a numeric editor");
    sketch_tap(&mut view, 3, [10.0, 10.0]);
    view.command("tool.cancel", &json!({})).unwrap();
    assert_eq!(engine.sketch_snapshot().unwrap().sketch.curves.len(), 4);
    assert_eq!(engine.session.undo.len(), undo + 1);
    view.command("tool.set", &json!({"tool":"sketch.line"})).unwrap();
    sketch_tap(&mut view, 4, [10.0, 10.0]);
    sketch_tap(&mut view, 5, [10.0, 10.0]);
    assert!(view.take_tool_work().is_none(), "tap on anchor must not create a zero-length line");
}
#[test]
fn sketch_construction_mode_creates_primitives_and_switches_back_without_converting_existing_curves() {
    for tool in ["sketch.line", "sketch.rectangle.two_point", "sketch.circle.center", "sketch.arc.three_point", "sketch.spline.fit_point"] {
        let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "select");
        let history = engine.session.undo.len();
        view.command("tool.construction", &json!({"enabled":true})).unwrap();
        view.command("tool.set", &json!({"tool":tool})).unwrap();
        assert_eq!(view.diagnostic()["input"]["sketch"]["construction"], true);
        sketch_tap(&mut view, 1, [10., 10.]);
        sketch_tap(&mut view, 2, [40., 30.]);
        if tool == "sketch.arc.three_point" || tool == "sketch.spline.fit_point" {
            sketch_tap(&mut view, 3, [10., 30.]);
        }
        if tool == "sketch.spline.fit_point" {
            view.command("tool.finish_chain", &json!({})).unwrap();
        }
        assert!(run_tool(&mut engine, &mut view), "{tool}");
        let construction = engine.sketch_snapshot().unwrap();
        assert!(!construction.sketch.curves.is_empty());
        assert!(construction.sketch.curves.iter().all(|curve| curve.construction), "{tool}");
        assert_eq!(construction.profiles, 0, "construction loops never become profiles");
        assert_eq!(engine.session.undo.len(), history + 1);
        view.command("tool.construction", &json!({"enabled":false})).unwrap();
        view.command("tool.set", &json!({"tool":"sketch.circle.center"})).unwrap();
        sketch_tap(&mut view, 4, [-30., -30.]);
        sketch_tap(&mut view, 5, [-15., -30.]);
        assert!(run_tool(&mut engine, &mut view));
        let mixed = engine.sketch_snapshot().unwrap();
        assert_eq!(mixed.sketch.curves.iter().filter(|curve| curve.construction).count(), construction.sketch.curves.len());
        assert!(!mixed.sketch.curves.last().unwrap().construction);
        assert_eq!(mixed.profiles, 1);
        engine.execute("edit.undo", json!({})).unwrap();
        assert_eq!(engine.sketch_snapshot().unwrap().sketch, construction.sketch);
        engine.execute("edit.redo", json!({})).unwrap();
        assert_eq!(engine.sketch_snapshot().unwrap().sketch, mixed.sketch);
    }
}

#[test]
fn sketch_construction_mode_invalidates_previews_and_resets_when_leaving_the_sketch() {
    let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "sketch.line");
    assert!(view.command("tool.construction", &json!({"enabled":"yes"})).is_err());
    sketch_tap(&mut view, 1, [10., 10.]);
    let start = point(&view, Vec3::new(40., 30., 0.));
    feed(&mut view, InputSource::Pencil, InputPhase::Began, 2, start, 2.);
    feed(&mut view, InputSource::Pencil, InputPhase::Moved, 2, [start[0] + 20., start[1]], 2.1);
    let work = view.take_tool_work().unwrap();
    let preview = engine.prepare_tool(work.clone()).unwrap();
    view.command("tool.construction", &json!({"enabled":true})).unwrap();
    assert!(!view.publish_preview(&work, preview.model));
    assert!(engine.sketch_snapshot().unwrap().sketch.curves.is_empty());
    assert_eq!(view.diagnostic()["input"]["sketch"]["construction"], true);
    engine.execute("sketch.finish", json!({})).unwrap();
    view.begin_update(2);
    view.install(2, engine.snapshot().unwrap());
    assert_eq!(view.diagnostic()["input"]["sketch"]["construction"], false);
    assert!(view.command("tool.construction", &json!({"enabled":true})).is_err());
}

#[test]
fn construction_dashes_preserve_colour_picking_and_screen_spacing_with_legacy_preference() {
    let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "select");
    engine.execute("sketch.line", json!({"points":[[-40,20],[40,20]],"construction":true,"ids":["guide"],"infer":false})).unwrap();
    engine.execute("sketch.circle.center", json!({"center":[0,-20],"radius":10,"construction":true})).unwrap();
    view.begin_update(2);
    view.install(2, engine.snapshot().unwrap());
    let original = view.snapshot().unwrap();
    let orange = solidcraft_engine::view::Palette::DEFAULT.sketch_construction;
    let count = |frame: &crate::RenderSnapshot| frame.highlight.lines.iter().filter(|line| line.color == orange).count();
    assert!(count(&original) > 10);
    assert!(original.model.scene.lines.iter().all(|line| !original.model_line_visible(line)));
    let unit = 2. * view.camera.half_height() / view.metrics.height_points;
    let first = original.highlight.lines.iter().find(|line| line.color == orange).unwrap();
    assert!(((first.points[1] - first.points[0]).len() / unit - 7.).abs() < 1e-6);
    // Picking still sees an unbroken curve, including the centre of a dash gap.
    let start = -40. + ((40. / (11. * unit)).floor() * 11. + 9.) * unit;
    assert!(matches!(view.pick_sketch(point(&view, Vec3::new(start, 20., 0.))), Some(Sel::SketchCurve { .. })));
    view.command("view.navigate", &json!({"motion":"zoom","factor":0.5})).unwrap();
    let zoomed = view.snapshot().unwrap();
    assert!(Arc::ptr_eq(&original.model, &zoomed.model), "zoom never republishes the CAD model");
    assert!(count(&zoomed) > count(&original));
    view.command("view.construction_dashes", &json!({"enabled":false})).unwrap();
    let legacy = view.snapshot().unwrap();
    assert!(legacy.model.scene.lines.iter().all(|line| legacy.model_line_visible(line)));
    assert_eq!(count(&legacy), 0);
    assert!(Arc::ptr_eq(&original.model, &legacy.model));
    view.command("view.construction_dashes", &json!({"enabled":true})).unwrap();
    engine.execute("sketch.finish", json!({})).unwrap();
    view.begin_update(3);
    view.install(3, engine.snapshot().unwrap());
    assert!(count(&view.snapshot().unwrap()) > 0, "finished construction sketches stay dashed");
}

#[test]
fn construction_dash_phase_crosses_tessellation_vertices_and_hostile_paths_are_bounded() {
    use solidcraft_render::{Scene, SceneLine};
    let mut line = SceneLine {
        points: (0..=30).map(|x| Vec3::new(f64::from(x), 0., 0.)).collect(),
        color: solidcraft_engine::view::Palette::DEFAULT.sketch_construction,
        alpha: 1.,
        width: 1.,
        on_top: true,
    };
    let mut scene = Scene::default();
    crate::construction::dash_line(&mut scene, &line, 1.);
    let length: f64 = scene.lines.iter().map(|line| (line.points[1] - line.points[0]).len()).sum();
    assert_eq!(length, 21., "three seven-unit dashes; tessellation never restarts the phase");
    assert!(scene.lines.iter().all(|line| {
        let midpoint = (line.points[0].x + line.points[1].x) / 2.;
        !(7. ..11.).contains(&midpoint) && !(18. ..22.).contains(&midpoint)
    }));
    line.points = vec![Vec3::ZERO, Vec3::new(1e100, 0., 0.)];
    scene.lines.clear();
    crate::construction::dash_line(&mut scene, &line, 1e-10);
    assert!(scene.lines.len() <= 4096);
    assert!(scene.lines.iter().flat_map(|line| &line.points).all(|p| p.is_finite()));
    line.points[1].x = f64::NAN;
    scene.lines.clear();
    crate::construction::dash_line(&mut scene, &line, 1.);
    assert!(scene.lines.is_empty());
}

#[test]
fn sketch_rectangle_repeat_preference_keeps_the_previous_creation_behavior() {
    for repeat in [false, true] {
        let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "sketch.rectangle.two_point");
        view.command("tool.repeat_rectangles", &json!({"enabled":repeat})).unwrap();
        let history = engine.session.undo.len();
        sketch_tap(&mut view, 1, [10., 10.]);
        sketch_tap(&mut view, 2, [50., 40.]);
        assert!(run_tool(&mut engine, &mut view));
        assert_eq!(view.diagnostic()["input"]["tool"], if repeat { "sketch.rectangle.two_point" } else { "select" });
        assert_eq!(engine.session.undo.len(), history + 1);
        assert!(view.selection().is_empty());
        assert!(view.diagnostic()["numeric_token"].is_null());
        if repeat {
            sketch_tap(&mut view, 3, [70., 10.]);
            sketch_tap(&mut view, 4, [90., 30.]);
            assert!(run_tool(&mut engine, &mut view));
            assert_eq!(engine.sketch_snapshot().unwrap().sketch.curves.len(), 8);
            assert_eq!(engine.session.undo.len(), history + 2);
        }
    }
}

#[test]
fn sketch_line_finish_anchor_tracks_committed_endpoint_and_never_changes_geometry() {
    for plane in [json!("XY"), json!("YZ")] {
        let (mut engine, mut view) = sketch_input_fixture(plane, "sketch.line");
        assert!(view.sketch_chain_end().is_none());
        sketch_tap(&mut view, 1, [0.0, 0.0]);
        assert!(view.sketch_chain_end().is_none(), "a first point is not a committed line");
        for (id, xy) in [(2, [20.0, 0.0]), (3, [20.0, 20.0])] {
            sketch_tap(&mut view, id, xy);
            assert!(view.sketch_chain_end().is_none(), "hide while accepting geometry");
            assert!(run_tool(&mut engine, &mut view));
            let plane = view.model.as_ref().unwrap().sketch.as_ref().unwrap().plane;
            let endpoint = plane.to_world(solidcore_record::Vec2::new(xy[0], xy[1]));
            let anchor = view.sketch_chain_end().unwrap();
            let expected = point(&view, endpoint);
            assert!((anchor[0] - expected[0]).abs() < 1e-6 && (anchor[1] - expected[1]).abs() < 1e-6);
            view.camera.target += Vec3::new(1.0, 1.0, 1.0);
            view.changed();
            let anchor = view.sketch_chain_end().unwrap();
            let expected = point(&view, endpoint);
            assert!((anchor[0] - expected[0]).abs() < 1e-6 && (anchor[1] - expected[1]).abs() < 1e-6, "anchor follows the camera");
        }
        let sketch = engine.sketch_snapshot().unwrap().sketch;
        let history = engine.session.undo.len();
        view.command("tool.finish_chain", &json!({})).unwrap();
        assert!(view.sketch_chain_end().is_none());
        assert_eq!(view.diagnostic()["input"]["sketch"]["finish_chain_at"], Value::Null);
        assert!(view.take_tool_work().is_none());
        assert_eq!(engine.sketch_snapshot().unwrap().sketch, sketch);
        assert_eq!(engine.session.undo.len(), history);
        sketch_tap(&mut view, 4, [-20.0, 0.0]);
        assert!(view.sketch_chain_end().is_none(), "new chain begins without connecting to the old one");
        sketch_tap(&mut view, 5, [-20.0, 10.0]);
        assert!(run_tool(&mut engine, &mut view));
        assert!(view.sketch_chain_end().is_some());
        let endpoint = view.sketch_chain_end().unwrap();
        feed(&mut view, InputSource::Pencil, InputPhase::Began, 6, endpoint, 6.0);
        assert!(view.sketch_chain_end().is_none(), "keep controls away from an active drawing contact");
        feed(&mut view, InputSource::Pencil, InputPhase::Cancelled, 6, endpoint, 6.1);
        assert!(view.sketch_chain_end().is_none());
    }
}

#[test]
fn sketch_pencil_chain_native_references_close_profile_and_segment_undo_is_independent() {
    let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "sketch.line");
    let undo = engine.session.undo.len();
    sketch_tap(&mut view, 1, [0.0, 0.0]);
    for (id, xy) in [(2, [20.0, 0.0]), (3, [20.0, 20.0]), (4, [0.0, 20.0]), (5, [0.0, 0.0])] {
        sketch_tap(&mut view, id, xy);
        assert!(run_tool(&mut engine, &mut view));
    }
    assert_eq!(engine.session.undo.len(), undo + 4);
    let sk = engine.sketch_snapshot().unwrap();
    assert_eq!(sk.profiles, 1);
    assert_eq!(sk.sketch.points.len(), 4, "four shared corners, including origin");
    view.command("tool.finish_chain", &json!({})).unwrap();
    assert!(view.diagnostic()["input"]["sketch"]["points"].as_array().unwrap().is_empty());
    engine.execute("edit.undo", json!({})).unwrap();
    assert_eq!(engine.sketch_snapshot().unwrap().sketch.curves.len(), 3);
    engine.execute("edit.redo", json!({})).unwrap();
    assert_eq!(engine.sketch_snapshot().unwrap().profiles, 1);
}
#[test]
fn sketch_pencil_curves_use_native_entities_and_stale_preview_is_rejected() {
    for tool in ["sketch.circle.center", "sketch.arc.three_point", "sketch.spline.fit_point"] {
        let (mut engine, mut view) = sketch_input_fixture(json!("XZ"), tool);
        sketch_tap(&mut view, 1, [0.0, 0.0]);
        sketch_tap(&mut view, 2, [20.0, 0.0]);
        if tool != "sketch.circle.center" {
            sketch_tap(&mut view, 3, [10.0, 10.0]);
        }
        if tool == "sketch.spline.fit_point" {
            view.command("tool.finish_chain", &json!({})).unwrap();
        }
        assert!(run_tool(&mut engine, &mut view));
        let kind = &engine.sketch_snapshot().unwrap().sketch.curves[0].kind;
        assert!(matches!(
            (tool, kind),
            ("sketch.circle.center", solidcraft_engine::sketch::CurveKind::Circle { .. })
                | ("sketch.arc.three_point", solidcraft_engine::sketch::CurveKind::Arc { .. })
                | ("sketch.spline.fit_point", solidcraft_engine::sketch::CurveKind::Spline { .. })
        ));
    }
    let (engine, mut view) = sketch_input_fixture(json!("XY"), "sketch.circle.center");
    let start = point(&view, Vec3::ZERO);
    let end = point(&view, Vec3::new(20.0, 0.0, 0.0));
    feed(&mut view, InputSource::Pencil, InputPhase::Began, 1, start, 1.0);
    feed(&mut view, InputSource::Pencil, InputPhase::Moved, 1, end, 2.0);
    let work = view.take_tool_work().unwrap();
    let preview = engine.prepare_tool(work.clone()).unwrap();
    view.command("tool.cancel", &json!({})).unwrap();
    assert!(!view.publish_preview(&work, preview.model));
    assert!(engine.sketch_snapshot().unwrap().sketch.curves.is_empty());
}
#[test]
fn sketch_plane_resize_and_fingers_cancel_only_provisional_pencil_work() {
    let plane = json!({"origin":[5,6,7],"x_dir":[0,1,0],"y_dir":[0,0,1]});
    let (mut engine, mut view) = sketch_input_fixture(plane, "sketch.line");
    let basis = view.camera.basis();
    assert!(basis.0.dot(Vec3::Y) > 0.999);
    assert!(basis.1.dot(Vec3::Z) > 0.999);
    sketch_tap(&mut view, 1, [0.0, 0.0]);
    sketch_tap(&mut view, 2, [20.0, 0.0]);
    run_tool(&mut engine, &mut view);
    let count = engine.sketch_snapshot().unwrap().sketch.curves.len();
    let start = point(&view, Vec3::new(5.0, 26.0, 7.0));
    let end = point(&view, Vec3::new(5.0, 26.0, 27.0));
    feed(&mut view, InputSource::Pencil, InputPhase::Began, 3, start, 3.0);
    feed(&mut view, InputSource::Pencil, InputPhase::Moved, 3, end, 3.1);
    let work = view.take_tool_work().unwrap();
    let prepared = engine.prepare_tool(work.clone()).unwrap();
    let back = view.camera.back();
    feed(&mut view, InputSource::Finger, InputPhase::Began, 4, [200.0, 200.0], 4.0);
    feed(&mut view, InputSource::Finger, InputPhase::Moved, 4, [250.0, 230.0], 4.2);
    assert_eq!(view.camera.back(), back);
    assert!(!view.publish_preview(&work, prepared.model));
    feed(&mut view, InputSource::Pencil, InputPhase::Ended, 3, end, 5.0);
    assert!(view.take_tool_work().is_none());
    assert_eq!(engine.sketch_snapshot().unwrap().sketch.curves.len(), count);
    view.set_metrics(metrics(500.0, 700.0, 3.0)).unwrap();
    assert_eq!(view.camera.back(), back);
}
#[test]
fn sketch_pencil_dimension_placement_and_annotation_drag_keep_driving_value() {
    let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "sketch.line");
    sketch_tap(&mut view, 1, [0.0, 0.0]);
    sketch_tap(&mut view, 2, [40.0, 0.0]);
    run_tool(&mut engine, &mut view);
    let sk = engine.sketch_snapshot().unwrap();
    let line = sk.sketch.curves[0].id.clone();
    view.command("select.entity", &json!({"entity":Sel::SketchCurve{id:line,sketch:Some(sk.id)}})).unwrap();
    view.command("tool.set", &json!({"tool":"sketch.dimension","dimension_type":"length"})).unwrap();
    sketch_tap(&mut view, 3, [20.0, -10.0]);
    assert!(run_tool(&mut engine, &mut view));
    assert!(matches!(view.selection.as_slice(), [Sel::SketchConstraint { .. }]));
    let sk = engine.sketch_snapshot().unwrap();
    let dimension = sk.sketch.constraints.iter().find(|c| c.kind.is_dimension()).unwrap();
    let before = solidcraft_engine::sketch::measure_dimension(&sk.sketch, &dimension.kind).unwrap();
    let annotations = view.diagnostic()["annotations"].clone();
    let xy = &annotations[0]["points"];
    let screen = [xy[0].as_f64().unwrap(), xy[1].as_f64().unwrap()];
    assert!(matches!(view.pick(screen, None), Some(Sel::SketchConstraint { .. })));
    view.command("tool.set", &json!({"tool":"sketch.move"})).unwrap();
    feed(&mut view, InputSource::Pencil, InputPhase::Began, 4, screen, 4.0);
    let end = [screen[0] + 40.0, screen[1] - 30.0];
    feed(&mut view, InputSource::Pencil, InputPhase::Ended, 4, end, 4.2);
    assert!(run_tool(&mut engine, &mut view));
    let moved = engine.sketch_snapshot().unwrap();
    let dimension = moved.sketch.constraints.iter().find(|c| c.kind.is_dimension()).unwrap();
    assert!((solidcraft_engine::sketch::measure_dimension(&moved.sketch, &dimension.kind).unwrap() - before).abs() < 1e-8);
}

#[test]
fn sketch_edge_readout_creates_one_dimension_and_reuses_it_for_inline_edits() {
    let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "select");
    engine.execute("sketch.line", json!({"points":[[0,0],[30,40]],"ids":["edge"],"infer":false})).unwrap();
    view.begin_update(2);
    view.install(2, engine.snapshot().unwrap());
    let history = engine.session.undo.len();
    let original = engine.session.doc.clone();
    let initial = view.sketch_annotations();
    assert_eq!(initial.as_array().unwrap().len(), 1, "a free size is visible before selection");
    assert_eq!(initial[0]["selected"], false);
    sketch_tap(&mut view, 1, [15., 20.]);
    let readout = view.sketch_annotations()[0].clone();
    assert_eq!(readout["dimension_type"], "length");
    assert_eq!(readout["selected"], true);
    assert!((readout["value"].as_f64().unwrap() - 50.).abs() < 1e-8);
    assert!(readout["points"].as_array().unwrap().iter().all(|v| v.as_f64().unwrap().is_finite()));
    assert!(Arc::ptr_eq(&original, &engine.session.doc), "selection must not create a constraint");
    assert_eq!(engine.session.undo.len(), history);
    let entity: Sel = serde_json::from_value(readout["entity"].clone()).unwrap();
    let selection = [entity];
    let edit = engine.begin_numeric_for(101, &selection, readout["dimension_type"].as_str()).unwrap();
    view.begin_numeric(edit.clone()).unwrap();
    let preview = engine.prepare_numeric(&edit, "60 mm", false).unwrap();
    view.publish_numeric(101, &preview).unwrap();
    assert!((view.sketch_annotations()[0]["value"].as_f64().unwrap() - 60.).abs() < 1e-6);
    view.finish_numeric(101, None).unwrap();
    assert!(Arc::ptr_eq(&original, &engine.session.doc));
    assert_eq!(engine.session.undo.len(), history, "preview/cancel must not author history");
    let prepared = engine.prepare_numeric(&edit, "60 mm", true).unwrap();
    let model = engine.accept_numeric(&edit, prepared).unwrap();
    view.begin_update(3);
    view.install(3, model);
    view.command("select.entity", &json!({"entity":selection[0]})).unwrap();
    let annotations = view.sketch_annotations();
    assert_eq!(annotations.as_array().unwrap().len(), 1, "the existing dimension replaces the temporary readout");
    assert!(annotations[0]["dimension_type"].is_null());
    assert_eq!(annotations[0]["selected"], true, "selecting the edge highlights its existing size");
    let entity: Sel = serde_json::from_value(annotations[0]["entity"].clone()).unwrap();
    numeric_apply(&mut engine, &[entity], None, "70 mm");
    let sketch = engine.sketch_snapshot().unwrap();
    let dimensions: Vec<_> = sketch.sketch.constraints.iter().filter(|c| c.kind.is_dimension()).collect();
    assert_eq!(dimensions.len(), 1, "repeat editing must not create conflicting duplicate dimensions");
    assert!((solidcraft_engine::sketch::measure_dimension(&sketch.sketch, &dimensions[0].kind).unwrap() - 70.).abs() < 1e-6);
    assert_eq!(engine.session.undo.len(), history + 2);
    engine.execute("edit.undo", json!({})).unwrap();
    let sketch = engine.sketch_snapshot().unwrap();
    let dimension = sketch.sketch.constraints.iter().find(|c| c.kind.is_dimension()).unwrap();
    assert!((solidcraft_engine::sketch::measure_dimension(&sketch.sketch, &dimension.kind).unwrap() - 60.).abs() < 1e-6);
    engine.execute("edit.undo", json!({})).unwrap();
    assert!(!engine.sketch_snapshot().unwrap().sketch.constraints.iter().any(|c| c.kind.is_dimension()));
}

#[test]
fn sketch_free_dimensions_are_visible_without_selection_and_preserve_legacy_preference() {
    let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "select");
    engine.execute("sketch.rectangle.two_point", json!({"p0":[10,10],"p1":[50,40],"connect":false})).unwrap();
    view.begin_update(2);
    view.install(2, engine.snapshot().unwrap());
    let history = engine.session.undo.len();
    let original = engine.session.doc.clone();
    let annotations = view.sketch_annotations();
    assert_eq!(annotations.as_array().unwrap().len(), 2, "only the top and right rectangle edges have size helpers");
    assert!(annotations.as_array().unwrap().iter().all(|a| a["selected"] == false && a["dimension_type"] == "length"));
    assert!(view.selection().is_empty());
    assert!(view.diagnostic()["numeric_token"].is_null(), "helpers never automatically open an editor");
    assert!(engine.sketch_snapshot().unwrap().sketch.constraints.iter().all(|c| !c.kind.is_dimension()));
    let helper_lines = view.snapshot().unwrap().highlight.lines.len();

    view.command("tool.free_dimensions", &json!({"enabled":false})).unwrap();
    assert!(view.sketch_annotations().as_array().unwrap().is_empty(), "the preference restores selected-only helpers");
    assert!(view.snapshot().unwrap().highlight.lines.len() < helper_lines, "hiding helpers also removes their cached arrows and lines");
    view.command("select.entity", &json!({"entity":annotations[0]["entity"]})).unwrap();
    assert_eq!(view.sketch_annotations().as_array().unwrap().len(), 1);
    view.command("tool.free_dimensions", &json!({"enabled":true})).unwrap();
    let annotations = view.sketch_annotations();
    assert_eq!(annotations.as_array().unwrap().len(), 2);
    assert_eq!(annotations.as_array().unwrap().iter().filter(|a| a["selected"] == true).count(), 1);
    assert!(Arc::ptr_eq(&original, &engine.session.doc), "helpers and their preference cannot modify the design");
    assert_eq!(engine.session.undo.len(), history);
}

#[test]
fn sketch_rectangle_helpers_use_top_and_right_outside_in_each_drawing_direction() {
    for (p0, p1) in [([10, 10], [50, 40]), ([50, 40], [10, 10]), ([50, 10], [10, 40]), ([10, 40], [50, 10])] {
        let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "select");
        let rectangle = engine.execute("sketch.rectangle.two_point", json!({"p0":p0,"p1":p1,"connect":false})).unwrap().value;
        // An unrelated edge with the same length still needs its own helper.
        engine.execute("sketch.line", json!({"points":[[70,10],[110,10]],"ids":["unrelated"],"infer":false,"connect":false})).unwrap();
        view.begin_update(2);
        view.install(2, engine.snapshot().unwrap());
        let original = engine.session.doc.clone();
        let history = engine.session.undo.len();
        let annotations = view.sketch_annotations();
        assert_eq!(annotations.as_array().unwrap().len(), 3);
        for (side, midpoint, axis, sign) in [("top", [30., 40., 0.], 1, -1.), ("right", [50., 25., 0.], 0, 1.)] {
            let id = format!("edge:{}", rectangle["sides"][side].as_str().unwrap());
            let annotation = annotations.as_array().unwrap().iter().find(|a| a["id"] == id).unwrap();
            let edge = view.command("view.project", &json!({"point":midpoint})).unwrap();
            let separation = sign * (annotation["points"][axis].as_f64().unwrap() - edge["points"][axis].as_f64().unwrap());
            assert!((separation - 36.).abs() < 1e-6, "{side} must be 36 display points outside, got {separation}");
        }
        // Selecting a bottom edge must not create a third rectangle helper.
        view.command("select.entity", &json!({"entity":{"type":"sketch_curve","id":rectangle["sides"]["bottom"]}})).unwrap();
        assert_eq!(view.sketch_annotations(), annotations);
        assert!(Arc::ptr_eq(&original, &engine.session.doc));
        assert_eq!(engine.session.undo.len(), history);
    }
}

#[test]
fn sketch_unconstrained_rectangular_loop_keeps_independent_edge_helpers() {
    let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "select");
    engine.execute("sketch.line", json!({"points":[[10,10],[50,10],[50,40],[10,40]],"closed":true,"infer":false,"connect":false})).unwrap();
    view.begin_update(2);
    view.install(2, engine.snapshot().unwrap());
    assert_eq!(view.sketch_annotations().as_array().unwrap().len(), 4, "without H/V constraints the four lengths can vary independently");
}

#[test]
fn sketch_rectangle_helpers_are_grouped_per_loop_and_keep_dragged_positions() {
    let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "select");
    let mut top = String::new();
    for x in [10, 80] {
        let rectangle = engine.execute("sketch.rectangle.two_point", json!({"p0":[x,10],"p1":[x+40,40],"connect":false})).unwrap().value;
        top = format!("edge:{}", rectangle["sides"]["top"].as_str().unwrap());
    }
    view.begin_update(2);
    view.install(2, engine.snapshot().unwrap());
    let annotations = view.sketch_annotations();
    assert_eq!(annotations.as_array().unwrap().len(), 4, "each rectangle has its own width and height");
    let annotation = annotations.as_array().unwrap().iter().find(|a| a["id"] == top).unwrap();
    let p = [annotation["points"][0].as_f64().unwrap(), annotation["points"][1].as_f64().unwrap()];
    feed(&mut view, InputSource::Pencil, InputPhase::Began, 1, p, 1.0);
    feed(&mut view, InputSource::Pencil, InputPhase::Moved, 1, [p[0], p[1] + 80.], 1.1);
    feed(&mut view, InputSource::Pencil, InputPhase::Ended, 1, [p[0], p[1] + 80.], 1.2);
    assert!(view.take_tool_work().is_none());
    let moved = view.sketch_annotations();
    let annotation = moved.as_array().unwrap().iter().find(|a| a["id"] == top).unwrap();
    assert!((annotation["points"][1].as_f64().unwrap() - p[1] - 80.).abs() < 1e-6);
    view.begin_update(3);
    view.install(3, engine.snapshot().unwrap());
    assert_eq!(view.sketch_annotations(), moved, "manual placement overrides the outside default");
}

#[test]
fn sketch_partial_dimensions_do_not_hide_a_length_that_can_still_change() {
    let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "select");
    engine.execute("sketch.line", json!({"points":[[10,10],[40,50]],"ids":["diagonal"],"infer":false,"connect":false})).unwrap();
    engine.execute("sketch.dimension", json!({"entities":["diagonal"],"type":"horizontal","value":"30 mm"})).unwrap();
    view.begin_update(2);
    view.install(2, engine.snapshot().unwrap());
    let annotations = view.sketch_annotations();
    assert_eq!(annotations.as_array().unwrap().len(), 2, "a horizontal distance leaves the diagonal's length free");
    let helper = annotations.as_array().unwrap().iter().find(|a| a["dimension_type"] == "length").unwrap();
    let entity: Sel = serde_json::from_value(helper["entity"].clone()).unwrap();
    let edit = engine.begin_numeric_for(992, &[entity], Some("length")).unwrap();
    assert!(engine.prepare_numeric(&edit, "60 mm", false).is_ok(), "the free length remains editable");
}

#[test]
fn sketch_free_dimensions_hide_indirectly_determined_sizes_while_the_sketch_can_move() {
    let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "select");
    let rectangle = engine.execute("sketch.rectangle.two_point", json!({"p0":[10,10],"p1":[50,40],"connect":false})).unwrap().value;
    let bottom = rectangle["sides"]["bottom"].as_str().unwrap();
    let right = rectangle["sides"]["right"].as_str().unwrap();
    let sketch_id = engine.session.active_sketch.unwrap();
    numeric_apply(&mut engine, &curve_selection(sketch_id, bottom), Some("length"), "40 mm");
    view.begin_update(2);
    view.install(2, engine.snapshot().unwrap());
    let annotations = view.sketch_annotations();
    let helpers: Vec<_> = annotations.as_array().unwrap().iter().filter(|a| a["dimension_type"].is_string()).collect();
    assert_eq!(helpers.len(), 1, "only the right height helper remains after width is dimensioned");
    assert!(helpers.iter().all(|a| (a["value"].as_f64().unwrap() - 30.).abs() < 1e-8));
    numeric_apply(&mut engine, &curve_selection(sketch_id, right), Some("length"), "30 mm");
    let snapshot = engine.sketch_snapshot().unwrap();
    assert!(snapshot.dof > 0, "the sized rectangle remains free to translate");
    assert_eq!(snapshot.sketch.constraints.iter().filter(|c| c.kind.is_dimension()).count(), 2);
    view.begin_update(3);
    view.install(3, engine.snapshot().unwrap());
    let history = engine.session.undo.len();
    let annotations = view.sketch_annotations();
    assert_eq!(annotations.as_array().unwrap().len(), 2, "existing dimensions remain visible without redundant helpers");
    assert!(annotations.as_array().unwrap().iter().all(|a| a["dimension_type"].is_null()));
    let entity: Sel = serde_json::from_value(annotations[0]["entity"].clone()).unwrap();
    let edit = engine.begin_numeric(991, &[entity]).unwrap();
    assert_eq!(edit.units, "mm");
    assert!(engine.prepare_numeric(&edit, &edit.expression, false).is_ok(), "existing dimensions remain editable");
    assert_eq!(engine.session.undo.len(), history, "checking a size never adds a constraint");
}

#[test]
fn sketch_free_dimensions_hide_lines_between_fixed_points_despite_other_free_geometry() {
    let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "select");
    engine.execute("sketch.line", json!({"points":[[10,10],[50,40]],"ids":["anchors"],"infer":false,"connect":false})).unwrap();
    engine.execute("sketch.constraint.fix", json!({"entity":"anchors","fixed":true})).unwrap();
    engine.execute("sketch.line", json!({"points":["anchors.start","anchors.end"],"ids":["joined"],"infer":false})).unwrap();
    engine.execute("sketch.line", json!({"points":[[70,20],[90,35]],"ids":["free"],"infer":false,"connect":false})).unwrap();
    assert!(engine.sketch_snapshot().unwrap().dof > 0);
    view.begin_update(2);
    view.install(2, engine.snapshot().unwrap());
    let annotations = view.sketch_annotations();
    assert_eq!(annotations.as_array().unwrap().len(), 1);
    assert_eq!(annotations[0]["id"], "edge:free", "only the line with a free length receives a helper");
    view.command("select.entity", &json!({"entity":{"type":"sketch_curve","id":"joined"}})).unwrap();
    assert_eq!(view.sketch_annotations().as_array().unwrap().len(), 1, "selecting the fixed-size edge cannot reveal a redundant helper");
}

#[test]
fn sketch_free_dimensions_offer_circle_and_arc_radii_and_hide_equal_driven_sizes() {
    let (mut engine, mut view) = sketch_input_fixture(json!("YZ"), "select");
    engine.execute("sketch.circle.center", json!({"center":[15,15],"radius":10,"id":"circle","connect":false})).unwrap();
    engine.execute("sketch.arc.three_point", json!({"start":[40,10],"end":[60,10],"through":[50,20],"id":"arc","connect":false})).unwrap();
    view.begin_update(2);
    view.install(2, engine.snapshot().unwrap());
    let annotations = view.sketch_annotations();
    assert_eq!(annotations.as_array().unwrap().len(), 2);
    assert!(annotations.as_array().unwrap().iter().all(|a| a["dimension_type"] == "radius" && a["selected"] == false));
    engine.execute("sketch.dimension", json!({"entities":["circle"],"type":"radius","value":"10 mm"})).unwrap();
    engine.execute("sketch.constraint.equal", json!({"a":"circle","b":"arc"})).unwrap();
    assert!(engine.sketch_snapshot().unwrap().dof > 0);
    view.begin_update(3);
    view.install(3, engine.snapshot().unwrap());
    let annotations = view.sketch_annotations();
    assert_eq!(annotations.as_array().unwrap().len(), 1, "the circle dimension determines the equal arc radius");
    assert!(annotations[0]["dimension_type"].is_null());
    assert_eq!(annotations[0]["driven"], false);
}

#[test]
fn sketch_reference_dimensions_create_convert_and_reject_redundant_driving_without_changes() {
    for dimension in ["length", "horizontal", "vertical", "angle", "radius", "diameter"] {
        let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "select");
        if matches!(dimension, "radius" | "diameter") {
            engine.execute("sketch.circle.center", json!({"center":[15,15],"radius":10,"id":"edge","connect":false})).unwrap();
        } else {
            engine.execute("sketch.line", json!({"points":[[10,10],[40,50]],"ids":["edge"],"infer":false,"connect":false})).unwrap();
        }
        let freedom = engine.sketch_snapshot().unwrap().dof;
        let parameters = engine.session.doc.params.len();
        let history = engine.session.undo.len();
        engine.execute("sketch.dimension", json!({"entities":["edge"],"type":dimension,"driven":true})).unwrap();
        let snapshot = engine.sketch_snapshot().unwrap();
        let constraint = snapshot.sketch.constraints.iter().find(|c| c.kind.is_dimension()).unwrap();
        let id = constraint.id.clone();
        let selection = [Sel::SketchConstraint { id: id.clone() }];
        assert!(constraint.driven && constraint.param.is_none());
        assert_eq!(snapshot.dof, freedom, "a reference must not constrain {dimension}");
        assert_eq!(engine.session.doc.params.len(), parameters);
        assert_eq!(engine.session.undo.len(), history + 1);
        assert!(engine.begin_numeric(800, &selection).is_err());

        engine.execute("sketch.toggle_driven", json!({"constraint":id,"driven":false})).unwrap();
        assert_eq!(engine.sketch_snapshot().unwrap().dof, freedom - 1);
        assert!(engine.begin_numeric(801, &selection).is_ok(), "converted {dimension} must be editable");
        assert_eq!(engine.session.doc.params.len(), parameters + 1);
        assert_eq!(engine.session.undo.len(), history + 2);
        engine.execute("edit.undo", json!({})).unwrap();
        assert_eq!(engine.sketch_snapshot().unwrap().dof, freedom);
        assert!(engine.begin_numeric(802, &selection).is_err());
        engine.execute("edit.redo", json!({})).unwrap();
        assert!(engine.begin_numeric(803, &selection).is_ok());
        engine.execute("sketch.toggle_driven", json!({"constraint":id,"driven":true})).unwrap();
        assert_eq!(engine.session.doc.params.len(), parameters);
        assert_eq!(engine.sketch_snapshot().unwrap().dof, freedom);
        view.begin_update(2);
        view.install(2, engine.snapshot().unwrap());
        let annotations = view.sketch_annotations();
        let annotation = annotations.as_array().unwrap().iter().find(|a| a["id"] == id).unwrap();
        assert_eq!(annotation["driven"], true);
        assert_eq!(annotation["units"], if dimension == "angle" { "deg" } else { "mm" });

        // Keep the original driving and add a redundant reference through the
        // exact creation path offered by the native inspector.
        engine.execute("sketch.toggle_driven", json!({"constraint":id,"driven":false})).unwrap();
        engine.execute("sketch.dimension", json!({"entities":["edge"],"type":dimension,"driven":true})).unwrap();
        let snapshot = engine.sketch_snapshot().unwrap();
        let duplicate = snapshot.sketch.constraints.iter().find(|c| c.kind.is_dimension() && c.driven).unwrap().id.clone();
        let document = engine.session.doc.clone();
        let history = engine.session.undo.len();
        assert!(engine.execute("sketch.toggle_driven", json!({"constraint":duplicate,"driven":false})).is_err());
        assert!(Arc::ptr_eq(&document, &engine.session.doc), "failed {dimension} conversion changed the design");
        assert_eq!(engine.session.undo.len(), history);
        assert!(engine.begin_numeric(804, &[Sel::SketchConstraint { id: duplicate }]).is_err());
    }
}

#[test]
fn sketch_edge_readouts_preserve_radial_and_endpoint_dimensions_and_reference_values() {
    for dimension in ["radius", "diameter", "horizontal", "vertical"] {
        let (mut engine, mut view) = sketch_input_fixture(json!("YZ"), "select");
        let radial = matches!(dimension, "radius" | "diameter");
        let command = if radial { "sketch.circle.center" } else { "sketch.line" };
        let params =
            if radial { json!({"center":[0,0],"radius":10,"id":"edge"}) } else { json!({"points":[[0,0],[30,40]],"ids":["edge"],"infer":false}) };
        engine.execute(command, params).unwrap();
        let snapshot = engine.sketch_snapshot().unwrap();
        let selection = curve_selection(snapshot.id, &snapshot.sketch.curves[0].id);
        view.begin_update(2);
        view.install(2, engine.snapshot().unwrap());
        view.command("select.entity", &json!({"entity":selection[0]})).unwrap();
        let readout = view.sketch_annotations();
        assert_eq!(readout.as_array().unwrap().len(), 1);
        assert_eq!(readout[0]["dimension_type"], if radial { "radius" } else { "length" });
        engine.execute("sketch.dimension", json!({"entities":[snapshot.sketch.curves[0].id],"type":dimension,"driven":true})).unwrap();
        view.begin_update(3);
        view.install(3, engine.snapshot().unwrap());
        view.command("select.entity", &json!({"entity":selection[0]})).unwrap();
        let annotations = view.sketch_annotations();
        assert_eq!(annotations.as_array().unwrap().len(), 2, "{dimension}: a reference dimension leaves the size free");
        assert_eq!(annotations[0]["driven"], true);
        assert_eq!(annotations[0]["selected"], true);
        assert!(annotations[0]["dimension_type"].is_null());
        let entity: Sel = serde_json::from_value(annotations[0]["entity"].clone()).unwrap();
        assert!(engine.begin_numeric(102, &[entity]).is_err(), "reference dimensions remain read-only");
        assert_eq!(annotations[1]["dimension_type"], if radial { "radius" } else { "length" });
        assert_ne!(annotations[0]["points"], annotations[1]["points"], "reference and free-size helpers remain separate");
        let helper: Sel = serde_json::from_value(annotations[1]["entity"].clone()).unwrap();
        assert!(engine.begin_numeric_for(103, &[helper], annotations[1]["dimension_type"].as_str()).is_ok());
        view.command("select.clear", &json!({})).unwrap();
        assert_eq!(view.sketch_annotations()[0]["selected"], false);
    }
}

#[test]
fn sketch_dimension_labels_drag_directly_with_preview_handles_and_one_undo_step() {
    use solidcore_record::Vec2;

    for source in [InputSource::Pencil, InputSource::Finger, InputSource::Indirect] {
        let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "select");
        engine.execute("sketch.line", json!({"points":[[0,0],[40,0]],"ids":["length"]})).unwrap();
        engine.execute("sketch.dimension", json!({"entities":["length"],"type":"length","text_at":[20,-10]})).unwrap();
        view.begin_update(2);
        view.install(2, engine.snapshot().unwrap());
        let original = engine.sketch_snapshot().unwrap();
        let history = engine.session.undo.len();
        let camera = view.diagnostic()["camera"].clone();
        let xy = view.diagnostic()["annotations"][0]["points"].clone();
        let center = [xy[0].as_f64().unwrap(), xy[1].as_f64().unwrap()];
        let grab = [center[0] + 5.0, center[1] + 4.0];
        let end = [grab[0] + 40.0, grab[1] - 30.0];
        feed(&mut view, source, InputPhase::Began, 1, grab, 1.0);
        assert!(view.take_tool_work().is_none(), "grabbing a label must not move it");
        feed(&mut view, source, InputPhase::Moved, 1, end, 1.1);
        let preview = view.take_tool_work().expect("dragging a label in Select must produce a preview");
        assert!(!preview.commit);
        let prepared = engine.prepare_tool(preview.clone()).unwrap();
        assert!(view.publish_preview(&preview, prepared.model));
        assert_eq!(engine.session.undo.len(), history);
        assert_eq!(engine.sketch_snapshot().unwrap().sketch, original.sketch);
        let expected = [center[0] + 40.0, center[1] - 30.0];
        let annotations = view.diagnostic()["annotations"].clone();
        for (axis, expected) in expected.into_iter().enumerate() {
            assert!((annotations[0]["points"][axis].as_f64().unwrap() - expected).abs() < 1e-6, "retain the grab offset");
        }
        assert_eq!(view.diagnostic()["camera"], camera, "dragging text must not pan the camera");
        let frame = view.snapshot().unwrap();
        let snapshot = frame.model.sketch.as_ref().unwrap();
        let dimension = snapshot.sketch.constraints.iter().find(|c| c.kind.is_dimension()).unwrap();
        let unit = 2.0 * view.camera.half_height() / view.metrics.height_points;
        let layout = solidcraft_engine::sketch::dim_layout(
            &solidcraft_engine::sketch::dim_frame(&snapshot.sketch, &dimension.kind).unwrap(),
            dimension.text,
            unit,
            Vec2::ZERO,
        );
        // Both arrow tips must stay above their measured endpoints at the label's new height.
        assert_eq!(layout.arrows.len(), 2);
        for (tip, _) in &layout.arrows {
            assert!((tip.y - layout.text.y).abs() < 1e-8);
            assert!(tip.x.abs() < 1e-8 || (tip.x - 40.0).abs() < 1e-8);
            let world = snapshot.plane.to_world(*tip);
            assert!(
                frame.highlight.lines.iter().any(|line| line.points.first().is_some_and(|p| p.dist(world) < 1e-8)),
                "rendered arrow follows text"
            );
        }
        feed(&mut view, source, InputPhase::Ended, 1, end, 1.2);
        assert!(run_tool(&mut engine, &mut view));
        assert_eq!(engine.session.undo.len(), history + 1);
        let moved = engine.sketch_snapshot().unwrap();
        assert_eq!(moved.sketch.points, original.sketch.points);
        assert_eq!(moved.sketch.curves, original.sketch.curves);
        assert_eq!(view.diagnostic()["input"]["tool"], "select");
        assert_ne!(moved.sketch.constraints, original.sketch.constraints);
        engine.execute("edit.undo", json!({})).unwrap();
        assert_eq!(engine.sketch_snapshot().unwrap().sketch, original.sketch);
        engine.execute("edit.redo", json!({})).unwrap();
        assert_eq!(engine.sketch_snapshot().unwrap().sketch, moved.sketch);
    }
}

#[test]
fn sketch_edge_measurement_labels_drag_without_creating_constraints_or_history() {
    for (command, params, curve_id) in [
        ("sketch.line", json!({"points":[[0,0],[40,0]],"ids":["edge"]}), "edge"),
        ("sketch.circle.center", json!({"center":[0,0],"radius":20,"id":"circle"}), "circle"),
    ] {
        for source in [InputSource::Pencil, InputSource::Finger, InputSource::Indirect] {
            let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "select");
            engine.execute(command, params.clone()).unwrap();
            view.begin_update(2);
            view.install(2, engine.snapshot().unwrap());
            let original = engine.sketch_snapshot().unwrap();
            let history = engine.session.undo.len();
            let annotation = view.diagnostic()["annotations"][0].clone();
            assert_eq!(annotation["id"], format!("edge:{curve_id}"));
            assert_eq!(annotation["selected"], false, "a helper can be dragged without first selecting its edge");
            let center = [annotation["points"][0].as_f64().unwrap(), annotation["points"][1].as_f64().unwrap()];
            let mut begin = sample(&view, source, InputPhase::Began, 1, [center[0] + 24.0, center[1]], 1.0);
            begin.sketch_annotation = annotation["id"].as_str().map(str::to_owned);
            view.input(begin).unwrap();
            let end = [center[0] + 64.0, center[1] - 30.0];
            feed(&mut view, source, InputPhase::Moved, 1, end, 1.1);
            let preview = view.diagnostic()["annotations"][0].clone();
            assert!((preview["points"][0].as_f64().unwrap() - center[0] - 40.0).abs() < 1e-6);
            assert!((preview["points"][1].as_f64().unwrap() - center[1] + 30.0).abs() < 1e-6);
            assert_eq!(preview["value"], annotation["value"]);
            assert!(view.take_tool_work().is_none(), "a measurement layout drag needs no engine evaluation");
            feed(&mut view, source, InputPhase::Ended, 1, end, 1.2);
            assert_eq!(view.diagnostic()["annotations"][0], preview);
            assert_eq!(engine.sketch_snapshot().unwrap().sketch, original.sketch);
            assert_eq!(engine.session.undo.len(), history);
            view.command("select.clear", &json!({})).unwrap();
            view.command("select.entity", &json!({"entity":{"type":"sketch_curve","id":curve_id}})).unwrap();
            assert_eq!(view.diagnostic()["annotations"][0], preview, "retain placement when reselecting the edge");
            let p = [preview["points"][0].as_f64().unwrap(), preview["points"][1].as_f64().unwrap()];
            feed(&mut view, source, InputPhase::Began, 2, p, 2.0);
            feed(&mut view, source, InputPhase::Moved, 2, [p[0] - 20.0, p[1] + 40.0], 2.1);
            feed(&mut view, source, InputPhase::Cancelled, 2, p, 2.2);
            assert_eq!(view.diagnostic()["annotations"][0], preview, "cancel restores the last placement");
        }
    }
}

#[test]
fn sketch_dimension_drag_cancellation_and_navigation_reject_late_previews() {
    let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "select");
    engine.execute("sketch.line", json!({"points":[[0,0],[40,0]],"ids":["length"]})).unwrap();
    engine.execute("sketch.dimension", json!({"entities":["length"],"type":"length","text_at":[20,-10],"driven":true})).unwrap();
    view.begin_update(2);
    view.install(2, engine.snapshot().unwrap());
    let original = engine.sketch_snapshot().unwrap();
    let history = engine.session.undo.len();
    for navigate in [false, true] {
        let annotation = view.diagnostic()["annotations"][0].clone();
        let p = [annotation["points"][0].as_f64().unwrap(), annotation["points"][1].as_f64().unwrap()];
        feed(&mut view, InputSource::Pencil, InputPhase::Began, 1, p, 1.0);
        feed(&mut view, InputSource::Pencil, InputPhase::Moved, 1, [p[0] + 40.0, p[1] - 30.0], 1.1);
        let work = view.take_tool_work().unwrap();
        let prepared = engine.prepare_tool(work.clone()).unwrap();
        if navigate {
            feed(&mut view, InputSource::Finger, InputPhase::Began, 2, [100.0, 100.0], 1.2);
            feed(&mut view, InputSource::Finger, InputPhase::Moved, 2, [160.0, 100.0], 1.4);
            feed(&mut view, InputSource::Finger, InputPhase::Ended, 2, [160.0, 100.0], 1.5);
        }
        feed(&mut view, InputSource::Pencil, InputPhase::Cancelled, 1, p, 1.6);
        assert!(!view.publish_preview(&work, prepared.model));
        assert!(view.take_tool_work().is_none());
        assert_eq!(engine.sketch_snapshot().unwrap().sketch, original.sketch);
        assert_eq!(engine.session.undo.len(), history);
    }
}

#[test]
fn sketch_constraint_indicators_disclose_related_geometry_and_are_pickable() {
    let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "sketch.line");
    sketch_tap(&mut view, 1, [0.0, 0.0]);
    sketch_tap(&mut view, 2, [30.0, 0.0]);
    run_tool(&mut engine, &mut view);
    assert!(view.diagnostic()["constraint_indicators"].as_array().unwrap().is_empty());
    let sk = engine.sketch_snapshot().unwrap();
    view.command("select.entity", &json!({"entity":Sel::SketchCurve{id:sk.sketch.curves[0].id.clone(),sketch:Some(sk.id)}})).unwrap();
    let indicators = view.diagnostic()["constraint_indicators"].clone();
    assert!(!indicators.as_array().unwrap().is_empty());
    let xy = &indicators[0]["points"];
    let screen = [xy[0].as_f64().unwrap(), xy[1].as_f64().unwrap()];
    assert!(matches!(view.pick(screen, None), Some(Sel::SketchConstraint { .. })));
}

#[test]
fn sketch_delete_taps_remove_only_the_hit_curve_and_repeat_with_history() {
    for source in [InputSource::Pencil, InputSource::Finger, InputSource::Indirect] {
        let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "sketch.delete");
        for (id, y) in [("target", -20), ("other", 20)] {
            engine.execute("sketch.line", json!({"points":[[-30,y],[30,y]],"ids":[id],"infer":false,"connect":false})).unwrap();
        }
        view.begin_update(2);
        view.install(2, engine.snapshot().unwrap());
        assert!(engine.workspace(&[], false, false)["tools"].as_array().unwrap().iter().any(|t| t["id"] == "sketch.delete" && t["enabled"] == true));
        view.command("select.entity", &json!({"entity":{"type":"sketch_curve","id":"other"}})).unwrap();
        let undo = engine.session.undo.len();
        let tap = |view: &mut Viewport, id, xy: [f64; 2]| {
            let screen = point(view, Vec3::new(xy[0], xy[1], 0.));
            feed(view, source, InputPhase::Began, id, screen, id as f64);
            feed(view, source, InputPhase::Ended, id, screen, id as f64 + 0.1);
        };
        // Blank taps never queue deletion. Endpoint picking must delete the line,
        // not its shared point, and preselection must not expand the deletion.
        tap(&mut view, 1, [0., 70.]);
        assert!(view.take_tool_work().is_none());
        tap(&mut view, 2, [-30., -20.]);
        assert!(run_tool(&mut engine, &mut view), "delete did not commit for {source:?}");
        let sk = engine.sketch_snapshot().unwrap();
        assert!(sk.sketch.curve_index("target").is_none());
        assert!(sk.sketch.curve_index("other").is_some());
        assert_eq!(engine.session.undo.len(), undo + 1);
        assert_eq!(view.diagnostic()["input"]["tool"], "sketch.delete");
        engine.execute("edit.undo", json!({})).unwrap();
        assert!(engine.sketch_snapshot().unwrap().sketch.curve_index("target").is_some());
        engine.execute("edit.redo", json!({})).unwrap();
        view.begin_update(3);
        view.install(3, engine.snapshot().unwrap());
        tap(&mut view, 3, [0., 20.]);
        assert!(run_tool(&mut engine, &mut view));
        assert!(engine.sketch_snapshot().unwrap().sketch.curves.is_empty());
        assert_eq!(engine.session.undo.len(), undo + 2);
    }
}

#[test]
fn sketch_delete_cancellation_preserves_geometry_and_rejects_late_preview() {
    let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "sketch.delete");
    engine.execute("sketch.line", json!({"points":[[-30,0],[30,0]],"ids":["target"],"infer":false})).unwrap();
    view.begin_update(2);
    view.install(2, engine.snapshot().unwrap());
    let original = engine.sketch_snapshot().unwrap().sketch;
    let undo = engine.session.undo.len();
    let screen = point(&view, Vec3::new(0., 0., 0.));
    feed(&mut view, InputSource::Pencil, InputPhase::Began, 1, screen, 1.);
    let end = point(&view, Vec3::new(5., 0., 0.));
    feed(&mut view, InputSource::Pencil, InputPhase::Moved, 1, end, 1.1);
    let work = view.take_tool_work().unwrap();
    let prepared = engine.prepare_tool(work.clone()).unwrap();
    feed(&mut view, InputSource::Pencil, InputPhase::Cancelled, 1, end, 1.2);
    assert!(!view.publish_tool_preview(prepared));
    assert!(view.take_tool_work().is_none());
    assert_eq!(engine.sketch_snapshot().unwrap().sketch, original);
    assert_eq!(engine.session.undo.len(), undo);
    engine.execute("sketch.finish", json!({})).unwrap();
    view.begin_update(3);
    view.install(3, engine.snapshot().unwrap());
    assert!(view.command("tool.set", &json!({"tool":"sketch.delete"})).is_err());
}

#[test]
fn sketch_trim_extend_and_transform_proposals_use_native_commands_once() {
    let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "select");
    engine.execute("sketch.line", json!({"points":[[0,0],[40,0]],"ids":["target"],"infer":false,"connect":false})).unwrap();
    for x in [10, 30] {
        engine.execute("sketch.line", json!({"points":[[x,-10],[x,10]],"infer":false})).unwrap();
    }
    view.begin_update(2);
    view.install(2, engine.snapshot().unwrap());
    let undo = engine.session.undo.len();
    view.command("tool.set", &json!({"tool":"sketch.trim"})).unwrap();
    sketch_tap(&mut view, 1, [20.0, 0.0]);
    assert!(run_tool(&mut engine, &mut view));
    assert_eq!(engine.session.undo.len(), undo + 1);
    engine.execute("edit.undo", json!({})).unwrap();
    view.begin_update(3);
    view.install(3, engine.snapshot().unwrap());
    view.command("tool.propose", &json!({"command":"sketch.move","params":{"entities":["target"],"translate":[0,5]},"commit":true})).unwrap();
    let proposal = view.take_tool_work().unwrap();
    let prepared = engine.prepare_tool(proposal.clone()).unwrap();
    let model = engine.accept_tool(prepared).unwrap();
    assert!(view.complete_tool(&proposal, model));
    let sk = engine.sketch_snapshot().unwrap();
    let end = sk.sketch.resolve_point("target.end").unwrap();
    assert!((sk.sketch.point(end).unwrap().y - 5.0).abs() < 1e-8);
    assert!(view.command("tool.propose", &json!({"command":"sketch.finish","params":{},"commit":true})).is_err());
    let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "select");
    engine.execute("sketch.line", json!({"points":[[0,0],[10,0]],"ids":["reach"],"infer":false})).unwrap();
    engine.execute("sketch.line", json!({"points":[[20,-10],[20,10]],"infer":false})).unwrap();
    view.begin_update(2);
    view.install(2, engine.snapshot().unwrap());
    let undo = engine.session.undo.len();
    view.command("tool.set", &json!({"tool":"sketch.extend"})).unwrap();
    sketch_tap(&mut view, 1, [9.5, 0.0]);
    assert!(run_tool(&mut engine, &mut view));
    let sk = engine.sketch_snapshot().unwrap();
    let endpoint = sk.sketch.resolve_point("reach.end").unwrap();
    assert!((sk.sketch.point(endpoint).unwrap().x - 20.0).abs() < 1e-8);
    assert_eq!(engine.session.undo.len(), undo + 1);
}

#[test]
fn sketch_fillet_corner_picking_previews_cancels_and_commits_one_undo_step() {
    for plane in ["XY", "XZ", "YZ"] {
        for source in [InputSource::Pencil, InputSource::Finger, InputSource::Indirect] {
            let (mut engine, mut view) = sketch_input_fixture(json!(plane), "sketch.fillet");
            engine.execute("parameters.add", json!({"name":"corner_radius","expression":"4 mm"})).unwrap();
            engine.execute("sketch.rectangle.two_point", json!({"p0":[-20,-15],"p1":[20,15],"connect":false})).unwrap();
            view.begin_update(2);
            view.install(2, engine.snapshot().unwrap());
            let original = engine.sketch_snapshot().unwrap();
            let undo = engine.session.undo.len();
            let screen = point(&view, original.plane.to_world(solidcore_record::Vec2::new(20., 15.)));
            feed(&mut view, source, InputPhase::Began, 1, screen, 1.);
            feed(&mut view, source, InputPhase::Ended, 1, screen, 1.1);
            assert!(matches!(view.selection(), [Sel::SketchPoint { .. }]), "{plane} {source:?}");
            assert!(view.take_tool_work().is_none(), "picking must not author geometry");
            let workspace = engine.workspace(view.selection(), false, false);
            assert!(workspace["tools"].as_array().unwrap().iter().any(|t| t["id"] == "sketch.fillet" && t["enabled"] == true));
            let mut params = workspace["sketch"]["fillet"]["params"].clone();
            assert_eq!(params["points"].as_array().unwrap().len(), 1);
            params["radius"] = json!("corner_radius");
            view.command("tool.propose", &json!({"command":"sketch.fillet","params":params,"commit":false})).unwrap();
            assert!(!run_tool(&mut engine, &mut view));
            assert_eq!(view.display_model().unwrap().sketch.as_ref().unwrap().sketch.curves.len(), 5);
            assert_eq!(engine.sketch_snapshot().unwrap().sketch, original.sketch);
            assert_eq!(engine.session.undo.len(), undo);
            view.command("tool.cancel", &json!({})).unwrap();
            assert_eq!(view.diagnostic()["input"]["tool"], "select");
            assert_eq!(view.display_model().unwrap().sketch.as_ref().unwrap().sketch, original.sketch);
            view.command("tool.set", &json!({"tool":"sketch.fillet"})).unwrap();
            view.command("tool.propose", &json!({"command":"sketch.fillet","params":params,"commit":true})).unwrap();
            assert!(run_tool(&mut engine, &mut view));
            assert_eq!(view.diagnostic()["input"]["tool"], "select");
            let filleted = engine.sketch_snapshot().unwrap().sketch;
            assert_eq!(filleted.curves.len(), 5);
            assert!(filleted.curves.iter().any(|curve| match curve.kind {
                solidcraft_engine::sketch::CurveKind::Arc { c, a, .. } =>
                    (filleted.point(c).unwrap().dist(filleted.point(a).unwrap()) - 4.).abs() < 1e-8,
                _ => false,
            }));
            let workspace = engine.workspace(&[], false, false);
            let area = workspace["sketch"]["profiles"][0]["area_mm2"].as_f64().unwrap();
            let expected = 1200. - (1. - std::f64::consts::FRAC_PI_4) * 16.;
            assert!((area - expected).abs() < 1e-6, "{area} versus {expected}");
            assert_eq!(engine.session.undo.len(), undo + 1);
            engine.execute("edit.undo", json!({})).unwrap();
            assert_eq!(engine.sketch_snapshot().unwrap().sketch, original.sketch);
            engine.execute("edit.redo", json!({})).unwrap();
            assert_eq!(engine.sketch_snapshot().unwrap().sketch, filleted);
            engine.execute("parameters.change", json!({"name":"corner_radius","expression":"5 mm"})).unwrap();
            let changed = engine.sketch_snapshot().unwrap().sketch;
            assert!(changed.curves.iter().any(|curve| match curve.kind {
                solidcraft_engine::sketch::CurveKind::Arc { c, a, .. } =>
                    (changed.point(c).unwrap().dist(changed.point(a).unwrap()) - 5.).abs() < 1e-8,
                _ => false,
            }));
            let reopened = modelling_roundtrip(&engine);
            assert_eq!(reopened.session.doc, engine.session.doc);
        }
    }
}

#[test]
fn sketch_fillet_curve_pair_and_multiple_corners_use_engine_references() {
    let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "sketch.fillet");
    engine.execute("sketch.rectangle.two_point", json!({"p0":[-20,-15],"p1":[20,15],"connect":false})).unwrap();
    view.begin_update(2);
    view.install(2, engine.snapshot().unwrap());
    sketch_tap(&mut view, 1, [0., -15.]);
    assert!(engine.workspace(view.selection(), false, false)["sketch"]["fillet"].is_null());
    sketch_tap(&mut view, 2, [20., 0.]);
    assert!(matches!(view.selection(), [Sel::SketchCurve { .. }, Sel::SketchCurve { .. }]));
    assert!(view.take_tool_work().is_none());
    let mut params = engine.workspace(view.selection(), false, false)["sketch"]["fillet"]["params"].clone();
    assert!(params["a"].is_string() && params["b"].is_string());
    params["radius"] = json!("3 mm");
    view.command("tool.propose", &json!({"command":"sketch.fillet","params":params,"commit":true})).unwrap();
    assert!(run_tool(&mut engine, &mut view));
    assert_eq!(engine.sketch_snapshot().unwrap().sketch.curves.len(), 5);
    engine.execute("edit.undo", json!({})).unwrap();
    view.begin_update(3);
    view.install(3, engine.snapshot().unwrap());
    view.select(None, false);
    view.command("tool.set", &json!({"tool":"sketch.fillet"})).unwrap();
    view.command("select.multiple", &json!({"enabled":true})).unwrap();
    sketch_tap(&mut view, 3, [-20., -15.]);
    sketch_tap(&mut view, 4, [20., 15.]);
    let mut params = engine.workspace(view.selection(), false, false)["sketch"]["fillet"]["params"].clone();
    assert_eq!(params["points"].as_array().unwrap().len(), 2);
    params["radius"] = json!("3 mm");
    view.command("tool.propose", &json!({"command":"sketch.fillet","params":params,"commit":true})).unwrap();
    assert!(run_tool(&mut engine, &mut view));
    assert_eq!(engine.sketch_snapshot().unwrap().sketch.curves.len(), 6);
}

#[test]
fn sketch_fillet_invalid_radius_and_stale_proposals_leave_the_document_unchanged() {
    let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "sketch.fillet");
    engine.execute("sketch.rectangle.two_point", json!({"p0":[-20,-15],"p1":[20,15],"connect":false})).unwrap();
    view.begin_update(2);
    view.install(2, engine.snapshot().unwrap());
    sketch_tap(&mut view, 1, [20., 15.]);
    let mut params = engine.workspace(view.selection(), false, false)["sketch"]["fillet"]["params"].clone();
    let before = engine.session.doc.clone();
    let undo = engine.session.undo.len();
    for radius in ["0 mm", "-1 mm", "100 mm", "missing_parameter"] {
        params["radius"] = json!(radius);
        view.command("tool.propose", &json!({"command":"sketch.fillet","params":params,"commit":true})).unwrap();
        let work = view.take_tool_work().unwrap();
        assert!(engine.prepare_tool(work).is_err(), "{radius}");
        assert!(Arc::ptr_eq(&before, &engine.session.doc));
        assert_eq!(engine.session.undo.len(), undo);
        view.command("tool.cancel", &json!({})).unwrap();
    }
    params["radius"] = json!("2 mm");
    view.command("tool.propose", &json!({"command":"sketch.fillet","params":params,"commit":true})).unwrap();
    let work = view.take_tool_work().unwrap();
    let prepared = engine.prepare_tool(work.clone()).unwrap();
    engine.execute("sketch.point", json!({"point":[50,50]})).unwrap();
    view.begin_update(3);
    view.install(3, engine.snapshot().unwrap());
    assert!(!view.tool_is_current(&work));
    assert!(engine.accept_tool(prepared).is_err());
    assert_eq!(engine.sketch_snapshot().unwrap().sketch.curves.len(), 4);
}

#[test]
fn sketch_offset_drag_previews_a_chain_and_commits_one_undo_step_on_each_plane() {
    for plane in ["XY", "XZ", "YZ"] {
        for y in [-5., 5.] {
            let (mut engine, mut view) = sketch_input_fixture(json!(plane), "sketch.offset");
            engine.execute("sketch.rectangle.two_point", json!({"p0":[-20,-15],"p1":[20,0],"connect":false})).unwrap();
            view.begin_update(2);
            view.install(2, engine.snapshot().unwrap());
            let original = engine.sketch_snapshot().unwrap();
            let undo = engine.session.undo.len();
            assert!(engine.workspace(view.selection(), false, false)["tools"].as_array().unwrap().iter().any(|tool| tool["id"] == "sketch.offset"));
            let start = point(&view, original.plane.to_world(solidcore_record::Vec2::new(0., 0.)));
            let end = point(&view, original.plane.to_world(solidcore_record::Vec2::new(0., y)));
            feed(&mut view, InputSource::Pencil, InputPhase::Began, 1, start, 1.);
            feed(&mut view, InputSource::Pencil, InputPhase::Moved, 1, end, 2.);
            assert!(!run_tool(&mut engine, &mut view));
            assert_eq!(engine.sketch_snapshot().unwrap().sketch, original.sketch);
            assert_eq!(engine.session.undo.len(), undo);
            let preview = view.display_model().unwrap().sketch.as_ref().unwrap();
            assert_eq!(preview.sketch.curves.len(), 8, "the whole connected rectangle is offset");
            let source_ids: Vec<_> = original.sketch.curves.iter().map(|c| c.id.as_str()).collect();
            let ys: Vec<_> = preview
                .sketch
                .curves
                .iter()
                .enumerate()
                .filter(|(_, c)| !source_ids.contains(&c.id.as_str()))
                .flat_map(|(i, _)| preview.sketch.polyline(i))
                .map(|p| p.y)
                .collect();
            assert!((ys.iter().copied().fold(f64::NEG_INFINITY, f64::max) - y).abs() < 1e-6, "pointer chooses inside/outside");
            feed(&mut view, InputSource::Pencil, InputPhase::Ended, 1, end, 3.);
            assert!(run_tool(&mut engine, &mut view));
            assert_eq!(engine.session.undo.len(), undo + 1);
            assert_eq!(engine.sketch_snapshot().unwrap().sketch.curves.len(), 8);
            engine.execute("edit.undo", json!({})).unwrap();
            assert_eq!(engine.sketch_snapshot().unwrap().sketch, original.sketch);
            engine.execute("edit.redo", json!({})).unwrap();
            assert_eq!(engine.sketch_snapshot().unwrap().sketch.curves.len(), 8);
        }
    }
}

#[test]
fn sketch_offset_round_curve_drag_cancel_and_zero_distance_leave_history_untouched() {
    for radius in [7., 13.] {
        let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "sketch.offset");
        engine.execute("sketch.circle.center", json!({"center":[0,0],"radius":10})).unwrap();
        view.begin_update(2);
        view.install(2, engine.snapshot().unwrap());
        let original = engine.sketch_snapshot().unwrap().sketch;
        let undo = engine.session.undo.len();
        let start = point(&view, Vec3::new(10., 0., 0.));
        let end = point(&view, Vec3::new(radius, 0., 0.));
        feed(&mut view, InputSource::Pencil, InputPhase::Began, 1, start, 1.);
        feed(&mut view, InputSource::Pencil, InputPhase::Moved, 1, end, 2.);
        assert!(!run_tool(&mut engine, &mut view));
        let model = view.display_model().unwrap();
        let sk = &model.sketch.as_ref().unwrap().sketch;
        assert!(matches!(sk.curves[1].kind, solidcraft_engine::sketch::CurveKind::Circle { r, .. } if (r-radius).abs()<1e-6));
        feed(&mut view, InputSource::Pencil, InputPhase::Cancelled, 1, end, 3.);
        assert_eq!(engine.sketch_snapshot().unwrap().sketch, original);
        assert_eq!(engine.session.undo.len(), undo);
        assert!(view.take_tool_work().is_none());
        // Moving along a straight source edge must not create a zero offset.
        engine.execute("sketch.line", json!({"points":[[-20,-20],[20,-20]],"infer":false})).unwrap();
        view.begin_update(3);
        view.install(3, engine.snapshot().unwrap());
        let start = point(&view, Vec3::new(0., -20., 0.));
        let end = point(&view, Vec3::new(5., -20., 0.));
        feed(&mut view, InputSource::Pencil, InputPhase::Began, 2, start, 4.);
        feed(&mut view, InputSource::Pencil, InputPhase::Moved, 2, end, 5.);
        feed(&mut view, InputSource::Pencil, InputPhase::Ended, 2, end, 6.);
        assert!(view.take_tool_work().is_none());
        assert_eq!(engine.session.undo.len(), undo + 1);
    }
}

#[test]
fn sketch_offset_exact_expression_preview_cancel_flip_invalid_and_single_edge() {
    let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "sketch.offset");
    engine.execute("parameters.add", json!({"name":"gap","expression":"3 mm"})).unwrap();
    let curves = engine.execute("sketch.rectangle.two_point", json!({"p0":[0,0],"p1":[40,30],"connect":false})).unwrap().value["curves"].clone();
    view.begin_update(2);
    view.install(2, engine.snapshot().unwrap());
    let original = engine.sketch_snapshot().unwrap().sketch;
    let undo = engine.session.undo.len();
    let params = json!({"curves":[curves[0]],"distance":"gap * 2","chain":false,"flip":true});
    view.command("tool.propose", &json!({"command":"sketch.offset","params":params,"commit":false})).unwrap();
    assert!(!run_tool(&mut engine, &mut view));
    assert_eq!(view.display_model().unwrap().sketch.as_ref().unwrap().sketch.curves.len(), 5);
    view.command("tool.cancel", &json!({})).unwrap();
    assert_eq!(engine.session.undo.len(), undo);
    assert_eq!(engine.sketch_snapshot().unwrap().sketch, original);
    view.command("tool.propose", &json!({"command":"sketch.offset","params":params,"commit":true})).unwrap();
    assert!(run_tool(&mut engine, &mut view));
    let sk = engine.sketch_snapshot().unwrap().sketch;
    let line = sk.polyline(4);
    assert!(line.iter().all(|p| (p.y + 6.).abs() < 1e-6));
    assert_eq!(engine.session.undo.len(), undo + 1);
    engine.execute("parameters.change", json!({"name":"gap","expression":"4 mm"})).unwrap();
    assert!(engine.sketch_snapshot().unwrap().sketch.polyline(4).iter().all(|p| (p.y + 8.).abs() < 1e-6), "offset retains its driving expression");
    view.begin_update(3);
    view.install(3, engine.snapshot().unwrap());
    let before = engine.session.doc.clone();
    view.command("tool.propose", &json!({"command":"sketch.offset","params":{"curves":[curves[0]],"distance":"0 mm"},"commit":true})).unwrap();
    let work = view.take_tool_work().unwrap();
    assert!(engine.prepare_tool(work).is_err());
    assert!(Arc::ptr_eq(&before, &engine.session.doc));
    view.command("tool.cancel", &json!({})).unwrap();
    // Tapping a curve selects it for exact entry, without creating geometry.
    view.command("tool.set", &json!({"tool":"sketch.offset"})).unwrap();
    sketch_tap(&mut view, 7, [20., 0.]);
    assert!(matches!(view.selection(), [Sel::SketchCurve { .. }]));
    assert!(view.take_tool_work().is_none());
}

#[test]
fn sketch_previous_tool_and_plane_preview_preserve_camera_and_modelling_history() {
    let (engine, mut view) = sketch_input_fixture(json!("YZ"), "sketch.circle.center");
    view.command("tool.previous", &json!({})).unwrap();
    assert_eq!(view.diagnostic()["input"]["tool"], "select");
    view.command("tool.previous", &json!({})).unwrap();
    assert_eq!(view.diagnostic()["input"]["tool"], "sketch.circle.center");
    assert!(engine.sketch_snapshot().unwrap().sketch.curves.is_empty());
    let (mut engine, mut view) = proof();
    let camera = view.camera;
    let undo = engine.session.undo.len();
    let plane = solidcore_record::Plane { origin: Vec3::new(5., 6., 7.), ..solidcore_record::Plane::YZ };
    view.command("view.sketch_plane", &json!({"plane":plane})).unwrap();
    assert!(view.camera.back().dot(plane.normal()) > 0.999);
    assert_eq!(engine.session.undo.len(), undo);
    view.command("view.cancel_plane", &json!({})).unwrap();
    assert_eq!(view.camera, camera);
    engine.execute("sketch.create", json!({"plane":"XY"})).unwrap();
    view.begin_update(2);
    view.install(2, engine.snapshot().unwrap());
    view.command("tool.set", &json!({"tool":"sketch.line"})).unwrap();
    engine.execute("sketch.finish", json!({})).unwrap();
    view.begin_update(3);
    view.install(3, engine.snapshot().unwrap());
    assert_eq!(view.diagnostic()["input"]["tool"], "select");
}

#[test]
fn sketch_dimension_editor_preserves_expression_units_focus_cancel_and_fixed_drag() {
    let mut adapter = SessionAdapter::new();
    let id = adapter.execute("sketch.create", json!({"plane":"XY"})).unwrap().value["sketch"].as_u64().unwrap();
    adapter.execute("parameters.add", json!({"name":"diameter","expression":"16 mm"})).unwrap();
    let circle =
        adapter.execute("sketch.circle.center", json!({"center":[0,0],"radius":8})).unwrap().value["curves"][0].as_str().unwrap().to_string();
    numeric_apply(&mut adapter, &curve_selection(id, &circle), Some("radius"), "diameter / 2");
    let snapshot = adapter.sketch_snapshot().unwrap();
    let constraint = snapshot.sketch.constraints.iter().find(|c| c.kind.is_dimension()).unwrap().id.clone();
    let selection = [Sel::SketchConstraint { id: constraint.clone() }];
    let mut view = Viewport::default();
    view.set_metrics(metrics(800., 600., 2.)).unwrap();
    view.begin_update(1);
    view.install(1, adapter.snapshot().unwrap());
    view.command("select.entity", &json!({"entity":selection[0]})).unwrap();
    let edit = adapter.begin_numeric(41, &selection).unwrap();
    assert_eq!(edit.expression, "diameter / 2");
    assert_eq!(edit.units, "mm");
    assert!((edit.value - 8.).abs() < 1e-8);
    let doc = adapter.session.doc.clone();
    let undo = adapter.session.undo.len();
    view.begin_numeric(edit.clone()).unwrap();
    for invalid in ["", "diameter /", "5 s", "0 mm", "-2 mm"] {
        assert!(adapter.prepare_numeric(&edit, invalid, true).is_err(), "{invalid}");
    }
    let preview = adapter.prepare_numeric(&edit, "diameter", false).unwrap();
    view.publish_numeric(41, &preview).unwrap();
    view.cancel_numeric();
    assert!(view.numeric_edit(41).is_err());
    assert!(Arc::ptr_eq(&doc, &adapter.session.doc));
    assert_eq!(adapter.session.undo.len(), undo);
    // At origin plus driving radius the entire circle is determined; a solver drag
    // cannot move its centre or silently remove either native relationship.
    let snapshot = adapter.sketch_snapshot().unwrap();
    assert_eq!(snapshot.dof, 0);
    let before = snapshot.sketch.clone();
    let work = ToolWork {
        interaction: 200,
        version: 1,
        document_id: adapter.session.document_id.clone(),
        geometry_revision: adapter.geometry_revision,
        commit: true,
        operation: ToolOperation::Sketch {
            sketch: id,
            commands: vec![
                ("sketch.drag".into(), json!({"entity":format!("{circle}.center"),"from":[0,0],"to":[10,10],"live":true})),
                ("sketch.drag_end".into(), json!({})),
            ],
        },
    };
    assert_eq!(adapter.prepare_tool(work).err().unwrap().code, "fixed_geometry");
    let after = adapter.sketch_snapshot().unwrap();
    assert_eq!(before.constraints, after.sketch.constraints);
    let center = after.sketch.center(after.sketch.curve_index(&circle).unwrap()).unwrap();
    assert!(center.len() < 1e-8);
}

#[test]
fn sketch_conflicting_existing_dimension_edit_preserves_fixed_geometry_expression_and_history() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("sketch.create", json!({"plane":"XY"})).unwrap();
    let line =
        adapter.execute("sketch.line", json!({"points":[[0,0],[30,0]],"infer":true})).unwrap().value["curves"][0].as_str().unwrap().to_string();
    let id = adapter.session.active_sketch.unwrap();
    numeric_apply(&mut adapter, &curve_selection(id, &line), Some("length"), "30 mm");
    adapter.execute("sketch.constraint.fix", json!({"entity":line,"fixed":true})).unwrap();
    let sk = adapter.sketch_snapshot().unwrap();
    let dimension = sk.sketch.constraints.iter().find(|c| c.kind.is_dimension()).unwrap();
    let selection = [Sel::SketchConstraint { id: dimension.id.clone() }];
    let edit = adapter.begin_numeric(84, &selection).unwrap();
    let doc = adapter.session.doc.clone();
    let undo = adapter.session.undo.len();
    let error = adapter.prepare_numeric(&edit, "40 mm", true).err().unwrap();
    assert!(!error.message.is_empty());
    assert!(Arc::ptr_eq(&doc, &adapter.session.doc));
    assert_eq!(undo, adapter.session.undo.len());
    assert_eq!(adapter.begin_numeric(85, &selection).unwrap().expression, "30 mm");
    let after = adapter.sketch_snapshot().unwrap();
    assert!(after.solved);
    assert_eq!(after.sketch, sk.sketch);
    // A recovery action is native/history based, and the earlier valid width remains.
    adapter.execute("sketch.constraint.fix", json!({"entity":line,"fixed":false})).unwrap();
    numeric_apply(&mut adapter, &selection, None, "40 mm");
    assert_eq!(adapter.begin_numeric(86, &selection).unwrap().expression, "40 mm");
}

#[test]
fn sketch_edit_and_native_roundtrip_preserve_unexposed_ellipse() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("sketch.create", json!({"plane":"XY"})).unwrap();
    let ellipse = adapter.execute("sketch.ellipse", json!({"center":[100,20],"major":[110,20],"minor_radius":4,"construction":true})).unwrap().value
        ["curves"][0]
        .as_str()
        .unwrap()
        .to_string();
    let initial = adapter.sketch_snapshot().unwrap();
    let index = initial.sketch.curve_index(&ellipse).unwrap();
    let entity = initial.sketch.curves[index].clone();
    let points: Vec<_> = entity.kind.point_ids().into_iter().map(|i| initial.sketch.points[i].clone()).collect();
    let circle =
        adapter.execute("sketch.circle.center", json!({"center":[0,0],"radius":5})).unwrap().value["curves"][0].as_str().unwrap().to_string();
    numeric_apply(&mut adapter, &curve_selection(initial.id, &circle), Some("radius"), "7 mm");
    adapter.execute("sketch.finish", json!({})).unwrap();
    let bytes = solidcraft_engine::io::write_design_container(&adapter.session.doc).unwrap();
    let doc = solidcraft_engine::io::DesignContainerReader::new(std::io::Cursor::new(bytes)).unwrap().read_document().unwrap();
    let mut loaded = SessionAdapter { session: solidcraft_engine::Session::new(doc), geometry_revision: 1, workspace_cache: None };
    loaded.execute("sketch.edit", json!({"sketch":initial.id})).unwrap();
    let sk = loaded.sketch_snapshot().unwrap();
    let e = &sk.sketch.curves[sk.sketch.curve_index(&ellipse).unwrap()];
    assert_eq!(e, &entity);
    let after: Vec<_> = e.kind.point_ids().into_iter().map(|i| sk.sketch.points[i].clone()).collect();
    assert_eq!(after, points);
}

#[test]
fn sketch_snapping_uses_saved_native_preference_and_failed_preview_can_recover() {
    let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "sketch.circle.center");
    view.command("tool.snapping", &json!({"enabled":false})).unwrap();
    assert!(run_tool(&mut engine, &mut view));
    assert!(engine.sketch_snapshot().unwrap().sketch.view.no_snap);
    assert_eq!(view.diagnostic()["input"]["sketch"]["snapping"], false);
    view.command("tool.snapping", &json!({"enabled":true})).unwrap();
    assert!(run_tool(&mut engine, &mut view));
    assert!(!engine.sketch_snapshot().unwrap().sketch.view.no_snap);
    let start = point(&view, Vec3::ZERO);
    let end = point(&view, Vec3::new(20., 0., 0.));
    feed(&mut view, InputSource::Pencil, InputPhase::Began, 1, start, 1.);
    feed(&mut view, InputSource::Pencil, InputPhase::Moved, 1, end, 2.);
    let work = view.take_tool_work().unwrap();
    let prepared = engine.prepare_tool(work.clone()).unwrap();
    assert!(view.publish_preview(&work, prepared.model));
    let old = view.contacts.preview.clone().unwrap();
    view.fail_tool(&work);
    assert!(Arc::ptr_eq(&old, view.contacts.preview.as_ref().unwrap()));
    feed(&mut view, InputSource::Pencil, InputPhase::Ended, 1, end, 3.);
    assert!(run_tool(&mut engine, &mut view));
    assert_eq!(engine.sketch_snapshot().unwrap().sketch.curves.len(), 1);
}

#[test]
fn sketch_exact_face_attachment_follows_source_height_and_survives_native_roundtrip() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("solid.box", json!({"length":40,"width":30,"height":10,"name":"Host","body_name":"Host body"})).unwrap();
    let faces = adapter.execute("model.faces", json!({"body":"Host body"})).unwrap().value;
    let top = faces["faces"].as_array().unwrap().iter().find(|face| face["plane_normal"][2].as_f64().is_some_and(|n| n > 0.999)).unwrap();
    let pick = json!({"face":top["point"],"body":"Host body","index":top["index"]});
    let preview = adapter.preview_sketch_plane(pick.clone()).unwrap();
    assert!((preview.origin.z - 10.0).abs() < 1e-8);
    let reply = adapter.execute("sketch.create", json!({"plane":pick,"name":"Attached sketch","project_edges":false})).unwrap().value;
    let sketch = reply["sketch"].as_u64().unwrap();
    assert_eq!(reply["attachment"], "exact_face");
    let feature = adapter.session.doc.feature(sketch).unwrap();
    assert!(matches!(&feature.kind, FeatureKind::Sketch { plane: solidcraft_engine::doc::PlaneRef::Face { anchor: Some(_), .. }, .. }));
    adapter.execute("sketch.rectangle.two_point", json!({"p0":[1,1],"p1":[3,3]})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    adapter.execute("timeline.edit", json!({"feature":"Host","set":{"height":"20 mm"}})).unwrap();
    adapter.execute("sketch.edit", json!({"sketch":sketch})).unwrap();
    assert!((adapter.sketch_snapshot().unwrap().plane.origin.z - 20.0).abs() < 1e-8);
    adapter.execute("sketch.finish", json!({})).unwrap();
    let bytes = solidcraft_engine::io::write_design_container(&adapter.session.doc).unwrap();
    let doc = solidcraft_engine::io::DesignContainerReader::new(std::io::Cursor::new(bytes)).unwrap().read_document().unwrap();
    let mut loaded = SessionAdapter { session: solidcraft_engine::Session::new(doc), geometry_revision: 1, workspace_cache: None };
    loaded.execute("sketch.edit", json!({"sketch":sketch})).unwrap();
    assert!((loaded.sketch_snapshot().unwrap().plane.origin.z - 20.0).abs() < 1e-8);
    assert!(matches!(
        &loaded.session.doc.feature(sketch).unwrap().kind,
        FeatureKind::Sketch { plane: solidcraft_engine::doc::PlaneRef::Face { anchor: Some(_), .. }, .. }
    ));
}

#[test]
fn sketch_existing_construction_plane_stays_a_native_reference_when_reopened() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("construct.plane.at_angle", json!({"base":"XY","axis":"X","angle":"35 deg","name":"Tilted"})).unwrap();
    let construction = adapter.session.doc.find_feature("Tilted").unwrap().id;
    let preview = adapter.preview_sketch_plane(json!("Tilted")).unwrap();
    assert!(preview.normal().dot(Vec3::Z).abs() < 0.9);
    let sketch = adapter.execute("sketch.create", json!({"plane":"Tilted","name":"On construction"})).unwrap().value["sketch"].as_u64().unwrap();
    assert!(
        matches!(&adapter.session.doc.feature(sketch).unwrap().kind,FeatureKind::Sketch{plane:solidcraft_engine::doc::PlaneRef::Construction{feature},..} if *feature==construction)
    );
    adapter.execute("sketch.line", json!({"points":[[1,2],[11,2]]})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    adapter.execute("sketch.edit", json!({"sketch":sketch})).unwrap();
    assert_eq!(adapter.sketch_snapshot().unwrap().plane, preview);
    assert!(adapter.workspace(&[], false, false)["planes"].as_array().unwrap().iter().any(|p| p["name"] == "Tilted"));
}

fn modelling_volume(adapter: &SessionAdapter) -> f64 {
    assert!(adapter.session.model.results.iter().all(|result| result.error.is_none() && !result.stale));
    adapter.session.world_state().unwrap().bodies.iter().map(|body| body.measure().unwrap().volume).sum()
}

fn modelling_commit(adapter: &mut SessionAdapter, edit: &ModelEdit, params: Value) -> Arc<SceneSnapshot> {
    let history = adapter.session.undo.len();
    let prepared = adapter.prepare_model(edit, params, true).unwrap();
    assert_eq!(adapter.session.undo.len(), history, "preparing a feature cannot alter authoritative history");
    let model = adapter.accept_model(edit, prepared).unwrap();
    assert_eq!(adapter.session.undo.len(), history + 1, "one completed feature is one native undo action");
    model
}

fn attachment_repair_fixture(changed: bool) -> (SessionAdapter, u64) {
    let mut adapter = SessionAdapter::new();
    adapter.execute("sketch.create", json!({"plane":"XY","name":"Base outline"})).unwrap();
    adapter.execute("sketch.rectangle.two_point", json!({"p0":[0,0],"p1":[40,20]})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    adapter.execute("solid.extrude", json!({"distance":"10 mm","name":"Extrude1","body_name":"Base"})).unwrap();
    let sketch =
        adapter.execute("sketch.create", json!({"plane":{"face":[10,10,10]},"name":"Stand Base","project_edges":false})).unwrap().value["sketch"]
            .as_u64()
            .unwrap();
    adapter.execute("sketch.rectangle.two_point", json!({"p0":[2,2],"p1":[8,8]})).unwrap();
    adapter.execute("sketch.dimension", json!({"entities":["l1"],"value":"6 mm"})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    if changed {
        adapter.execute("sketch.edit", json!({"sketch":"Base outline"})).unwrap();
        adapter.execute("sketch.delete", json!({"entities":["l2"]})).unwrap();
        adapter.execute("sketch.line", json!({"points":[[40,0],[40,20]]})).unwrap();
        adapter.execute("sketch.finish", json!({})).unwrap();
        // Explicitly repair the base extrusion's profile first, leaving only
        // the dependent sketch's changed face certificate to be rebound.
        adapter.execute("timeline.edit", json!({"feature":"Extrude1","set":{"profiles":[{"point":[10,10]}]}})).unwrap();
        assert!(adapter.session.model.result(sketch).unwrap().warning.as_ref().unwrap().contains("the face this sketch sits on"));
        assert!(adapter.session.model.results.iter().filter(|r| r.name != "Stand Base").all(|r| r.warning.is_none()));
    }
    (adapter, sketch)
}

fn attachment_top(edit: &ModelEdit) -> Value {
    edit.fields[0]["options"]
        .as_array()
        .unwrap()
        .iter()
        .find(|option| option["value"]["face"][2].as_f64().is_some_and(|z| (z - 10.).abs() < 1e-8))
        .unwrap()["value"]
        .clone()
}

#[test]
fn modelling_sketch_attachment_repairs_changed_cap_with_preview_undo_roundtrip_and_extrusion() {
    let (mut adapter, sketch) = attachment_repair_fixture(true);
    let original = adapter.session.doc.clone();
    let geometry = adapter.session.doc.sketch(sketch).unwrap().clone();
    let ids: Vec<_> = adapter.session.doc.features.iter().map(|f| f.id).collect();
    let undo = adapter.session.undo.len();
    let plane = adapter.session.world_state().unwrap().sketch(sketch).unwrap().plane;
    let edit = adapter.begin_model(890, Some("sketch.redefine"), Some(sketch), &[]).unwrap();
    assert!(edit.repair_source.is_some());
    assert!(edit.params["plane"].is_null(), "opening a repair must not silently choose a face");
    assert!(adapter.prepare_model(&edit, edit.params.clone(), true).is_err());
    let mut params = edit.params.clone();
    params["plane"] = attachment_top(&edit);
    // JSONDecoder/JSONSerialization on the native host normalise 10.0 to 10.
    params["plane"]["face"][2] = json!(10);
    let preview = adapter.prepare_model(&edit, params.clone(), false).unwrap();
    assert!((preview.model.volume_mm3 - 8000.).abs() < 1e-5);
    assert!(Arc::ptr_eq(&original, &adapter.session.doc));
    assert_eq!(adapter.session.undo.len(), undo);
    modelling_commit(&mut adapter, &edit, params);
    assert_eq!(adapter.session.doc.marker, original.marker);
    assert_eq!(adapter.session.doc.features.iter().map(|f| f.id).collect::<Vec<_>>(), ids);
    assert_eq!(*adapter.session.doc.sketch(sketch).unwrap(), geometry, "authored geometry and dimensions survive the repair");
    assert_eq!(adapter.session.world_state().unwrap().sketch(sketch).unwrap().plane, plane);
    assert!(adapter.session.model.results.iter().all(|r| r.error.is_none() && r.warning.is_none()));
    adapter.execute("edit.undo", json!({})).unwrap();
    assert_eq!(*adapter.session.doc, *original);
    adapter.execute("edit.redo", json!({})).unwrap();
    let mut reopened = modelling_roundtrip(&adapter);
    let extrude = reopened.begin_model(891, Some("solid.extrude"), None, &[Sel::Feature { id: sketch }]).unwrap();
    let mut params = extrude.params.clone();
    params["distance"] = json!("18.8 mm");
    modelling_commit(&mut reopened, &extrude, params);
    assert!((modelling_volume(&reopened) - (8000. + 36. * 18.8)).abs() < 1e-5);
}

#[test]
fn modelling_sketch_attachment_cancel_and_exact_selection_reject_invalid_or_future_faces() {
    for changed in [false, true] {
        let (mut adapter, sketch) = attachment_repair_fixture(changed);
        adapter.execute("solid.box", json!({"length":2,"width":2,"height":2,"corner":[60,0,0],"body_name":"Future"})).unwrap();
        let original = adapter.session.doc.clone();
        let mut viewport = Viewport::default();
        viewport.set_metrics(metrics(800., 600., 2.)).unwrap();
        let original_scene = adapter.snapshot().unwrap();
        viewport.begin_update(1);
        assert!(viewport.install(1, original_scene.clone()));
        if changed {
            viewport.begin_update(2);
            viewport.fail_update(2);
        }
        let edit = adapter.begin_model(892, Some("sketch.redefine"), Some(sketch), &[]).unwrap();
        assert!(edit.fields[0]["options"].as_array().unwrap().iter().all(|option| option["value"]["body"] != "Future"));
        for plane in [Value::Null, json!({"face":[10,10,10]}), json!({"body":"Future","index":0,"face":[60,0,0],"path":[]})] {
            assert!(adapter.prepare_model(&edit, json!({"plane":plane}), true).is_err());
        }
        let top = attachment_top(&edit);
        let selected = Sel::Face {
            body: "Base".into(),
            index: top["index"].as_u64().unwrap() as usize,
            point: serde_json::from_value(top["face"].clone()).unwrap(),
        };
        assert_eq!(edit.fields[0]["kind"], "face_pick");
        assert_eq!(edit.selection_field.as_deref(), Some("plane"), "repair starts ready for a visual face pick");
        assert!(edit.fields[0]["options"].as_array().unwrap().iter().all(|option| option["label"] == "ui.ipados.attachment.selected_face"));
        let mut selecting = edit.clone();
        selecting.selection_field = Some("plane".into());
        let refreshed = adapter.refresh_model(&selecting, std::slice::from_ref(&selected), None).unwrap();
        assert_eq!(refreshed.params["plane"], top);
        assert!(adapter.refresh_model(&refreshed, &[], None).unwrap().params["plane"].is_null());
        viewport.begin_model(edit).unwrap();
        viewport.select_model_inputs(892, Some("plane")).unwrap();
        viewport.command("view.top", &json!({})).unwrap();
        viewport.command("view.fit", &json!({})).unwrap();
        let position = point(&viewport, serde_json::from_value(top["face"].clone()).unwrap());
        for id in 1..=2 {
            for phase in [InputPhase::Began, InputPhase::Ended] {
                let input = sample(&viewport, InputSource::Pencil, phase, id, position, id as f64);
                viewport.input(input).unwrap();
            }
            assert!(matches!(viewport.selection.as_slice(), [Sel::Face { .. }]), "repair picks one face even with Multiple enabled");
        }
        let current = viewport.model_edit(892).unwrap();
        let current = adapter.refresh_model(&current, viewport.selection(), None).unwrap();
        assert_eq!(current.params["plane"], top, "the real viewport selection reaches the repair form");
        viewport.update_model(current.clone()).unwrap();
        let preview = adapter.prepare_model(&current, current.params.clone(), false).unwrap();
        let chosen = viewport.selection.clone();
        viewport.selection.clear();
        assert_eq!(viewport.publish_model(892, &preview).unwrap_err().code, "stale_preview", "late results cannot consume a newer face pick");
        viewport.selection = chosen;
        viewport.publish_model(892, &preview).unwrap();
        viewport.select_model_inputs(892, Some("plane")).unwrap();
        assert!(viewport.model_edit(892).unwrap().params["plane"].is_null(), "changing the face clears the previous input and preview");
        assert!(!viewport.diagnostic()["preview"].as_bool().unwrap());
        viewport.finish_model(892, None).unwrap();
        assert_eq!(viewport.is_updating(), changed, "cancel restores the original scene's validity");
        if !changed {
            assert!(Arc::ptr_eq(viewport.model.as_ref().unwrap(), &original_scene));
        }
        assert!(Arc::ptr_eq(&original, &adapter.session.doc));
        adapter.execute("body.rename", json!({"body":"Future","name":"Later"})).unwrap();
        assert!(adapter.prepare_model(&refreshed, refreshed.params.clone(), true).is_err(), "a document change invalidates old repair choices");
    }
}

#[test]
fn modelling_sketch_attachment_visual_pick_has_no_dropdown_face_count_limit() {
    let mut adapter = SessionAdapter::new();
    for index in 0..44 {
        adapter.execute("solid.box", json!({"length":2,"width":2,"height":2,"corner":[index*4,0,0],"body_name":format!("Part{index}")})).unwrap();
    }
    let sketch =
        adapter.execute("sketch.create", json!({"plane":{"face":[173,1,2]},"project_edges":false})).unwrap().value["sketch"].as_u64().unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    let edit = adapter.begin_model(893, Some("sketch.redefine"), Some(sketch), &[]).unwrap();
    assert!(edit.fields[0]["options"].as_array().unwrap().len() > 256);
    let top = edit.fields[0]["options"]
        .as_array()
        .unwrap()
        .iter()
        .find(|option| option["value"]["body"] == "Part43" && option["value"]["face"][2] == 2.0)
        .unwrap()["value"]
        .clone();
    let selection = Sel::Face {
        body: "Part43".into(),
        index: top["index"].as_u64().unwrap() as usize,
        point: serde_json::from_value(top["face"].clone()).unwrap(),
    };
    let refreshed = adapter.refresh_model(&edit, &[selection], None).unwrap();
    let prepared = adapter.prepare_model(&refreshed, refreshed.params.clone(), false).unwrap();
    assert!((prepared.model.volume_mm3 - 44. * 8.).abs() < 1e-5);
}

fn modelling_plate(width: f64, depth: &str) -> (SessionAdapter, u64, String, u64) {
    let mut adapter = SessionAdapter::new();
    let sketch = adapter.execute("sketch.create", json!({"plane":"XY","name":"Parametric plate sketch"})).unwrap().value["sketch"].as_u64().unwrap();
    let rectangle = adapter.execute("sketch.rectangle.two_point", json!({"p0":[0,0],"p1":[width,30]})).unwrap().value;
    let bottom = rectangle["sides"]["bottom"].as_str().unwrap();
    let right = rectangle["sides"]["right"].as_str().unwrap();
    numeric_apply(&mut adapter, &curve_selection(sketch, bottom), Some("length"), &format!("{width} mm"));
    numeric_apply(&mut adapter, &curve_selection(sketch, right), Some("length"), "30 mm");
    let state = adapter.sketch_snapshot().unwrap();
    assert!(state.solved && state.dof == 0);
    let width_constraint = state
        .sketch
        .constraints
        .iter()
        .find(|constraint| constraint.kind.is_dimension() && constraint.kind.indices().1.iter().any(|index| state.sketch.curves[*index].id == bottom))
        .unwrap()
        .id
        .clone();
    adapter.execute("sketch.finish", json!({})).unwrap();
    let edit = adapter.begin_model(700, Some("solid.extrude"), None, &[Sel::Feature { id: sketch }]).unwrap();
    let mut params = edit.params.clone();
    params["distance"] = json!(depth);
    params["operation"] = json!("new");
    modelling_commit(&mut adapter, &edit, params);
    let extrude = adapter.session.doc.features.last().unwrap().id;
    (adapter, sketch, width_constraint, extrude)
}

fn modelling_roundtrip(adapter: &SessionAdapter) -> SessionAdapter {
    let bytes = solidcraft_engine::io::write_design_container(&adapter.session.doc).unwrap();
    let doc = solidcraft_engine::io::DesignContainerReader::new(std::io::Cursor::new(bytes)).unwrap().read_document().unwrap();
    let mut loaded = SessionAdapter::new();
    loaded.session = solidcraft_engine::Session::new(doc);
    loaded
}

fn modelling_face(adapter: &SessionAdapter, body: &str, point: Vec3) -> Sel {
    let world = adapter.session.world_state().unwrap();
    let index = solidcraft_engine::doc::appearance::face_index_at(world.body(body).unwrap(), point).unwrap();
    Sel::Face { body: body.into(), index, point }
}

#[test]
fn modelling_subtract_sketch_points_into_stock_and_keeps_manual_direction() {
    for (z, direction) in [(20., "negative"), (0., "positive")] {
        let mut adapter = SessionAdapter::new();
        adapter.execute("solid.box", json!({"length":20,"width":20,"height":20,"body_name":"Stock"})).unwrap();
        let sketch = adapter.execute("sketch.create", json!({"plane":"XY","offset":z})).unwrap().value["sketch"].as_u64().unwrap();
        adapter.execute("sketch.rectangle.two_point", json!({"p0":[2,2],"p1":[8,8]})).unwrap();
        adapter.execute("sketch.finish", json!({})).unwrap();
        let edit = adapter.begin_model(784, Some("solid.extrude"), None, &[Sel::Feature { id: sketch }]).unwrap();
        let source = edit.params["profile"].clone();
        let mut params = edit.params.clone();
        params["operation"] = json!("cut");
        params["targets"] = json!(["Stock"]);
        params["distance"] = json!("5 mm");
        let edit = adapter.refresh_model(&edit, &[], Some(params)).unwrap();
        assert_eq!(edit.params["profile"], source);
        assert_eq!(edit.params["direction"], direction);
        let preview = adapter.prepare_model(&edit, edit.params.clone(), false).unwrap();
        assert!((preview.model.volume_mm3 - 7820.).abs() < 1e-5);
        let mut manual = edit.params.clone();
        manual["direction"] = json!(if direction == "negative" { "positive" } else { "negative" });
        let manual = adapter.refresh_model(&edit, &[], Some(manual.clone())).unwrap();
        assert_ne!(manual.params["direction"], direction);
        modelling_commit(&mut adapter, &edit, edit.params.clone());
        assert!((modelling_volume(&adapter) - 7820.).abs() < 1e-5);
    }
}

#[test]
fn modelling_subtract_target_picks_preserve_the_profile_for_pencil_finger_and_pointer() {
    for device in [InputSource::Pencil, InputSource::Finger, InputSource::Indirect] {
        let mut adapter = SessionAdapter::new();
        adapter.execute("solid.box", json!({"length":20,"width":20,"height":20,"body_name":"Stock"})).unwrap();
        adapter.execute("solid.box", json!({"corner":[30,0,0],"length":20,"width":20,"height":20,"body_name":"Other"})).unwrap();
        let sketch = adapter.execute("sketch.create", json!({"plane":"XY","offset":20})).unwrap().value["sketch"].as_u64().unwrap();
        adapter.execute("sketch.rectangle.two_point", json!({"p0":[2,2],"p1":[8,8]})).unwrap();
        adapter.execute("sketch.finish", json!({})).unwrap();
        let mut viewport = Viewport::default();
        viewport.set_metrics(metrics(800., 600., 1.)).unwrap();
        viewport.begin_update(1);
        viewport.install(1, adapter.snapshot().unwrap());
        viewport.command("view.top", &json!({})).unwrap();
        viewport.command("select.filter", &json!({"filter":"face"})).unwrap();
        let edit = adapter.begin_model(785, Some("solid.extrude"), None, &[Sel::Feature { id: sketch }]).unwrap();
        let mut params = edit.params.clone();
        params["operation"] = json!("cut");
        params["distance"] = json!("3 mm");
        let edit = adapter.refresh_model(&edit, &[], Some(params)).unwrap();
        let source = edit.params["profile"].clone();
        viewport.begin_model(edit).unwrap();
        viewport.select_model_inputs(785, Some("targets")).unwrap();
        let before = viewport.camera;
        for (i, (name, x)) in [("Stock", 10.), ("Other", 40.)].into_iter().enumerate() {
            let tap = point(&viewport, Vec3::new(x, 10., 20.));
            feed(&mut viewport, device, InputPhase::Began, i as u64 + 10, tap, i as f64 + 1.);
            feed(&mut viewport, device, InputPhase::Ended, i as u64 + 10, tap, i as f64 + 1.1);
            assert!(viewport.selection().contains(&Sel::Body { name: name.into() }));
            let edit = viewport.model_edit(785).unwrap();
            let edit = adapter.refresh_model(&edit, viewport.selection(), None).unwrap();
            assert_eq!(edit.params["profile"], source, "target picks must not replace the source");
            assert_eq!(edit.params["distance"], "3 mm");
            viewport.update_model(edit).unwrap();
        }
        let edit = viewport.model_edit(785).unwrap();
        assert_eq!(edit.params["targets"], json!(["Stock", "Other"]));
        assert_eq!(viewport.camera, before);
        assert_eq!(viewport.diagnostic()["filter"], "face", "temporary target picking must preserve the saved filter");
        let prepared = adapter.prepare_model(&edit, edit.params.clone(), false).unwrap();
        assert!((prepared.model.volume_mm3 - (16000. - 108.)).abs() < 1e-5);
        viewport.cancel_model();
        assert!((modelling_volume(&adapter) - 16000.).abs() < 1e-5);
    }
}

#[test]
fn modelling_automatic_subtract_infers_scope_restores_outward_and_keeps_explicit_new() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("solid.box", json!({"length":20,"width":20,"height":20,"body_name":"Stock"})).unwrap();
    adapter.execute("solid.box", json!({"corner":[30,0,0],"length":20,"width":20,"height":20,"body_name":"Other"})).unwrap();
    let sketch = adapter.execute("sketch.create", json!({"plane":"XY","offset":20})).unwrap().value["sketch"].as_u64().unwrap();
    adapter.execute("sketch.rectangle.two_point", json!({"p0":[2,2],"p1":[8,8]})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    let edit = adapter.begin_model(786, Some("solid.extrude"), None, &[Sel::Feature { id: sketch }]).unwrap();
    let mut params = edit.params.clone();
    params["distance"] = json!("-5 mm");
    params["_auto_subtract"] = json!(true);
    let prepared = adapter.prepare_model(&edit, params.clone(), false).unwrap();
    assert_eq!(prepared.params["operation"], "cut");
    assert_eq!(prepared.params["targets"], json!(["Stock"]));
    assert!((prepared.model.volume_mm3 - 15820.).abs() < 1e-5);
    // Subsequent form submissions are based on the published inferred values.
    let current = ModelEdit { params: prepared.params.clone(), ..edit.clone() };
    let mut outward = prepared.params.clone();
    outward["distance"] = json!("5 mm");
    let restored = adapter.prepare_model(&current, outward, false).unwrap();
    assert_eq!(restored.params["operation"], "new");
    assert_eq!(restored.params["targets"], json!([]));
    assert!((restored.model.volume_mm3 - 16180.).abs() < 1e-5);
    params["_auto_subtract"] = json!(false);
    let manual = adapter.prepare_model(&edit, params, false).unwrap();
    assert_eq!(manual.params["operation"], "new");
    assert!((manual.model.volume_mm3 - 16180.).abs() < 1e-5);
    modelling_commit(&mut adapter, &current, prepared.params);
    assert!((modelling_volume(&adapter) - 15820.).abs() < 1e-5);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!((modelling_volume(&adapter) - 16000.).abs() < 1e-5);
    adapter.execute("edit.redo", json!({})).unwrap();
    assert!((modelling_volume(&adapter) - 15820.).abs() < 1e-5);
}

#[test]
fn modelling_automatic_subtract_explicit_targets_replace_scope_and_unchanged_inspection_preserves_it() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("solid.box", json!({"length":20,"width":20,"height":20,"body_name":"Stock"})).unwrap();
    adapter.execute("solid.box", json!({"corner":[0,0,-30],"length":20,"width":20,"height":20,"body_name":"Other"})).unwrap();
    let sketch = adapter.execute("sketch.create", json!({"plane":"XY","offset":20})).unwrap().value["sketch"].as_u64().unwrap();
    adapter.execute("sketch.rectangle.two_point", json!({"p0":[2,2],"p1":[8,8]})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    let history = adapter.session.undo.len();
    let mut viewport = Viewport::default();
    viewport.begin_update(1);
    assert!(viewport.install(1, adapter.snapshot().unwrap()));
    let edit = adapter.begin_model(788, Some("solid.extrude"), None, &[Sel::Feature { id: sketch }]).unwrap();
    viewport.begin_model(edit.clone()).unwrap();
    let mut params = edit.params.clone();
    params["distance"] = json!("-5 mm");
    params["targets"] = json!(["Stock", "Other"]);
    params["_auto_subtract"] = json!(true);
    let prepared = adapter.prepare_model(&edit, params, false).unwrap();
    assert_eq!(prepared.params["targets"], json!(["Stock"]));
    viewport.publish_model(788, &prepared).unwrap();

    // A completed preview may intersect only part of the intended scope. Merely
    // inspecting those inferred targets must keep the farther body eligible.
    let edit = viewport.model_edit(788).unwrap();
    for submitted in [None, Some(edit.params.clone())] {
        let inspected = adapter.refresh_model(&edit, &[], submitted).unwrap();
        assert_eq!(inspected.params["_auto_subtract_scope"], json!(["Stock", "Other"]));
    }
    let mut deeper = edit.params.clone();
    deeper["distance"] = json!("-45 mm");
    let direct = adapter.prepare_model(&edit, deeper.clone(), false).unwrap();
    assert_eq!(direct.params["targets"], json!(["Stock", "Other"]), "direct Preview must preserve an unchanged broader scope");
    assert!((direct.model.volume_mm3 - 14740.).abs() < 1e-5);
    let edit = adapter.refresh_model(&edit, &[], Some(deeper)).unwrap();
    let prepared = adapter.prepare_model(&edit, edit.params.clone(), false).unwrap();
    assert_eq!(prepared.params["targets"], json!(["Stock", "Other"]));
    assert!((prepared.model.volume_mm3 - 14740.).abs() < 1e-5, "the cut removes 36 mm² through 20 + 15 mm of stock");
    viewport.update_model(edit).unwrap();
    viewport.publish_model(788, &prepared).unwrap();

    for (targets, volume) in [(json!(["Stock"]), 15280.), (json!(["Stock", "Other"]), 14740.), (json!(["Other"]), 15460.)] {
        let edit = viewport.model_edit(788).unwrap();
        let mut submitted = edit.params.clone();
        submitted["targets"] = targets.clone();
        // Reference-list edits submit Preview directly, without an inspection.
        let direct = adapter.prepare_model(&edit, submitted.clone(), false).unwrap();
        assert_eq!(direct.params["targets"], targets);
        assert_eq!(direct.params["_auto_subtract_scope"], targets);
        assert!((direct.model.volume_mm3 - volume).abs() < 1e-5);
        let edited = adapter.refresh_model(&edit, &[], Some(submitted)).unwrap();
        assert!(edited.params.get("_auto_subtract_scope").is_none(), "explicit add/remove must replace the cached scope");
        let prepared = adapter.prepare_model(&edited, edited.params.clone(), false).unwrap();
        assert_eq!(prepared.params["targets"], targets);
        assert_eq!(prepared.params["_auto_subtract_scope"], targets);
        assert!((prepared.model.volume_mm3 - volume).abs() < 1e-5);
        viewport.update_model(edited).unwrap();
        viewport.publish_model(788, &prepared).unwrap();
    }
    assert_eq!(adapter.session.undo.len(), history);
    assert!((modelling_volume(&adapter) - 16000.).abs() < 1e-5);
}

#[test]
fn modelling_automatic_subtract_final_drag_preview_publishes_inferred_params_after_release() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("solid.box", json!({"length":20,"width":20,"height":20,"body_name":"Stock"})).unwrap();
    let sketch = adapter.execute("sketch.create", json!({"plane":"XY","offset":20})).unwrap().value["sketch"].as_u64().unwrap();
    adapter.execute("sketch.rectangle.two_point", json!({"p0":[2,2],"p1":[8,8]})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    let mut viewport = Viewport::default();
    viewport.set_metrics(metrics(800., 600., 1.)).unwrap();
    viewport.begin_update(1);
    assert!(viewport.install(1, adapter.snapshot().unwrap()));
    viewport.command("view.isometric", &json!({})).unwrap();
    viewport.command("view.fit", &json!({})).unwrap();
    let edit = adapter.begin_model(789, Some("solid.extrude"), None, &[Sel::Feature { id: sketch }]).unwrap();
    viewport.begin_model(edit.clone()).unwrap();
    let prepared = adapter.prepare_model(&edit, edit.params.clone(), false).unwrap();
    viewport.publish_model(789, &prepared).unwrap();
    let history = adapter.session.undo.len();

    for (index, (depth, operation, targets, volume)) in [(-5., "cut", json!(["Stock"]), 7820.), (5., "new", json!([]), 8180.)].into_iter().enumerate()
    {
        let edit = viewport.model_edit(789).unwrap();
        let start = modelling_handle_position(&viewport, edit.handle.as_ref().unwrap().distance);
        let end = modelling_handle_position(&viewport, depth);
        let time = 1. + index as f64 * 3.;
        feed(&mut viewport, InputSource::Pencil, InputPhase::Began, 10, start, time);
        feed(&mut viewport, InputSource::Pencil, InputPhase::Moved, 10, end, time + 1.);
        let stale = adapter.prepare_tool(viewport.take_tool_work().unwrap()).unwrap();
        feed(&mut viewport, InputSource::Pencil, InputPhase::Ended, 10, end, time + 2.);
        let pending = viewport.model_edit(789).unwrap().params;
        assert!(!viewport.publish_tool_preview(stale), "the final actual sample invalidates the earlier move result");
        assert_eq!(viewport.model_edit(789).unwrap().params, pending);
        assert_eq!(viewport.diagnostic()["input"]["contacts"], 0);
        assert!(!run_tool(&mut adapter, &mut viewport), "release validates a proposal without committing the document");
        let published = viewport.model_edit(789).unwrap();
        assert_eq!(published.params["operation"], operation);
        assert_eq!(published.params["targets"], targets);
        assert_eq!(published.params["_auto_subtract_scope"], json!([]));
        assert_eq!(adapter.model_descriptor(&published)["valid"], true);
        assert!((viewport.snapshot().unwrap().model.volume_mm3 - volume).abs() < 1e-5);
        assert!(viewport.diagnostic()["input"]["edit"].is_null(), "final publication releases the drag");
        assert_eq!(adapter.session.undo.len(), history);
    }
    assert!((modelling_volume(&adapter) - 8000.).abs() < 1e-5);
}

#[test]
fn modelling_inward_handle_drag_switches_to_subtract_and_preference_restores_new_body() {
    for automatic in [true, false] {
        let mut adapter = SessionAdapter::new();
        adapter.execute("solid.box", json!({"length":20,"width":20,"height":20,"body_name":"Stock"})).unwrap();
        let sketch = adapter.execute("sketch.create", json!({"plane":"XY","offset":20})).unwrap().value["sketch"].as_u64().unwrap();
        adapter.execute("sketch.rectangle.two_point", json!({"p0":[2,2],"p1":[8,8]})).unwrap();
        adapter.execute("sketch.finish", json!({})).unwrap();
        let mut viewport = Viewport::default();
        viewport.set_metrics(metrics(800., 600., 1.)).unwrap();
        viewport.begin_update(1);
        viewport.install(1, adapter.snapshot().unwrap());
        viewport.command("view.automatic_subtract", &json!({"enabled":automatic})).unwrap();
        viewport.command("view.isometric", &json!({})).unwrap();
        viewport.command("view.fit", &json!({})).unwrap();
        let edit = adapter.begin_model(787, Some("solid.extrude"), None, &[Sel::Feature { id: sketch }]).unwrap();
        viewport.begin_model(edit.clone()).unwrap();
        let preview = adapter.prepare_model(&edit, edit.params.clone(), false).unwrap();
        viewport.publish_model(787, &preview).unwrap();
        let history = adapter.session.undo.len();
        let start = modelling_handle_position(&viewport, 10.);
        feed(&mut viewport, InputSource::Pencil, InputPhase::Began, 10, start, 1.);
        let end = modelling_handle_position(&viewport, -5.);
        feed(&mut viewport, InputSource::Pencil, InputPhase::Moved, 10, end, 2.);
        run_tool(&mut adapter, &mut viewport);
        let live = viewport.model_edit(787).unwrap();
        assert_eq!(live.params["operation"], if automatic { "cut" } else { "new" });
        assert_eq!(live.params["targets"], if automatic { json!(["Stock"]) } else { json!([]) });
        assert!((viewport.snapshot().unwrap().model.volume_mm3 - if automatic { 7820. } else { 8180. }).abs() < 1e-5);
        // Continue the same contact outwards; automatic inference must not leave
        // a sticky Cut operation or jump the distance handle's origin.
        let outward = modelling_handle_position(&viewport, 5.);
        feed(&mut viewport, InputSource::Pencil, InputPhase::Moved, 10, outward, 3.);
        run_tool(&mut adapter, &mut viewport);
        assert_eq!(viewport.model_edit(787).unwrap().params["operation"], "new");
        feed(&mut viewport, InputSource::Pencil, InputPhase::Ended, 10, outward, 4.);
        run_tool(&mut adapter, &mut viewport);
        assert_eq!(adapter.session.undo.len(), history);
        viewport.cancel_model();
        assert!((modelling_volume(&adapter) - 8000.).abs() < 1e-5);
    }
}

#[test]
fn modelling_face_extrusion_subtract_selection_alone_points_into_the_body_and_removes_material() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("solid.box", json!({"length":10,"width":10,"height":20,"body_name":"Stock"})).unwrap();
    let face = modelling_face(&adapter, "Stock", Vec3::new(2., 2., 20.));
    let edit = adapter.begin_model(783, Some("solid.extrude"), None, std::slice::from_ref(&face)).unwrap();
    let mut params = edit.params.clone();
    params["operation"] = json!("cut");
    let edit = adapter.refresh_model(&edit, std::slice::from_ref(&face), Some(params)).unwrap();
    assert_eq!(edit.params["direction"], "negative", "choosing Subtract must cut into the picked body's face");
    assert_eq!(edit.fields.iter().find(|field| field["key"] == "operation").unwrap()["kind"], "inline_choice");
    let preview = adapter.prepare_model(&edit, edit.params.clone(), false).unwrap();
    assert!((preview.model.volume_mm3 - 1000.).abs() < 1e-5);
    assert!((modelling_volume(&adapter) - 2000.).abs() < 1e-5);
    // Direction remains an explicit input after selecting the operation.
    let mut outward = edit.params.clone();
    outward["direction"] = json!("positive");
    let outward = adapter.refresh_model(&edit, std::slice::from_ref(&face), Some(outward)).unwrap();
    assert_eq!(outward.params["direction"], "positive");
    for operation in ["new", "join", "intersect", "cut"] {
        let mut params = edit.params.clone();
        params["operation"] = json!(operation);
        let changed = adapter.refresh_model(&edit, std::slice::from_ref(&face), Some(params)).unwrap();
        assert_eq!(changed.params["direction"], if matches!(operation, "new" | "join") { "positive" } else { "negative" });
    }
    modelling_commit(&mut adapter, &edit, edit.params.clone());
    assert!((modelling_volume(&adapter) - 1000.).abs() < 1e-5);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!((modelling_volume(&adapter) - 2000.).abs() < 1e-5);
}

#[test]
fn modelling_face_extrusion_offers_four_boolean_results_on_the_picked_body() {
    let expected_options = [
        ("ui.ipados.face_extrude.new_body", "new"),
        ("ui.ipados.face_extrude.union", "join"),
        ("ui.ipados.face_extrude.intersect", "intersect"),
        ("ui.ipados.face_extrude.subtract", "cut"),
    ];
    for (option_index, direction, expected_volume, body_count, min_z, max_z) in [
        (0, "positive", 2500., 3, 0., 10.),
        (1, "positive", 2500., 2, 0., 15.),
        (2, "negative", 1500., 2, 5., 10.),
        (3, "negative", 1500., 2, 0., 5.),
    ] {
        let mut adapter = SessionAdapter::new();
        adapter.execute("solid.box", json!({"length":10,"width":10,"height":10,"body_name":"Stock"})).unwrap();
        adapter.execute("solid.box", json!({"corner":[30,0,0],"length":10,"width":10,"height":10,"body_name":"Other"})).unwrap();
        let face = modelling_face(&adapter, "Stock", Vec3::new(2., 2., 10.));
        let edit = adapter.begin_model(780, Some("solid.extrude"), None, std::slice::from_ref(&face)).unwrap();
        let options = edit.fields.iter().find(|field| field["key"] == "operation").unwrap()["options"].as_array().unwrap();
        assert_eq!(options.len(), 4);
        for (option, (label, value)) in options.iter().zip(expected_options) {
            assert_eq!(option["label"], label);
            assert_eq!(option["value"], value);
        }
        assert_eq!(edit.params["operation"], "new", "New Body stays the initial operation");
        assert_eq!(edit.params["targets"], json!(["Stock"]), "the selected face supplies the Boolean target");
        let mut params = edit.params.clone();
        params["operation"] = options[option_index]["value"].clone();
        params["distance"] = json!("5 mm");
        params["direction"] = json!(direction);
        let edit = adapter.refresh_model(&edit, &[face], Some(params)).unwrap();
        let preview = adapter.prepare_model(&edit, edit.params.clone(), false).unwrap();
        assert!((preview.model.volume_mm3 - expected_volume).abs() < 1e-5);
        let display = preview.model.preview_scene.as_ref().expect("each operation needs distinct provisional feedback");
        assert_eq!(display.names.len(), display.scene.meshes.len());
        let palette = solidcraft_engine::view::Palette::DEFAULT;
        let color = match option_index {
            0 => solidcraft_engine::view::colors::PROFILE,
            1 => palette.axes[1],
            2 => palette.body_selected,
            _ => palette.axes[0],
        };
        assert!(display.scene.meshes.iter().any(|mesh| mesh.color == color));
        if option_index == 3 {
            assert!(display.scene.meshes.iter().any(|mesh| mesh.color == color && mesh.glass()), "removed material must be a see-through red tool");
        }
        if option_index == 2 {
            assert!(display.scene.meshes.iter().any(|mesh| mesh.color == color && !mesh.glass()), "kept intersection must be an opaque blue result");
        }
        assert!((modelling_volume(&adapter) - 2000.).abs() < 1e-5);
        modelling_commit(&mut adapter, &edit, edit.params.clone());
        assert!(adapter.snapshot().unwrap().preview_scene.is_none(), "preview colours must never become saved body appearances");
        let workspace = adapter.workspace(&[], false, false);
        assert!(workspace["rows"].as_array().unwrap().iter().all(|row| row["kind"] != "sketch"), "face extrusion must not add a user sketch");
        assert_eq!(workspace["modelling"]["timeline"].as_array().unwrap().len(), 3, "only the two boxes and extrusion belong in the timeline");
        let world = adapter.session.world_state().unwrap();
        assert_eq!(world.bodies.len(), body_count);
        let bounds = world.body("Stock").unwrap().mesh().bounds();
        assert!((bounds.min.z - min_z).abs() < 1e-8);
        assert!((bounds.max.z - max_z).abs() < 1e-8);
        assert!((world.body("Other").unwrap().measure().unwrap().volume - 1000.).abs() < 1e-5);
        let feature = adapter.session.doc.features.last().unwrap().id;
        let edit = adapter.begin_model(781, None, Some(feature), &[]).unwrap();
        let options = edit.fields.iter().find(|field| field["key"] == "operation").unwrap()["options"].as_array().unwrap();
        assert_eq!(options.len(), 4);
        assert_eq!(options[0]["label"], "ui.ipados.face_extrude.new_body");
        assert_eq!(options[0]["value"], "new_body");
    }
}

#[test]
fn modelling_face_extrusion_keeps_its_boundary_internal_and_authored_face_sketches_available() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("solid.box", json!({"length":10,"width":10,"height":10,"body_name":"Stock"})).unwrap();
    let face = modelling_face(&adapter, "Stock", Vec3::new(2., 2., 10.));
    let Sel::Face { index, point, .. } = &face else { panic!("face") };
    let authored =
        adapter.execute("sketch.create", json!({"plane":{"face":point,"body":"Stock","index":index},"project_edges":false})).unwrap().value["sketch"]
            .as_u64()
            .unwrap();
    adapter.execute("sketch.rectangle.two_point", json!({"p0":[1,1],"p1":[3,3]})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    let history = adapter.session.undo.len();
    let edit = adapter.begin_model(790, Some("solid.extrude"), None, &[face]).unwrap();
    let mut params = edit.params.clone();
    params["distance"] = json!("5 mm");
    params["operation"] = json!("join");
    modelling_commit(&mut adapter, &edit, params);
    assert_eq!(adapter.session.undo.len(), history + 1);
    let extrusion = adapter.session.doc.features.last().unwrap().id;
    let FeatureKind::Extrude { sketch, .. } = adapter.session.doc.feature(extrusion).unwrap().kind else { panic!("extrude") };
    let internal = sketch.0;
    assert!(adapter.session.doc.feature(internal).unwrap().is_internal_profile());
    assert!(!adapter.session.doc.feature(authored).unwrap().is_internal_profile());
    assert!(!solidcraft_engine::view::sketch_shown(&adapter.session, internal));

    let mut loaded = modelling_roundtrip(&adapter);
    assert_eq!(*loaded.session.doc, *adapter.session.doc);
    let workspace = loaded.workspace(&[], false, false);
    let sketches: Vec<_> = workspace["rows"].as_array().unwrap().iter().filter(|row| row["kind"] == "sketch").collect();
    assert_eq!(sketches.len(), 1, "only the explicitly created face sketch is listed");
    assert_eq!(sketches[0]["entity"]["id"], authored);
    let timeline = workspace["modelling"]["timeline"].as_array().unwrap();
    assert_eq!(timeline.iter().map(|row| row["id"].as_u64().unwrap()).collect::<Vec<_>>(), [1, authored, extrusion]);
    assert_eq!(timeline.last().unwrap()["order"], 2, "visible history has no gap for the generated boundary");
    assert_eq!(timeline.last().unwrap()["document_order"], 3, "timeline commands retain actual document positions");
    assert_eq!(workspace["modelling"]["position"], 3);
    let edit = loaded.begin_model(791, Some("solid.extrude"), None, &[]).unwrap();
    let sources = edit.fields.iter().find(|field| field["key"] == "profile").unwrap()["options"].as_array().unwrap();
    assert!(!sources.is_empty());
    assert!(sources.iter().all(|source| source["value"]["sketch"] == authored));
    let edit = loaded.begin_model(792, None, Some(extrusion), &[]).unwrap();
    let mut params = edit.params.clone();
    params["extent"]["distance"] = json!("7 mm");
    modelling_commit(&mut loaded, &edit, params);
    assert!((modelling_volume(&loaded) - 1700.).abs() < 1e-5);
    // Rolling just before the operation must not expose its generated boundary.
    loaded.execute("timeline.roll_to", json!({"position":3})).unwrap();
    assert!(!solidcraft_engine::view::sketch_shown(&loaded.session, internal));
    assert_eq!(loaded.modelling_workspace()["position"], 2, "rollback counts visible features before the operation");
    // Implicit profile commands still use the last authored sketch.
    let reply = adapter.execute("solid.extrude", json!({"distance":2,"operation":"new"})).unwrap().value;
    let FeatureKind::Extrude { sketch, .. } = adapter.session.doc.feature(reply["feature"].as_u64().unwrap()).unwrap().kind else {
        panic!("extrude")
    };
    assert_eq!(sketch.0, authored);
    adapter.execute("edit.undo", json!({})).unwrap();
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!(adapter.session.doc.feature(internal).is_none());
    assert!((modelling_volume(&adapter) - 1000.).abs() < 1e-5);
    adapter.execute("edit.redo", json!({})).unwrap();
    assert!(adapter.session.doc.feature(internal).unwrap().is_internal_profile());
    assert!((modelling_volume(&adapter) - 1500.).abs() < 1e-5);
    // Deletion owns the generated boundary too, in the same undo step. Keeping
    // it would strand a hidden face reference when earlier features are removed.
    let history = adapter.session.undo.len();
    let deleted = adapter.execute("timeline.delete", json!({"feature":extrusion})).unwrap();
    assert!(deleted.value["deleted"].as_array().unwrap().contains(&json!(internal)));
    assert!(adapter.session.doc.feature(internal).is_none());
    assert!(adapter.session.doc.feature(authored).is_some(), "authored face sketches remain independent");
    assert_eq!(adapter.session.undo.len(), history + 1);
    assert!((modelling_volume(&adapter) - 1000.).abs() < 1e-5);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!(adapter.session.doc.feature(internal).is_some());
    assert!((modelling_volume(&adapter) - 1500.).abs() < 1e-5);
    adapter.execute("edit.redo", json!({})).unwrap();
    assert!(adapter.session.doc.feature(internal).is_none());
}

#[test]
fn modelling_face_extrusion_reselection_updates_the_automatic_boolean_target() {
    let mut adapter = SessionAdapter::new();
    for (name, x) in [("First", 0.), ("Second", 30.)] {
        adapter.execute("solid.box", json!({"corner":[x,0,0],"length":10,"width":10,"height":10,"body_name":name})).unwrap();
    }
    let first = modelling_face(&adapter, "First", Vec3::new(2., 2., 10.));
    let second = modelling_face(&adapter, "Second", Vec3::new(32., 2., 10.));
    let edit = adapter.begin_model(782, Some("solid.extrude"), None, &[]).unwrap();
    let mut params = edit.params.clone();
    params["operation"] = json!("join");
    params["distance"] = json!("2 mm");
    let edit = adapter.refresh_model(&edit, &[], Some(params)).unwrap();
    let edit = adapter.refresh_model(&edit, &[first], None).unwrap();
    assert_eq!(edit.params["targets"], json!(["First"]));
    let edit = adapter.refresh_model(&edit, &[second], None).unwrap();
    assert_eq!(edit.params["targets"], json!(["Second"]));
    assert_eq!(edit.params["operation"], "join");
    assert_eq!(edit.params["distance"], "2 mm");
    modelling_commit(&mut adapter, &edit, edit.params.clone());
    let world = adapter.session.world_state().unwrap();
    assert!((world.body("First").unwrap().measure().unwrap().volume - 1000.).abs() < 1e-5);
    assert!((world.body("Second").unwrap().measure().unwrap().volume - 1200.).abs() < 1e-5);
}

#[test]
fn modelling_face_extrusion_uses_picked_body_with_preview_handle_and_native_history() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("solid.box", json!({"length":40,"width":30,"height":10,"body_name":"Stock"})).unwrap();
    let selection = [modelling_face(&adapter, "Stock", Vec3::new(10., 10., 10.))];
    let edit = adapter.begin_model(770, Some("solid.extrude"), None, &selection).unwrap();
    assert_eq!(
        edit.fields.iter().find(|field| field["key"] == "distance").and_then(|field| field["units"].as_str()),
        Some(solidcraft_engine::doc::expr::Kind::Length.default_unit()),
        "distance entry must receive its unit from the native dimension kind"
    );
    assert_eq!(edit.params["profile"]["face_body"], "Stock", "a body face must seed the extrusion source");
    let mut params = edit.params.clone();
    params["distance"] = json!("5 mm");
    params["operation"] = json!("join");
    params["targets"] = json!(["Stock"]);
    let history = adapter.session.undo.len();
    let before = adapter.session.doc.to_json();
    let preview = adapter.prepare_model(&edit, params.clone(), false).unwrap();
    assert!((preview.model.volume_mm3 - 18_000.).abs() < 1e-5);
    let handle = preview.handle.as_ref().expect("a planar face supplies a distance handle");
    assert!(handle.normal.dist(Vec3::Z) < 1e-8);
    assert!((handle.point().z - 15.).abs() < 1e-8, "the handle must stay on the source face, not the preview's moved cap");
    assert_eq!(adapter.session.doc.to_json(), before);
    assert_eq!(adapter.session.undo.len(), history);
    modelling_commit(&mut adapter, &edit, params);
    let feature = adapter.session.doc.features.last().unwrap().id;
    assert_eq!(adapter.session.model.state().bodies.len(), 1);
    assert!((modelling_volume(&adapter) - 18_000.).abs() < 1e-5);
    let mut loaded = modelling_roundtrip(&adapter);
    let edit = loaded.begin_model(771, None, Some(feature), &[]).unwrap();
    let mut params = edit.params.clone();
    params["extent"]["distance"] = json!("7 mm");
    modelling_commit(&mut loaded, &edit, params);
    assert_eq!(loaded.session.doc.features.last().unwrap().id, feature);
    assert!((modelling_volume(&loaded) - 20_400.).abs() < 1e-5);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!((modelling_volume(&adapter) - 12_000.).abs() < 1e-5);
    adapter.execute("edit.redo", json!({})).unwrap();
    assert!((modelling_volume(&adapter) - 18_000.).abs() < 1e-5);
}

#[test]
fn modelling_face_extrusion_tool_first_reselection_preserves_values_and_switches_between_sources() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("solid.box", json!({"length":40,"width":30,"height":10,"body_name":"Stock"})).unwrap();
    let sketch = adapter.execute("sketch.create", json!({"plane":"XY"})).unwrap().value["sketch"].as_u64().unwrap();
    adapter.execute("sketch.rectangle.two_point", json!({"p0":[50,50],"p1":[60,60]})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    let face = modelling_face(&adapter, "Stock", Vec3::new(10., 10., 10.));
    let edit = adapter.begin_model(772, Some("solid.extrude"), None, &[]).unwrap();
    assert!(adapter.prepare_model(&edit, edit.params.clone(), false).is_err(), "an empty source must not fall back to the last sketch");
    let mut params = edit.params.clone();
    params["distance"] = json!("5 mm");
    let edit = adapter.refresh_model(&edit, &[], Some(params)).unwrap();
    let edit = adapter.refresh_model(&edit, std::slice::from_ref(&face), None).unwrap();
    assert_eq!(edit.params["distance"], "5 mm");
    assert_eq!(edit.params["operation"], "new");
    assert_eq!(edit.params["profile"]["face_body"], "Stock");
    let source = edit.fields.iter().find(|field| field["key"] == "profile").unwrap();
    assert_eq!(source["label"], "ui.ipados.face_extrude.source");
    assert!(source["options"].as_array().unwrap().iter().any(|option| option["value"] == edit.params["profile"]));
    let preview = adapter.prepare_model(&edit, edit.params.clone(), false).unwrap();
    assert!((preview.model.volume_mm3 - 18_000.).abs() < 1e-5);
    let edit = adapter.refresh_model(&edit, &[Sel::Profile { sketch, index: 0 }], None).unwrap();
    assert_eq!(edit.params["profile"]["sketch"], sketch);
    assert!(edit.params["profile"].get("faces").is_none());
    assert_eq!(edit.params["distance"], "5 mm");
    let preview = adapter.prepare_model(&edit, edit.params.clone(), false).unwrap();
    assert!((preview.model.volume_mm3 - 12_500.).abs() < 1e-5);
    let edit = adapter.refresh_model(&edit, &[face], None).unwrap();
    let preview = adapter.prepare_model(&edit, edit.params.clone(), false).unwrap();
    assert!((preview.model.volume_mm3 - 18_000.).abs() < 1e-5);
    assert!((modelling_volume(&adapter) - 12_000.).abs() < 1e-5);
}

#[test]
fn modelling_face_extrusion_cut_and_symmetric_extents_follow_the_face_normal() {
    for (operation, direction, expected_volume, endpoint) in [("cut", "negative", 6000., 5.), ("new", "symmetric", 18_000., 12.5)] {
        let mut adapter = SessionAdapter::new();
        adapter.execute("solid.box", json!({"length":40,"width":30,"height":10,"body_name":"Stock"})).unwrap();
        let face = modelling_face(&adapter, "Stock", Vec3::new(10., 10., 10.));
        let edit = adapter.begin_model(773, Some("solid.extrude"), None, &[face]).unwrap();
        let mut params = edit.params.clone();
        params["distance"] = json!("5 mm");
        params["direction"] = json!(direction);
        params["operation"] = json!(operation);
        params["targets"] = json!(["Stock"]);
        let preview = adapter.prepare_model(&edit, params.clone(), false).unwrap();
        assert!((preview.model.volume_mm3 - expected_volume).abs() < 1e-5);
        assert!((preview.handle.as_ref().unwrap().point().z - endpoint).abs() < 1e-8);
        modelling_commit(&mut adapter, &edit, params);
        assert!((modelling_volume(&adapter) - expected_volume).abs() < 1e-5);
    }
}

#[test]
fn modelling_face_extrusion_groups_coplanar_faces_and_rejects_invalid_sources_without_mutation() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("solid.box", json!({"length":10,"width":10,"height":10,"body_name":"Stock"})).unwrap();
    let sketch = adapter.execute("sketch.create", json!({"plane":"XY","offset":"10 mm"})).unwrap().value["sketch"].as_u64().unwrap();
    adapter.execute("sketch.rectangle.two_point", json!({"p0":[4,-1],"p1":[6,11]})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    adapter.execute("solid.extrude", json!({"sketch":sketch,"distance":5,"direction":"negative","operation":"cut","targets":["Stock"]})).unwrap();
    adapter.execute("solid.cylinder", json!({"radius":5,"height":10,"base":[30,0,0],"body_name":"Cylinder"})).unwrap();
    let top = modelling_face(&adapter, "Stock", Vec3::new(2., 5., 10.));
    let other_top = modelling_face(&adapter, "Stock", Vec3::new(8., 5., 10.));
    let curve = modelling_face(&adapter, "Cylinder", Vec3::new(35., 0., 5.));
    let history = adapter.session.undo.len();
    let before = adapter.session.doc.to_json();
    let valid = adapter.begin_model(774, Some("solid.extrude"), None, &[top.clone(), other_top]).unwrap();
    assert_eq!(valid.params["profile"]["faces"].as_array().unwrap().len(), 2);
    let mut params = valid.params.clone();
    params["distance"] = json!("2 mm");
    // The two cap regions total 80 mm²; the channel removed 100 mm³.
    let preview = adapter.prepare_model(&valid, params, false).unwrap();
    let before_volume = adapter.snapshot().unwrap().volume_mm3;
    assert!((preview.model.volume_mm3 - before_volume - 160.).abs() < 1e-5);
    let invalid = adapter.begin_model(775, Some("solid.extrude"), None, std::slice::from_ref(&curve)).unwrap();
    assert!(adapter.prepare_model(&invalid, invalid.params.clone(), false).is_err());
    assert!(adapter.begin_model(776, Some("solid.extrude"), None, &[top.clone(), curve]).is_err());
    assert!(adapter.begin_model(777, Some("solid.extrude"), None, &[top, Sel::Profile { sketch, index: 0 }]).is_err());
    for source in [
        json!({}),
        json!({"faces":[],"face_body":"Stock"}),
        json!({"faces":[[1,2]],"face_body":"Stock"}),
        json!({"faces":[[1,2,10]],"face_body":"Missing"}),
        json!({"faces":[[1000,1000,1000]],"face_body":"Stock"}),
    ] {
        let mut params = valid.params.clone();
        params["profile"] = source;
        assert!(adapter.prepare_model(&valid, params, false).is_err());
    }
    assert_eq!(adapter.session.undo.len(), history);
    assert_eq!(adapter.session.doc.to_json(), before);
}

#[test]
fn modelling_face_extrusion_uses_each_plane_normal_with_atomic_preview_commit_and_undo() {
    for (operation, direction, expected) in [("new", "positive", 1400.), ("join", "positive", 1400.), ("cut", "negative", 640.)] {
        let mut adapter = SessionAdapter::new();
        adapter.execute("solid.box", json!({"length":10,"width":10,"height":10,"body_name":"Stock"})).unwrap();
        let x = modelling_face(&adapter, "Stock", Vec3::new(10., 5., 5.));
        let y = modelling_face(&adapter, "Stock", Vec3::new(5., 10., 5.));
        let selection = [x.clone(), y, x]; // Repeated picks must not extrude a face twice.
        let before = adapter.session.doc.to_json();
        let history = adapter.session.undo.len();
        let edit = adapter.begin_model(774, Some("solid.extrude"), None, &selection).unwrap();
        let mut params = edit.params.clone();
        params["distance"] = json!("2 mm");
        params["operation"] = json!(operation);
        params["direction"] = json!(direction);
        params["targets"] = if operation == "new" { json!([]) } else { json!(["Stock"]) };
        let preview = adapter.prepare_model(&edit, params.clone(), false).unwrap();
        assert!((preview.model.volume_mm3 - expected).abs() < 1e-5, "{operation}: {}", preview.model.volume_mm3);
        if operation != "new" {
            let display = preview.model.preview_scene.as_ref().unwrap();
            assert_eq!(display.names.iter().filter(|name| name.is_empty()).count(), 2, "both normal directions need tool feedback");
        }
        assert_eq!(adapter.session.doc.to_json(), before);
        assert_eq!(adapter.session.undo.len(), history);
        let committed = modelling_commit(&mut adapter, &edit, params);
        assert!((committed.volume_mm3 - expected).abs() < 1e-5);
        let state = adapter.session.model.state();
        let planes: Vec<_> = state.sketches.iter().map(|sketch| sketch.plane.normal()).collect();
        assert!(planes.iter().any(|normal| normal.dist(Vec3::X) < 1e-8));
        assert!(planes.iter().any(|normal| normal.dist(Vec3::Y) < 1e-8));
        let bounds = committed.bounds;
        let end = if operation == "cut" { 8. } else { 12. };
        assert!((bounds.max.x - end).abs() < 1e-5 && (bounds.max.y - end).abs() < 1e-5, "{operation}: {bounds:?}");
        assert!((bounds.max.z - 10.).abs() < 1e-5);
        let reopened = modelling_roundtrip(&adapter);
        assert!((modelling_volume(&reopened) - expected).abs() < 1e-5);
        adapter.execute("edit.undo", json!({})).unwrap();
        assert_eq!(adapter.session.doc.to_json(), before);
        adapter.execute("edit.redo", json!({})).unwrap();
        assert!((modelling_volume(&adapter) - expected).abs() < 1e-5);
    }
}

#[test]
fn modelling_face_extrusion_keeps_opposite_normals_separate_and_infers_auto_before_cutting() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("solid.box", json!({"length":10,"width":10,"height":10,"body_name":"Stock"})).unwrap();
    let reply = adapter
        .execute(
            "solid.extrude",
            json!({"faces":[[10,5,5],[0,5,5],[5,10,5],[5,0,5]],
        "face_body":"Stock","distance":2,"operation":"new"}),
        )
        .unwrap()
        .value;
    assert_eq!(reply["created_bodies"].as_array().unwrap().len(), 4);
    let model = adapter.snapshot().unwrap();
    assert!((model.volume_mm3 - 1800.).abs() < 1e-5);
    assert!(model.bounds.min.dist(Vec3::new(-2., -2., 0.)) < 1e-5);
    assert!(model.bounds.max.dist(Vec3::new(12., 12., 10.)) < 1e-5);
    adapter.execute("edit.undo", json!({})).unwrap();
    adapter
        .execute(
            "solid.extrude",
            json!({"faces":[[10,5,5],[5,10,5]],"face_body":"Stock",
        "distance":6,"direction":"negative","operation":"auto","targets":["Stock"]}),
        )
        .unwrap();
    // The first cut removes the second pick. Both operations still use the
    // original face: remaining stock is 4 × 4 × 10, rather than a new second body.
    assert!((modelling_volume(&adapter) - 160.).abs() < 1e-5);
    assert_eq!(adapter.snapshot().unwrap().body_count, 1);
}

#[test]
fn modelling_face_extrusion_rolls_back_when_a_later_plane_fails() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("solid.box", json!({"length":10,"width":10,"height":10,"body_name":"Stock"})).unwrap();
    let before = adapter.session.doc.to_json();
    let history = adapter.session.undo.len();
    // X reaches the target, but Y would have zero extent. Neither may be committed.
    assert!(
        adapter
            .execute(
                "solid.extrude",
                json!({"faces":[[10,5,5],[5,10,5]],"face_body":"Stock",
        "to":[12,10,5],"operation":"join","targets":["Stock"]})
            )
            .is_err()
    );
    assert_eq!(adapter.session.doc.to_json(), before);
    assert_eq!(adapter.session.undo.len(), history);
    assert!((modelling_volume(&adapter) - 1000.).abs() < 1e-5);
}

#[test]
fn modelling_face_extrusion_keeps_translated_coincident_body_scope_and_side_normal() {
    let mut adapter = SessionAdapter::new();
    for body in ["Stock", "Neighbor"] {
        adapter.execute("solid.box", json!({"corner":[80,40,30],"length":40,"width":30,"height":10,"body_name":body})).unwrap();
    }
    let selection = [modelling_face(&adapter, "Stock", Vec3::new(120., 50., 35.))];
    let edit = adapter.begin_model(778, Some("solid.extrude"), None, &selection).unwrap();
    let mut params = edit.params.clone();
    params["operation"] = json!("join");
    params["targets"] = json!(["Stock"]);
    params["distance"] = json!("5 mm");
    let preview = adapter.prepare_model(&edit, params.clone(), false).unwrap();
    let handle = preview.handle.as_ref().unwrap();
    assert!(handle.normal.dist(Vec3::X) < 1e-8);
    assert!((handle.point().x - 125.).abs() < 1e-8);
    modelling_commit(&mut adapter, &edit, params);
    let world = adapter.session.world_state().unwrap();
    assert!((world.body("Stock").unwrap().measure().unwrap().volume - 13_500.).abs() < 1e-5);
    assert!((world.body("Neighbor").unwrap().measure().unwrap().volume - 12_000.).abs() < 1e-5);
}

#[test]
fn modelling_face_deselection_moves_the_rendered_extrusion_handle() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("solid.box", json!({"length":10,"width":10,"height":10,"body_name":"Stock"})).unwrap();
    let sketch = adapter.execute("sketch.create", json!({"plane":"XY","offset":"10 mm"})).unwrap().value["sketch"].as_u64().unwrap();
    adapter.execute("sketch.rectangle.two_point", json!({"p0":[4,-1],"p1":[6,11]})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    adapter.execute("solid.extrude", json!({"sketch":sketch,"distance":5,"direction":"negative","operation":"cut","targets":["Stock"]})).unwrap();
    let first = modelling_face(&adapter, "Stock", Vec3::new(2., 5., 10.));
    let remaining = modelling_face(&adapter, "Stock", Vec3::new(8., 5., 10.));
    let mut viewport = Viewport::default();
    viewport.set_metrics(metrics(800., 600., 2.)).unwrap();
    viewport.begin_update(1);
    viewport.install(1, adapter.snapshot().unwrap());
    viewport.command("view.isometric", &json!({})).unwrap();
    viewport.select(Some(first.clone()), false);
    viewport.select(Some(remaining.clone()), true);
    let edit = adapter.begin_model(779, Some("solid.extrude"), None, viewport.selection()).unwrap();
    let old_point = edit.handle.as_ref().unwrap().point();
    viewport.begin_model(edit.clone()).unwrap();
    viewport.select(Some(first), true);
    assert_eq!(viewport.selection(), &[remaining]);
    let before = viewport.snapshot().unwrap();
    let updated = adapter.refresh_model(&edit, viewport.selection(), None).unwrap();
    let new_point = updated.handle.as_ref().unwrap().point();
    assert!(old_point.dist(new_point) > 1.);
    viewport.update_model(updated).unwrap();
    let after = viewport.snapshot().unwrap();
    assert!(after.highlight_revision > before.highlight_revision);
    assert!(
        after
            .highlight
            .lines
            .iter()
            .any(|line| { line.on_top && line.points.len() == 2 && ((line.points[0] + line.points[1]) * 0.5).dist(new_point) < 1e-8 }),
        "the rendered arrow must be centered on the remaining face's handle"
    );
    viewport.changed();
    assert_eq!(viewport.snapshot().unwrap().highlight_revision, after.highlight_revision, "an unchanged handle should reuse its drawing");
}

#[test]
fn modelling_face_taps_at_different_positions_and_on_handle_toggle_selection() {
    for source in [InputSource::Pencil, InputSource::Indirect] {
        let mut adapter = SessionAdapter::new();
        adapter.execute("solid.box", json!({"length":40,"width":30,"height":10,"body_name":"Stock"})).unwrap();
        let mut viewport = Viewport::default();
        viewport.set_metrics(metrics(800., 600., 2.)).unwrap();
        viewport.begin_update(1);
        viewport.install(1, adapter.snapshot().unwrap());
        viewport.command("view.isometric", &json!({})).unwrap();
        viewport.command("select.filter", &json!({"filter":"face"})).unwrap();
        for (index, position) in [Vec3::new(20., 15., 10.), Vec3::new(22., 18., 10.)].into_iter().enumerate() {
            let tap = point(&viewport, position);
            for phase in [InputPhase::Began, InputPhase::Ended] {
                feed(&mut viewport, source, phase, 90, tap, 1. + index as f64);
            }
            assert!(matches!(viewport.selection(), [Sel::Face { body, .. }] if body == "Stock"));
            let edit = adapter.begin_model(779, Some("solid.extrude"), None, viewport.selection()).unwrap();
            viewport.begin_model(edit).unwrap();
            let handle = viewport.modelling.as_ref().unwrap().handle.as_ref().unwrap();
            let tap = point(&viewport, handle.point());
            assert!(matches!(viewport.pick(tap, Some(viewport.model.as_ref().unwrap().geometry_revision)), Some(Sel::Face { .. })));
            for phase in [InputPhase::Began, InputPhase::Ended] {
                feed(&mut viewport, source, phase, 91, tap, 3. + index as f64);
            }
            assert!(viewport.selection().is_empty(), "stationary {source:?} handle capture swallowed face deselection");
            assert!(viewport.take_tool_work().is_none(), "a tap created modelling work");
            viewport.cancel_model();
        }
        assert_eq!(adapter.session.undo.len(), 1, "selection must not edit the document");
    }
}

#[test]
fn modelling_face_extrusion_pencil_handle_keeps_its_source_during_inspection_and_cancel() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("solid.box", json!({"length":40,"width":30,"height":10,"body_name":"Stock"})).unwrap();
    let mut viewport = Viewport::default();
    viewport.set_metrics(metrics(800., 600., 2.)).unwrap();
    viewport.begin_update(1);
    viewport.install(1, adapter.snapshot().unwrap());
    viewport.command("view.top", &json!({})).unwrap();
    viewport.command("select.filter", &json!({"filter":"face"})).unwrap();
    let tap = point(&viewport, Vec3::new(10., 10., 10.));
    feed(&mut viewport, InputSource::Pencil, InputPhase::Began, 90, tap, 1.);
    feed(&mut viewport, InputSource::Pencil, InputPhase::Ended, 90, tap, 1.1);
    assert!(matches!(viewport.selection(), [Sel::Face { body, .. }] if body == "Stock"));
    let selection = viewport.selection().to_vec();
    let edit = adapter.begin_model(779, Some("solid.extrude"), None, &selection).unwrap();
    viewport.begin_model(edit.clone()).unwrap();
    let preview = adapter.prepare_model(&edit, edit.params.clone(), false).unwrap();
    viewport.publish_model(779, &preview).unwrap();
    viewport.command("view.isometric", &json!({})).unwrap();
    viewport.command("view.fit", &json!({})).unwrap();
    let camera = viewport.camera;
    let history = adapter.session.undo.len();
    let start = modelling_handle_position(&viewport, 10.);
    feed(&mut viewport, InputSource::Pencil, InputPhase::Began, 91, start, 2.);
    for (index, distance) in [12., 15., 7.].into_iter().enumerate() {
        let end = modelling_handle_position(&viewport, distance);
        feed(&mut viewport, InputSource::Pencil, InputPhase::Moved, 91, end, 3. + index as f64);
        let work = viewport.take_tool_work().unwrap();
        let prepared = adapter.prepare_tool(work.clone()).unwrap();
        assert!((prepared.model.volume_mm3 - 1200. * (10. + distance)).abs() < 1e-5);
        viewport.publish_model_tool_details(&work, prepared.reply, prepared.feedback, prepared.handle);
        assert!(viewport.publish_preview(&work, prepared.model));
        let current = viewport.model_edit(779).unwrap();
        let refreshed = adapter.refresh_model(&current, &selection, None).unwrap();
        assert_eq!(refreshed.version, current.version);
        assert!((refreshed.handle.as_ref().unwrap().origin.z - 10.).abs() < 1e-8);
        viewport.update_model(refreshed).unwrap();
    }
    assert_eq!(viewport.camera, camera);
    viewport.finish_model(779, None).unwrap();
    assert_eq!(adapter.session.undo.len(), history);
    assert!((modelling_volume(&adapter) - 12_000.).abs() < 1e-5);
    assert!((viewport.snapshot().unwrap().model.volume_mm3 - 12_000.).abs() < 1e-5);
}

#[test]
fn modelling_editable_extrusion_regenerates_same_feature_chain_and_native_container() {
    let (mut adapter, sketch, width, extrude) = modelling_plate(40.0, "10 mm");
    let ids: Vec<_> = adapter.session.doc.features.iter().map(|feature| feature.id).collect();
    assert_eq!(ids, [sketch, extrude]);
    assert_eq!(adapter.session.model.state().bodies.len(), 1);
    assert!((modelling_volume(&adapter) - 12_000.0).abs() < 1e-6);
    let bounds = adapter.session.world_state().unwrap().bodies[0].mesh().bounds();
    assert!(bounds.min.dist(Vec3::ZERO) < 1e-7);
    assert!(bounds.max.dist(Vec3::new(40.0, 30.0, 10.0)) < 1e-7);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!(adapter.session.model.state().bodies.is_empty());
    adapter.execute("edit.redo", json!({})).unwrap();
    assert!((modelling_volume(&adapter) - 12_000.0).abs() < 1e-6);

    let edit = adapter.begin_model(701, None, Some(extrude), &[]).unwrap();
    assert_eq!(edit.params["extent"]["distance"], "10 mm");
    let mut params = edit.params.clone();
    params["extent"]["distance"] = json!("15 mm");
    modelling_commit(&mut adapter, &edit, params);
    assert!((modelling_volume(&adapter) - 18_000.0).abs() < 1e-6);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!((modelling_volume(&adapter) - 12_000.0).abs() < 1e-6);
    adapter.execute("edit.redo", json!({})).unwrap();
    assert!((modelling_volume(&adapter) - 18_000.0).abs() < 1e-6);

    adapter.execute("sketch.edit", json!({"sketch":sketch})).unwrap();
    let selection = [Sel::SketchConstraint { id: width.clone() }];
    numeric_apply(&mut adapter, &selection, None, "60 mm");
    assert!((modelling_volume(&adapter) - 27_000.0).abs() < 1e-6);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!((modelling_volume(&adapter) - 18_000.0).abs() < 1e-6);
    adapter.execute("edit.redo", json!({})).unwrap();
    assert!((modelling_volume(&adapter) - 27_000.0).abs() < 1e-6);
    adapter.execute("sketch.finish", json!({})).unwrap();
    assert_eq!(adapter.session.doc.features.iter().map(|feature| feature.id).collect::<Vec<_>>(), ids);
    let FeatureKind::Extrude { sketch: source, .. } = &adapter.session.doc.feature(extrude).unwrap().kind else { panic!("extrusion retained") };
    assert_eq!(source.0, sketch);

    let mut loaded = modelling_roundtrip(&adapter);
    assert_eq!(*loaded.session.doc, *adapter.session.doc, "native format retains all CAD history and metadata");
    assert!((modelling_volume(&loaded) - 27_000.0).abs() < 1e-6);
    let edit = loaded.begin_model(702, None, Some(extrude), &[]).unwrap();
    assert_eq!(edit.params["extent"]["distance"], "15 mm");
    loaded.execute("sketch.edit", json!({"sketch":sketch})).unwrap();
    let edit = numeric_apply(&mut loaded, &selection, None, "40 mm");
    assert_eq!(edit.expression, "60 mm");
    assert!((modelling_volume(&loaded) - 18_000.0).abs() < 1e-6);
}

#[test]
fn modelling_explicit_target_cut_keeps_editable_circle_and_extrusion_after_regeneration() {
    let (mut adapter, _, _, extrude) = modelling_plate(60.0, "15 mm");
    let target = adapter.session.world_state().unwrap().bodies[0].name.clone();
    let circle_sketch =
        adapter.execute("sketch.create", json!({"plane":"XY","offset":"15 mm","name":"Cut sketch"})).unwrap().value["sketch"].as_u64().unwrap();
    let circle =
        adapter.execute("sketch.circle.center", json!({"center":[30,15],"radius":5})).unwrap().value["curves"][0].as_str().unwrap().to_string();
    numeric_apply(&mut adapter, &curve_selection(circle_sketch, &circle), Some("radius"), "5 mm");
    let state = adapter.sketch_snapshot().unwrap();
    assert!((state.plane.origin.z - 15.0).abs() < 1e-8);
    let constraint = state.sketch.constraints.iter().find(|constraint| constraint.kind.is_dimension()).unwrap().id.clone();
    adapter.execute("sketch.finish", json!({})).unwrap();
    let edit = adapter.begin_model(703, Some("solid.extrude"), None, &[Sel::Feature { id: circle_sketch }]).unwrap();
    let mut params = edit.params.clone();
    params["operation"] = json!("cut");
    params["targets"] = json!([target]);
    params["distance"] = json!("15 mm");
    params["direction"] = json!("negative");
    modelling_commit(&mut adapter, &edit, params);
    let cut = adapter.session.doc.features.last().unwrap().id;
    let ids: Vec<_> = adapter.session.doc.features.iter().map(|feature| feature.id).collect();
    let removed = |adapter: &SessionAdapter| 27_000.0 - modelling_volume(adapter);
    let expected = 375.0 * std::f64::consts::PI;
    assert!((removed(&adapter) - expected).abs() / expected < 5e-4, "analytic through cut: {} versus {expected}", removed(&adapter));
    assert_eq!(adapter.session.model.state().bodies.len(), 1);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!((modelling_volume(&adapter) - 27_000.0).abs() < 1e-6);
    adapter.execute("edit.redo", json!({})).unwrap();
    assert!((removed(&adapter) - expected).abs() / expected < 5e-4);

    adapter.execute("sketch.edit", json!({"sketch":circle_sketch})).unwrap();
    numeric_apply(&mut adapter, &[Sel::SketchConstraint { id: constraint.clone() }], None, "3 mm");
    let smaller = 135.0 * std::f64::consts::PI;
    assert!((removed(&adapter) - smaller).abs() / smaller < 5e-4, "edited radius: {} versus {smaller}", removed(&adapter));
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!((removed(&adapter) - expected).abs() / expected < 5e-4);
    adapter.execute("edit.redo", json!({})).unwrap();
    assert!((removed(&adapter) - smaller).abs() / smaller < 5e-4);
    adapter.execute("sketch.finish", json!({})).unwrap();
    assert_eq!(adapter.session.doc.features.iter().map(|feature| feature.id).collect::<Vec<_>>(), ids);
    for feature in [extrude, cut] {
        let edit = adapter.begin_model(704, None, Some(feature), &[]).unwrap();
        assert_eq!(edit.feature, Some(feature));
        assert_eq!(edit.params["extent"]["distance"], "15 mm");
    }
    let mut loaded = modelling_roundtrip(&adapter);
    assert_eq!(*loaded.session.doc, *adapter.session.doc);
    assert!((removed(&loaded) - smaller).abs() / smaller < 5e-4);
    loaded.execute("sketch.edit", json!({"sketch":circle_sketch})).unwrap();
    numeric_apply(&mut loaded, &[Sel::SketchConstraint { id: constraint }], None, "5 mm");
    assert!((removed(&loaded) - expected).abs() / expected < 5e-4);
}

#[test]
fn modelling_proposals_cancel_invalid_and_superseded_work_without_authoritative_mutation() {
    let (mut adapter, sketch, _, extrude) = modelling_plate(40.0, "10 mm");
    let edit = adapter.begin_model(705, None, Some(extrude), &[]).unwrap();
    let original = adapter.session.doc.clone();
    let history = adapter.session.undo.len();
    let mut params = edit.params.clone();
    params["extent"]["distance"] = json!("2 +");
    assert!(adapter.prepare_model(&edit, params, false).is_err());
    assert!(Arc::ptr_eq(&original, &adapter.session.doc));
    assert_eq!(adapter.session.undo.len(), history);
    let mut params = edit.params.clone();
    params["extent"]["distance"] = json!("20 mm");
    let preview = adapter.prepare_model(&edit, params.clone(), false).unwrap();
    drop(preview); // Cancel is discarding the scratch result, never undoing the document.
    assert!(Arc::ptr_eq(&original, &adapter.session.doc));
    assert!((modelling_volume(&adapter) - 12_000.0).abs() < 1e-6);
    assert_eq!(adapter.session.undo.len(), history);

    let first = adapter.prepare_model(&edit, params.clone(), true).unwrap();
    let duplicate = adapter.prepare_model(&edit, params, true).unwrap();
    adapter.accept_model(&edit, first).unwrap();
    assert!(adapter.accept_model(&edit, duplicate).is_err(), "double confirmation cannot commit twice");
    assert_eq!(adapter.session.undo.len(), history + 1);
    assert_eq!(adapter.session.doc.features.len(), 2);
    assert!((modelling_volume(&adapter) - 24_000.0).abs() < 1e-6);
    let stale = adapter.begin_model(706, None, Some(extrude), &[]).unwrap();
    let pending = adapter.prepare_model(&stale, stale.params.clone(), true).unwrap();
    adapter.execute("timeline.rename", json!({"feature":sketch,"name":"Changed while pending"})).unwrap();
    assert!(adapter.accept_model(&stale, pending).is_err());
    assert_eq!(adapter.session.doc.feature(sketch).unwrap().name, "Changed while pending");
    assert!((modelling_volume(&adapter) - 24_000.0).abs() < 1e-6);
}

#[test]
fn modelling_named_expressions_and_suppression_survive_unchanged_edit_and_native_roundtrip() {
    let (mut adapter, _, _, extrude) = modelling_plate(40.0, "10 mm");
    adapter.execute("parameters.add", json!({"name":"plate_depth","expression":"15 mm"})).unwrap();
    let edit = adapter.begin_model(707, None, Some(extrude), &[]).unwrap();
    let mut params = edit.params.clone();
    params["extent"]["distance"] = json!("plate_depth");
    modelling_commit(&mut adapter, &edit, params);
    assert!((modelling_volume(&adapter) - 18_000.0).abs() < 1e-6);
    let edit = adapter.begin_model(708, None, Some(extrude), &[]).unwrap();
    assert_eq!(edit.params["extent"]["distance"], "plate_depth");
    modelling_commit(&mut adapter, &edit, edit.params.clone());
    assert_eq!(adapter.begin_model(709, None, Some(extrude), &[]).unwrap().params["extent"]["distance"], "plate_depth");
    let edit = adapter.begin_model(710, None, Some(extrude), &[]).unwrap();
    for invalid in ["2 +", "5 deg", "missing_parameter"] {
        let mut params = edit.params.clone();
        params["extent"]["distance"] = json!(invalid);
        assert!(adapter.prepare_model(&edit, params, false).is_err());
        assert!((modelling_volume(&adapter) - 18_000.0).abs() < 1e-6);
    }
    adapter.execute("timeline.suppress", json!({"feature":extrude,"suppressed":true})).unwrap();
    let mut loaded = modelling_roundtrip(&adapter);
    assert!(loaded.session.doc.feature(extrude).unwrap().suppressed);
    assert!(loaded.session.model.state().bodies.is_empty());
    loaded.execute("timeline.suppress", json!({"feature":extrude,"suppressed":false})).unwrap();
    assert!((modelling_volume(&loaded) - 18_000.0).abs() < 1e-6);
    loaded.execute("parameters.change", json!({"name":"plate_depth","expression":"20 mm"})).unwrap();
    assert!((modelling_volume(&loaded) - 24_000.0).abs() < 1e-6);
}

fn modelling_edge(adapter: &mut SessionAdapter, body: &str, midpoint: Vec3) -> Sel {
    let edges = adapter.execute("model.edges", json!({"body":body})).unwrap().value;
    let edge = edges["edges"]
        .as_array()
        .unwrap()
        .iter()
        .find(|edge| serde_json::from_value::<Vec3>(edge["mid"].clone()).unwrap().dist(midpoint) < 1e-6)
        .unwrap();
    Sel::Edge { body: body.into(), index: edge["index"].as_u64().unwrap() as usize, point: midpoint }
}

#[test]
fn modelling_surface_handles_drag_preview_numeric_edit_and_cancel_keep_history_atomic() {
    for source in [InputSource::Pencil, InputSource::Finger, InputSource::Indirect] {
        for (operation, parameter) in [("solid.fillet", "radius"), ("solid.chamfer", "distance"), ("solid.shell", "thickness")] {
            let mut adapter = SessionAdapter::new();
            adapter.execute("solid.box", json!({"length":20,"width":20,"height":20,"body_name":"Block"})).unwrap();
            let selection = if operation == "solid.shell" {
                let faces = adapter.execute("model.faces", json!({"body":"Block"})).unwrap().value;
                let face = faces["faces"].as_array().unwrap().iter().find(|face| face["plane_normal"][2].as_f64().is_some_and(|z| z > 0.99)).unwrap();
                Sel::Face {
                    body: "Block".into(),
                    index: face["index"].as_u64().unwrap() as usize,
                    point: serde_json::from_value(face["point"].clone()).unwrap(),
                }
            } else {
                modelling_edge(&mut adapter, "Block", Vec3::new(20., 0., 10.))
            };
            let edit = adapter.begin_model(911, Some(operation), None, &[selection]).unwrap();
            let handle = edit.handle.clone().expect("source handle");
            assert_eq!(handle.parameter, parameter);
            assert_eq!(handle.units, "mm");
            let history = adapter.session.undo.len();
            let mut viewport = Viewport::default();
            viewport.set_metrics(metrics(800., 600., 2.)).unwrap();
            viewport.begin_update(1);
            assert!(viewport.install(1, adapter.snapshot().unwrap()));
            viewport.begin_model(edit.clone()).unwrap();
            let camera = viewport.camera;
            let start = modelling_handle_position(&viewport, 1.);
            let end = modelling_handle_position(&viewport, 3.);
            for (phase, position, time) in [(InputPhase::Began, start, 1.), (InputPhase::Moved, end, 2.), (InputPhase::Ended, end, 3.)] {
                let event = sample(&viewport, source, phase, 91, position, time);
                viewport.input(event).unwrap();
            }
            let work = viewport.take_tool_work().expect("handle drag proposal");
            assert!(!work.commit);
            let prepared = adapter.prepare_tool(work.clone()).unwrap();
            assert!(viewport.publish_preview(&work, prepared.model));
            assert_eq!(viewport.diagnostic()["model_value"]["value"], 3.);
            assert_eq!(adapter.session.undo.len(), history);
            assert!((modelling_volume(&adapter) - 8000.).abs() < 1e-6);
            let edit = viewport.model_edit(911).unwrap();
            assert_eq!(edit.handle.as_ref().unwrap().origin, handle.origin, "preview moved the source anchor");
            let mut numeric = edit.params.clone();
            numeric[parameter] = json!("2 mm");
            let prepared = adapter.prepare_model(&edit, numeric.clone(), false).unwrap();
            viewport.publish_model(911, &prepared).unwrap();
            assert_eq!(viewport.diagnostic()["model_value"]["value"], 2.);
            assert_eq!(viewport.camera, camera);
            viewport.cancel_model();
            assert_eq!(adapter.session.undo.len(), history);
            modelling_commit(&mut adapter, &edit, numeric);
            assert_eq!(adapter.session.undo.len(), history + 1);
            let feature = adapter.session.doc.features.last().unwrap().id;
            let reopened = adapter.begin_model(912, None, Some(feature), &[]).unwrap();
            assert_eq!(reopened.handle.as_ref().unwrap().parameter, parameter);
            assert_eq!(reopened.handle.as_ref().unwrap().distance, 2.);
            let inspected = adapter.refresh_model(&reopened, &[], None).unwrap();
            assert_eq!(inspected.params, reopened.params, "post-drag inspection erased a saved feature's references");
            assert!(inspected.handle.is_some());
            adapter.execute("edit.undo", json!({})).unwrap();
            assert!((modelling_volume(&adapter) - 8000.).abs() < 1e-6);
        }
    }
}

#[test]
fn modelling_surface_live_preview_keeps_multiple_source_edge_picks() {
    for operation in ["solid.fillet", "solid.chamfer"] {
        for source in [InputSource::Pencil, InputSource::Finger] {
            let mut adapter = SessionAdapter::new();
            adapter.execute("solid.box", json!({"length":20,"width":20,"height":20,"body_name":"Block"})).unwrap();
            let mut viewport = Viewport::default();
            viewport.set_metrics(metrics(800., 600., 2.)).unwrap();
            // List-valued tool inputs accumulate independently of the ordinary
            // single-selection preference and saved topology filter.
            viewport.multiple = false;
            viewport.filter = PickFilter::Body;
            let mut feature = None;
            for ticket in 1..=2 {
                let original = adapter.snapshot().unwrap();
                viewport.begin_update(ticket);
                assert!(viewport.install(ticket, original.clone()));
                let doc = adapter.session.doc.clone();
                let history = adapter.session.undo.len();
                let edit = adapter.begin_model(915, feature.is_none().then_some(operation), feature, &[]).unwrap();
                viewport.begin_model(edit).unwrap();
                if feature.is_some() {
                    assert_eq!(viewport.selection().len(), 2, "reopening an operation retains its original edge inputs");
                    let edit = viewport.model_edit(915).unwrap();
                    let preview = adapter.prepare_model(&edit, edit.params.clone(), false).unwrap();
                    viewport.publish_model(915, &preview).unwrap();
                    let p = point(&viewport, Vec3::new(10., 0., 20.));
                    for phase in [InputPhase::Began, InputPhase::Ended] {
                        let mut input = sample(&viewport, source, phase, 916, p, 0.5);
                        input.display_preview = true;
                        viewport.input(input).unwrap();
                    }
                    assert_eq!(viewport.selection().len(), 3, "an edit can add a shortened surviving edge without replacing its saved inputs");
                    viewport.select_model_inputs(915, None).unwrap();
                    assert!(!Arc::ptr_eq(viewport.display_model().unwrap(), &original), "editing reveals the geometry before the blend");
                }
                assert_eq!(viewport.model_edit(915).unwrap().selection_field.as_deref(), Some("edges"));
                let mut earlier_preview = None;
                for (index, location) in [Vec3::new(20., 0., 10.), Vec3::new(20., 20., 10.)].into_iter().enumerate() {
                    let p = point(&viewport, location);
                    for phase in [InputPhase::Began, InputPhase::Ended] {
                        let mut input = sample(&viewport, source, phase, 915, p, 1. + index as f64);
                        input.display_preview = viewport.diagnostic()["preview"] == true;
                        viewport.input(input).unwrap();
                    }
                    assert_eq!(viewport.selection().len(), index + 1, "{operation}: {source:?} must accumulate source edges");
                    if let Some(stale) = earlier_preview.take() {
                        assert_eq!(viewport.diagnostic()["preview"], true, "keep the previous preview visible while adding an edge");
                        assert_eq!(viewport.diagnostic()["preview_stale"], true, "the previous edge set cannot validate the new selection");
                        assert!(viewport.publish_model(915, &stale).is_err(), "a late preview cannot close picking with the previous edge set");
                    }
                    let edit = adapter.refresh_model(&viewport.model_edit(915).unwrap(), viewport.selection(), None).unwrap();
                    assert_eq!(edit.params["edges"].as_array().unwrap().len(), index + 1);
                    assert_eq!(edit.selection_field.as_deref(), Some("edges"));
                    viewport.update_model(edit.clone()).unwrap();
                    let preview = adapter.prepare_model(&edit, edit.params.clone(), false).unwrap();
                    viewport.publish_model(915, &preview).unwrap();
                    assert_eq!(viewport.diagnostic()["preview"], true, "every picked edge updates the live preview");
                    assert_eq!(viewport.model_edit(915).unwrap().selection_field.as_deref(), Some("edges"));
                    if index == 0 {
                        let generated = point(&viewport, Vec3::new(20., 1., 10.));
                        assert!(
                            viewport.pick(generated, Some(adapter.geometry_revision)).is_none(),
                            "a new blend boundary is not an original source edge"
                        );
                        for phase in [InputPhase::Began, InputPhase::Ended] {
                            let mut input = sample(&viewport, source, phase, 917, generated, 1.5);
                            input.display_preview = true;
                            viewport.input(input).unwrap();
                        }
                        assert_eq!(viewport.selection().len(), 1, "tapping a generated preview boundary must keep the original inputs");
                    }
                    earlier_preview = Some(preview);
                }
                let edit = viewport.model_edit(915).unwrap();
                assert!(Arc::ptr_eq(&doc, &adapter.session.doc));
                assert_eq!(adapter.session.undo.len(), history);
                viewport.cancel_model();
                assert!(Arc::ptr_eq(viewport.display_model().unwrap(), &original), "Cancel restores the complete current model");
                modelling_commit(&mut adapter, &edit, edit.params.clone());
                assert_eq!(adapter.session.undo.len(), history + 1);
                let applied = adapter.session.doc.features.last().unwrap().id;
                if let Some(feature) = feature {
                    assert_eq!(applied, feature, "editing retains the existing feature identity");
                }
                feature = Some(applied);
            }
        }
    }
}

#[test]
fn modelling_surface_live_preview_can_add_curved_edges() {
    for operation in ["solid.fillet", "solid.chamfer"] {
        let mut adapter = SessionAdapter::new();
        adapter.execute("solid.cylinder", json!({"radius":10,"height":20,"body_name":"Round"})).unwrap();
        let mut viewport = Viewport::default();
        viewport.set_metrics(metrics(800., 600., 2.)).unwrap();
        viewport.begin_update(1);
        assert!(viewport.install(1, adapter.snapshot().unwrap()));
        viewport.filter = PickFilter::Edge;
        viewport.camera.target = Vec3::Z * 10.;
        viewport.camera = viewport.camera.looking_from(Vec3::X + Vec3::Z * 2.);
        viewport.changed();
        let first = viewport.pick(point(&viewport, Vec3::new(10., 0., 20.)), Some(adapter.geometry_revision)).unwrap();
        let edit = adapter.begin_model(918, Some(operation), None, &[first]).unwrap();
        viewport.begin_model(edit.clone()).unwrap();
        let preview = adapter.prepare_model(&edit, edit.params.clone(), false).unwrap();
        viewport.publish_model(918, &preview).unwrap();
        viewport.camera = viewport.camera.looking_from(Vec3::X - Vec3::Z * 2.);
        viewport.changed();
        let p = point(&viewport, Vec3::new(10., 0., 0.));
        for phase in [InputPhase::Began, InputPhase::Ended] {
            let mut input = sample(&viewport, InputSource::Pencil, phase, 918, p, 1.);
            input.display_preview = true;
            viewport.input(input).unwrap();
        }
        assert_eq!(viewport.selection().len(), 2, "{operation}: a visible curved rim maps to its original edge");
        let edit = adapter.refresh_model(&viewport.model_edit(918).unwrap(), viewport.selection(), None).unwrap();
        assert_eq!(edit.params["edges"].as_array().unwrap().len(), 2);
        let preview = adapter.prepare_model(&edit, edit.params.clone(), false).unwrap();
        viewport.update_model(edit).unwrap();
        viewport.publish_model(918, &preview).unwrap();
        assert_eq!(viewport.diagnostic()["preview"], true);
        assert_eq!(adapter.session.doc.features.len(), 1, "live edge picking does not commit the preview");
    }
}

#[test]
fn modelling_surface_source_picks_replace_old_references_without_resetting_size() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("solid.box", json!({"length":20,"width":20,"height":20,"body_name":"Block"})).unwrap();
    let first = modelling_edge(&mut adapter, "Block", Vec3::new(20., 0., 10.));
    let next = modelling_edge(&mut adapter, "Block", Vec3::new(20., 20., 10.));
    let edit = adapter.begin_model(913, Some("solid.fillet"), None, &[first]).unwrap();
    let mut params = edit.params.clone();
    params["radius"] = json!("2 mm");
    let edit = adapter.refresh_model(&edit, &[], Some(params)).unwrap();
    let refreshed = adapter.refresh_model(&edit, std::slice::from_ref(&next), None).unwrap();
    assert_eq!(refreshed.params["radius"], "2 mm");
    assert_ne!(refreshed.params["edges"], edit.params["edges"]);
    assert_eq!(refreshed.handle.as_ref().unwrap().origin, Vec3::new(20., 20., 10.));
    let empty = adapter.refresh_model(&refreshed, &[], None).unwrap();
    assert_eq!(empty.params["edges"], json!([]));
    assert!(empty.handle.is_none());
    assert!(adapter.prepare_model(&empty, empty.params.clone(), false).is_err());
    let mut viewport = Viewport::default();
    viewport.set_metrics(metrics(800., 600., 2.)).unwrap();
    viewport.begin_update(1);
    assert!(viewport.install(1, adapter.snapshot().unwrap()));
    viewport.begin_model(refreshed).unwrap();
    viewport.command("select.filter", &json!({"filter":"body"})).unwrap();
    viewport.select_model_inputs(913, Some("edges")).unwrap();
    let p = point(&viewport, Vec3::new(20., 0., 10.));
    assert!(
        matches!(viewport.pick(p, Some(adapter.geometry_revision)), Some(Sel::Edge { .. })),
        "explicit source picking must override the saved body filter"
    );
    viewport.cancel_model();
    let shell = adapter.begin_model(914, Some("solid.shell"), None, &[]).unwrap();
    viewport.begin_model(shell).unwrap();
    viewport.command("select.filter", &json!({"filter":"edge"})).unwrap();
    viewport.select_model_inputs(914, Some("faces")).unwrap();
    let p = point(&viewport, Vec3::new(10., 10., 20.));
    assert!(
        matches!(viewport.pick(p, Some(adapter.geometry_revision)), Some(Sel::Face { .. })),
        "removal-face picking must override the saved edge filter"
    );
}

#[test]
fn modelling_blends_accept_sharp_borders_of_existing_treatments() {
    for first in ["solid.fillet", "solid.chamfer"] {
        for second in ["solid.fillet", "solid.chamfer"] {
            let mut adapter = SessionAdapter::new();
            adapter.execute("solid.box", json!({"length":20,"width":20,"height":20,"body_name":"Block"})).unwrap();
            let first_field = if first == "solid.fillet" { "radius" } else { "distance" };
            adapter.execute(first, json!({"body":"Block","edges":[[20,0,10]],first_field:2})).unwrap();
            let before = modelling_volume(&adapter);
            let near = if first == "solid.fillet" { Vec3::new(19.414213562, 0.585786438, 20.) } else { Vec3::new(19., 1., 20.) };
            let world = adapter.session.world_state().unwrap();
            let mesh = world.body("Block").unwrap().mesh();
            let index = mesh
                .edges
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| {
                    let distance = |points: &[Vec3]| points.windows(2).map(|s| near.dist_to_segment(s[0], s[1])).fold(f64::INFINITY, f64::min);
                    distance(a).total_cmp(&distance(b))
                })
                .unwrap()
                .0;
            let selection = [Sel::Edge { body: "Block".into(), index, point: near }];
            let edit = adapter.begin_model(715, Some(second), None, &selection).unwrap();
            let mut params = edit.params.clone();
            params[if second == "solid.fillet" { "radius" } else { "distance" }] = json!("0.5 mm");
            let result = adapter.prepare_model(&edit, params, true);
            let prepared = result.unwrap_or_else(|e| panic!("{first} then {second}: {e}"));
            adapter.accept_model(&edit, prepared).unwrap();
            assert!(modelling_volume(&adapter) < before - 0.01);
            assert_eq!(adapter.session.doc.features.len(), 3);
        }
    }
}

#[test]
fn modelling_chamfer_cuts_across_a_fillet_face_join_without_replacing_the_fillet() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("solid.box", json!({"length":20,"width":20,"height":20,"body_name":"Block"})).unwrap();
    let fillet = adapter.execute("solid.fillet", json!({"body":"Block","edges":[[20,0,10]],"radius":2})).unwrap().value["feature"].as_u64().unwrap();
    let original = adapter.session.doc.feature(fillet).unwrap().clone();
    let before = modelling_volume(&adapter);
    let selection = [modelling_edge(&mut adapter, "Block", Vec3::new(20., 2., 10.))];
    let edit = adapter.begin_model(716, Some("solid.chamfer"), None, &selection).unwrap();
    assert_eq!(edit.feature, None);
    let mut params = edit.params.clone();
    params["distance"] = json!("1 mm");
    modelling_commit(&mut adapter, &edit, params);
    let after = modelling_volume(&adapter);
    assert!(after < before - 0.1, "the new bevel must remove material");
    assert_eq!(adapter.session.doc.feature(fillet).unwrap(), &original);
    assert_eq!(adapter.session.doc.features.len(), 3);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!((modelling_volume(&adapter) - before).abs() < 1e-6);
    adapter.execute("edit.redo", json!({})).unwrap();
    assert!((modelling_volume(&adapter) - after).abs() < 1e-6);
    let mut loaded = modelling_roundtrip(&adapter);
    assert!((modelling_volume(&loaded) - after).abs() < 1e-6);
    // The bevel introduces a real corner on the flat face, which can receive
    // a further round without touching the original Fillet feature.
    let border = modelling_edge(&mut loaded, "Block", Vec3::new(20., 3., 10.));
    let edit = loaded.begin_model(717, Some("solid.fillet"), None, &[border]).unwrap();
    let mut params = edit.params.clone();
    params["radius"] = json!("1 mm");
    modelling_commit(&mut loaded, &edit, params);
    assert!(modelling_volume(&loaded) < after);
    assert_eq!(loaded.session.doc.feature(fillet).unwrap(), &original);
    assert_eq!(loaded.session.doc.features.len(), 4);
}

#[test]
fn modelling_fillet_and_chamfer_use_real_edges_edit_original_and_refuse_invalid_sizes() {
    for (operation, field) in [("solid.fillet", "radius"), ("solid.chamfer", "distance")] {
        let mut adapter = SessionAdapter::new();
        adapter.execute("solid.box", json!({"length":20,"width":20,"height":20,"name":"Blend base","body_name":"Blend body"})).unwrap();
        let selection = [modelling_edge(&mut adapter, "Blend body", Vec3::new(20.0, 0.0, 10.0))];
        let edit = adapter.begin_model(711, Some(operation), None, &selection).unwrap();
        let mut viewport = Viewport::default();
        viewport.set_metrics(metrics(800.0, 600.0, 2.0)).unwrap();
        viewport.begin_update(1);
        assert!(viewport.install(1, adapter.snapshot().unwrap()));
        viewport.begin_model(edit.clone()).unwrap();
        let preview = adapter.prepare_model(&edit, edit.params.clone(), false).unwrap();
        viewport.publish_model(edit.token, &preview).unwrap();
        assert!(viewport.model_active());
        viewport.cancel_model();
        assert!(!viewport.model_active());
        assert!(viewport.publish_model(edit.token, &preview).is_err(), "a cancelled interaction rejects late geometry");
        drop(preview);
        assert_eq!(adapter.session.doc.features.len(), 1);
        assert!((modelling_volume(&adapter) - 8_000.0).abs() < 1e-6);
        modelling_commit(&mut adapter, &edit, edit.params.clone());
        let feature = adapter.session.doc.features.last().unwrap().id;
        let removed = |radius: f64| {
            if operation == "solid.fillet" { (1.0 - std::f64::consts::PI / 4.0) * radius * radius * 20.0 } else { radius * radius * 10.0 }
        };
        let check = |adapter: &SessionAdapter, size: f64| {
            let actual = 8_000.0 - modelling_volume(adapter);
            // Match the existing blend-edit tests' analytic absolute tolerance;
            // native mass measurement uses an adaptive chord tessellation.
            assert!((actual - removed(size)).abs() < 0.05, "{operation}: {actual} versus {}", removed(size));
        };
        check(&adapter, 1.0);
        let edit = adapter.begin_model(712, None, Some(feature), &[]).unwrap();
        let mut params = edit.params.clone();
        params[field] = json!("2 mm");
        modelling_commit(&mut adapter, &edit, params);
        check(&adapter, 2.0);
        assert_eq!(adapter.session.doc.features.last().unwrap().id, feature);
        adapter.execute("edit.undo", json!({})).unwrap();
        check(&adapter, 1.0);
        adapter.execute("edit.redo", json!({})).unwrap();
        check(&adapter, 2.0);
        let edit = adapter.begin_model(713, None, Some(feature), &[]).unwrap();
        let doc = adapter.session.doc.clone();
        let history = adapter.session.undo.len();
        let mut invalid = edit.params.clone();
        invalid[field] = json!("100 mm");
        assert!(adapter.prepare_model(&edit, invalid, false).is_err());
        assert!(Arc::ptr_eq(&doc, &adapter.session.doc));
        assert_eq!(adapter.session.undo.len(), history);
        check(&adapter, 2.0);
        let loaded = modelling_roundtrip(&adapter);
        check(&loaded, 2.0);
        assert_eq!(*loaded.session.doc, *adapter.session.doc);
        let stale = adapter.begin_model(714, Some(operation), None, &selection).unwrap();
        adapter.execute("timeline.delete", json!({"feature":"Blend base"})).unwrap();
        assert!(adapter.prepare_model(&stale, stale.params.clone(), false).is_err(), "invalidated edges cannot attach to another edge");
    }
}

#[test]
fn modelling_shell_removal_face_thickness_edit_cancel_failure_and_native_roundtrip() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("solid.box", json!({"length":40,"width":30,"height":20,"body_name":"Shell body"})).unwrap();
    let faces = adapter.execute("model.faces", json!({"body":"Shell body"})).unwrap().value;
    let face = faces["faces"].as_array().unwrap().iter().find(|face| face["plane_normal"][2].as_f64().is_some_and(|n| n > 0.999)).unwrap();
    let selection = [Sel::Face {
        body: "Shell body".into(),
        index: face["index"].as_u64().unwrap() as usize,
        point: serde_json::from_value(face["point"].clone()).unwrap(),
    }];
    let edit = adapter.begin_model(715, Some("solid.shell"), None, &selection).unwrap();
    let history = adapter.session.undo.len();
    let preview = adapter.prepare_model(&edit, edit.params.clone(), false).unwrap();
    drop(preview);
    assert_eq!(adapter.session.undo.len(), history);
    assert!((modelling_volume(&adapter) - 24_000.0).abs() < 1e-6);
    modelling_commit(&mut adapter, &edit, edit.params.clone());
    let feature = adapter.session.doc.features.last().unwrap().id;
    let volume = |thickness: f64| 24_000.0 - (40.0 - 2.0 * thickness) * (30.0 - 2.0 * thickness) * (20.0 - thickness);
    assert!((modelling_volume(&adapter) - volume(1.0)).abs() < 1e-6);
    let edit = adapter.begin_model(716, None, Some(feature), &[]).unwrap();
    let mut params = edit.params.clone();
    params["thickness"] = json!("2 mm");
    modelling_commit(&mut adapter, &edit, params);
    assert!((modelling_volume(&adapter) - volume(2.0)).abs() < 1e-6);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!((modelling_volume(&adapter) - volume(1.0)).abs() < 1e-6);
    adapter.execute("edit.redo", json!({})).unwrap();
    assert!((modelling_volume(&adapter) - volume(2.0)).abs() < 1e-6);
    let edit = adapter.begin_model(717, None, Some(feature), &[]).unwrap();
    let mut invalid = edit.params.clone();
    invalid["thickness"] = json!("30 mm");
    let doc = adapter.session.doc.clone();
    assert!(adapter.prepare_model(&edit, invalid, false).is_err());
    assert!(Arc::ptr_eq(&doc, &adapter.session.doc));
    let loaded = modelling_roundtrip(&adapter);
    assert_eq!(loaded.session.doc.feature(feature).unwrap().id, feature);
    assert!((modelling_volume(&loaded) - volume(2.0)).abs() < 1e-6);
}

fn modelling_check_edit(adapter: &mut SessionAdapter, feature: u64, params: Value, before: f64, after: f64, tolerance: f64) {
    let check = |adapter: &SessionAdapter, expected: f64| {
        assert!((modelling_volume(adapter) - expected).abs() < tolerance, "{} versus {expected}", modelling_volume(adapter));
    };
    check(adapter, before);
    let edit = adapter.begin_model(720, None, Some(feature), &[]).unwrap();
    modelling_commit(adapter, &edit, params);
    assert_eq!(adapter.session.doc.features.last().unwrap().id, feature);
    check(adapter, after);
    adapter.execute("edit.undo", json!({})).unwrap();
    check(adapter, before);
    adapter.execute("edit.redo", json!({})).unwrap();
    check(adapter, after);
    let loaded = modelling_roundtrip(adapter);
    // The native reader names newly introduced optional inputs (for example a
    // hole's Depth after Through All). Its public naming API keeps existing names.
    let mut expected = (*adapter.session.doc).clone();
    expected.name_feature_inputs(feature);
    assert_eq!(*loaded.session.doc, expected);
    check(&loaded, after);
}

#[test]
fn modelling_revolve_profile_axis_angle_edit_invalid_and_history() {
    let mut adapter = SessionAdapter::new();
    let sketch = adapter.execute("sketch.create", json!({"plane":"XZ"})).unwrap().value["sketch"].as_u64().unwrap();
    adapter.execute("sketch.rectangle.two_point", json!({"p0":[10,0],"p1":[15,5]})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    let edit = adapter.begin_model(721, Some("solid.revolve"), None, &[Sel::Feature { id: sketch }]).unwrap();
    let mut params = edit.params.clone();
    params["axis"] = json!("y");
    modelling_commit(&mut adapter, &edit, params);
    let feature = adapter.session.doc.features.last().unwrap().id;
    let edit = adapter.begin_model(722, None, Some(feature), &[]).unwrap();
    let mut params = edit.params.clone();
    params["angle"] = json!("180 deg");
    let full = std::f64::consts::PI * 125.0 * 5.0;
    modelling_check_edit(&mut adapter, feature, params, full, full / 2.0, full * 5e-4);
    assert_eq!(adapter.session.model.state().bodies.len(), 1);
    let edit = adapter.begin_model(723, None, Some(feature), &[]).unwrap();
    let mut params = edit.params.clone();
    params["angle"] = json!("0 deg");
    assert!(adapter.prepare_model(&edit, params, false).is_err());
}

#[test]
fn modelling_hole_uses_real_sketch_centres_explicit_targets_and_original_diameter_edit() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("solid.box", json!({"length":40,"width":30,"height":10,"body_name":"Drilled plate"})).unwrap();
    let sketch = adapter.execute("sketch.create", json!({"plane":"XY","offset":"10 mm"})).unwrap().value["sketch"].as_u64().unwrap();
    adapter.execute("sketch.circle.center", json!({"center":[20,15],"radius":1})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    let edit = adapter.begin_model(724, Some("solid.hole"), None, &[Sel::Feature { id: sketch }]).unwrap();
    let mut params = edit.params.clone();
    assert!(adapter.prepare_model(&edit, params.clone(), false).is_err(), "target bodies are mandatory");
    params["targets"] = json!(["Drilled plate"]);
    modelling_commit(&mut adapter, &edit, params);
    let feature = adapter.session.doc.features.last().unwrap().id;
    let edit = adapter.begin_model(725, None, Some(feature), &[]).unwrap();
    let mut params = edit.params.clone();
    params["diameter"] = json!("6 mm");
    let pi = std::f64::consts::PI;
    modelling_check_edit(&mut adapter, feature, params, 12_000.0 - 62.5 * pi, 12_000.0 - 90.0 * pi, 0.05);
    assert_eq!(adapter.session.model.state().bodies.len(), 1);
    let edit = adapter.begin_model(726, None, Some(feature), &[]).unwrap();
    let mut invalid = edit.params.clone();
    invalid["diameter"] = json!("-1 mm");
    assert!(adapter.prepare_model(&edit, invalid, false).is_err());
}

#[test]
fn modelling_combine_visual_picks_whole_bodies_and_keeps_roles_separate() {
    for device in [InputSource::Pencil, InputSource::Finger, InputSource::Indirect] {
        let mut adapter = SessionAdapter::new();
        adapter.execute("solid.box", json!({"length":20,"width":20,"height":10,"body_name":"Target"})).unwrap();
        for (name, x, width) in [("Tool A", 5, 5), ("Tool B", 15, 3)] {
            adapter
                .execute("solid.box", json!({"length":width,"width":30,"height":14,"corner":[x,-5,-2],"operation":"new","body_name":name}))
                .unwrap();
        }
        let original = adapter.snapshot().unwrap();
        let history = adapter.session.undo.len();
        let mut viewport = Viewport::default();
        viewport.set_metrics(metrics(800., 600., 1.)).unwrap();
        viewport.begin_update(1);
        viewport.install(1, original.clone());
        viewport.command("view.top", &json!({})).unwrap();
        viewport.command("select.filter", &json!({"filter":"edge"})).unwrap();
        viewport.command("select.multiple", &json!({"enabled":false})).unwrap();
        let mut edit = adapter.begin_model(786, Some("solid.combine"), None, &[]).unwrap();
        edit.params["operation"] = json!("cut");
        edit.params["keep_tools"] = json!(false);
        assert!(edit.fields.iter().any(|field| field["key"] == "target" && field["kind"] == "body_pick"));
        assert!(edit.fields.iter().any(|field| field["key"] == "tools" && field["kind"] == "bodies_pick"));
        let seeded = adapter.begin_model(780, Some("solid.combine"), None, &[Sel::Body { name: "Target".into() }]).unwrap();
        assert_eq!(seeded.selection_field.as_deref(), Some("tools"), "a preselected target starts ready for tool bodies");
        viewport.begin_model(edit).unwrap();
        // Two quick taps can arrive before the host inspects either selection.
        let target = point(&viewport, Vec3::new(2., 5., 10.));
        feed(&mut viewport, device, InputPhase::Began, 1, target, 1.);
        feed(&mut viewport, device, InputPhase::Ended, 1, target, 1.1);
        assert_eq!(viewport.model_edit(786).unwrap().selection_field.as_deref(), Some("tools"));
        assert_eq!(viewport.snapshot().unwrap().selected_bodies, ["Target"]);
        let tool = point(&viewport, Vec3::new(6., 5., 12.));
        feed(&mut viewport, device, InputPhase::Began, 2, tool, 2.);
        feed(&mut viewport, device, InputPhase::Ended, 2, tool, 2.1);
        assert_eq!(viewport.model_edit(786).unwrap().params["target"], "Target", "a quick tool tap must not replace the target");
        assert_eq!(viewport.selection(), &[Sel::Body { name: "Target".into() }, Sel::Body { name: "Tool A".into() }]);
        let edit = adapter.refresh_model(&viewport.model_edit(786).unwrap(), viewport.selection(), None).unwrap();
        viewport.update_model(edit).unwrap();
        let mut sequence = 2;
        let mut choose = |viewport: &mut Viewport, world: Vec3| {
            sequence += 1;
            let tap = point(viewport, world);
            feed(viewport, device, InputPhase::Began, sequence, tap, sequence as f64);
            feed(viewport, device, InputPhase::Ended, sequence, tap, sequence as f64 + 0.1);
            let edit = adapter.refresh_model(&viewport.model_edit(786).unwrap(), viewport.selection(), None).unwrap();
            viewport.update_model(edit).unwrap();
        };
        choose(&mut viewport, Vec3::new(16., 5., 12.));
        choose(&mut viewport, Vec3::new(2., 5., 10.));
        assert_eq!(viewport.model_edit(786).unwrap().params["tools"], json!(["Tool A", "Tool B"]));
        assert_eq!(viewport.snapshot().unwrap().selected_bodies, ["Target", "Tool A", "Tool B"]);
        viewport.select_model_inputs(786, Some("target")).unwrap();
        choose(&mut viewport, Vec3::new(16., 5., 12.));
        let changed = viewport.model_edit(786).unwrap();
        assert_eq!(changed.params["target"], "Tool B");
        assert_eq!(changed.params["tools"], json!(["Tool A"]), "the replacement target must leave the tool set");
        assert_eq!(changed.selection_field.as_deref(), Some("tools"));
        viewport.select_model_inputs(786, Some("target")).unwrap();
        choose(&mut viewport, Vec3::new(2., 5., 10.));
        choose(&mut viewport, Vec3::new(16., 5., 12.));
        let edit = viewport.model_edit(786).unwrap();
        let stale = adapter.prepare_model(&edit, edit.params.clone(), false).unwrap();
        choose(&mut viewport, Vec3::new(6., 5., 12.));
        assert!(viewport.publish_model(786, &stale).is_err(), "a late preview must not validate a removed tool");
        assert_eq!(viewport.model_edit(786).unwrap().params["tools"], json!(["Tool B"]));
        choose(&mut viewport, Vec3::new(6., 5., 12.));
        // Re-entering a role retains its picks; empty space clears only that role.
        viewport.select_model_inputs(786, Some("tools")).unwrap();
        feed(&mut viewport, device, InputPhase::Began, 99, [1., 1.], 20.);
        feed(&mut viewport, device, InputPhase::Ended, 99, [1., 1.], 20.1);
        assert!(viewport.model_active(), "empty space must clear the active role without dismissing Combine");
        let empty = adapter.refresh_model(&viewport.model_edit(786).unwrap(), viewport.selection(), None).unwrap();
        assert_eq!(empty.params["target"], "Target");
        assert_eq!(empty.params["tools"], json!([]));
        assert!(adapter.prepare_model(&empty, empty.params.clone(), false).is_err());
        // Restore the complete selection and explicitly preview.
        viewport.select_model_inputs(786, Some("tools")).unwrap();
        let edit = viewport.model_edit(786).unwrap();
        let prepared = adapter.prepare_model(&edit, edit.params.clone(), false).unwrap();
        assert!((prepared.model.volume_mm3 - 2400.).abs() < 1e-5);
        viewport.publish_model(786, &prepared).unwrap();
        assert!(viewport.model_edit(786).unwrap().selection_field.is_none());
        viewport.select_model_inputs(786, Some("tools")).unwrap();
        assert!(Arc::ptr_eq(&viewport.snapshot().unwrap().model, &original), "reselection reveals consumed tools");
        assert_eq!(viewport.selection().len(), 3);
        assert_eq!(viewport.diagnostic()["filter"], "edge");
        assert_eq!(adapter.session.undo.len(), history);
        viewport.cancel_model();
        assert!(Arc::ptr_eq(&viewport.snapshot().unwrap().model, &original));
    }
}

#[test]
fn modelling_combine_history_picking_reveals_original_tools_and_cancel_restores_result() {
    let mut adapter = SessionAdapter::new();
    for (name, x) in [("Target", 0), ("Tool", 5)] {
        adapter.execute("solid.box", json!({"length":10,"width":10,"height":10,"corner":[x,0,0],"operation":"new","body_name":name})).unwrap();
    }
    let edit = adapter.begin_model(787, Some("solid.combine"), None, &[]).unwrap();
    let mut params = edit.params.clone();
    params["target"] = json!("Target");
    params["tools"] = json!(["Tool"]);
    params["keep_tools"] = json!(false);
    modelling_commit(&mut adapter, &edit, params);
    let feature = adapter.session.doc.features.last().unwrap().id;
    let result = adapter.snapshot().unwrap();
    let mut viewport = Viewport::default();
    viewport.set_metrics(metrics(800., 600., 1.)).unwrap();
    viewport.begin_update(1);
    viewport.install(1, result.clone());
    let edit = adapter.begin_model(788, None, Some(feature), &[]).unwrap();
    assert!(edit.repair_source.is_some());
    viewport.begin_model(edit).unwrap();
    viewport.select_model_inputs(788, Some("tools")).unwrap();
    assert_eq!(viewport.selection(), &[Sel::Body { name: "Target".into() }, Sel::Body { name: "Tool".into() }]);
    assert_eq!(viewport.snapshot().unwrap().model.scene.meshes.len(), 2);
    viewport.cancel_model();
    assert!(Arc::ptr_eq(&viewport.snapshot().unwrap().model, &result));
    assert!(!viewport.is_updating());
}

#[test]
fn modelling_combine_is_exact_without_retry_and_preserves_preview_commit_or_later_features() {
    let mut adapter = SessionAdapter::new();
    let slope = 1_f64.to_radians().tan();
    for (command, params) in [
        ("sketch.create", json!({"plane":"XZ","name":"Floor section"})),
        ("sketch.line", json!({"points":[[0,0],[100,0],[100,-10.-slope*100.],[0,-10.]],"closed":true})),
        ("sketch.finish", json!({})),
        ("solid.extrude", json!({"sketch":"Floor section","distance":80,"body_name":"Floor"})),
        ("solid.cylinder", json!({"base":[50,40,-5],"radius":10,"height":30,"operation":"cut"})),
        ("sketch.create", json!({"plane":"XY","name":"Ring section"})),
        ("sketch.circle.center", json!({"center":[50,40],"radius":20})),
        ("sketch.circle.center", json!({"center":[50,40],"radius":10})),
        ("sketch.finish", json!({})),
        ("solid.extrude", json!({"sketch":"Ring section","profiles":[{"point":[65,40]}],"start_offset":-5,"distance":7,"body_name":"Ring"})),
    ] {
        adapter.execute(command, params).unwrap();
    }
    let original = adapter.session.doc.clone();
    let history = adapter.session.undo.len();
    let mut edit = adapter.begin_model(790, Some("solid.combine"), None, &[]).unwrap();
    edit.params = json!({"target":"Floor","tools":["Ring"],"operation":"join","keep_tools":false});
    let preview = adapter.prepare_model(&edit, edit.params.clone(), false).unwrap();
    assert!(preview.feedback.is_empty(), "solidcore builds this union without truck’s Boolean retry: {:?}", preview.feedback);
    assert_eq!(preview.model.body_count, 1);
    assert!(Arc::ptr_eq(&adapter.session.doc, &original));
    assert_eq!(adapter.session.undo.len(), history);
    let prepared = adapter.prepare_model(&edit, edit.params.clone(), true).unwrap();
    assert_eq!(prepared.feedback, preview.feedback);
    adapter.accept_model(&edit, prepared).unwrap();
    assert_eq!(adapter.session.undo.len(), history + 1);
    let feature = adapter.session.doc.features.last().unwrap().id;
    let warning = adapter.session.model.result(feature).unwrap().warning.clone();
    assert!(warning.is_none(), "the exact solidcore union needs no retry warning: {warning:?}");
    let expected = (10. + slope * 50.) * (8000. - std::f64::consts::PI * 100.) + std::f64::consts::PI * 300. * 5.;
    assert!((modelling_volume(&adapter) - expected).abs() / expected < 2e-5);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert_eq!(adapter.session.model.state().bodies.len(), 2);
    adapter.execute("edit.redo", json!({})).unwrap();
    assert_eq!(adapter.session.model.state().bodies.len(), 1);
    assert_eq!(adapter.session.model.result(feature).unwrap().warning, warning);
    let reopened = solidcraft_engine::doc::Document::from_json(&adapter.session.doc.to_json()).unwrap();
    adapter.session = solidcraft_engine::Session::new(reopened);
    adapter.snapshot().unwrap();
    assert_eq!(adapter.session.model.result(feature).unwrap().warning, warning);
    let edit = adapter.begin_model(791, Some("solid.pattern.rectangular"), None, &[Sel::Body { name: "Floor".into() }]).unwrap();
    let prepared = adapter.prepare_model(&edit, edit.params.clone(), false).unwrap();
    assert!(prepared.feedback.is_empty(), "the exact union also needs no retry warning in later features: {:?}", prepared.feedback);
    modelling_commit(&mut adapter, &edit, edit.params.clone());
    assert!((modelling_volume(&adapter) - 2. * expected).abs() / expected < 4e-5);
}

#[test]
fn modelling_boolean_modes_distinguish_target_tools_and_retain_behavior_with_history() {
    for (operation, expected) in [("join", 1500.0), ("cut", 500.0), ("intersect", 500.0)] {
        let mut adapter = SessionAdapter::new();
        for (name, x) in [("Target", 0), ("Tool", 5)] {
            adapter.execute("solid.box", json!({"length":10,"width":10,"height":10,"corner":[x,0,0],"operation":"new","body_name":name})).unwrap();
        }
        let edit = adapter.begin_model(727, Some("solid.combine"), None, &[]).unwrap();
        let mut params = edit.params.clone();
        params["target"] = json!("Target");
        params["tools"] = json!(["Tool"]);
        params["operation"] = json!(operation);
        params["keep_tools"] = json!(false);
        modelling_commit(&mut adapter, &edit, params);
        let feature = adapter.session.doc.features.last().unwrap().id;
        assert_eq!(adapter.session.model.state().bodies.len(), 1);
        let edit = adapter.begin_model(728, None, Some(feature), &[]).unwrap();
        let mut params = edit.params.clone();
        params["keep_tools"] = json!(true);
        modelling_check_edit(&mut adapter, feature, params, expected, expected + 1000.0, 1e-6);
        assert_eq!(adapter.session.model.state().bodies.len(), 2);
        let edit = adapter.begin_model(729, None, Some(feature), &[]).unwrap();
        let mut invalid = edit.params.clone();
        invalid["tools"] = json!(["Missing tool"]);
        assert!(adapter.prepare_model(&edit, invalid, false).is_err());
    }
}

#[test]
fn modelling_body_patterns_keep_source_relationship_count_edits_and_roundtrip() {
    for operation in ["solid.pattern.rectangular", "solid.pattern.circular"] {
        let mut adapter = SessionAdapter::new();
        adapter.execute("solid.box", json!({"length":2,"width":2,"height":2,"corner":[10,0,0],"body_name":"Pattern source"})).unwrap();
        let edit = adapter.begin_model(730, Some(operation), None, &[Sel::Body { name: "Pattern source".into() }]).unwrap();
        modelling_commit(&mut adapter, &edit, edit.params.clone());
        let feature = adapter.session.doc.features.last().unwrap().id;
        let edit = adapter.begin_model(731, None, Some(feature), &[]).unwrap();
        let mut params = edit.params.clone();
        let (key, old_count, new_count) = if operation.ends_with("rectangular") { ("count1", 2, 3) } else { ("count", 3, 4) };
        params["pattern"][key] = json!(new_count.to_string());
        modelling_check_edit(&mut adapter, feature, params, 8.0 * f64::from(old_count), 8.0 * f64::from(new_count), 1e-6);
        assert_eq!(adapter.session.model.state().bodies.len(), new_count as usize);
        let edit = adapter.begin_model(732, None, Some(feature), &[]).unwrap();
        let mut invalid = edit.params.clone();
        invalid["pattern"][key] = json!("0");
        assert!(adapter.prepare_model(&edit, invalid, false).is_err());
        let FeatureKind::Pattern { bodies, .. } = &adapter.session.doc.feature(feature).unwrap().kind else { panic!("native pattern") };
        assert_eq!(bodies.iter().map(|name| name.0.as_str()).collect::<Vec<_>>(), ["Pattern source"]);
    }
}

#[test]
fn modelling_body_mirror_plane_edit_keeps_original_feature_and_native_history() {
    let mut adapter = SessionAdapter::new();
    adapter.execute("solid.box", json!({"length":2,"width":2,"height":2,"corner":[10,4,2],"body_name":"Mirror source"})).unwrap();
    let edit = adapter.begin_model(733, Some("solid.mirror"), None, &[Sel::Body { name: "Mirror source".into() }]).unwrap();
    modelling_commit(&mut adapter, &edit, edit.params.clone());
    let feature = adapter.session.doc.features.last().unwrap().id;
    let negative_x = |adapter: &SessionAdapter| adapter.session.world_state().unwrap().bodies.iter().any(|body| body.mesh().bounds().max.x < 0.0);
    let negative_y = |adapter: &SessionAdapter| adapter.session.world_state().unwrap().bodies.iter().any(|body| body.mesh().bounds().max.y < 0.0);
    assert!(negative_x(&adapter));
    let edit = adapter.begin_model(734, None, Some(feature), &[]).unwrap();
    let mut params = edit.params.clone();
    params["plane"] = json!({"type":"origin","name":"XZ"});
    modelling_check_edit(&mut adapter, feature, params, 16.0, 16.0, 1e-6);
    assert!(!negative_x(&adapter) && negative_y(&adapter));
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!(negative_x(&adapter) && !negative_y(&adapter));
    adapter.execute("edit.redo", json!({})).unwrap();
    assert!(!negative_x(&adapter) && negative_y(&adapter));
    let loaded = modelling_roundtrip(&adapter);
    assert!(!negative_x(&loaded) && negative_y(&loaded));
    let edit = adapter.begin_model(735, None, Some(feature), &[]).unwrap();
    let mut invalid = edit.params.clone();
    invalid["bodies"] = json!(["Missing source"]);
    assert!(adapter.prepare_model(&edit, invalid, false).is_err());
}

#[test]
fn modelling_sweep_uses_ordered_native_path_and_edits_original_end_scale() {
    let mut adapter = SessionAdapter::new();
    let profile = adapter.execute("sketch.create", json!({"plane":"XY","name":"Sweep profile"})).unwrap().value["sketch"].as_u64().unwrap();
    adapter.execute("sketch.rectangle.two_point", json!({"p0":[-5,-5],"p1":[5,5]})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    let path = adapter.execute("sketch.create", json!({"plane":"XZ","name":"Sweep path"})).unwrap().value["sketch"].as_u64().unwrap();
    let curve = adapter.execute("sketch.line", json!({"points":[[0,0],[0,-40]]})).unwrap().value["curves"][0].as_str().unwrap().to_string();
    adapter.execute("sketch.finish", json!({})).unwrap();
    let edit = adapter.begin_model(736, Some("solid.sweep"), None, &[Sel::Feature { id: profile }]).unwrap();
    let mut params = edit.params.clone();
    params["path_sketch"] = json!(path);
    params["path_curves"] = json!([{"sketch":path,"curve":curve}]);
    modelling_commit(&mut adapter, &edit, params);
    let feature = adapter.session.doc.features.last().unwrap().id;
    let edit = adapter.begin_model(737, None, Some(feature), &[]).unwrap();
    let mut params = edit.params.clone();
    params["scale"] = json!("0.5");
    modelling_check_edit(&mut adapter, feature, params, 4_000.0, 100.0 * 40.0 * (1.0 + 0.5 + 0.25) / 3.0, 1e-5);
    assert_eq!(adapter.session.model.state().bodies.len(), 1);
    let FeatureKind::Sweep { sketch, path_sketch, path: saved, .. } = &adapter.session.doc.feature(feature).unwrap().kind else {
        panic!("native sweep")
    };
    assert_eq!((sketch.0, path_sketch.0), (profile, path));
    assert_eq!(saved, &[curve]);
    let edit = adapter.begin_model(738, None, Some(feature), &[]).unwrap();
    let mut invalid = edit.params.clone();
    invalid["scale"] = json!("0");
    assert!(adapter.prepare_model(&edit, invalid, false).is_err());
}

#[test]
fn modelling_loft_ordered_sections_are_editable_without_replacing_original_history() {
    let mut adapter = SessionAdapter::new();
    let mut sketches = Vec::new();
    for (name, z, side) in [("Loft base", 0, 10), ("Loft top", 20, 10), ("Loft wide top", 20, 20)] {
        sketches.push(adapter.execute("sketch.create", json!({"plane":"XY","offset":z,"name":name})).unwrap().value["sketch"].as_u64().unwrap());
        adapter.execute("sketch.rectangle.two_point", json!({"p0":[0,0],"p1":[side,side]})).unwrap();
        adapter.execute("sketch.finish", json!({})).unwrap();
    }
    let edit = adapter.begin_model(739, Some("solid.loft"), None, &[]).unwrap();
    let descriptor = adapter.model_descriptor(&edit);
    let options = descriptor["fields"].as_array().unwrap().iter().find(|field| field["key"] == "sections").unwrap()["options"].as_array().unwrap();
    let section = |sketch: u64| options.iter().find(|option| option["value"]["sketch"] == sketch).unwrap()["value"].clone();
    let mut params = edit.params.clone();
    params["sections"] = json!([section(sketches[0]), section(sketches[1])]);
    modelling_commit(&mut adapter, &edit, params);
    let feature = adapter.session.doc.features.last().unwrap().id;
    assert_eq!(adapter.session.model.state().bodies.len(), 1);
    let edit = adapter.begin_model(740, None, Some(feature), &[]).unwrap();
    let mut params = edit.params.clone();
    params["sections"] = json!([section(sketches[0]), section(sketches[2])]);
    modelling_check_edit(&mut adapter, feature, params, 2_000.0, 20.0 * (100.0 + 200.0 + 400.0) / 3.0, 1e-5);
    let FeatureKind::Loft { sections, .. } = &adapter.session.doc.feature(feature).unwrap().kind else { panic!("native loft") };
    assert_eq!(sections.iter().map(|section| section.sketch.0).collect::<Vec<_>>(), [sketches[0], sketches[2]]);
    let edit = adapter.begin_model(741, None, Some(feature), &[]).unwrap();
    let mut invalid = edit.params.clone();
    invalid["sections"] = json!([section(sketches[0])]);
    assert!(adapter.prepare_model(&edit, invalid, false).is_err());
}

fn modelling_input_fixture() -> (SessionAdapter, Viewport, u64) {
    let (mut adapter, _, _, extrude) = modelling_plate(40.0, "10 mm");
    let model = adapter.snapshot().unwrap();
    let mut viewport = Viewport::default();
    viewport.set_metrics(metrics(800.0, 600.0, 2.0)).unwrap();
    viewport.begin_update(1);
    assert!(viewport.install(1, model));
    let edit = adapter.begin_model(750, None, Some(extrude), &[]).unwrap();
    assert!(edit.handle.is_some());
    viewport.begin_model(edit).unwrap();
    (adapter, viewport, extrude)
}

fn modelling_handle_position(viewport: &Viewport, depth: f64) -> [f64; 2] {
    let handle = viewport.modelling.as_ref().unwrap().handle.as_ref().unwrap();
    point(viewport, handle.origin + handle.normal * depth * handle.factor)
}

#[test]
fn modelling_empty_tap_cancels_source_and_preview_without_changing_history() {
    for source in [InputSource::Finger, InputSource::Pencil, InputSource::Indirect] {
        for preview in [false, true] {
            let (adapter, mut viewport, _) = modelling_input_fixture();
            let edit = viewport.model_edit(750).unwrap();
            let mut params = edit.params.clone();
            params["extent"]["distance"] = json!("15 mm");
            let prepared = adapter.prepare_model(&edit, params, false).unwrap();
            if preview {
                viewport.publish_model(750, &prepared).unwrap();
            }
            viewport.selection = vec![Sel::Body { name: "Plate".into() }];
            let history = adapter.session.undo.len();
            let volume = modelling_volume(&adapter);
            for phase in [InputPhase::Began, InputPhase::Ended] {
                let mut event = sample(&viewport, source, phase, 92, [5., 5.], if phase == InputPhase::Began { 1. } else { 2. });
                event.display_preview = preview;
                viewport.input(event).unwrap();
            }
            assert!(!viewport.model_active(), "empty {source:?} tap retained the edit, preview={preview}");
            assert!(viewport.selection().is_empty(), "empty-space dismissal retained the selection");
            assert_eq!(viewport.diagnostic()["input"]["dismissed_model_token"], 750);
            assert!(viewport.publish_model(750, &prepared).is_err(), "a late preview resurrected a dismissed edit");
            assert_eq!(adapter.session.undo.len(), history);
            assert_eq!(modelling_volume(&adapter), volume);
            assert_eq!(viewport.snapshot().unwrap().model.volume_mm3, volume);
        }
    }
}

#[test]
fn modelling_dismissal_preserves_navigation_geometry_hits_and_legacy_preference() {
    for source in [InputSource::Finger, InputSource::Pencil, InputSource::Indirect] {
        for case in ["legacy", "navigation", "navigation_during_tap", "geometry", "cancelled", "consumed", "stale_camera", "new_edit"] {
            let (adapter, mut viewport, _) = modelling_input_fixture();
            viewport.command("view.tool_handles", &json!({"enabled":false})).unwrap();
            let edit = viewport.model_edit(750).unwrap();
            let prepared = adapter.prepare_model(&edit, edit.params.clone(), false).unwrap();
            viewport.publish_model(750, &prepared).unwrap();
            if case == "legacy" {
                viewport.command("view.tool_dismissal", &json!({"cancel_on_dismiss":false})).unwrap();
            }
            let position = if case == "geometry" { point(&viewport, Vec3::new(20., 15., 10.)) } else { [5., 5.] };
            if case == "geometry" {
                let pixel = viewport.metrics.pixel(position).unwrap();
                assert!(
                    viewport
                        .display_model()
                        .unwrap()
                        .picks
                        .pick(&viewport.camera, viewport.vp(), viewport.metrics, pixel, PickFilter::Body)
                        .is_some()
                );
            }
            let mut began = sample(&viewport, source, InputPhase::Began, 93, position, 1.);
            began.display_preview = true;
            began.consumed = case == "consumed";
            if case == "stale_camera" {
                began.camera_epoch = Some(viewport.camera_epoch + 1);
            }
            viewport.input(began).unwrap();
            if case == "navigation_during_tap" {
                feed(&mut viewport, InputSource::Finger, InputPhase::Began, 94, [600., 400.], 1.1);
                feed(&mut viewport, InputSource::Finger, InputPhase::Moved, 94, [650., 430.], 1.2);
                feed(&mut viewport, InputSource::Finger, InputPhase::Ended, 94, [650., 430.], 1.3);
            }
            if case == "new_edit" {
                // A contact beginning in one form cannot dismiss a newer form.
                let mut next = edit.clone();
                next.token = 751;
                viewport.modelling = Some(next);
            }
            let end = if case == "navigation" { [position[0] + 40., position[1]] } else { position };
            let mut moved = sample(&viewport, source, InputPhase::Moved, 93, end, 2.);
            moved.display_preview = true;
            viewport.input(moved).unwrap();
            let phase = if case == "cancelled" { InputPhase::Cancelled } else { InputPhase::Ended };
            let mut ended = sample(&viewport, source, phase, 93, end, 3.);
            ended.display_preview = true;
            viewport.input(ended).unwrap();
            assert!(viewport.model_active(), "{source:?} {case} incorrectly dismissed the edit");
            assert!(viewport.diagnostic()["input"]["dismissed_model_token"].is_null());
        }
    }
    let (_, mut viewport, _) = modelling_input_fixture();
    assert!(viewport.command("view.tool_dismissal", &json!({"cancel_on_dismiss":"invalid"})).is_err());
    assert_eq!(viewport.diagnostic()["tool_dismissal"]["cancel_on_dismiss"], true);
}

#[test]
fn modelling_direct_handle_accepts_finger_and_pointer_and_preserves_legacy_preference() {
    for source in [InputSource::Finger, InputSource::Indirect] {
        let (adapter, mut viewport, _) = modelling_input_fixture();
        let camera = viewport.camera;
        let history = adapter.session.undo.len();
        let start = modelling_handle_position(&viewport, 10.0);
        let end = modelling_handle_position(&viewport, 15.0);
        for (phase, point, time) in [(InputPhase::Began, start, 1.), (InputPhase::Moved, end, 2.), (InputPhase::Ended, end, 3.)] {
            let mut event = sample(&viewport, source, phase, 91, point, time);
            event.display_preview = true;
            viewport.input(event).unwrap();
        }
        assert_eq!(viewport.camera, camera, "the handle owns its contact, not camera navigation");
        let work = viewport.take_tool_work().unwrap();
        assert!(!work.commit);
        let prepared = adapter.prepare_tool(work.clone()).unwrap();
        assert!((prepared.model.volume_mm3 - 18_000.).abs() < 1e-5);
        assert_eq!(adapter.session.undo.len(), history);
        assert!(viewport.publish_preview(&work, prepared.model));
        let value = &viewport.diagnostic()["model_value"];
        assert_eq!(value["parameter"], "extent.distance");
        assert!((value["value"].as_f64().unwrap() - 15.).abs() < 1e-7);
        assert_eq!(value["units"], "mm");
        assert_eq!(value["valid"], true);
        viewport.command("view.tool_handles", &json!({"enabled":false})).unwrap();
        assert!(viewport.diagnostic()["model_value"].is_null());
        let start = modelling_handle_position(&viewport, 15.);
        feed(&mut viewport, InputSource::Finger, InputPhase::Began, 92, start, 4.);
        feed(&mut viewport, InputSource::Finger, InputPhase::Moved, 92, [start[0] + 60., start[1]], 5.);
        assert_ne!(viewport.camera, camera, "legacy finger drag must retain navigation");
        assert!(viewport.take_tool_work().is_none());
    }
}

#[test]
fn modelling_push_pull_uses_native_face_offset_signed_expression_history_and_cancel() {
    let (mut adapter, _, _, _) = modelling_plate(40., "10 mm");
    let body = &adapter.session.world_state().unwrap().bodies[0];
    let (index, spot) = solidcraft_engine::doc::naming::face_spots(body)
        .into_iter()
        .enumerate()
        .find_map(|(index, spot)| {
            let spot = spot?;
            matches!(spot.surface, Some(solidcraft_engine::kernel::FaceSurface::Plane { normal, .. }) if normal.z > 0.999).then_some((index, spot))
        })
        .unwrap();
    let selection = [Sel::Face { body: body.name.clone(), index, point: spot.point }];
    let edit = adapter.begin_model(760, Some("solid.offset_face"), None, &selection).unwrap();
    let handle = edit.handle.as_ref().unwrap();
    assert!(handle.normal.dist(Vec3::Z) < 1e-8);
    assert!((handle.point().z - 11.).abs() < 1e-8);
    let history = adapter.session.undo.len();
    let mut params = edit.params.clone();
    params["distance"] = json!("(2 + 3) mm");
    let preview = adapter.prepare_model(&edit, params.clone(), false).unwrap();
    assert!((preview.model.volume_mm3 - 18_000.).abs() < 1e-5);
    assert!((preview.handle.as_ref().unwrap().point().z - 15.).abs() < 1e-8, "the preview handle follows the original face plus the proposed offset");
    assert_eq!(adapter.session.undo.len(), history);
    assert!((modelling_volume(&adapter) - 12_000.).abs() < 1e-5);
    let mut inferred = params.clone();
    inferred.as_object_mut().unwrap().remove("body");
    let inferred_preview = adapter.prepare_model(&edit, inferred, false).unwrap();
    assert!((inferred_preview.model.volume_mm3 - 18_000.).abs() < 1e-5);
    assert!((inferred_preview.handle.as_ref().unwrap().point().z - 15.).abs() < 1e-8);
    let mut invalid = params.clone();
    invalid["faces"] = json!([]);
    assert!(adapter.prepare_model(&edit, invalid, false).is_err());
    let mut invalid = params.clone();
    invalid["distance"] = json!("3 deg");
    assert!(adapter.prepare_model(&edit, invalid, false).is_err());
    modelling_commit(&mut adapter, &edit, params);
    let feature = adapter.session.doc.features.last().unwrap().id;
    assert!((modelling_volume(&adapter) - 18_000.).abs() < 1e-5);
    let edit = adapter.begin_model(761, None, Some(feature), &[]).unwrap();
    assert!((edit.handle.as_ref().unwrap().point().z - 15.).abs() < 1e-8);
    let mut params = edit.params.clone();
    params["distance"] = json!("-3 mm");
    modelling_commit(&mut adapter, &edit, params);
    assert!((modelling_volume(&adapter) - 8_400.).abs() < 1e-5);
    assert_eq!(adapter.session.doc.features.last().unwrap().id, feature);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!((modelling_volume(&adapter) - 18_000.).abs() < 1e-5);
    adapter.execute("edit.redo", json!({})).unwrap();
    assert!((modelling_volume(&adapter) - 8_400.).abs() < 1e-5);
    let mut loaded = modelling_roundtrip(&adapter);
    assert!((modelling_volume(&loaded) - 8_400.).abs() < 1e-5);
    let edit = loaded.begin_model(762, None, Some(feature), &[]).unwrap();
    let mut viewport = Viewport::default();
    viewport.set_metrics(metrics(800., 600., 2.)).unwrap();
    viewport.begin_update(1);
    viewport.install(1, loaded.snapshot().unwrap());
    viewport.begin_model(edit.clone()).unwrap();
    let preview = loaded.prepare_model(&edit, edit.params.clone(), false).unwrap();
    viewport.publish_model(762, &preview).unwrap();
    viewport.finish_model(762, None).unwrap();
    assert!(viewport.diagnostic()["model_value"].is_null());
    assert!((modelling_volume(&loaded) - 8_400.).abs() < 1e-5);
}

#[test]
fn modelling_drag_resolution_rounds_absolute_lengths_and_preserves_continuous_mode() {
    let (_, viewport, _) = modelling_input_fixture();
    assert_eq!(viewport.diagnostic()["drag_step_mm"], json!(0.1));
    for (step, expected) in [(0.1, 15.1), (0.01, 15.06), (0.5, 15.), (1., 15.), (0., 15.064)] {
        let (_, mut viewport, _) = modelling_input_fixture();
        viewport.command("view.drag_precision", &json!({"step_mm":step})).unwrap();
        let current = viewport.modelling.as_ref().unwrap().handle.as_ref().unwrap().distance;
        let start = modelling_handle_position(&viewport, current);
        feed(&mut viewport, InputSource::Pencil, InputPhase::Began, 91, start, 1.);
        let target = modelling_handle_position(&viewport, 15.064);
        feed(&mut viewport, InputSource::Pencil, InputPhase::Moved, 91, target, 2.);
        let work = viewport.take_tool_work().unwrap_or_else(|| panic!("missing drag work for step {step}: {}", viewport.diagnostic()));
        let ToolOperation::Modelling { params, .. } = work.operation else { panic!("expected modelling work") };
        let actual = params["extent"]["distance"].as_str().unwrap().trim_end_matches(" mm").parse::<f64>().unwrap();
        assert!((actual - expected).abs() < 1e-8, "step {step}: {actual} != {expected}");
    }
}

#[test]
fn modelling_drag_resolution_accumulates_small_moves_and_reverses_without_drift() {
    let (_, mut viewport, _) = modelling_input_fixture();
    let start = modelling_handle_position(&viewport, 10.);
    feed(&mut viewport, InputSource::Pencil, InputPhase::Began, 91, start, 1.);
    for (index, (target, expected)) in [(15.04, Some(15.)), (15.06, Some(15.1)), (15.14, None), (14.94, Some(14.9))].into_iter().enumerate() {
        let p = modelling_handle_position(&viewport, target);
        feed(&mut viewport, InputSource::Pencil, InputPhase::Moved, 91, p, index as f64 + 2.);
        let work = viewport.take_tool_work();
        assert_eq!(work.is_some(), expected.is_some());
        if let Some(expected) = expected {
            assert!((viewport.modelling.as_ref().unwrap().handle.as_ref().unwrap().distance - expected).abs() < 1e-8);
        }
    }
}

#[test]
fn drag_resolution_rejects_invalid_values_without_changing_the_preference() {
    let mut viewport = Viewport::default();
    for step in [json!(-0.1), json!(0.0001), json!(10.1), json!("0.1"), Value::Null] {
        assert!(viewport.command("view.drag_precision", &json!({"step_mm":step})).is_err());
        assert_eq!(viewport.diagnostic()["drag_step_mm"], json!(0.1));
    }
}

#[test]
fn modelling_pencil_extrusion_handle_proposes_engine_geometry_and_lift_requires_explicit_apply() {
    let (mut adapter, mut viewport, feature) = modelling_input_fixture();
    let history = adapter.session.undo.len();
    let camera = viewport.camera;
    let start = modelling_handle_position(&viewport, 10.0);
    feed(&mut viewport, InputSource::Pencil, InputPhase::Began, 91, start, 1.0);
    let end = modelling_handle_position(&viewport, 15.0);
    feed(&mut viewport, InputSource::Pencil, InputPhase::Moved, 91, end, 2.0);
    let work = viewport.take_tool_work().unwrap();
    assert!(!work.commit);
    let prepared = adapter.prepare_tool(work.clone()).unwrap();
    assert!(viewport.publish_preview(&work, prepared.model));
    assert!((viewport.snapshot().unwrap().model.volume_mm3 - 18_000.0).abs() < 1e-5);
    assert!((modelling_volume(&adapter) - 12_000.0).abs() < 1e-6);
    assert_eq!(adapter.session.undo.len(), history);
    feed(&mut viewport, InputSource::Pencil, InputPhase::Ended, 91, end, 3.0);
    let work = viewport.take_tool_work().unwrap();
    assert!(!work.commit, "Pencil lift finalises the draft, never commits the feature");
    let prepared = adapter.prepare_tool(work.clone()).unwrap();
    assert!(viewport.publish_preview(&work, prepared.model));
    assert_eq!(adapter.session.undo.len(), history);
    // Adjust the same provisional feature again after lifting the Pencil.
    for (phase, depth, time) in [(InputPhase::Began, 15.0, 4.0), (InputPhase::Moved, 20.0, 5.0), (InputPhase::Ended, 20.0, 6.0)] {
        let position = modelling_handle_position(&viewport, depth);
        let mut event = sample(&viewport, InputSource::Pencil, phase, 92, position, time);
        event.display_preview = true;
        viewport.input(event).unwrap();
    }
    let work = viewport.take_tool_work().unwrap();
    assert!(!work.commit);
    let prepared = adapter.prepare_tool(work.clone()).unwrap();
    assert!(viewport.publish_preview(&work, prepared.model));
    assert!((viewport.snapshot().unwrap().model.volume_mm3 - 24_000.0).abs() < 1e-5);
    let edit = viewport.model_edit(750).unwrap();
    let prepared = adapter.prepare_model(&edit, edit.params.clone(), true).unwrap();
    let model = adapter.accept_model(&edit, prepared).unwrap();
    viewport.finish_model(750, Some(model)).unwrap();
    assert!((modelling_volume(&adapter) - 24_000.0).abs() < 1e-5);
    assert_eq!(adapter.session.undo.len(), history + 1);
    assert_eq!(adapter.session.doc.features.last().unwrap().id, feature);
    assert_eq!(viewport.camera, camera);
}

#[test]
fn modelling_handle_can_be_grabbed_after_a_tap_or_before_the_previous_lift_preview_finishes() {
    let (adapter, mut viewport, _) = modelling_input_fixture();
    let start = modelling_handle_position(&viewport, 10.0);
    feed(&mut viewport, InputSource::Pencil, InputPhase::Began, 91, start, 1.0);
    feed(&mut viewport, InputSource::Pencil, InputPhase::Ended, 91, start, 2.0);
    assert_eq!(viewport.diagnostic()["input"]["edit"], Value::Null, "a tap must release handle capture");
    feed(&mut viewport, InputSource::Pencil, InputPhase::Began, 92, start, 3.0);
    let next = modelling_handle_position(&viewport, 15.0);
    feed(&mut viewport, InputSource::Pencil, InputPhase::Moved, 92, next, 4.0);
    feed(&mut viewport, InputSource::Pencil, InputPhase::Ended, 92, next, 5.0);
    let previous = viewport.take_tool_work().unwrap();
    let previous_result = adapter.prepare_tool(previous.clone()).unwrap();
    // The engine lane is still evaluating the previous lift when Pencil returns.
    feed(&mut viewport, InputSource::Pencil, InputPhase::Began, 93, next, 6.0);
    let target = modelling_handle_position(&viewport, 20.0);
    feed(&mut viewport, InputSource::Pencil, InputPhase::Moved, 93, target, 7.0);
    let latest = viewport.take_tool_work().unwrap();
    assert_ne!(latest.interaction, previous.interaction);
    assert!(!viewport.publish_preview(&previous, previous_result.model), "an earlier contact cannot replace the new proposal");
    let latest_result = adapter.prepare_tool(latest.clone()).unwrap();
    assert!(viewport.publish_preview(&latest, latest_result.model));
    assert!((viewport.snapshot().unwrap().model.volume_mm3 - 24_000.0).abs() < 1e-5);
    assert!((modelling_volume(&adapter) - 12_000.0).abs() < 1e-6, "repeated grabs remain provisional");
}

#[test]
fn modelling_new_extrusion_inspection_preserves_one_continuous_pencil_contact() {
    let mut adapter = SessionAdapter::new();
    let sketch = adapter.execute("sketch.create", json!({"plane":"XY"})).unwrap().value["sketch"].as_u64().unwrap();
    adapter.execute("sketch.rectangle.two_point", json!({"p0":[0,0],"p1":[40,30]})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    let selection = [Sel::Feature { id: sketch }];
    let edit = adapter.begin_model(750, Some("solid.extrude"), None, &selection).unwrap();
    let mut viewport = Viewport::default();
    viewport.set_metrics(metrics(800., 600., 2.)).unwrap();
    viewport.begin_update(1);
    viewport.install(1, adapter.snapshot().unwrap());
    viewport.select(Some(selection[0].clone()), false);
    viewport.begin_model(edit.clone()).unwrap();
    let preview = adapter.prepare_model(&edit, edit.params.clone(), false).unwrap();
    viewport.publish_model(750, &preview).unwrap();
    viewport.command("view.isometric", &json!({})).unwrap();
    viewport.command("view.fit", &json!({})).unwrap();
    let history = adapter.session.undo.len();
    let start = modelling_handle_position(&viewport, 10.);
    feed(&mut viewport, InputSource::Pencil, InputPhase::Began, 91, start, 1.);
    for (index, depth) in [11., 15., 20., 12.].into_iter().enumerate() {
        let position = modelling_handle_position(&viewport, depth);
        feed(&mut viewport, InputSource::Pencil, InputPhase::Moved, 91, position, 2. + index as f64);
        let work = viewport.take_tool_work().expect("inspection must preserve the live contact");
        let prepared = adapter.prepare_tool(work.clone()).unwrap();
        viewport.publish_model_tool_details(&work, prepared.reply, prepared.feedback, prepared.handle);
        assert!(viewport.publish_preview(&work, prepared.model));
        let current = viewport.model_edit(750).unwrap();
        let refreshed = adapter.refresh_model(&current, &selection, None).unwrap();
        assert_eq!(refreshed.version, current.version, "inspecting a new operation must retain its proposal version");
        viewport.update_model(refreshed).unwrap();
        assert_eq!(viewport.diagnostic()["input"]["edit"], "dragging");
        assert!((viewport.snapshot().unwrap().model.volume_mm3 - 1200. * depth).abs() < 1e-5);
        assert_eq!(adapter.session.undo.len(), history);
    }
}

#[test]
fn modelling_continuous_motion_displays_completed_geometry_without_validating_or_rewinding_the_latest_proposal() {
    let (adapter, mut viewport, _) = modelling_input_fixture();
    let history = adapter.session.undo.len();
    let start = modelling_handle_position(&viewport, 10.);
    feed(&mut viewport, InputSource::Pencil, InputPhase::Began, 91, start, 1.);
    let mut previous = None;
    for (index, depth) in [15., 20., 12.].into_iter().enumerate() {
        let target = modelling_handle_position(&viewport, depth);
        feed(&mut viewport, InputSource::Pencil, InputPhase::Moved, 91, target, 2. + index as f64);
        let work = viewport.take_tool_work().unwrap();
        let prepared = adapter.prepare_tool(work.clone()).unwrap();
        // Deterministically deliver newer actual motion while CAD is in flight.
        let target = modelling_handle_position(&viewport, depth + 1.);
        feed(&mut viewport, InputSource::Pencil, InputPhase::Moved, 91, target, 2.5 + index as f64);
        assert!(!viewport.tool_is_current(&work));
        assert!(viewport.publish_model_progress(&work, prepared.model.clone()));
        assert!((viewport.snapshot().unwrap().model.volume_mm3 - 1200. * depth).abs() < 1e-5);
        let current = viewport.model_edit(750).unwrap();
        assert!((current.handle.as_ref().unwrap().distance - depth - 1.).abs() < 1e-7, "progress must not pull the handle backwards");
        assert!(!adapter.model_descriptor(&current)["valid"].as_bool().unwrap());
        assert_eq!(viewport.diagnostic()["preview_stale"], true);
        assert_eq!(viewport.diagnostic()["input"]["edit"], "dragging");
        assert!(!viewport.publish_model_progress(&work, prepared.model.clone()), "duplicate geometry cannot be published again");
        if let Some((work, model)) = previous {
            assert!(!viewport.publish_model_progress(&work, model), "an older completion must not rewind displayed progress");
        }
        previous = Some((work, prepared.model));
        assert_eq!(adapter.session.undo.len(), history);
    }
    let latest = viewport.take_tool_work().unwrap();
    let prepared = adapter.prepare_tool(latest.clone()).unwrap();
    viewport.publish_model_tool_details(&latest, prepared.reply, prepared.feedback, prepared.handle);
    assert!(viewport.publish_preview(&latest, prepared.model));
    assert_eq!(viewport.diagnostic()["model_value"]["valid"], true);
    assert_eq!(viewport.diagnostic()["preview_stale"], false);
    assert!((viewport.snapshot().unwrap().model.volume_mm3 - 15_600.).abs() < 1e-5);
    assert_eq!(adapter.session.undo.len(), history);
}

#[test]
fn modelling_in_flight_progress_cannot_outlive_its_contact_or_navigation_boundary() {
    for boundary in ["lift", "cancel", "new_contact", "suspend", "resume", "resize", "form", "document"] {
        let (adapter, mut viewport, _) = modelling_input_fixture();
        let start = modelling_handle_position(&viewport, 10.);
        let target = modelling_handle_position(&viewport, 15.);
        feed(&mut viewport, InputSource::Pencil, InputPhase::Began, 91, start, 1.);
        feed(&mut viewport, InputSource::Pencil, InputPhase::Moved, 91, target, 2.);
        let work = viewport.take_tool_work().unwrap();
        let prepared = adapter.prepare_tool(work.clone()).unwrap();
        let target = modelling_handle_position(&viewport, 16.);
        feed(&mut viewport, InputSource::Pencil, InputPhase::Moved, 91, target, 2.1);
        match boundary {
            "lift" | "new_contact" => {
                feed(&mut viewport, InputSource::Pencil, InputPhase::Ended, 91, target, 3.);
                if boundary == "new_contact" {
                    feed(&mut viewport, InputSource::Pencil, InputPhase::Began, 92, target, 4.);
                }
            }
            "cancel" => viewport.cancel_interactions(),
            "suspend" | "resume" => {
                feed(&mut viewport, InputSource::Finger, InputPhase::Began, 92, [600., 400.], 3.);
                feed(&mut viewport, InputSource::Finger, InputPhase::Moved, 92, [650., 430.], 4.);
                if boundary == "resume" {
                    feed(&mut viewport, InputSource::Finger, InputPhase::Ended, 92, [650., 430.], 5.);
                    let target = modelling_handle_position(&viewport, 16.);
                    feed(&mut viewport, InputSource::Pencil, InputPhase::Moved, 91, target, 6.);
                }
            }
            "resize" => viewport.set_metrics(metrics(600., 800., 2.)).unwrap(),
            "form" => {
                viewport.pend_model(750).unwrap();
            }
            "document" => viewport.begin_update(2),
            _ => panic!("unknown interaction boundary"),
        }
        assert!(!viewport.publish_model_progress(&work, prepared.model), "late progress survived {boundary}");
    }
}

#[test]
fn modelling_stationary_pencil_samples_do_not_starve_an_in_flight_preview() {
    let (adapter, mut viewport, _) = modelling_input_fixture();
    let start = modelling_handle_position(&viewport, 10.0);
    let end = modelling_handle_position(&viewport, 15.0);
    feed(&mut viewport, InputSource::Pencil, InputPhase::Began, 91, start, 1.0);
    feed(&mut viewport, InputSource::Pencil, InputPhase::Moved, 91, end, 2.0);
    let work = viewport.take_tool_work().unwrap();
    let result = adapter.prepare_tool(work.clone()).unwrap();
    for sequence in 1..=20 {
        feed(&mut viewport, InputSource::Pencil, InputPhase::Moved, 91, end, 2.0 + f64::from(sequence) * 0.01);
    }
    assert!(viewport.take_tool_work().is_none(), "the same position must not queue another evaluation");
    assert!(viewport.publish_preview(&work, result.model), "stationary samples must not invalidate useful completed geometry");
    assert!((viewport.snapshot().unwrap().model.volume_mm3 - 18_000.0).abs() < 1e-5);
}

#[test]
fn modelling_form_previews_reject_old_completion_and_document_changes_deterministically() {
    let (mut adapter, mut viewport, feature) = modelling_input_fixture();
    let original = adapter.session.doc.clone();
    let edit = viewport.model_edit(750).unwrap();
    let mut params = edit.params.clone();
    params["extent"]["distance"] = json!("20 mm");
    let older = adapter.refresh_model(&edit, &[], Some(params)).unwrap();
    viewport.update_model(older.clone()).unwrap();
    let older_result = adapter.prepare_model(&older, older.params.clone(), false).unwrap();
    let mut params = older.params.clone();
    params["extent"]["distance"] = json!("30 mm");
    let newer = adapter.refresh_model(&older, &[], Some(params)).unwrap();
    viewport.update_model(newer.clone()).unwrap();
    let newer_result = adapter.prepare_model(&newer, newer.params.clone(), false).unwrap();
    viewport.publish_model(750, &newer_result).unwrap();
    assert!(viewport.publish_model(750, &older_result).is_err());
    assert!((viewport.snapshot().unwrap().model.volume_mm3 - 36_000.0).abs() < 1e-5);
    assert!(Arc::ptr_eq(&original, &adapter.session.doc));

    let mut invalid = newer.params.clone();
    invalid["extent"]["distance"] = json!("2 +");
    let invalid = adapter.refresh_model(&newer, &[], Some(invalid)).unwrap();
    viewport.update_model(invalid.clone()).unwrap();
    assert!(adapter.prepare_model(&invalid, invalid.params.clone(), false).is_err());
    viewport.fail_model(750).unwrap();
    assert_eq!(adapter.model_descriptor(&viewport.model_edit(750).unwrap())["valid"], false);
    assert!((viewport.snapshot().unwrap().model.volume_mm3 - 12_000.0).abs() < 1e-6);
    adapter.execute("timeline.rename", json!({"feature":feature,"name":"Document changed"})).unwrap();
    viewport.begin_update(2);
    assert!(viewport.install(2, adapter.snapshot().unwrap()));
    assert!(viewport.publish_model(750, &newer_result).is_err());
    assert!(viewport.selection().is_empty());
}

#[test]
fn modelling_finger_navigation_suspends_pencil_and_cancel_resize_reject_pending_work() {
    let (adapter, mut viewport, _) = modelling_input_fixture();
    let history = adapter.session.undo.len();
    let start = modelling_handle_position(&viewport, 10.0);
    feed(&mut viewport, InputSource::Pencil, InputPhase::Began, 91, start, 1.0);
    let moved = modelling_handle_position(&viewport, 14.0);
    feed(&mut viewport, InputSource::Pencil, InputPhase::Moved, 91, moved, 2.0);
    let work = viewport.take_tool_work().unwrap();
    let prepared = adapter.prepare_tool(work.clone()).unwrap();
    assert!(viewport.publish_preview(&work, prepared.model));
    let camera = viewport.camera;
    for (id, point) in [(1, [200.0, 300.0]), (2, [500.0, 300.0])] {
        feed(&mut viewport, InputSource::Finger, InputPhase::Began, id, point, 3.0);
    }
    feed(&mut viewport, InputSource::Finger, InputPhase::Moved, 2, [550.0, 330.0], 4.0);
    assert_ne!(viewport.camera, camera);
    let far = modelling_handle_position(&viewport, 30.0);
    feed(&mut viewport, InputSource::Pencil, InputPhase::Moved, 91, far, 5.0);
    assert!(viewport.take_tool_work().is_none());
    assert_eq!(viewport.diagnostic()["input"]["edit"], "suspended");
    for id in [2, 1] {
        feed(&mut viewport, InputSource::Finger, InputPhase::Ended, id, [200.0, 300.0], 6.0);
    }
    feed(&mut viewport, InputSource::Pencil, InputPhase::Moved, 91, far, 7.0);
    assert!(viewport.take_tool_work().is_none(), "first move after navigation only rebases");
    feed(&mut viewport, InputSource::Pencil, InputPhase::Cancelled, 91, far, 8.0);
    assert!(viewport.take_tool_work().is_none());
    assert_eq!(adapter.session.undo.len(), history);
    assert!((modelling_volume(&adapter) - 12_000.0).abs() < 1e-6);
    viewport.cancel_model();
    assert!(viewport.model_edit(750).is_err());

    let (adapter, mut viewport, _) = modelling_input_fixture();
    let start = modelling_handle_position(&viewport, 10.0);
    feed(&mut viewport, InputSource::Pencil, InputPhase::Began, 91, start, 1.0);
    let moved = modelling_handle_position(&viewport, 15.0);
    feed(&mut viewport, InputSource::Pencil, InputPhase::Moved, 91, moved, 2.0);
    let work = viewport.take_tool_work().unwrap();
    let prepared = adapter.prepare_tool(work.clone()).unwrap();
    viewport.set_metrics(metrics(600.0, 800.0, 2.0)).unwrap();
    assert!(!viewport.publish_preview(&work, prepared.model));
    assert!(viewport.take_tool_work().is_none());
    assert!((modelling_volume(&adapter) - 12_000.0).abs() < 1e-6);
}

#[test]
fn modelling_extrusion_uses_native_hole_classification_and_non_world_z_plane() {
    let mut adapter = SessionAdapter::new();
    let sketch = adapter.execute("sketch.create", json!({"plane":"XZ","name":"Ring plate"})).unwrap().value["sketch"].as_u64().unwrap();
    adapter.execute("sketch.rectangle.two_point", json!({"p0":[0,0],"p1":[40,30]})).unwrap();
    adapter.execute("sketch.circle.center", json!({"center":[20,15],"radius":3})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    let edit = adapter.begin_model(751, Some("solid.extrude"), None, &[Sel::Feature { id: sketch }]).unwrap();
    let descriptor = adapter.model_descriptor(&edit);
    let profiles = descriptor["fields"].as_array().unwrap().iter().find(|field| field["key"] == "profile").unwrap()["options"].as_array().unwrap();
    let outer = profiles.iter().find(|option| option["value"]["profiles"][0]["curves"].as_array().is_some_and(|curves| curves.len() == 4)).unwrap();
    let mut params = edit.params.clone();
    params["profile"] = outer["value"].clone();
    params["distance"] = json!("5 mm");
    modelling_commit(&mut adapter, &edit, params);
    let expected = (1200.0 - 9.0 * std::f64::consts::PI) * 5.0;
    assert!((modelling_volume(&adapter) - expected).abs() < 0.05);
    let world = adapter.session.world_state().unwrap();
    assert_eq!(world.bodies.len(), 1);
    let bounds = world.bodies[0].mesh().bounds();
    assert!(bounds.min.dist(Vec3::new(0.0, 0.0, -30.0)) < 1e-6, "{bounds:?}");
    assert!(bounds.max.dist(Vec3::new(40.0, 5.0, 0.0)) < 1e-6, "{bounds:?}");
    let feature = adapter.session.doc.features.last().unwrap().id;
    let edit = adapter.begin_model(752, None, Some(feature), &[]).unwrap();
    assert!(edit.handle.as_ref().unwrap().normal.dist(Vec3::Y) < 1e-7);
    let loaded = modelling_roundtrip(&adapter);
    assert!((modelling_volume(&loaded) - expected).abs() < 0.05);
}

#[test]
fn modelling_mixed_body_edge_and_removal_face_selections_are_rejected_explicitly() {
    let mut adapter = SessionAdapter::new();
    for (name, x) in [("First body", 0), ("Second body", 50)] {
        adapter.execute("solid.box", json!({"length":20,"width":20,"height":20,"corner":[x,0,0],"body_name":name,"operation":"new"})).unwrap();
    }
    let edges = [
        modelling_edge(&mut adapter, "First body", Vec3::new(20.0, 0.0, 10.0)),
        modelling_edge(&mut adapter, "Second body", Vec3::new(70.0, 0.0, 10.0)),
    ];
    for operation in ["solid.fillet", "solid.chamfer"] {
        let failure = match adapter.begin_model(753, Some(operation), None, &edges) {
            Err(failure) => failure,
            Ok(_) => panic!("{operation} cannot silently discard an edge on another body"),
        };
        assert!(failure.message.to_ascii_lowercase().contains("body"), "{}", failure.message);
    }
    let mut faces = Vec::new();
    for body in ["First body", "Second body"] {
        let result = adapter.execute("model.faces", json!({"body":body})).unwrap().value;
        let face = result["faces"].as_array().unwrap().iter().find(|face| face["plane_normal"][2].as_f64().is_some_and(|n| n > 0.999)).unwrap();
        faces.push(Sel::Face {
            body: body.into(),
            index: face["index"].as_u64().unwrap() as usize,
            point: serde_json::from_value(face["point"].clone()).unwrap(),
        });
    }
    let failure = match adapter.begin_model(754, Some("solid.shell"), None, &faces) {
        Err(failure) => failure,
        Ok(_) => panic!("shell cannot silently use another body's removal face"),
    };
    assert!(failure.message.to_ascii_lowercase().contains("body"), "{}", failure.message);
    assert_eq!(adapter.session.doc.features.len(), 2);
    assert!((modelling_volume(&adapter) - 16_000.0).abs() < 1e-6);
}

#[test]
fn modelling_original_new_body_edit_requires_explicit_targets_when_switched_to_cut() {
    let (adapter, _, _, feature) = modelling_plate(40.0, "10 mm");
    let edit = adapter.begin_model(755, None, Some(feature), &[]).unwrap();
    let descriptor = adapter.model_descriptor(&edit);
    assert!(
        descriptor["fields"].as_array().unwrap().iter().any(|field| field["key"] == "targets"),
        "an existing new-body feature must expose targets when its mode changes"
    );
    let mut params = edit.params.clone();
    params["operation"] = json!("cut");
    params["targets"] = json!([]);
    let failure = match adapter.prepare_model(&edit, params, false) {
        Err(failure) => failure,
        Ok(_) => panic!("an edited extrusion cannot guess which bodies to cut"),
    };
    assert!(failure.message.to_ascii_lowercase().contains("target"), "{}", failure.message);
    assert!((modelling_volume(&adapter) - 12_000.0).abs() < 1e-6);
}

#[test]
fn modelling_handle_result_is_rejected_after_a_newer_numeric_form_proposal() {
    let (adapter, mut viewport, _) = modelling_input_fixture();
    let start = modelling_handle_position(&viewport, 10.0);
    feed(&mut viewport, InputSource::Pencil, InputPhase::Began, 91, start, 1.0);
    let moved = modelling_handle_position(&viewport, 15.0);
    feed(&mut viewport, InputSource::Pencil, InputPhase::Moved, 91, moved, 2.0);
    let work = viewport.take_tool_work().unwrap();
    let pending = adapter.prepare_tool(work.clone()).unwrap();
    let edit = viewport.model_edit(750).unwrap();
    let mut params = edit.params.clone();
    params["extent"]["distance"] = json!("20 mm");
    let newer = adapter.refresh_model(&edit, &[], Some(params)).unwrap();
    viewport.update_model(newer.clone()).unwrap();
    let current = adapter.prepare_model(&newer, newer.params.clone(), false).unwrap();
    viewport.publish_model(750, &current).unwrap();
    assert!(!viewport.publish_preview(&work, pending.model), "late handle geometry cannot overwrite a newer precise value");
    assert!((viewport.snapshot().unwrap().model.volume_mm3 - 24_000.0).abs() < 1e-5);
    assert_eq!(viewport.model_edit(750).unwrap().params["extent"]["distance"], "20 mm");
    assert!((modelling_volume(&adapter) - 12_000.0).abs() < 1e-6);
}

#[test]
fn modelling_failed_downstream_profile_can_reselect_original_feature_and_cancel_repair_safely() {
    let mut adapter = SessionAdapter::new();
    let sketch = adapter.execute("sketch.create", json!({"plane":"XY","name":"Repair sketch"})).unwrap().value["sketch"].as_u64().unwrap();
    let first = adapter.execute("sketch.rectangle.two_point", json!({"p0":[0,0],"p1":[10,10]})).unwrap().value;
    adapter.execute("sketch.rectangle.two_point", json!({"p0":[20,0],"p1":[30,10]})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    let feature = adapter
        .execute("solid.extrude", json!({"sketch":sketch,"profiles":[{"curves":first["curves"]}],"distance":"10 mm","operation":"new"}))
        .unwrap()
        .value["feature"]
        .as_u64()
        .unwrap();
    let mut viewport = Viewport::default();
    viewport.set_metrics(metrics(800.0, 600.0, 2.0)).unwrap();
    viewport.begin_update(1);
    assert!(viewport.install(1, adapter.snapshot().unwrap()));
    adapter.execute("sketch.edit", json!({"sketch":sketch})).unwrap();
    adapter.execute("sketch.delete", json!({"entities":[first["sides"]["bottom"]]})).unwrap();
    adapter.execute("sketch.finish", json!({})).unwrap();
    assert!(adapter.session.model.result(feature).unwrap().error.is_some());
    assert!(adapter.snapshot().is_err(), "last-good geometry is not current regenerated geometry");
    let failed_doc = adapter.session.doc.clone();
    viewport.begin_update(2);
    viewport.fail_update(2);
    assert!(viewport.is_updating());
    let edit = adapter.begin_model(756, None, Some(feature), &[]).unwrap();
    viewport.begin_model(edit).unwrap();
    assert!(!viewport.is_updating(), "repair uses an explicitly bounded upstream picking scene");
    assert!(viewport.model_edit(756).is_ok());
    viewport.cancel_model();
    assert!(viewport.is_updating(), "cancel restores the failed-state display contract");
    assert!(Arc::ptr_eq(&failed_doc, &adapter.session.doc));
    assert!(adapter.session.model.result(feature).unwrap().error.is_some());
    let edit = adapter.begin_model(757, None, Some(feature), &[]).unwrap();
    let descriptor = adapter.model_descriptor(&edit);
    let profile = descriptor["fields"].as_array().unwrap().iter().find(|field| field["key"] == "profile").unwrap()["options"][0]["value"].clone();
    assert!(!profile.is_null(), "the surviving region is an explicit repair choice");
    viewport.begin_model(edit.clone()).unwrap();
    let mut params = edit.params.clone();
    params["profile"] = profile;
    let prepared = adapter.prepare_model(&edit, params, true).unwrap();
    let model = adapter.accept_model(&edit, prepared).unwrap();
    viewport.finish_model(757, Some(model)).unwrap();
    assert!(!viewport.is_updating());
    assert_eq!(adapter.session.doc.features.last().unwrap().id, feature);
    assert!((modelling_volume(&adapter) - 1_000.0).abs() < 1e-6);
    assert!((adapter.session.world_state().unwrap().bodies[0].mesh().bounds().min.x - 20.0).abs() < 1e-6);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!(adapter.session.model.result(feature).unwrap().error.is_some(), "undo retains the recoverable failed history");
    adapter.execute("edit.redo", json!({})).unwrap();
    assert!((modelling_volume(&adapter) - 1_000.0).abs() < 1e-6);
    assert_eq!(*modelling_roundtrip(&adapter).session.doc, *adapter.session.doc);
}

#[test]
fn modelling_feature_and_path_patterns_preserve_native_sources_and_regenerate_after_source_edit() {
    for (operation, source_mode) in [
        ("solid.pattern.rectangular", "features"),
        ("solid.pattern.circular", "features"),
        ("solid.pattern.path", "features"),
        ("solid.pattern.path", "bodies"),
    ] {
        let mut adapter = SessionAdapter::new();
        let seed = adapter
            .execute("solid.box", json!({"length":2,"width":2,"height":2,"corner":[10,0,0],"name":"Pattern seed","body_name":"Pattern seed body"}))
            .unwrap()
            .value["feature"]
            .as_u64()
            .unwrap();
        let path = if operation.ends_with("path") {
            let sketch = adapter.execute("sketch.create", json!({"plane":"XY","name":"Pattern path"})).unwrap().value["sketch"].as_u64().unwrap();
            let curve = adapter.execute("sketch.line", json!({"points":[[0,0],[80,0]]})).unwrap().value["curves"][0].as_str().unwrap().to_string();
            adapter.execute("sketch.finish", json!({})).unwrap();
            Some((sketch, curve))
        } else {
            None
        };
        let edit = adapter.begin_model(758, Some(operation), None, &[Sel::Body { name: "Pattern seed body".into() }]).unwrap();
        let mut params = edit.params.clone();
        params["source_mode"] = json!(source_mode);
        params["features"] = json!(["Pattern seed"]);
        params["bodies"] = json!(["Pattern seed body"]);
        if let Some((sketch, curve)) = &path {
            params["path_sketch"] = json!(sketch);
            params["path_curves"] = json!([{"sketch":sketch,"curve":curve}]);
        }
        modelling_commit(&mut adapter, &edit, params);
        let feature = adapter.session.doc.features.last().unwrap().id;
        let edit = adapter.begin_model(759, None, Some(feature), &[]).unwrap();
        assert_eq!(edit.params["source_mode"], source_mode);
        let mut params = edit.params.clone();
        let key = if operation.ends_with("rectangular") { "count1" } else { "count" };
        let old_count = adapter.session.doc.eval(params["pattern"][key].as_str().unwrap(), solidcraft_engine::doc::expr::Kind::Unitless).unwrap();
        params["pattern"][key] = json!("4");
        modelling_check_edit(&mut adapter, feature, params, 8.0 * old_count, 32.0, 1e-6);
        assert_eq!(adapter.session.model.state().bodies.len(), 4);
        let FeatureKind::Pattern { features, bodies, .. } = &adapter.session.doc.feature(feature).unwrap().kind else { panic!("native pattern") };
        if source_mode == "features" {
            assert_eq!(features.iter().map(|feature| feature.0).collect::<Vec<_>>(), [seed]);
            assert!(bodies.is_empty());
        } else {
            assert!(features.is_empty());
            assert_eq!(bodies.iter().map(|body| body.0.as_str()).collect::<Vec<_>>(), ["Pattern seed body"]);
        }
        let edit = adapter.begin_model(760, None, Some(feature), &[]).unwrap();
        let mut invalid = edit.params.clone();
        invalid["pattern"][key] = json!("0");
        assert!(adapter.prepare_model(&edit, invalid, false).is_err());
        adapter.execute("timeline.edit", json!({"feature":seed,"set":{"height":"3 mm"}})).unwrap();
        assert!((modelling_volume(&adapter) - 48.0).abs() < 1e-6, "{operation} follows its source feature");
        assert_eq!(adapter.session.doc.features.last().unwrap().id, feature);
        assert!((modelling_volume(&modelling_roundtrip(&adapter)) - 48.0).abs() < 1e-6);
    }
}

#[test]
fn modelling_through_all_extrusion_and_hole_can_edit_original_to_blind_depth() {
    for operation in ["solid.extrude", "solid.hole"] {
        let (mut adapter, _, _, _) = modelling_plate(60.0, "15 mm");
        let target = adapter.session.world_state().unwrap().bodies[0].name.clone();
        let sketch = adapter.execute("sketch.create", json!({"plane":"XY","offset":"15 mm","name":"Through profile"})).unwrap().value["sketch"]
            .as_u64()
            .unwrap();
        adapter.execute("sketch.circle.center", json!({"center":[30,15],"radius":3})).unwrap();
        adapter.execute("sketch.finish", json!({})).unwrap();
        let edit = adapter.begin_model(761, Some(operation), None, &[Sel::Feature { id: sketch }]).unwrap();
        assert!(adapter.model_descriptor(&edit)["fields"].as_array().unwrap().iter().any(|field| field["key"] == "through_all"));
        let mut params = edit.params.clone();
        params["targets"] = json!([target]);
        params["through_all"] = json!(true);
        if operation == "solid.extrude" {
            params["operation"] = json!("cut");
            params["direction"] = json!("negative");
        } else {
            params["diameter"] = json!("6 mm");
        }
        modelling_commit(&mut adapter, &edit, params);
        let feature = adapter.session.doc.features.last().unwrap().id;
        let edit = adapter.begin_model(762, None, Some(feature), &[]).unwrap();
        let mut params = edit.params.clone();
        if operation == "solid.extrude" {
            assert_eq!(params["extent"]["through_all"], true);
            params["extent"]["through_all"] = json!(false);
            params["extent"]["distance"] = json!("5 mm");
        } else {
            assert_eq!(params["through_all"], true);
            params["through_all"] = json!(false);
            params["depth"] = json!("5 mm");
        }
        let pi = std::f64::consts::PI;
        // The same relative 5e-4 curved-volume allowance as the cut acceptance,
        // bounded here by the smaller analytic blind-hole removal volume.
        modelling_check_edit(&mut adapter, feature, params, 27_000.0 - 135.0 * pi, 27_000.0 - 45.0 * pi, 45.0 * pi * 5e-4);
        assert_eq!(adapter.session.model.state().bodies.len(), 1);
    }
}

fn assembly_accept(adapter: &mut SessionAdapter, operation: &str, params: Value, selection: &[Sel]) -> Value {
    let edit = adapter.begin_model(800, Some(operation), None, selection).unwrap();
    let history = adapter.session.undo.len();
    let document = adapter.session.doc.clone();
    let prepared = adapter.prepare_model(&edit, params, true).unwrap();
    let reply = prepared.reply.clone();
    assert!(Arc::ptr_eq(&document, &adapter.session.doc), "assembly preparation is provisional");
    assert_eq!(adapter.session.undo.len(), history);
    adapter.accept_model(&edit, prepared).unwrap();
    assert_eq!(adapter.session.undo.len(), history + 1, "one assembly operation is one native undo action");
    reply
}

fn assembly_marker(adapter: &SessionAdapter, path: &[u64], local: Vec3) -> Vec3 {
    solidcraft_engine::doc::apply_point(&adapter.session.occurrence_placement(path).unwrap().world, local)
}

fn assembly_volume(adapter: &SessionAdapter, path: &[u64]) -> f64 {
    let placement = adapter.session.occurrence_placement(path).unwrap();
    let world = adapter.session.world_state().unwrap();
    placement.bodies.iter().map(|placed| world.body(&placed.world).unwrap().measure().unwrap().volume).sum()
}

fn assembly_create_component(adapter: &mut SessionAdapter, name: &str, grounded: bool) -> (u64, u64) {
    let created = assembly_accept(adapter, "component.create", json!({"name":name,"parent":0,"activate":false,"grounded":grounded}), &[]);
    (created["component"].as_u64().unwrap(), created["occurrence"].as_u64().unwrap())
}

#[test]
fn assembly_component_transfer_instances_and_independent_copy_keep_native_history_and_geometry() {
    let mut adapter = SessionAdapter::new();
    let feature = adapter
        .execute("solid.box", json!({"length":40,"width":30,"height":10,"name":"Shared plate source","body_name":"Shared plate"}))
        .unwrap()
        .value["feature"]
        .as_u64()
        .unwrap();
    let original = adapter.snapshot().unwrap();
    let (component, first) = assembly_create_component(&mut adapter, "Plate component", false);
    assembly_accept(&mut adapter, "component.move_bodies", json!({"bodies":["Shared plate"],"component":component,"component_path":[first]}), &[]);
    assert!(adapter.session.doc.features.iter().any(|feature| matches!(feature.kind, FeatureKind::BodyTransfer { .. })));
    assert!((assembly_volume(&adapter, &[first]) - 12_000.0).abs() < 1e-6);
    assert_eq!(adapter.snapshot().unwrap().bounds, original.bounds, "transfer preserves the actual world pose");
    let copied = assembly_accept(
        &mut adapter,
        "occurrence.copy",
        json!({"occurrence":first,"path":[first],"parent":0,"translate":[60,0,0]}),
        &[Sel::Occurrence { path: vec![first] }],
    );
    let second = copied["occurrence"].as_u64().unwrap();
    assert_eq!(adapter.session.doc.occurrences.iter().find(|occurrence| occurrence.id == second).unwrap().component, component);
    assert_eq!(adapter.session.doc.components.len(), 1, "an instance shares its component definition");
    assert!(assembly_marker(&adapter, &[second], Vec3::ZERO).dist(Vec3::new(60.0, 0.0, 0.0)) < 1e-9);
    adapter.execute("timeline.edit", json!({"feature":feature,"set":{"length":"60 mm"}})).unwrap();
    for occurrence in [first, second] {
        assert!((assembly_volume(&adapter, &[occurrence]) - 18_000.0).abs() < 1e-6);
    }
    let independent = assembly_accept(
        &mut adapter,
        "component.paste_new",
        json!({"component":component,"occurrence":first,"path":[first],"parent":0,"translate":[0,60,0]}),
        &[Sel::Occurrence { path: vec![first] }],
    );
    let independent_component = independent["component"].as_u64().unwrap();
    assert_ne!(independent_component, component);
    let independent_occurrence = adapter.session.doc.occurrence_of(independent_component).unwrap().id;
    assert!((assembly_volume(&adapter, &[independent_occurrence]) - 18_000.0).abs() < 1e-6);
    adapter.execute("timeline.edit", json!({"feature":feature,"set":{"length":"80 mm"}})).unwrap();
    assert!((assembly_volume(&adapter, &[first]) - 24_000.0).abs() < 1e-6);
    assert!((assembly_volume(&adapter, &[second]) - 24_000.0).abs() < 1e-6);
    assert!((assembly_volume(&adapter, &[independent_occurrence]) - 18_000.0).abs() < 1e-6);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!((assembly_volume(&adapter, &[first]) - 18_000.0).abs() < 1e-6);
    adapter.execute("edit.redo", json!({})).unwrap();
    let loaded = modelling_roundtrip(&adapter);
    assert_eq!(*loaded.session.doc, *adapter.session.doc);
    assert!((assembly_volume(&loaded, &[second]) - 24_000.0).abs() < 1e-6);
    assert!((assembly_volume(&loaded, &[independent_occurrence]) - 18_000.0).abs() < 1e-6);
}

#[test]
fn assembly_nested_exact_paths_use_world_axes_and_preserve_shared_definition_semantics() {
    let mut adapter = SessionAdapter::new();
    let (parent_component, parent) = assembly_create_component(&mut adapter, "Rotated parent", false);
    assembly_accept(
        &mut adapter,
        "occurrence.move",
        json!({"occurrence":parent,"path":[parent],"space":"world","translate":[20,30,0],"axis":[0,0,1],"angle":"90 deg","capture":true}),
        &[Sel::Occurrence { path: vec![parent] }],
    );
    let created = assembly_accept(
        &mut adapter,
        "component.create",
        json!({"name":"Nested leaf","parent":parent_component,"parent_path":[parent],"activate":false,"grounded":false}),
        &[],
    );
    let child_component = created["component"].as_u64().unwrap();
    let child = created["occurrence"].as_u64().unwrap();
    adapter.execute("component.activate", json!({"component":child_component,"occurrence":child,"path":[parent,child]})).unwrap();
    adapter.execute("solid.box", json!({"length":2,"width":2,"height":2,"body_name":"Nested marker body"})).unwrap();
    adapter.execute("component.activate", json!({"component":0})).unwrap();
    assembly_accept(
        &mut adapter,
        "occurrence.move",
        json!({"occurrence":child,"path":[parent,child],"space":"world","translate":[5,0,0],"capture":true}),
        &[Sel::Occurrence { path: vec![parent, child] }],
    );
    let marker = Vec3::new(10.0, 0.0, 0.0);
    assert!(assembly_marker(&adapter, &[parent, child], marker).dist(Vec3::new(25.0, 40.0, 0.0)) < 1e-8);
    let copied = assembly_accept(
        &mut adapter,
        "occurrence.copy",
        json!({"occurrence":parent,"path":[parent],"parent":0,"translate":[100,0,0],"in_place":true}),
        &[Sel::Occurrence { path: vec![parent] }],
    );
    let second_parent = copied["occurrence"].as_u64().unwrap();
    assert!(assembly_marker(&adapter, &[second_parent, child], marker).dist(Vec3::new(125.0, 40.0, 0.0)) < 1e-8);
    assembly_accept(
        &mut adapter,
        "occurrence.move",
        json!({"occurrence":child,"path":[second_parent,child],"space":"world","translate":[0,10,0],"capture":true}),
        &[Sel::Occurrence { path: vec![second_parent, child] }],
    );
    // The native child definition is shared by both repeated parents.
    assert!(assembly_marker(&adapter, &[parent, child], marker).dist(Vec3::new(25.0, 50.0, 0.0)) < 1e-8);
    assert!(assembly_marker(&adapter, &[second_parent, child], marker).dist(Vec3::new(125.0, 50.0, 0.0)) < 1e-8);
    let edit = adapter.begin_model(801, Some("occurrence.move"), None, &[Sel::Occurrence { path: vec![second_parent, child] }]).unwrap();
    let document = adapter.session.doc.clone();
    assert!(
        adapter.prepare_model(&edit, json!({"occurrence":child,"path":[child],"space":"world","translate":[10,0,0]}), false).is_err(),
        "ambiguous leaf ids cannot replace a full path"
    );
    assert!(Arc::ptr_eq(&document, &adapter.session.doc));
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!(assembly_marker(&adapter, &[second_parent, child], marker).dist(Vec3::new(125.0, 40.0, 0.0)) < 1e-8);
    adapter.execute("edit.redo", json!({})).unwrap();
    let loaded = modelling_roundtrip(&adapter);
    assert!(assembly_marker(&loaded, &[parent, child], marker).dist(Vec3::new(25.0, 50.0, 0.0)) < 1e-8);
    assert!(assembly_marker(&loaded, &[second_parent, child], marker).dist(Vec3::new(125.0, 50.0, 0.0)) < 1e-8);
}

#[test]
fn assembly_position_capture_preserves_unrelated_pending_moves_and_grounded_parts_refuse_movement() {
    let mut adapter = SessionAdapter::new();
    let (_, first) = assembly_create_component(&mut adapter, "First free", false);
    let (_, second) = assembly_create_component(&mut adapter, "Second free", false);
    for (occurrence, translation) in [(first, [10, 0, 0]), (second, [0, 20, 0])] {
        adapter
            .execute("occurrence.move", json!({"occurrence":occurrence,"path":[occurrence],"space":"world","translate":translation,"capture":false}))
            .unwrap();
    }
    assembly_accept(&mut adapter, "component.capture_position", json!({"occurrences":[first]}), &[Sel::Occurrence { path: vec![first] }]);
    assert!(!adapter.session.pending_moves.contains_key(&first));
    assert!(adapter.session.pending_moves.contains_key(&second));
    assert!(assembly_marker(&adapter, &[first], Vec3::ZERO).dist(Vec3::new(10.0, 0.0, 0.0)) < 1e-9);
    assert!(assembly_marker(&adapter, &[second], Vec3::ZERO).dist(Vec3::new(0.0, 20.0, 0.0)) < 1e-9);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!(adapter.session.pending_moves.contains_key(&first));
    assert!(adapter.session.pending_moves.contains_key(&second));
    adapter.execute("edit.redo", json!({})).unwrap();
    assert!(!adapter.session.pending_moves.contains_key(&first));
    assert!(adapter.session.pending_moves.contains_key(&second));
    assembly_accept(
        &mut adapter,
        "occurrence.ground",
        json!({"occurrence":first,"path":[first],"grounded":true}),
        &[Sel::Occurrence { path: vec![first] }],
    );
    let edit = adapter.begin_model(802, Some("occurrence.move"), None, &[Sel::Occurrence { path: vec![first] }]).unwrap();
    let document = adapter.session.doc.clone();
    let failure = match adapter.prepare_model(&edit, json!({"occurrence":first,"path":[first],"translate":[5,0,0]}), false) {
        Err(failure) => failure,
        Ok(_) => panic!("a grounded placement must not move"),
    };
    assert!(failure.message.to_ascii_lowercase().contains("grounded"));
    assert!(Arc::ptr_eq(&document, &adapter.session.doc));
}

fn assembly_part(adapter: &mut SessionAdapter, name: &str, grounded: bool, shape: Value) -> (u64, u64) {
    let (component, occurrence) = assembly_create_component(adapter, name, grounded);
    adapter.execute("component.activate", json!({"component":component,"occurrence":occurrence,"path":[occurrence]})).unwrap();
    adapter.execute("solid.box", shape).unwrap();
    adapter.execute("component.activate", json!({"component":0})).unwrap();
    (component, occurrence)
}

fn assembly_pair() -> (SessionAdapter, u64, u64) {
    let mut adapter = SessionAdapter::new();
    let (_, base) =
        assembly_part(&mut adapter, "Grounded frame", true, json!({"length":10,"width":10,"height":10,"corner":[-5,-5,-5],"body_name":"Frame body"}));
    let (_, arm) = assembly_part(&mut adapter, "Moving arm", false, json!({"length":100,"width":2,"height":2,"body_name":"Arm body"}));
    (adapter, base, arm)
}

fn assembly_joint_origins(base: u64, arm: u64, axis: [f64; 3]) -> Value {
    json!({"a":{"occurrence":base,"point":[0,0,0],"z":axis},"b":{"occurrence":arm,"point":[0,0,0],"z":axis},"flip":true})
}

fn assembly_pair_selection(base: u64, arm: u64) -> Vec<Sel> {
    vec![Sel::Occurrence { path: vec![base] }, Sel::Occurrence { path: vec![arm] }]
}

#[test]
fn assembly_grounded_hinge_preview_drive_edit_and_roundtrip_match_analytic_marker_poses() {
    let (mut adapter, base, arm) = assembly_pair();
    let before = adapter.session.doc.occurrences.clone();
    let mut params = assembly_joint_origins(base, arm, [0.0, 0.0, 1.0]);
    params["type"] = json!("revolute");
    params["name"] = json!("Analytic hinge");
    let joint = assembly_accept(&mut adapter, "joint.create", params, &assembly_pair_selection(base, arm))["joint"].as_u64().unwrap();
    assert_eq!(adapter.session.doc.occurrences, before, "matching origins with explicit flip preserve the initial pose");
    let marker = Vec3::new(100.0, 0.0, 0.0);
    let edit = adapter.begin_model(803, Some("joint.drive"), None, &[]).unwrap();
    let document = adapter.session.doc.clone();
    let history = adapter.session.undo.len();
    let preview = adapter.prepare_model(&edit, json!({"joint":joint,"value":"90 deg"}), false).unwrap();
    assert_eq!(preview.reply["moved"], true);
    assert_eq!(preview.reply["values"][0]["unit"], "deg");
    assert!((preview.reply["values"][0]["value"].as_f64().unwrap() - 90.0).abs() < 1e-9);
    assert!((preview.model.bounds.max.y - 100.0).abs() < 1e-6, "preview uses the actual solved world geometry");
    drop(preview);
    assert!(Arc::ptr_eq(&document, &adapter.session.doc));
    assert_eq!(adapter.session.undo.len(), history);
    assert!(assembly_marker(&adapter, &[arm], marker).dist(marker) < 1e-8);
    let drive = assembly_accept(&mut adapter, "joint.drive", json!({"joint":joint,"value":"90 deg"}), &[]);
    assert_eq!(drive["moved"], true);
    assert!(assembly_marker(&adapter, &[arm], marker).dist(Vec3::new(0.0, 100.0, 0.0)) < 1e-6);
    assembly_accept(&mut adapter, "joint.drive", json!({"joint":joint,"value":"30 deg"}), &[]);
    let expected = Vec3::new(100.0 * 30_f64.to_radians().cos(), 50.0, 0.0);
    assert!(assembly_marker(&adapter, &[arm], marker).dist(expected) < 1e-6);
    assert!(assembly_marker(&adapter, &[base], Vec3::ZERO).dist(Vec3::ZERO) < 1e-9);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!(assembly_marker(&adapter, &[arm], marker).dist(Vec3::new(0.0, 100.0, 0.0)) < 1e-6);
    adapter.execute("edit.redo", json!({})).unwrap();
    let loaded = modelling_roundtrip(&adapter);
    assert_eq!(*loaded.session.doc, *adapter.session.doc);
    assert_eq!(loaded.session.doc.assembly.joints[0].id, joint);
    assert!(assembly_marker(&loaded, &[arm], marker).dist(expected) < 1e-6);
    assembly_accept(&mut adapter, "joint.edit", json!({"joint":joint,"name":"Edited hinge","offset":"2 mm"}), &[]);
    assert_eq!(adapter.session.doc.assembly.joints[0].id, joint);
    assert_eq!(adapter.session.doc.assembly.joints[0].name, "Edited hinge");
    assert!(assembly_marker(&adapter, &[arm], marker).dist(expected + Vec3::new(0.0, 0.0, 2.0)) < 1e-6);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!(assembly_marker(&adapter, &[arm], marker).dist(expected) < 1e-6);
    adapter.execute("edit.redo", json!({})).unwrap();
    assert!(assembly_marker(&modelling_roundtrip(&adapter), &[arm], marker).dist(expected + Vec3::new(0.0, 0.0, 2.0)) < 1e-6);
}

#[test]
fn assembly_slider_axis_limits_actual_value_and_history_are_engine_authoritative() {
    let (mut adapter, base, slider) = assembly_pair();
    let mut params = assembly_joint_origins(base, slider, [1.0, 0.0, 0.0]);
    params["type"] = json!("slider");
    params["name"] = json!("Analytic rail");
    params["limits"] = json!([[0, 50]]);
    let joint = assembly_accept(&mut adapter, "joint.create", params, &assembly_pair_selection(base, slider))["joint"].as_u64().unwrap();
    let drive = assembly_accept(&mut adapter, "joint.drive", json!({"joint":joint,"value":"20 mm"}), &[]);
    assert_eq!(drive["moved"], true);
    assert_eq!(drive["values"][0]["unit"], "mm");
    assert!((drive["values"][0]["value"].as_f64().unwrap() - 20.0).abs() < 1e-9);
    assert!(assembly_marker(&adapter, &[slider], Vec3::ZERO).dist(Vec3::new(20.0, 0.0, 0.0)) < 1e-6);
    let drive = assembly_accept(&mut adapter, "joint.drive", json!({"joint":joint,"value":"75 mm"}), &[]);
    assert_eq!(drive["moved"], true);
    assert_eq!(drive["values"][0]["limits"], json!([0.0, 50.0]));
    assert!((drive["values"][0]["value"].as_f64().unwrap() - 50.0).abs() < 1e-9, "report the clamped actual position");
    assert!(assembly_marker(&adapter, &[slider], Vec3::ZERO).dist(Vec3::new(50.0, 0.0, 0.0)) < 1e-6);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!(assembly_marker(&adapter, &[slider], Vec3::ZERO).dist(Vec3::new(20.0, 0.0, 0.0)) < 1e-6);
    adapter.execute("edit.redo", json!({})).unwrap();
    assembly_accept(&mut adapter, "joint.limits", json!({"joint":joint,"min":"0 mm","max":"40 mm"}), &[]);
    assert_eq!(adapter.session.doc.assembly.joints[0].id, joint);
    assert!(assembly_marker(&adapter, &[slider], Vec3::ZERO).dist(Vec3::new(40.0, 0.0, 0.0)) < 1e-6);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!(assembly_marker(&adapter, &[slider], Vec3::ZERO).dist(Vec3::new(50.0, 0.0, 0.0)) < 1e-6);
    adapter.execute("edit.redo", json!({})).unwrap();
    let loaded = modelling_roundtrip(&adapter);
    assert_eq!(*loaded.session.doc, *adapter.session.doc);
    assert!(assembly_marker(&loaded, &[slider], Vec3::ZERO).dist(Vec3::new(40.0, 0.0, 0.0)) < 1e-6);
}

#[test]
fn assembly_as_built_joint_preserves_actual_transformed_poses_and_rigid_drive_is_refused() {
    let (mut adapter, base, arm) = assembly_pair();
    assembly_accept(
        &mut adapter,
        "occurrence.move",
        json!({"occurrence":arm,"path":[arm],"space":"world","translate":[20,15,10],"axis":[0,0,1],"angle":"37 deg","capture":true}),
        &[Sel::Occurrence { path: vec![arm] }],
    );
    let before = adapter.session.doc.occurrences.clone();
    let marker = assembly_marker(&adapter, &[arm], Vec3::new(100.0, 0.0, 0.0));
    let joint = assembly_accept(
        &mut adapter,
        "joint.as_built",
        json!({"type":"rigid","a":base,"b":arm,"at":{"occurrence":base,"point":[0,0,0],"z":[0,0,1]},"name":"As built fixed"}),
        &assembly_pair_selection(base, arm),
    )["joint"]
        .as_u64()
        .unwrap();
    assert_eq!(adapter.session.doc.occurrences, before);
    assert!(assembly_marker(&adapter, &[arm], Vec3::new(100.0, 0.0, 0.0)).dist(marker) < 1e-8);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!(adapter.session.doc.assembly.joints.is_empty());
    assert_eq!(adapter.session.doc.occurrences, before);
    adapter.execute("edit.redo", json!({})).unwrap();
    let edit = adapter.begin_model(804, Some("joint.drive"), None, &[]).unwrap();
    let document = adapter.session.doc.clone();
    let failure = match adapter.prepare_model(&edit, json!({"joint":joint,"value":90}), false) {
        Err(failure) => failure,
        Ok(_) => panic!("a fixed joint cannot report driven motion"),
    };
    assert!(failure.message.to_ascii_lowercase().contains("rigid"));
    assert!(Arc::ptr_eq(&document, &adapter.session.doc));
    let loaded = modelling_roundtrip(&adapter);
    assert_eq!(*loaded.session.doc, *adapter.session.doc);
    assert!(assembly_marker(&loaded, &[arm], Vec3::new(100.0, 0.0, 0.0)).dist(marker) < 1e-8);
}

#[test]
fn assembly_floating_suppressed_and_unchanged_drives_report_actual_no_motion_reasons() {
    let (mut adapter, grounded_base, grounded_arm) = assembly_pair();
    let mut fixed_chain = assembly_joint_origins(grounded_base, grounded_arm, [0.0, 0.0, 1.0]);
    fixed_chain["type"] = json!("revolute");
    fixed_chain["name"] = json!("Grounded chain");
    assembly_accept(&mut adapter, "joint.create", fixed_chain, &assembly_pair_selection(grounded_base, grounded_arm));
    let (_, base) = assembly_part(&mut adapter, "Floating base", false, json!({"length":2,"width":2,"height":2,"body_name":"Floating base body"}));
    let (_, arm) = assembly_part(&mut adapter, "Floating arm", false, json!({"length":100,"width":2,"height":2,"body_name":"Floating arm body"}));
    let mut params = assembly_joint_origins(base, arm, [0.0, 0.0, 1.0]);
    params["type"] = json!("revolute");
    params["name"] = json!("Floating hinge");
    let joint = assembly_accept(&mut adapter, "joint.create", params, &assembly_pair_selection(base, arm))["joint"].as_u64().unwrap();
    let marker = Vec3::new(100.0, 0.0, 0.0);
    let initial = assembly_marker(&adapter, &[arm], marker);
    let edit = adapter.begin_model(805, Some("joint.drive"), None, &[]).unwrap();
    let prepared = adapter.prepare_model(&edit, json!({"joint":joint,"value":"45 deg"}), true).unwrap();
    assert_eq!(prepared.reply["moved"], false);
    assert!(prepared.reply["reason"].as_str().unwrap().contains("not connected to anything grounded"));
    assert!(prepared.feedback.iter().any(|message| message.contains("not connected to anything grounded")));
    assert!(
        (prepared.reply["values"][0]["value"].as_f64().unwrap() - 45.0).abs() < 1e-9,
        "native drive stores the value even when its floating chain cannot move"
    );
    adapter.accept_model(&edit, prepared).unwrap();
    assert!(assembly_marker(&adapter, &[arm], marker).dist(initial) < 1e-8);
    let status = adapter.execute("joint.list", json!({})).unwrap().value;
    assert!(!status["floating"].as_array().unwrap().is_empty());
    assembly_accept(&mut adapter, "joint.edit", json!({"joint":joint,"suppressed":true}), &[]);
    let edit = adapter.begin_model(806, Some("joint.drive"), None, &[]).unwrap();
    let prepared = adapter.prepare_model(&edit, json!({"joint":joint,"value":"30 deg"}), true).unwrap();
    assert_eq!(prepared.reply["moved"], false);
    assert!(prepared.reply["reason"].as_str().unwrap().contains("suppressed"));
    assert!(prepared.feedback.iter().any(|message| message.contains("suppressed")));
    adapter.accept_model(&edit, prepared).unwrap();
    assert!(assembly_marker(&adapter, &[arm], marker).dist(initial) < 1e-8);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!(assembly_marker(&adapter, &[arm], marker).dist(initial) < 1e-8);
    adapter.execute("edit.redo", json!({})).unwrap();
    let loaded = modelling_roundtrip(&adapter);
    assert!(loaded.session.doc.assembly.joints.iter().find(|candidate| candidate.id == joint).unwrap().suppressed);
    assert!(assembly_marker(&loaded, &[arm], marker).dist(initial) < 1e-8);

    let (mut adapter, base, arm) = assembly_pair();
    let mut params = assembly_joint_origins(base, arm, [0.0, 0.0, 1.0]);
    params["type"] = json!("revolute");
    let joint = assembly_accept(&mut adapter, "joint.create", params, &assembly_pair_selection(base, arm))["joint"].as_u64().unwrap();
    let edit = adapter.begin_model(807, Some("joint.drive"), None, &[]).unwrap();
    let prepared = adapter.prepare_model(&edit, json!({"joint":joint,"value":0}), false).unwrap();
    assert_eq!(prepared.reply["moved"], false);
    assert!(prepared.reply["reason"].is_null());
    assert!(!prepared.feedback.is_empty(), "a no-op preview must explain that no placement moved");
}

#[test]
fn assembly_cancel_stale_results_duplicate_confirmation_and_invalid_joint_inputs_preserve_work() {
    let (mut adapter, base, arm) = assembly_pair();
    let mut params = assembly_joint_origins(base, arm, [0.0, 0.0, 1.0]);
    params["type"] = json!("revolute");
    params["name"] = json!("Single hinge");
    let edit = adapter.begin_model(808, Some("joint.create"), None, &assembly_pair_selection(base, arm)).unwrap();
    let history = adapter.session.undo.len();
    let document = adapter.session.doc.clone();
    let preview = adapter.prepare_model(&edit, params.clone(), false).unwrap();
    drop(preview);
    assert!(Arc::ptr_eq(&document, &adapter.session.doc));
    assert_eq!(adapter.session.undo.len(), history);
    for invalid in [
        json!({"type":"revolute","a":{"occurrence":base,"point":[0,0,0]},"b":{"occurrence":base,"point":[0,0,0]}}),
        json!({"type":"slider","a":{"occurrence":base,"point":[0,0,0]},"b":{"occurrence":999999,"point":[0,0,0]}}),
        json!({"type":"unsupported_joint","a":{"occurrence":base,"point":[0,0,0]},"b":{"occurrence":arm,"point":[0,0,0]}}),
    ] {
        assert!(adapter.prepare_model(&edit, invalid, false).is_err());
        assert!(Arc::ptr_eq(&document, &adapter.session.doc));
    }
    let first = adapter.prepare_model(&edit, params.clone(), true).unwrap();
    let duplicate = adapter.prepare_model(&edit, params, true).unwrap();
    let joint = first.reply["joint"].as_u64().unwrap();
    adapter.accept_model(&edit, first).unwrap();
    assert!(adapter.accept_model(&edit, duplicate).is_err());
    assert_eq!(adapter.session.doc.assembly.joints.len(), 1);
    assert_eq!(adapter.session.undo.len(), history + 1);
    let edit = adapter.begin_model(809, Some("joint.drive"), None, &[]).unwrap();
    let pending = adapter.prepare_model(&edit, json!({"joint":joint,"value":"90 deg"}), true).unwrap();
    let component = adapter.session.doc.occurrences.iter().find(|occurrence| occurrence.id == arm).unwrap().component;
    adapter.execute("component.rename", json!({"component":component,"name":"Renamed while computing"})).unwrap();
    assert!(adapter.accept_model(&edit, pending).is_err());
    assert!(assembly_marker(&adapter, &[arm], Vec3::new(100.0, 0.0, 0.0)).dist(Vec3::new(100.0, 0.0, 0.0)) < 1e-8);
}

#[test]
fn assembly_lost_named_origin_never_attaches_to_replacement_face_and_repairs_same_joint_explicitly() {
    let (mut adapter, base, arm) = assembly_pair();
    let arm_feature = adapter.session.doc.features.iter().find(|feature| feature.body_names.iter().any(|name| name == "Arm body")).unwrap().id;
    let params = json!({"type":"revolute","name":"Repairable hinge","flip":true,
        "a":{"occurrence":base,"point":[50,1,2],"z":[0,0,1]},
        "b":{"occurrence":arm,"face":[50,1,2]}});
    let joint = assembly_accept(&mut adapter, "joint.create", params, &assembly_pair_selection(base, arm))["joint"].as_u64().unwrap();
    adapter.execute("timeline.delete", json!({"feature":arm_feature})).unwrap();
    let inspect = adapter.execute("joint.inspect", json!({"joint":joint})).unwrap().value;
    assert_eq!(inspect["joints"][0]["b"]["resolves"], false);
    assert!(inspect["joints"][0]["b"]["error"].as_str().is_some_and(|message| !message.is_empty()));
    let component = adapter.session.doc.occurrences.iter().find(|occurrence| occurrence.id == arm).unwrap().component;
    adapter.execute("component.activate", json!({"component":component,"occurrence":arm,"path":[arm]})).unwrap();
    adapter.execute("solid.box", json!({"length":100,"width":2,"height":2,"body_name":"Arm body"})).unwrap();
    adapter.execute("component.activate", json!({"component":0})).unwrap();
    let inspect = adapter.execute("joint.inspect", json!({"joint":joint})).unwrap().value;
    assert_eq!(inspect["joints"][0]["b"]["resolves"], false, "a new nearby face with the same body label cannot replace lost native lineage");
    let edit = adapter.begin_model(810, Some("joint.edit"), None, &[]).unwrap();
    let document = adapter.session.doc.clone();
    let params = json!({"joint":joint,"b":{"occurrence":arm,"face":[50,1,2]}});
    let preview = adapter.prepare_model(&edit, params.clone(), false).unwrap();
    drop(preview);
    assert!(Arc::ptr_eq(&document, &adapter.session.doc), "reselection remains provisional until explicit confirmation");
    assembly_accept(&mut adapter, "joint.edit", params, &[]);
    assert_eq!(adapter.session.doc.assembly.joints[0].id, joint);
    let inspect = adapter.execute("joint.inspect", json!({"joint":joint})).unwrap().value;
    assert_eq!(inspect["joints"][0]["b"]["resolves"], true);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert_eq!(adapter.execute("joint.inspect", json!({"joint":joint})).unwrap().value["joints"][0]["b"]["resolves"], false);
    adapter.execute("edit.redo", json!({})).unwrap();
    let mut loaded = modelling_roundtrip(&adapter);
    assert_eq!(loaded.execute("joint.inspect", json!({"joint":joint})).unwrap().value["joints"][0]["b"]["resolves"], true);
    assert_eq!(loaded.session.doc.assembly.joints[0].id, joint);
}

#[test]
fn assembly_joint_origins_refuse_ambiguous_nested_placements_and_pending_source_positions() {
    let (mut adapter, base, arm) = assembly_pair();
    adapter.execute("occurrence.move", json!({"occurrence":arm,"path":[arm],"space":"world","translate":[10,0,0],"capture":false})).unwrap();
    let mut params = assembly_joint_origins(base, arm, [0.0, 0.0, 1.0]);
    params["type"] = json!("revolute");
    let prepared = adapter
        .begin_model(811, Some("joint.create"), None, &assembly_pair_selection(base, arm))
        .and_then(|edit| adapter.prepare_model(&edit, params, false));
    let failure = match prepared {
        Err(failure) => failure,
        Ok(_) => panic!("joint origins cannot silently ignore pending occurrence positions"),
    };
    assert!(
        failure.message.to_ascii_lowercase().contains("pending") || failure.message.to_ascii_lowercase().contains("capture"),
        "{}",
        failure.message
    );
    assert!(adapter.session.pending_moves.contains_key(&arm));
    assert!(adapter.session.doc.assembly.joints.is_empty());
    assembly_accept(&mut adapter, "component.capture_position", json!({"occurrences":[arm]}), &[Sel::Occurrence { path: vec![arm] }]);
    let (parent_component, parent) = assembly_create_component(&mut adapter, "Shared parent", false);
    let child = assembly_accept(
        &mut adapter,
        "component.create",
        json!({"name":"Shared nested origin","parent":parent_component,"parent_path":[parent],"grounded":false,"activate":false}),
        &[],
    )["occurrence"]
        .as_u64()
        .unwrap();
    let second = assembly_accept(
        &mut adapter,
        "occurrence.copy",
        json!({"occurrence":parent,"path":[parent],"parent":0,"translate":[100,0,0]}),
        &[Sel::Occurrence { path: vec![parent] }],
    )["occurrence"]
        .as_u64()
        .unwrap();
    let selected = [Sel::Occurrence { path: vec![base] }, Sel::Occurrence { path: vec![second, child] }];
    let params = json!({"type":"revolute","flip":true,"a":{"occurrence":base,"point":[0,0,0],"z":[0,0,1]},"b":{"occurrence":child,"path":[second,child],"point":[100,0,0],"z":[0,0,1]}});
    let document = adapter.session.doc.clone();
    let prepared = adapter.begin_model(812, Some("joint.create"), None, &selected).and_then(|edit| adapter.prepare_model(&edit, params, false));
    let failure = match prepared {
        Err(failure) => failure,
        Ok(_) => panic!("native joint storage cannot independently address a repeated-parent leaf placement"),
    };
    let explanation = failure.message.to_ascii_lowercase();
    assert!(["placement", "repeated", "shared", "ambiguous"].iter().any(|term| explanation.contains(term)), "{}", failure.message);
    assert!(Arc::ptr_eq(&document, &adapter.session.doc));
    assert!(adapter.session.doc.assembly.joints.is_empty());
}

#[test]
fn assembly_pencil_and_browser_select_exact_occurrences_while_fingers_and_controls_keep_ownership() {
    let (mut adapter, base, arm) = assembly_pair();
    let mut viewport = Viewport::default();
    viewport.set_metrics(metrics(800.0, 600.0, 2.0)).unwrap();
    viewport.begin_update(1);
    assert!(viewport.install(1, adapter.snapshot().unwrap()));
    viewport.command("view.top", &json!({})).unwrap();
    viewport.command("select.filter", &json!({"filter":"occurrence"})).unwrap();
    let point_on_arm = point(&viewport, Vec3::new(50.0, 1.0, 2.0));
    feed(&mut viewport, InputSource::Pencil, InputPhase::Began, 91, point_on_arm, 1.0);
    feed(&mut viewport, InputSource::Pencil, InputPhase::Ended, 91, point_on_arm, 2.0);
    assert_eq!(viewport.selection(), &[Sel::Occurrence { path: vec![arm] }]);
    let workspace = adapter.workspace(viewport.selection(), false, false);
    assert!(
        workspace["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|row| row["entity"] == json!(Sel::Occurrence { path: vec![arm] }) && row["selected"] == true)
    );
    viewport.command("select.entities", &json!({"items":assembly_pair_selection(base,arm)})).unwrap();
    assert_eq!(viewport.selection(), assembly_pair_selection(base, arm));
    let before = viewport.selection().to_vec();
    let camera = viewport.camera;
    let mut consumed = sample(&viewport, InputSource::Pencil, InputPhase::Began, 92, point_on_arm, 3.0);
    consumed.consumed = true;
    viewport.input(consumed).unwrap();
    feed(&mut viewport, InputSource::Pencil, InputPhase::Ended, 92, point_on_arm, 4.0);
    assert_eq!(viewport.selection(), before);
    assert_eq!(viewport.camera, camera);
    feed(&mut viewport, InputSource::Finger, InputPhase::Began, 1, [200.0, 300.0], 5.0);
    feed(&mut viewport, InputSource::Finger, InputPhase::Moved, 1, [260.0, 330.0], 6.0);
    feed(&mut viewport, InputSource::Finger, InputPhase::Ended, 1, [260.0, 330.0], 7.0);
    assert_ne!(viewport.camera, camera);
    assert_eq!(viewport.selection(), before, "navigation cannot change assembly endpoints");
    assert!(viewport.take_tool_work().is_none());
    let stale = sample(&viewport, InputSource::Pencil, InputPhase::Began, 93, [400.0, 300.0], 8.0);
    viewport.set_metrics(metrics(600.0, 800.0, 2.0)).unwrap();
    viewport.input(stale).unwrap();
    assert_eq!(viewport.selection(), before);
}

#[test]
fn assembly_preview_completion_order_cancel_and_document_revision_keep_authoritative_pose() {
    let (mut adapter, base, arm) = assembly_pair();
    let mut params = assembly_joint_origins(base, arm, [0.0, 0.0, 1.0]);
    params["type"] = json!("revolute");
    let joint = assembly_accept(&mut adapter, "joint.create", params, &assembly_pair_selection(base, arm))["joint"].as_u64().unwrap();
    let document = adapter.session.doc.clone();
    let history = adapter.session.undo.len();
    let mut viewport = Viewport::default();
    viewport.set_metrics(metrics(800.0, 600.0, 2.0)).unwrap();
    viewport.begin_update(1);
    assert!(viewport.install(1, adapter.snapshot().unwrap()));
    let edit = adapter.begin_model(813, Some("joint.drive"), None, &[]).unwrap();
    viewport.begin_model(edit.clone()).unwrap();
    let older = adapter.refresh_model(&edit, &[], Some(json!({"joint":joint,"value":"30 deg"}))).unwrap();
    viewport.update_model(older.clone()).unwrap();
    let older_result = adapter.prepare_model(&older, older.params.clone(), false).unwrap();
    let newer = adapter.refresh_model(&older, &[], Some(json!({"joint":joint,"value":"90 deg"}))).unwrap();
    viewport.update_model(newer.clone()).unwrap();
    let newer_result = adapter.prepare_model(&newer, newer.params.clone(), false).unwrap();
    viewport.publish_model(813, &newer_result).unwrap();
    assert!(viewport.publish_model(813, &older_result).is_err());
    assert!((viewport.snapshot().unwrap().model.bounds.max.y - 100.0).abs() < 1e-6);
    assert!(Arc::ptr_eq(&document, &adapter.session.doc));
    assert_eq!(adapter.session.undo.len(), history);
    let camera = viewport.camera;
    feed(&mut viewport, InputSource::Finger, InputPhase::Began, 1, [200.0, 300.0], 1.0);
    feed(&mut viewport, InputSource::Finger, InputPhase::Moved, 1, [250.0, 330.0], 2.0);
    feed(&mut viewport, InputSource::Finger, InputPhase::Ended, 1, [250.0, 330.0], 3.0);
    assert_ne!(viewport.camera, camera);
    viewport.cancel_model();
    assert!(viewport.publish_model(813, &newer_result).is_err());
    assert!(assembly_marker(&adapter, &[arm], Vec3::new(100.0, 0.0, 0.0)).dist(Vec3::new(100.0, 0.0, 0.0)) < 1e-8);
    assert_eq!(adapter.session.undo.len(), history);
    let edit = adapter.begin_model(814, Some("joint.drive"), None, &[]).unwrap();
    viewport.begin_model(edit.clone()).unwrap();
    let pending = adapter.prepare_model(&edit, json!({"joint":joint,"value":"45 deg"}), false).unwrap();
    adapter.execute("joint.edit", json!({"joint":joint,"name":"Renamed during preview"})).unwrap();
    viewport.begin_update(2);
    assert!(viewport.install(2, adapter.snapshot().unwrap()));
    assert!(viewport.publish_model(814, &pending).is_err());
    assert!(viewport.selection().is_empty());
    assert_eq!(adapter.session.doc.assembly.joints[0].name, "Renamed during preview");
}

#[test]
fn assembly_strict_reference_checks_reject_lost_faces_ends_edges_and_x_references() {
    use solidcraft_engine::doc::joints::{Snap, XRef};
    for kind in ["face", "circle", "edge"] {
        let mut adapter = SessionAdapter::new();
        let base = adapter.execute("component.create", json!({"name":"Base","grounded":true})).unwrap().value["occurrence"].as_u64().unwrap();
        adapter.execute("solid.box", json!({"length":20,"width":20,"height":5})).unwrap();
        adapter.execute("component.activate", json!({"component":0})).unwrap();
        let moving = adapter.execute("component.create", json!({"name":"Moving","grounded":false})).unwrap().value["occurrence"].as_u64().unwrap();
        adapter.execute("solid.cylinder", json!({"radius":2,"height":5,"base":[30,10,0]})).unwrap();
        let a = if kind == "face" {
            json!({"occurrence":base,"face":[10,10,5],"x":{"face":[20,10,2.5]}})
        } else if kind == "edge" {
            json!({"occurrence":base,"edge":[10,0,5],"corner":[0,0,5]})
        } else {
            json!({"occurrence":base,"point":[0,0,0],"z":[0,0,1]})
        };
        let b = if kind == "circle" {
            json!({"occurrence":moving,"circle":[32,10,0]})
        } else {
            json!({"occurrence":moving,"frame":{"origin":[30,10,0],"z":[0,0,1],"x":[1,0,0]}})
        };
        let result = adapter.execute("joint.create", json!({"name":"Referenced joint","type":"rigid","a":a,"b":b,"flip":true})).unwrap().value;
        assert_eq!(result["errors"], json!([]), "{kind}: {result}");
        assert_eq!(result["conflicts"], json!([]), "{kind}: {result}");
        adapter.validate_joint_references().unwrap_or_else(|failure| panic!("{kind}: {failure}"));
        let original = adapter.session.doc.clone();
        let history = adapter.session.undo.len();
        let corruptions: &[&str] = match kind {
            "face" => &["face", "x"],
            "circle" => &["face", "end"],
            _ => &["edge", "face", "corner"],
        };
        for role in corruptions {
            let joint = &mut Arc::make_mut(&mut adapter.session.doc).assembly.joints[0];
            let snap = if kind == "circle" { &mut joint.b.snap } else { &mut joint.a.snap };
            match (snap, *role) {
                (Snap::FaceCenter { face, .. } | Snap::CircleCenter { face, .. }, "face") => *face = Some("missing native face".into()),
                (Snap::FaceCenter { x, .. }, "x") => {
                    assert!(matches!(x, Some(XRef::Face { .. })));
                    *x = Some(XRef::Face { face: "missing x face".into(), pick: None });
                }
                (Snap::CircleCenter { end, .. }, "end") => *end = Some("missing cylinder end".into()),
                (Snap::Edge { edge, .. }, "edge") => *edge = Some("missing native edge".into()),
                (Snap::Edge { face, .. }, "face") => *face = Some("missing edge support".into()),
                (Snap::Edge { corner, .. }, "corner") => *corner = Some("missing corner edge".into()),
                _ => panic!("unexpected native {kind} snap"),
            }
            let failed = adapter.session.doc.clone();
            let error = adapter.validate_joint_references().unwrap_err();
            assert_eq!(error.code, "joint_reference");
            assert!(error.message.contains("Referenced joint"), "{}", error.message);
            assert!(Arc::ptr_eq(&failed, &adapter.session.doc), "validation must not repair or retarget a reference");
            assert_eq!(adapter.session.undo.len(), history);
            adapter.session.doc = original.clone();
        }
    }
}

fn assembly_handle_view(adapter: &mut SessionAdapter, edit: ModelEdit) -> Viewport {
    let mut viewport = Viewport::default();
    viewport.set_metrics(metrics(800.0, 600.0, 2.0)).unwrap();
    viewport.begin_update(1);
    assert!(viewport.install(1, adapter.snapshot().unwrap()));
    viewport.command("view.top", &json!({})).unwrap();
    viewport.command("view.navigate", &json!({"motion":"zoom","factor":2.5})).unwrap();
    assert!(edit.handle.is_some(), "the operation exposes a real parameter handle");
    viewport.begin_model(edit).unwrap();
    viewport
}

fn assembly_handle_position(viewport: &Viewport, value: f64) -> [f64; 2] {
    let mut handle = viewport.modelling.as_ref().unwrap().handle.as_ref().unwrap().clone();
    handle.distance = value;
    point(viewport, handle.point())
}

fn assembly_pencil_proposal(adapter: &mut SessionAdapter, viewport: &mut Viewport, contact: u64, value: f64, time: f64) {
    let handle = viewport.modelling.as_ref().unwrap().handle.as_ref().unwrap();
    let start = point(viewport, handle.point());
    let target = assembly_handle_position(viewport, value);
    assert!(
        viewport.metrics.pixel(start).is_some() && viewport.metrics.pixel(target).is_some(),
        "handle {start:?} to {target:?} must be inside the actual viewport"
    );
    feed(viewport, InputSource::Pencil, InputPhase::Began, contact, start, time);
    feed(viewport, InputSource::Pencil, InputPhase::Moved, contact, target, time + 1.0);
    assert!(!run_tool(adapter, viewport));
    feed(viewport, InputSource::Pencil, InputPhase::Ended, contact, target, time + 2.0);
    assert!(!run_tool(adapter, viewport));
}

#[test]
fn assembly_empty_tap_discards_joint_preview_and_retains_authoritative_placement() {
    let (mut adapter, base, arm) = assembly_pair();
    let mut params = assembly_joint_origins(base, arm, [0., 0., 1.]);
    params["type"] = json!("revolute");
    let joint = assembly_accept(&mut adapter, "joint.create", params, &assembly_pair_selection(base, arm))["joint"].as_u64().unwrap();
    let edit = adapter.begin_model(849, Some("joint.drive"), None, &[]).unwrap();
    let edit = adapter.refresh_model(&edit, &[], Some(json!({"joint":joint,"value":"90 deg"}))).unwrap();
    let mut viewport = assembly_handle_view(&mut adapter, edit.clone());
    let history = adapter.session.undo.len();
    let document = adapter.session.doc.clone();
    let authoritative = viewport.snapshot().unwrap().model;
    let prepared = adapter.prepare_model(&edit, edit.params.clone(), false).unwrap();
    viewport.publish_model(849, &prepared).unwrap();
    assert!(!Arc::ptr_eq(&authoritative, &viewport.snapshot().unwrap().model));
    for (phase, time) in [(InputPhase::Began, 1.), (InputPhase::Ended, 2.)] {
        let mut event = sample(&viewport, InputSource::Finger, phase, 94, [5., 5.], time);
        event.display_preview = true;
        viewport.input(event).unwrap();
    }
    assert!(!viewport.model_active());
    assert!(Arc::ptr_eq(&authoritative, &viewport.snapshot().unwrap().model));
    assert!(Arc::ptr_eq(&document, &adapter.session.doc));
    assert_eq!(adapter.session.undo.len(), history);
    assert!(viewport.publish_model(849, &prepared).is_err());
}

#[test]
fn assembly_pencil_joint_drive_uses_native_angular_and_linear_values_limits_and_explicit_apply() {
    for (kind, axis, requested, actual, expected) in
        [("revolute", [0.0, 0.0, 1.0], 90.0, 90.0, Vec3::new(0.0, 100.0, 0.0)), ("slider", [1.0, 0.0, 0.0], 75.0, 50.0, Vec3::new(150.0, 0.0, 0.0))]
    {
        let (mut adapter, base, arm) = assembly_pair();
        let mut params = assembly_joint_origins(base, arm, axis);
        params["type"] = json!(kind);
        if kind == "slider" {
            params["limits"] = json!([[0, 50]]);
        }
        let joint = assembly_accept(&mut adapter, "joint.create", params, &assembly_pair_selection(base, arm))["joint"].as_u64().unwrap();
        let edit = adapter.begin_model(850, Some("joint.drive"), None, &[]).unwrap();
        let mut params = edit.params.clone();
        params["joint"] = json!(joint);
        let edit = adapter.refresh_model(&edit, &[], Some(params)).unwrap();
        let mut viewport = assembly_handle_view(&mut adapter, edit);
        let history = adapter.session.undo.len();
        assembly_pencil_proposal(&mut adapter, &mut viewport, 91, requested, 1.0);
        let edit = viewport.model_edit(850).unwrap();
        assert_eq!(edit.reply["moved"], true);
        assert!((edit.reply["values"][0]["value"].as_f64().unwrap() - actual).abs() < 1e-6);
        assert!((edit.handle.as_ref().unwrap().distance - actual).abs() < 1e-6, "handle follows the solved/clamped value");
        assert!(!edit.feedback.is_empty());
        assert_eq!(adapter.session.undo.len(), history, "Pencil lift keeps a provisional drive");
        assert!(assembly_marker(&adapter, &[arm], Vec3::new(100.0, 0.0, 0.0)).dist(Vec3::new(100.0, 0.0, 0.0)) < 1e-8);
        if kind == "revolute" {
            assembly_pencil_proposal(&mut adapter, &mut viewport, 92, 30.0, 5.0);
            assert!((viewport.model_edit(850).unwrap().reply["values"][0]["value"].as_f64().unwrap() - 30.0).abs() < 1e-6);
            viewport.cancel_model();
            assert_eq!(adapter.session.undo.len(), history);
            let edit = adapter.begin_model(851, Some("joint.drive"), None, &[]).unwrap();
            let mut params = edit.params.clone();
            params["joint"] = json!(joint);
            let edit = adapter.refresh_model(&edit, &[], Some(params)).unwrap();
            viewport.begin_model(edit).unwrap();
            assembly_pencil_proposal(&mut adapter, &mut viewport, 93, requested, 10.0);
        }
        let token = viewport.diagnostic()["model_token"].as_u64().unwrap();
        let edit = viewport.model_edit(token).unwrap();
        let prepared = adapter.prepare_model(&edit, edit.params.clone(), true).unwrap();
        let model = adapter.accept_model(&edit, prepared).unwrap();
        viewport.finish_model(token, Some(model)).unwrap();
        assert_eq!(adapter.session.undo.len(), history + 1);
        assert!(assembly_marker(&adapter, &[arm], Vec3::new(100.0, 0.0, 0.0)).dist(expected) < 1e-6);
        adapter.execute("edit.undo", json!({})).unwrap();
        assert!(assembly_marker(&adapter, &[arm], Vec3::new(100.0, 0.0, 0.0)).dist(Vec3::new(100.0, 0.0, 0.0)) < 1e-6);
        adapter.execute("edit.redo", json!({})).unwrap();
        assert!(assembly_marker(&modelling_roundtrip(&adapter), &[arm], Vec3::new(100.0, 0.0, 0.0)).dist(expected) < 1e-6);
    }
}

#[test]
fn assembly_pencil_pending_preview_keeps_last_good_scene_until_latest_result_without_auto_apply() {
    let (mut adapter, base, arm) = assembly_pair();
    let mut params = assembly_joint_origins(base, arm, [0.0, 0.0, 1.0]);
    params["type"] = json!("revolute");
    let joint = assembly_accept(&mut adapter, "joint.create", params, &assembly_pair_selection(base, arm))["joint"].as_u64().unwrap();
    let edit = adapter.begin_model(859, Some("joint.drive"), None, &[]).unwrap();
    let mut params = edit.params.clone();
    params["joint"] = json!(joint);
    let edit = adapter.refresh_model(&edit, &[], Some(params)).unwrap();
    let mut viewport = assembly_handle_view(&mut adapter, edit);
    let source = viewport.model.as_ref().unwrap().clone();
    let history = adapter.session.undo.len();
    assembly_pencil_proposal(&mut adapter, &mut viewport, 95, 90.0, 1.0);
    let last_good = viewport.display_model().unwrap().clone();
    assert!(!Arc::ptr_eq(&last_good, &source));
    assert!((viewport.model_edit(859).unwrap().reply["values"][0]["value"].as_f64().unwrap() - 90.0).abs() < 1e-8);

    let start = point(&viewport, viewport.modelling.as_ref().unwrap().handle.as_ref().unwrap().point());
    feed(&mut viewport, InputSource::Pencil, InputPhase::Began, 96, start, 5.0);
    let target = assembly_handle_position(&viewport, 30.0);
    feed(&mut viewport, InputSource::Pencil, InputPhase::Moved, 96, target, 6.0);
    let edit = viewport.model_edit(859).unwrap();
    assert_eq!(adapter.model_descriptor(&edit)["valid"], false, "pending parameters cannot enable Apply");
    assert!(
        Arc::ptr_eq(viewport.display_model().unwrap(), &last_good),
        "with the worker withheld, an actual Pencil sample must keep the last valid90 scene rather than flash the committed0 scene"
    );
    assert_eq!(viewport.diagnostic()["preview"], true, "the retained scene must remain labelled provisional for source-aware input");
    assert_eq!(viewport.diagnostic()["preview_stale"], true, "retained geometry must be visibly distinguished from current validation");
    let older = viewport.take_tool_work().unwrap();
    let old_result = adapter.prepare_tool(older.clone()).unwrap();

    for (value, time) in [(40.0, 6.1), (60.0, 6.2)] {
        let target = assembly_handle_position(&viewport, value);
        feed(&mut viewport, InputSource::Pencil, InputPhase::Moved, 96, target, time);
        assert!(Arc::ptr_eq(viewport.display_model().unwrap(), &last_good));
        assert_eq!(adapter.model_descriptor(&viewport.model_edit(859).unwrap())["valid"], false);
    }
    assert!(!viewport.publish_preview(&older, old_result.model), "superseded geometry cannot replace the retained last-good scene");
    let newer = viewport.take_tool_work().unwrap();
    assert!(viewport.take_tool_work().is_none(), "coalesced intermediate moves occupy one replaceable pending slot");
    let newer_result = adapter.prepare_tool(newer.clone()).unwrap();
    let target = assembly_handle_position(&viewport, 60.0);
    feed(&mut viewport, InputSource::Pencil, InputPhase::Ended, 96, target, 7.0);
    assert!(Arc::ptr_eq(viewport.display_model().unwrap(), &last_good), "lifting while work is pending must not flash the committed source");
    assert!(!viewport.publish_preview(&newer, newer_result.model), "the final actual end sample supersedes the in-flight move result");
    let final_work = viewport.take_tool_work().unwrap();
    assert!(!final_work.commit, "Pencil lift must never become Apply");
    let final_result = adapter.prepare_tool(final_work.clone()).unwrap();
    viewport.publish_model_tool_details(&final_work, final_result.reply, final_result.feedback, final_result.handle);
    assert!(viewport.publish_preview(&final_work, final_result.model));
    let edit = viewport.model_edit(859).unwrap();
    assert_eq!(adapter.model_descriptor(&edit)["valid"], true);
    assert_eq!(viewport.diagnostic()["preview_stale"], false);
    assert!((edit.reply["values"][0]["value"].as_f64().unwrap() - 60.0).abs() < 1e-8);
    assert!(!Arc::ptr_eq(viewport.display_model().unwrap(), &last_good), "the latest validated scene replaces the retained scene");
    assert_eq!(adapter.session.undo.len(), history);
    assert!(assembly_marker(&adapter, &[arm], Vec3::new(100.0, 0.0, 0.0)).dist(Vec3::new(100.0, 0.0, 0.0)) < 1e-8);
    let latest = viewport.display_model().unwrap().clone();
    viewport.pend_model(859).unwrap();
    let retained_count = Arc::strong_count(&latest);
    for _ in 0..3 {
        viewport.pend_model(859).unwrap();
        assert!(Arc::ptr_eq(viewport.display_model().unwrap(), &latest), "numeric/slider evaluation must retain the actual last valid60 scene");
        assert_eq!(adapter.model_descriptor(&viewport.model_edit(859).unwrap())["valid"], false);
        assert_eq!(viewport.diagnostic()["preview_stale"], true);
        assert_eq!(Arc::strong_count(&latest), retained_count, "repeated pending proposals keep one display reference");
    }
    viewport.fail_model(859).unwrap();
    assert!(
        Arc::ptr_eq(viewport.display_model().unwrap(), &source),
        "explicit source selection reveals authoritative geometry instead of a retained preview"
    );
    assert_eq!(viewport.diagnostic()["preview_stale"], false);
    assert_eq!(viewport.diagnostic()["preview"], false);
    assert_eq!(adapter.model_descriptor(&viewport.model_edit(859).unwrap())["valid"], false);
    viewport.cancel_model();
    assert!(Arc::ptr_eq(viewport.display_model().unwrap(), &source), "Cancel removes provisional geometry and restores the authoritative scene");
    assert_eq!(viewport.diagnostic()["preview_stale"], false);
    assert_eq!(adapter.session.undo.len(), history);
}

#[test]
fn assembly_pencil_occurrence_translation_and_rotation_keep_navigation_cancel_and_history() {
    let (mut adapter, _, arm) = assembly_pair();
    let selection = [Sel::Occurrence { path: vec![arm] }];
    let edit = adapter.begin_model(852, Some("occurrence.move"), None, &selection).unwrap();
    let mut viewport = assembly_handle_view(&mut adapter, edit);
    let history = adapter.session.undo.len();
    assembly_pencil_proposal(&mut adapter, &mut viewport, 91, 20.0, 1.0);
    assert!((viewport.snapshot().unwrap().model.bounds.max.x - 120.0).abs() < 1e-6);
    let start = assembly_handle_position(&viewport, 20.0);
    feed(&mut viewport, InputSource::Pencil, InputPhase::Began, 92, start, 5.0);
    let target = assembly_handle_position(&viewport, 25.0);
    feed(&mut viewport, InputSource::Pencil, InputPhase::Moved, 92, target, 6.0);
    run_tool(&mut adapter, &mut viewport);
    let camera = viewport.camera;
    feed(&mut viewport, InputSource::Finger, InputPhase::Began, 1, [200.0, 300.0], 7.0);
    feed(&mut viewport, InputSource::Finger, InputPhase::Began, 2, [500.0, 300.0], 7.0);
    feed(&mut viewport, InputSource::Finger, InputPhase::Moved, 2, [550.0, 330.0], 8.0);
    assert_ne!(viewport.camera, camera);
    assert_eq!(viewport.diagnostic()["input"]["edit"], "suspended");
    feed(&mut viewport, InputSource::Pencil, InputPhase::Cancelled, 92, target, 9.0);
    assert!(viewport.take_tool_work().is_none());
    viewport.cancel_model();
    assert_eq!(adapter.session.undo.len(), history);
    assert!(assembly_marker(&adapter, &[arm], Vec3::ZERO).dist(Vec3::ZERO) < 1e-8);
    let edit = adapter.begin_model(853, Some("occurrence.move"), None, &selection).unwrap();
    let mut viewport = assembly_handle_view(&mut adapter, edit);
    assembly_pencil_proposal(&mut adapter, &mut viewport, 93, 20.0, 12.0);
    let edit = viewport.model_edit(853).unwrap();
    let prepared = adapter.prepare_model(&edit, edit.params.clone(), true).unwrap();
    let model = adapter.accept_model(&edit, prepared).unwrap();
    viewport.finish_model(853, Some(model)).unwrap();
    assert!(assembly_marker(&adapter, &[arm], Vec3::ZERO).dist(Vec3::new(20.0, 0.0, 0.0)) < 1e-6);
    let edit = adapter.begin_model(854, Some("occurrence.move"), None, &selection).unwrap();
    let mut params = edit.params.clone();
    params["motion"] = json!("rotation");
    params["axis"] = json!([0, 0, 1]);
    let edit = adapter.refresh_model(&edit, &selection, Some(params)).unwrap();
    let mut viewport = assembly_handle_view(&mut adapter, edit);
    assembly_pencil_proposal(&mut adapter, &mut viewport, 94, 90.0, 16.0);
    let edit = viewport.model_edit(854).unwrap();
    let prepared = adapter.prepare_model(&edit, edit.params.clone(), true).unwrap();
    let model = adapter.accept_model(&edit, prepared).unwrap();
    viewport.finish_model(854, Some(model)).unwrap();
    assert_eq!(adapter.session.undo.len(), history + 2);
    assert!(assembly_marker(&adapter, &[arm], Vec3::new(100.0, 0.0, 0.0)).dist(Vec3::new(0.0, 120.0, 0.0)) < 1e-6);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!(assembly_marker(&adapter, &[arm], Vec3::new(100.0, 0.0, 0.0)).dist(Vec3::new(120.0, 0.0, 0.0)) < 1e-6);
}

#[test]
fn assembly_pencil_constrained_reach_shows_native_residuals_and_never_moves_geometry_freely() {
    let (mut adapter, base, arm) = assembly_pair();
    let mut params = assembly_joint_origins(base, arm, [0.0, 0.0, 1.0]);
    params["type"] = json!("revolute");
    assembly_accept(&mut adapter, "joint.create", params, &assembly_pair_selection(base, arm));
    let selection = [Sel::Occurrence { path: vec![arm] }];
    let edit = adapter.begin_model(855, Some("joint.reach"), None, &selection).unwrap();
    let mut params = edit.params.clone();
    for (key, value) in [("at_x", "100 mm"), ("at_y", "0 mm"), ("at_z", "0 mm"), ("point_x", "100 mm"), ("point_y", "0 mm"), ("point_z", "0 mm")] {
        params[key] = json!(value);
    }
    params["handle_axis"] = json!([0, 1, 0]);
    let edit = adapter.refresh_model(&edit, &selection, Some(params)).unwrap();
    let no_op = adapter.prepare_model(&edit, edit.params.clone(), false).unwrap();
    assert_eq!(no_op.reply["reached"], true);
    assert_eq!(no_op.reply["moved"], false);
    assert!(!no_op.feedback.is_empty());
    drop(no_op);
    let mut viewport = assembly_handle_view(&mut adapter, edit);
    let history = adapter.session.undo.len();
    assembly_pencil_proposal(&mut adapter, &mut viewport, 91, 100.0, 1.0);
    let edit = viewport.model_edit(855).unwrap();
    assert_eq!(edit.reply["reached"], false);
    assert!(edit.reply["error_mm"].as_f64().unwrap() > 40.0);
    assert!(!edit.feedback.is_empty());
    let reached: Vec3 = serde_json::from_value(edit.reply["point"].clone()).unwrap();
    assert!((reached.len() - 100.0).abs() < 1e-6, "the hinge keeps its exact100mm radius for an unreachable target");
    let mut params = edit.params.clone();
    params["handle_axis"] = json!([1, 0, 0]);
    let edit = adapter.refresh_model(&edit, &selection, Some(params)).unwrap();
    viewport.update_model(edit).unwrap();
    assembly_pencil_proposal(&mut adapter, &mut viewport, 92, 0.0, 5.0);
    let edit = viewport.model_edit(855).unwrap();
    assert_eq!(edit.reply["reached"], true);
    assert!(edit.reply["error_mm"].as_f64().unwrap() < 1e-6);
    let reached: Vec3 = serde_json::from_value(edit.reply["point"].clone()).unwrap();
    assert!(reached.dist(Vec3::new(0.0, 100.0, 0.0)) < 1e-6);
    assert_eq!(adapter.session.undo.len(), history);
    assert!(assembly_marker(&adapter, &[arm], Vec3::new(100.0, 0.0, 0.0)).dist(Vec3::new(100.0, 0.0, 0.0)) < 1e-8);
    let prepared = adapter.prepare_model(&edit, edit.params.clone(), true).unwrap();
    let model = adapter.accept_model(&edit, prepared).unwrap();
    viewport.finish_model(855, Some(model)).unwrap();
    assert_eq!(adapter.session.undo.len(), history + 1);
    assert!(assembly_marker(&adapter, &[arm], Vec3::new(100.0, 0.0, 0.0)).dist(Vec3::new(0.0, 100.0, 0.0)) < 1e-6);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!(assembly_marker(&adapter, &[arm], Vec3::new(100.0, 0.0, 0.0)).dist(Vec3::new(100.0, 0.0, 0.0)) < 1e-6);
}

#[test]
fn assembly_additional_joint_types_preserve_typed_values_edits_and_native_roundtrip() {
    let turn = 30_f64.to_radians();
    for (kind, values, expected) in [
        ("cylindrical", json!(["30 deg", "5 mm"]), Vec3::new(100.0 * turn.cos(), 50.0, 5.0)),
        ("pin_slot", json!(["30 deg", "5 mm"]), Vec3::new(100.0 * turn.cos() + 5.0, 50.0, 0.0)),
        ("planar", json!(["10 mm", "20 mm", "90 deg"]), Vec3::new(10.0, 120.0, 0.0)),
        ("ball", json!(["30 deg", "0 deg", "0 deg"]), Vec3::new(100.0 * turn.cos(), 50.0, 0.0)),
    ] {
        let (mut adapter, base, arm) = assembly_pair();
        let mut params = assembly_joint_origins(base, arm, [0.0, 0.0, 1.0]);
        params["type"] = json!(kind);
        params["a"]["x"] = json!([1, 0, 0]);
        params["b"]["x"] = json!([1, 0, 0]);
        let joint = assembly_accept(&mut adapter, "joint.create", params, &assembly_pair_selection(base, arm))["joint"].as_u64().unwrap();
        let reply = assembly_accept(&mut adapter, "joint.drive", json!({"joint":joint,"values":values}), &[]);
        assert_eq!(reply["values"].as_array().unwrap().len(), values.as_array().unwrap().len());
        assert_eq!(reply["moved"], true);
        assert!(assembly_marker(&adapter, &[arm], Vec3::new(100.0, 0.0, 0.0)).dist(expected) < 1e-6, "{kind}");
        assembly_accept(&mut adapter, "joint.edit", json!({"joint":joint,"name":format!("Edited {kind}"),"offset":"2 mm"}), &[]);
        assert_eq!(adapter.session.doc.assembly.joints[0].id, joint);
        assert!(assembly_marker(&adapter, &[arm], Vec3::new(100.0, 0.0, 0.0)).dist(expected + Vec3::new(0.0, 0.0, 2.0)) < 1e-6);
        adapter.execute("edit.undo", json!({})).unwrap();
        assert!(assembly_marker(&adapter, &[arm], Vec3::new(100.0, 0.0, 0.0)).dist(expected) < 1e-6);
        adapter.execute("edit.redo", json!({})).unwrap();
        let loaded = modelling_roundtrip(&adapter);
        assert_eq!(*loaded.session.doc, *adapter.session.doc);
        assert!(assembly_marker(&loaded, &[arm], Vec3::new(100.0, 0.0, 0.0)).dist(expected + Vec3::new(0.0, 0.0, 2.0)) < 1e-6);
    }
}

#[test]
fn assembly_joint_limits_compare_mixed_angle_units_and_reject_reversed_or_incompatible_values() {
    let (mut adapter, base, arm) = assembly_pair();
    let mut params = assembly_joint_origins(base, arm, [0.0, 0.0, 1.0]);
    params["type"] = json!("revolute");
    let joint = assembly_accept(&mut adapter, "joint.create", params, &assembly_pair_selection(base, arm))["joint"].as_u64().unwrap();
    assembly_accept(&mut adapter, "joint.limits", json!({"joint":joint,"min":90,"max":"2 rad"}), &[]);
    let limits = adapter.session.doc.assembly.joints[0].limits[0].unwrap();
    assert!((limits.0 - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    assert!((limits.1 - 2.0).abs() < 1e-9);
    let edit = adapter.begin_model(856, Some("joint.limits"), None, &[]).unwrap();
    let document = adapter.session.doc.clone();
    for (min, max) in [("2 rad", "90 deg"), ("90 mm", "2 rad")] {
        assert!(adapter.prepare_model(&edit, json!({"joint":joint,"min":min,"max":max}), false).is_err());
        assert!(Arc::ptr_eq(&document, &adapter.session.doc));
    }
    assert_eq!(*modelling_roundtrip(&adapter).session.doc, *adapter.session.doc);
}

#[test]
fn assembly_pending_guard_tracks_connected_mechanism_and_preserves_unrelated_branch_positions() {
    let (mut adapter, base, arm) = assembly_pair();
    let mut params = assembly_joint_origins(base, arm, [0.0, 0.0, 1.0]);
    params["type"] = json!("revolute");
    let joint = assembly_accept(&mut adapter, "joint.create", params, &assembly_pair_selection(base, arm))["joint"].as_u64().unwrap();
    let (_, unrelated) = assembly_create_component(&mut adapter, "Unrelated pending branch", false);
    adapter.execute("occurrence.move", json!({"occurrence":unrelated,"path":[unrelated],"translate":[0,20,0],"capture":false})).unwrap();
    assembly_accept(&mut adapter, "joint.drive", json!({"joint":joint,"value":"45 deg"}), &[]);
    assert!(adapter.session.pending_moves.contains_key(&unrelated));
    assert!(assembly_marker(&adapter, &[unrelated], Vec3::ZERO).dist(Vec3::new(0.0, 20.0, 0.0)) < 1e-9);
    adapter.execute("occurrence.move", json!({"occurrence":arm,"path":[arm],"translate":[10,0,0],"capture":false})).unwrap();
    let edit = adapter.begin_model(857, Some("joint.drive"), None, &[]).unwrap();
    let document = adapter.session.doc.clone();
    let failure = match adapter.prepare_model(&edit, json!({"joint":joint,"value":"90 deg"}), false) {
        Err(failure) => failure,
        Ok(_) => panic!("a pending occurrence in the connected mechanism must be captured before driving"),
    };
    assert!(failure.message.to_ascii_lowercase().contains("pending") || failure.message.to_ascii_lowercase().contains("capture"));
    assert!(Arc::ptr_eq(&document, &adapter.session.doc));
    assert!(adapter.session.pending_moves.contains_key(&unrelated) && adapter.session.pending_moves.contains_key(&arm));
}

#[test]
fn assembly_form_context_switch_refreshes_drive_units_actual_values_and_limit_index() {
    let (mut adapter, base, arm) = assembly_pair();
    let mut params = assembly_joint_origins(base, arm, [0.0, 0.0, 1.0]);
    params["type"] = json!("revolute");
    params["limits"] = json!([[-90, 120]]);
    let hinge = assembly_accept(&mut adapter, "joint.create", params, &assembly_pair_selection(base, arm))["joint"].as_u64().unwrap();
    let (_, slide) = assembly_part(
        &mut adapter,
        "Context slider",
        false,
        json!({"length":2,"width":2,"height":2,"corner":[0,20,0],"body_name":"Context slide body"}),
    );
    let mut params = assembly_joint_origins(base, slide, [1.0, 0.0, 0.0]);
    params["type"] = json!("slider");
    params["limits"] = json!([[0, 50]]);
    let slider = assembly_accept(&mut adapter, "joint.create", params, &assembly_pair_selection(base, slide))["joint"].as_u64().unwrap();
    assembly_accept(&mut adapter, "joint.drive", json!({"joint":hinge,"value":"30 deg"}), &[]);
    assembly_accept(&mut adapter, "joint.drive", json!({"joint":slider,"value":"20 mm"}), &[]);
    let edit = adapter.begin_model(858, Some("joint.drive"), None, &[]).unwrap();
    let mut params = edit.params.clone();
    params["joint"] = json!(hinge);
    let edit = adapter.refresh_model(&edit, &[], Some(params)).unwrap();
    let descriptor = adapter.model_descriptor(&edit);
    let field = descriptor["fields"].as_array().unwrap().iter().find(|field| field["key"] == "value").unwrap();
    assert_eq!(field["units"], "deg");
    assert!((field["value"].as_f64().unwrap() - 30.0).abs() < 1e-9);
    let mut params = edit.params.clone();
    params["joint"] = json!(slider);
    let edit = adapter.refresh_model(&edit, &[], Some(params)).unwrap();
    let descriptor = adapter.model_descriptor(&edit);
    let field = descriptor["fields"].as_array().unwrap().iter().find(|field| field["key"] == "value").unwrap();
    assert_eq!(field["units"], "mm");
    assert!((field["value"].as_f64().unwrap() - 20.0).abs() < 1e-9);
    assert_eq!(field["min"], 0.0);
    assert_eq!(field["max"], 50.0);
    assert!(
        (adapter.session.doc.eval(edit.params["value"].as_str().unwrap(), solidcraft_engine::doc::expr::Kind::Length).unwrap() - 20.0).abs() < 1e-9
    );
    assembly_accept(&mut adapter, "joint.edit", json!({"joint":hinge,"type":"cylindrical"}), &[]);
    assembly_accept(&mut adapter, "joint.limits", json!({"joint":hinge,"index":1,"min":"5 mm","max":"25 mm"}), &[]);
    let edit = adapter.begin_model(859, Some("joint.limits"), None, &[]).unwrap();
    let mut params = edit.params.clone();
    params["joint"] = json!(hinge);
    params["index"] = json!(0);
    let edit = adapter.refresh_model(&edit, &[], Some(params)).unwrap();
    assert!(
        (adapter.session.doc.eval(edit.params["min"].as_str().unwrap(), solidcraft_engine::doc::expr::Kind::Angle).unwrap()
            + std::f64::consts::FRAC_PI_2)
            .abs()
            < 1e-9
    );
    let mut params = edit.params.clone();
    params["index"] = json!(1);
    let edit = adapter.refresh_model(&edit, &[], Some(params)).unwrap();
    assert!((adapter.session.doc.eval(edit.params["min"].as_str().unwrap(), solidcraft_engine::doc::expr::Kind::Length).unwrap() - 5.0).abs() < 1e-9);
    assert!(
        (adapter.session.doc.eval(edit.params["max"].as_str().unwrap(), solidcraft_engine::doc::expr::Kind::Length).unwrap() - 25.0).abs() < 1e-9
    );
}

fn assembly_occurrence_row_action(adapter: &mut SessionAdapter, path: &[u64], label: &str) -> Value {
    let workspace = adapter.workspace(&[], false, false);
    let row = workspace["rows"].as_array().unwrap().iter().find(|row| row["entity"] == json!(Sel::Occurrence { path: path.to_vec() })).unwrap();
    row["assembly"]["actions"].as_array().unwrap().iter().find(|action| action["label"] == format!("ui.ipados.assembly.{label}")).unwrap().clone()
}

fn assembly_run_browser_command(adapter: &mut SessionAdapter, action: Value) -> Value {
    adapter.execute(action["command"].as_str().unwrap(), action["params"].clone()).unwrap().value
}

#[test]
fn assembly_browser_visibility_isolation_activation_duplicate_labels_and_native_file_roundtrip() {
    let mut adapter = SessionAdapter::new();
    let (parent_component, parent) = assembly_create_component(&mut adapter, "Repeated assembly", false);
    let child = assembly_accept(
        &mut adapter,
        "component.create",
        json!({"name":"Parameter leaf","parent":parent_component,"parent_path":[parent],"activate":false,"grounded":false}),
        &[],
    );
    let child_component = child["component"].as_u64().unwrap();
    let leaf = child["occurrence"].as_u64().unwrap();
    adapter.execute("parameters.add", json!({"name":"leaf_length","expression":"10 mm"})).unwrap();
    let activate = assembly_occurrence_row_action(&mut adapter, &[parent, leaf], "activate");
    assembly_run_browser_command(&mut adapter, activate);
    assert_eq!(adapter.session.active_component, child_component);
    assert_eq!(adapter.session.active_occurrence_path, Some(vec![parent, leaf]));
    let source = adapter
        .execute("solid.box", json!({"length":"leaf_length","width":3,"height":4,"name":"Leaf parametric box","body_name":"Leaf body"}))
        .unwrap()
        .value["feature"]
        .as_u64()
        .unwrap();
    assert_eq!(adapter.session.doc.feature(source).unwrap().component, child_component);
    let workspace = adapter.workspace(&[], false, false);
    let root =
        workspace["assembly"]["actions"].as_array().unwrap().iter().find(|action| action["label"] == "ui.ipados.assembly.root").unwrap().clone();
    assembly_run_browser_command(&mut adapter, root);
    assert_eq!(adapter.session.active_component, 0);
    assert!(adapter.session.active_occurrence_path.is_none());
    let root_feature = adapter
        .execute("solid.box", json!({"length":1,"width":1,"height":1,"corner":[-30,0,0],"name":"Root context box","body_name":"Root context body"}))
        .unwrap()
        .value["feature"]
        .as_u64()
        .unwrap();
    assert_eq!(adapter.session.doc.feature(root_feature).unwrap().component, 0);
    let second = assembly_accept(
        &mut adapter,
        "occurrence.copy",
        json!({"occurrence":parent,"path":[parent],"parent":0,"translate":[30,0,0]}),
        &[Sel::Occurrence { path: vec![parent] }],
    )["occurrence"]
        .as_u64()
        .unwrap();
    let (other_component, other) = assembly_part(
        &mut adapter,
        "Unrelated definition",
        false,
        json!({"length":3,"width":3,"height":3,"corner":[0,30,0],"body_name":"Unrelated body"}),
    );
    let first_path = vec![parent, leaf];
    let second_path = vec![second, leaf];
    let hide = assembly_occurrence_row_action(&mut adapter, &first_path, "visibility");
    assert_eq!(hide["params"]["paths"], json!([first_path]));
    assert_eq!(hide["params"]["visible"], false);
    let history = adapter.session.undo.len();
    assembly_run_browser_command(&mut adapter, hide);
    assert_eq!(adapter.session.undo.len(), history + 1);
    assert!(!adapter.session.occurrence_visible(&first_path));
    assert!(adapter.session.occurrence_visible(&second_path) && adapter.session.occurrence_visible(&[other]));
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!(adapter.session.occurrence_visible(&first_path));
    adapter.execute("edit.redo", json!({})).unwrap();
    assert!(!adapter.session.occurrence_visible(&first_path));
    let show = assembly_occurrence_row_action(&mut adapter, &first_path, "visibility");
    assert_eq!(show["params"]["visible"], true);
    assembly_run_browser_command(&mut adapter, show);
    let isolate = assembly_occurrence_row_action(&mut adapter, &second_path, "isolate");
    assembly_run_browser_command(&mut adapter, isolate);
    assert!(!adapter.session.occurrence_visible(&first_path));
    assert!(adapter.session.occurrence_visible(&second_path));
    assert!(!adapter.session.occurrence_visible(&[other]));
    assert!(!adapter.session.body_visible("Root context body"));
    assert_eq!(adapter.snapshot().unwrap().body_count, 1);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!(adapter.session.occurrence_visible(&first_path) && adapter.session.occurrence_visible(&[other]));
    assert!(adapter.session.body_visible("Root context body"));
    adapter.execute("edit.redo", json!({})).unwrap();
    assert_eq!(adapter.snapshot().unwrap().body_count, 1);
    let workspace = adapter.workspace(&[], false, false);
    let row = workspace["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["kind"] == "component" && row["assembly"]["component"] == other_component)
        .unwrap();
    let rename = row["assembly"]["actions"].as_array().unwrap().iter().find(|action| action["label"] == "ui.ipados.assembly.rename").unwrap();
    let mut params = rename["params"].clone();
    params["name"] = json!("Repeated assembly");
    assembly_accept(&mut adapter, rename["operation"].as_str().unwrap(), params, &[]);
    let workspace = adapter.workspace(&[], false, false);
    assert!(
        workspace["rows"].as_array().unwrap().iter().filter(|row| row["kind"] == "occurrence" && row["label"] == "Repeated assembly:1").count() >= 2
    );
    for path in [&first_path, &second_path] {
        assert!(workspace["rows"].as_array().unwrap().iter().any(|row| row["entity"] == json!(Sel::Occurrence { path: path.to_vec() })));
    }
    assert!(!adapter.session.occurrence_visible(&first_path));
    assert!(adapter.session.occurrence_visible(&second_path) && !adapter.session.occurrence_visible(&[other]));
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!(adapter.session.occurrence_visible(&second_path) && !adapter.session.occurrence_visible(&[other]));
    adapter.execute("edit.redo", json!({})).unwrap();
    let visibility = adapter.session.visibility.clone();
    let ids: Vec<_> = adapter.session.doc.features.iter().map(|feature| feature.id).collect();
    let path = std::env::temp_dir().join(format!("solidcraft-ipados-assembly-visibility-{}.solidcraft", std::process::id()));
    adapter.execute("file.save_as", json!({"path":path})).unwrap();
    let mut loaded = SessionAdapter::new();
    loaded.execute("doc.open", json!({"path":path})).unwrap();
    assert_eq!(loaded.session.visibility, visibility);
    assert_eq!(loaded.session.doc.features.iter().map(|feature| feature.id).collect::<Vec<_>>(), ids);
    assert!(!loaded.session.occurrence_visible(&first_path));
    assert!(loaded.session.occurrence_visible(&second_path) && !loaded.session.occurrence_visible(&[other]));
    assert_eq!(loaded.snapshot().unwrap().body_count, 1);
    assert_eq!(loaded.session.doc.feature(source).unwrap().component, child_component);
    loaded.execute("parameters.change", json!({"name":"leaf_length","expression":"20 mm"})).unwrap();
    assert!((assembly_volume(&loaded, &first_path) - 240.0).abs() < 1e-6);
    assert!((assembly_volume(&loaded, &second_path) - 240.0).abs() < 1e-6);
    assert!((assembly_volume(&loaded, &[other]) - 27.0).abs() < 1e-6);
    assert_eq!(loaded.snapshot().unwrap().body_count, 1);
    std::fs::remove_file(&path).unwrap();
    let backup = std::path::PathBuf::from(format!("{}.bak", path.display()));
    if backup.exists() {
        std::fs::remove_file(backup).unwrap();
    }
}

#[test]
fn assembly_dragging_floating_rigid_chain_moves_both_placements_and_preserves_relative_frame() {
    let mut adapter = SessionAdapter::new();
    let (_, first) = assembly_part(&mut adapter, "Free rigid A", false, json!({"length":2,"width":3,"height":4,"body_name":"Free A body"}));
    let (_, second) = assembly_part(&mut adapter, "Free rigid B", false, json!({"length":5,"width":5,"height":5,"body_name":"Free B body"}));
    for (occurrence, translation, angle) in [(first, [10, 0, 0], "37 deg"), (second, [30, 5, 2], "-12 deg")] {
        assembly_accept(
            &mut adapter,
            "occurrence.move",
            json!({"occurrence":occurrence,"path":[occurrence],"space":"world","translate":translation,"axis":[0,0,1],"angle":angle,"capture":true}),
            &[Sel::Occurrence { path: vec![occurrence] }],
        );
    }
    assembly_accept(
        &mut adapter,
        "joint.as_built",
        json!({"type":"rigid","a":first,"b":second,"name":"Floating fixed relationship"}),
        &assembly_pair_selection(first, second),
    );
    let first_marker = assembly_marker(&adapter, &[first], Vec3::new(1.0, 1.0, 1.0));
    let second_marker = assembly_marker(&adapter, &[second], Vec3::new(2.0, 2.0, 2.0));
    let relative = |adapter: &SessionAdapter| {
        let first = adapter.session.occurrence_placement(&[first]).unwrap().world;
        let second = adapter.session.occurrence_placement(&[second]).unwrap().world;
        solidcraft_engine::doc::mat_mul(&solidcraft_engine::doc::mat_inverse(&first).unwrap(), &second)
    };
    let initial_relative = relative(&adapter);
    let ids: Vec<_> = adapter.session.doc.features.iter().map(|feature| feature.id).collect();
    let delta = Vec3::new(20.0, -10.0, 5.0);
    let target = first_marker + delta;
    let reply = assembly_accept(
        &mut adapter,
        "occurrence.drag",
        json!({"occurrence":first,"path":[first],"at":[first_marker.x,first_marker.y,first_marker.z],"point":[target.x,target.y,target.z]}),
        &[Sel::Occurrence { path: vec![first] }],
    );
    assert_eq!(reply["moved"].as_array().unwrap().len(), 2);
    assert!(assembly_marker(&adapter, &[first], Vec3::new(1.0, 1.0, 1.0)).dist(first_marker + delta) < 1e-8);
    assert!(assembly_marker(&adapter, &[second], Vec3::new(2.0, 2.0, 2.0)).dist(second_marker + delta) < 1e-8);
    assert!(relative(&adapter).iter().flatten().zip(initial_relative.iter().flatten()).all(|(a, b)| (a - b).abs() < 1e-9));
    assert!((assembly_volume(&adapter, &[first]) - 24.0).abs() < 1e-6);
    assert!((assembly_volume(&adapter, &[second]) - 125.0).abs() < 1e-6);
    assert_eq!(adapter.session.doc.features.iter().map(|feature| feature.id).collect::<Vec<_>>(), ids);
    adapter.execute("edit.undo", json!({})).unwrap();
    assert!(assembly_marker(&adapter, &[first], Vec3::new(1.0, 1.0, 1.0)).dist(first_marker) < 1e-8);
    assert!(assembly_marker(&adapter, &[second], Vec3::new(2.0, 2.0, 2.0)).dist(second_marker) < 1e-8);
    adapter.execute("edit.redo", json!({})).unwrap();
    let loaded = modelling_roundtrip(&adapter);
    assert_eq!(*loaded.session.doc, *adapter.session.doc);
    assert!(assembly_marker(&loaded, &[first], Vec3::new(1.0, 1.0, 1.0)).dist(first_marker + delta) < 1e-8);
    assert!(assembly_marker(&loaded, &[second], Vec3::new(2.0, 2.0, 2.0)).dist(second_marker + delta) < 1e-8);
}

fn projection_fixture() -> (SessionAdapter, u64, u64) {
    let mut a = SessionAdapter::new();
    a.execute("parameters.add", json!({"name":"width","expression":"20 mm"})).unwrap();
    a.execute("sketch.create", json!({"plane":"XY","name":"Source"})).unwrap();
    let source = a.session.active_sketch.unwrap();
    a.execute("sketch.line", json!({"points":[[0,0],["width",0]],"infer":false})).unwrap();
    a.execute("sketch.circle.center", json!({"center":[5,8],"radius":2})).unwrap();
    a.execute("sketch.point", json!({"point":[25,15]})).unwrap();
    a.execute("sketch.finish", json!({})).unwrap();
    a.execute("sketch.create", json!({"plane":"XY","offset":10,"name":"Destination"})).unwrap();
    let target = a.session.active_sketch.unwrap();
    (a, source, target)
}
fn projection_request(a: &SessionAdapter, source: u64, all: bool) -> SketchProjection {
    SketchProjection {
        document_id: a.session.document_id.clone(),
        geometry_revision: a.geometry_revision,
        sketch: a.session.active_sketch.unwrap(),
        source,
        entities: vec!["l1".into()],
        all,
        construction: true,
        linked: true,
    }
}

#[test]
fn sketch_projection_individual_construction_is_linked_and_one_undo() {
    let (mut a, source, target) = projection_fixture();
    let history = a.session.undo.len();
    let (_, scene) = a.project_sketch(projection_request(&a, source, false)).unwrap();
    assert_eq!(a.session.undo.len(), history + 1);
    assert_eq!(scene.geometry_revision, a.geometry_revision);
    let state = a.session.model.state();
    let sk = &state.sketch(target).unwrap().sketch;
    assert_eq!(sk.curves.len(), 1);
    assert!(sk.curves[0].construction && sk.curves[0].link.is_some());
    assert!(a.session.model.state().sketch(target).unwrap().profiles.is_empty());
    a.execute("edit.undo", json!({})).unwrap();
    assert!(a.session.model.state().sketch(target).unwrap().sketch.curves.is_empty());
    a.execute("edit.redo", json!({})).unwrap();
    a.execute("parameters.change", json!({"name":"width","expression":"35 mm"})).unwrap();
    let state = a.session.model.state();
    let sk = &state.sketch(target).unwrap().sketch;
    assert!(sk.polyline(0).iter().any(|p| (p.x - 35.0).abs() < 1e-6));
    let id = sk.curves[0].id.clone();
    a.execute("sketch.break_link", json!({"entities":[id]})).unwrap();
    a.execute("parameters.change", json!({"name":"width","expression":"45 mm"})).unwrap();
    let state = a.session.model.state();
    let sk = &state.sketch(target).unwrap().sketch;
    assert!(sk.links.is_empty());
    assert!(sk.curves[0].construction);
    assert!(sk.polyline(0).iter().any(|p| (p.x - 35.0).abs() < 1e-6));
}

#[test]
fn sketch_projection_all_includes_lone_points_and_can_create_detached_profiles() {
    let (mut a, source, target) = projection_fixture();
    let choices = a.projection_sources();
    assert_eq!(choices.len(), 1);
    assert_eq!(choices[0]["id"], source);
    assert_eq!(choices[0]["entities"].as_array().unwrap().iter().filter(|e| e["all_geometry"] == true).count(), 3);
    let mut request = projection_request(&a, source, true);
    request.construction = false;
    request.linked = false;
    a.project_sketch(request).unwrap();
    let state = a.session.model.state();
    let ss = state.sketch(target).unwrap();
    assert_eq!(ss.sketch.curves.len(), 2);
    assert!(ss.sketch.links.is_empty());
    assert!(ss.sketch.curves.iter().all(|c| c.link.is_none() && !c.construction));
    assert!(ss.sketch.points.iter().any(|p| (p.pos.x - 25.0).abs() < 1e-6 && (p.pos.y - 15.0).abs() < 1e-6));
    assert_eq!(ss.profiles.len(), 1);
    assert!((ss.profiles[0].area - std::f64::consts::PI * 4.0).abs() < 0.01);
}

#[test]
fn sketch_projection_rejects_stale_future_invalid_and_duplicate_inputs_atomically() {
    let (mut a, source, target) = projection_fixture();
    let mut stale = projection_request(&a, source, true);
    stale.geometry_revision -= 1;
    let mut wrong_document = projection_request(&a, source, true);
    wrong_document.document_id = "replaced".into();
    let mut wrong_sketch = projection_request(&a, source, true);
    wrong_sketch.sketch = source;
    let mut invalid = projection_request(&a, source, false);
    invalid.entities.push("missing".into());
    let mut empty = projection_request(&a, source, false);
    empty.entities.clear();
    let mut own = projection_request(&a, target, true);
    own.source = target;
    let mut too_many = projection_request(&a, source, false);
    too_many.entities = vec!["l1".into(); 1001];
    for request in [stale, wrong_document, wrong_sketch, invalid, empty, own, too_many] {
        let before = a.session.doc.clone();
        let history = a.session.undo.len();
        let revision = a.geometry_revision;
        assert!(a.project_sketch(request).is_err());
        assert!(Arc::ptr_eq(&before, &a.session.doc));
        assert_eq!(a.session.undo.len(), history);
        assert_eq!(a.geometry_revision, revision);
    }
    a.project_sketch(projection_request(&a, source, false)).unwrap();
    let before = a.session.doc.clone();
    assert!(a.project_sketch(projection_request(&a, source, false)).is_err());
    assert!(Arc::ptr_eq(&before, &a.session.doc));
    a.execute("sketch.finish", json!({})).unwrap();
    a.execute("sketch.edit", json!({"sketch":source})).unwrap();
    assert!(a.projection_sources().is_empty(), "later sketches must not create a dependency cycle");
}

#[test]
fn sketch_projection_links_and_construction_round_trip_and_detach_independently() {
    let (mut a, source, target) = projection_fixture();
    a.project_sketch(projection_request(&a, source, true)).unwrap();
    let path = std::env::temp_dir().join(format!("solidcraft-projection-{}-{}.sc", std::process::id(), target));
    a.execute("file.save_as", json!({"path":path.to_str().unwrap()})).unwrap();
    let mut loaded = SessionAdapter::new();
    loaded.execute("doc.open", json!({"path":path.to_str().unwrap()})).unwrap();
    std::fs::remove_file(path).unwrap();
    loaded.execute("sketch.edit", json!({"sketch":target})).unwrap();
    let state = loaded.session.model.state();
    let ss = state.sketch(target).unwrap();
    assert_eq!(ss.sketch.links.len(), 3);
    assert!(ss.sketch.curves.iter().all(|c| c.construction));
    let link = ss.sketch.links[0].id.clone();
    loaded.execute("sketch.break_link", json!({"link":link})).unwrap();
    assert_eq!(loaded.session.model.state().sketch(target).unwrap().sketch.links.len(), 2);
    loaded.execute("edit.undo", json!({})).unwrap();
    assert_eq!(loaded.session.model.state().sketch(target).unwrap().sketch.links.len(), 3);
}

#[test]
fn sketch_projection_pencil_picks_curves_endpoints_centres_and_origin_without_editing() {
    let (mut a, source, _) = projection_fixture();
    let history = a.session.undo.len();
    let doc = a.session.doc.clone();
    let mut view = Viewport::default();
    view.set_metrics(metrics(900., 700., 2.)).unwrap();
    view.begin_update(1);
    view.install(1, a.snapshot().unwrap());
    view.command("view.sketch_projection", &json!({"action":"begin"})).unwrap();
    for (world, kind) in
        [(Vec3::new(12., 0., 0.), "curve"), (Vec3::new(20., 0., 0.), "point"), (Vec3::new(5., 8., 0.), "point"), (Vec3::ZERO, "point")]
    {
        let p = point(&view, world);
        let hit = view
            .model
            .as_ref()
            .unwrap()
            .projection_sources
            .iter()
            .find(|s| s.id == source)
            .unwrap()
            .entities
            .iter()
            .find(|e| e.point == (kind == "point") && if e.point { e.points[0].dist(world) < 1e-6 } else { e.id == "l1" })
            .unwrap()
            .id
            .clone();
        feed(&mut view, InputSource::Pencil, InputPhase::Began, 199, p, 1.);
        feed(&mut view, InputSource::Pencil, InputPhase::Ended, 199, p, 1.1);
        assert_eq!(view.projection_presentation()["source"], source);
        assert_eq!(view.projection_presentation()["selected"], json!([hit]));
        assert!(view.take_tool_work().is_none(), "a reference tap must not drag or author active geometry");
        feed(&mut view, InputSource::Pencil, InputPhase::Began, 199, p, 2.);
        feed(&mut view, InputSource::Pencil, InputPhase::Ended, 199, p, 2.1);
        assert!(view.projection_presentation()["selected"].as_array().unwrap().is_empty());
    }
    assert!(Arc::ptr_eq(&a.session.doc, &doc));
    assert_eq!(a.session.undo.len(), history);
    view.command("view.sketch_projection", &json!({"action":"all"})).unwrap();
    assert_eq!(
        view.projection_presentation()["selected"].as_array().unwrap().len(),
        3,
        "All geometry must not double-project curve endpoints or the implicit origin"
    );
    view.command("view.sketch_projection", &json!({"action":"cancel"})).unwrap();
    assert!(view.projection_presentation().is_null());
}

#[test]
fn sketch_projection_visual_filters_stale_samples_navigation_and_context_changes() {
    let (mut a, _, _) = projection_fixture();
    let mut view = Viewport::default();
    view.set_metrics(metrics(900., 700., 2.)).unwrap();
    view.begin_update(1);
    view.install(1, a.snapshot().unwrap());
    view.command("view.sketch_projection", &json!({"action":"begin"})).unwrap();
    view.command("view.sketch_projection", &json!({"action":"filter","filter":"curves"})).unwrap();
    let p = point(&view, Vec3::ZERO);
    feed(&mut view, InputSource::Pencil, InputPhase::Began, 200, p, 1.);
    feed(&mut view, InputSource::Pencil, InputPhase::Ended, 200, p, 1.1);
    assert_eq!(view.projection_presentation()["selected"], json!(["l1"]));
    view.command("view.sketch_projection", &json!({"action":"clear"})).unwrap();
    view.command("view.sketch_projection", &json!({"action":"filter","filter":"points"})).unwrap();
    let p = point(&view, Vec3::new(12., 0., 0.));
    feed(&mut view, InputSource::Pencil, InputPhase::Began, 201, p, 2.);
    feed(&mut view, InputSource::Pencil, InputPhase::Ended, 201, p, 2.1);
    assert!(view.projection_presentation()["selected"].as_array().unwrap().is_empty());
    let p = point(&view, Vec3::new(5., 8., 0.));
    let mut stale = sample(&view, InputSource::Pencil, InputPhase::Began, 202, p, 3.);
    stale.geometry_revision = Some(0);
    view.input(stale).unwrap();
    feed(&mut view, InputSource::Pencil, InputPhase::Ended, 202, p, 3.1);
    assert!(view.projection_presentation()["selected"].as_array().unwrap().is_empty());
    let before = view.camera;
    feed(&mut view, InputSource::Finger, InputPhase::Began, 203, [450., 350.], 4.);
    feed(&mut view, InputSource::Finger, InputPhase::Moved, 203, [510., 380.], 4.1);
    feed(&mut view, InputSource::Finger, InputPhase::Ended, 203, [510., 380.], 4.2);
    assert_ne!(view.camera, before);
    assert!(view.projection.is_some());
    assert!(view.projection_presentation()["selected"].as_array().unwrap().is_empty());
    a.execute("sketch.line", json!({"points":[[40,40],[50,50]]})).unwrap();
    view.begin_update(2);
    view.install(2, a.snapshot().unwrap());
    assert!(view.projection.is_none(), "a changed document must discard visual reference picks");
}

#[test]
fn sketch_projection_visually_selected_source_point_remains_linked() {
    let (mut a, source, target) = projection_fixture();
    let endpoint = a.session.model.state().sketch(source).unwrap().sketch.points.iter().find(|p| (p.pos.x - 20.).abs() < 1e-6).unwrap().id.clone();
    let mut request = projection_request(&a, source, false);
    request.entities = vec![endpoint];
    a.project_sketch(request).unwrap();
    assert!(a.session.model.state().sketch(target).unwrap().sketch.curves.is_empty());
    a.execute("parameters.change", json!({"name":"width","expression":"42 mm"})).unwrap();
    assert!(a.session.model.state().sketch(target).unwrap().sketch.points.iter().any(|p| p.link.is_some() && (p.pos.x - 42.).abs() < 1e-6));
}

#[test]
fn connected_edge_walk_uses_only_endpoints_and_refuses_partial_oversized_groups() {
    use crate::picking::endpoint_chain;
    let p = |x, y| Vec3::new(x, y, 0.);
    let edges = vec![
        (0, [p(0., 0.), p(10., 0.)]),
        (1, [p(10., 10.), p(10., 0.)]),        // reversed endpoint
        (2, [p(10., 0.), p(20., -10.)]),       // endpoint branch
        (3, [p(5., 0.), p(5., 10.)]),          // T-junction
        (4, [p(3., -5.), p(3., 5.)]),          // crossing
        (5, [p(20., -10.0001), p(30., -10.)]), // close, but separate
        (6, [p(10. + 1e-9, 10.), p(0., 0.)]),  // numerical coincidence and cycle
    ];
    assert_eq!(endpoint_chain(&edges, 0), Some(vec![0, 1, 2, 6]));
    assert_eq!(endpoint_chain(&edges, 3), Some(vec![3]));
    assert_eq!(endpoint_chain(&edges, 4), Some(vec![4]));
    assert_eq!(endpoint_chain(&edges, 5), Some(vec![5]));
    let long: Vec<_> = (0..1025).map(|i| (i, [p(i as f64, 0.), p(i as f64 + 1., 0.)])).collect();
    assert_eq!(endpoint_chain(&long[..1024], 0).unwrap().len(), 1024);
    assert!(endpoint_chain(&long, 0).is_none());
}

#[test]
fn connected_edge_double_taps_expand_sketch_endpoints_without_editing_or_toggling_the_group_off() {
    for (source, handles, scale) in [
        (InputSource::Pencil, true, 1.),
        (InputSource::Indirect, true, 2.),
        (InputSource::Finger, true, 2.),
        (InputSource::Pencil, false, 3.),
        (InputSource::Indirect, false, 2.),
    ] {
        let (mut engine, mut view) = sketch_input_fixture(json!("XY"), "select");
        for (id, points) in [
            ("bottom", [[10., 20.], [70., 20.]]),
            ("right", [[70., 60.], [70., 20.]]),
            ("top", [[70., 60.], [10., 60.]]),
            ("left", [[10., 60.], [10., 20.]]),
            ("branch", [[70., 60.], [90., 80.]]),
            ("tee", [[40., 20.], [40., 0.]]),
            ("crossing", [[30., 10.], [30., 30.]]),
            ("near", [[70.0001, 20.], [90., 0.]]),
        ] {
            engine.execute("sketch.line", json!({"points":points,"ids":[id],"infer":false,"connect":false})).unwrap();
        }
        view.set_metrics(metrics(800., 600., scale)).unwrap();
        view.begin_update(2);
        view.install(2, engine.snapshot().unwrap());
        view.command("view.tool_handles", &json!({"enabled":handles})).unwrap();
        view.command("view.fit", &json!({})).unwrap();
        let original = engine.session.doc.clone();
        let history = engine.session.undo.len();
        let model = view.snapshot().unwrap().model;
        let at = point(&view, Vec3::new(55., 20., 0.));
        assert!(matches!(view.pick(at, None), Some(Sel::SketchCurve { ref id, .. }) if id == "bottom"));
        for (contact, count, expected) in [(1, 1, 1), (2, 2, 5), (3, 2, 5), (4, 0, 4), (5, 2, 5)] {
            for phase in [InputPhase::Began, InputPhase::Ended] {
                let mut e = sample(&view, source, phase, contact, at, contact as f64);
                e.tap_count = count;
                view.input(e).unwrap();
            }
            assert_eq!(view.selection().len(), expected, "{source:?}, handles={handles}, scale={scale}, count={count}");
            assert!(view.take_tool_work().is_none());
        }
        let ids: std::collections::BTreeSet<_> = view
            .selection()
            .iter()
            .filter_map(|s| match s {
                Sel::SketchCurve { id, .. } => Some(id.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(ids, ["bottom", "branch", "left", "right", "top"].into_iter().collect());
        // Single-selection replaces with the entire chain; it must not truncate it.
        view.command("select.multiple", &json!({"enabled":false})).unwrap();
        view.select_tap(view.pick(at, None), false, 2);
        assert_eq!(view.selection().len(), 5);
        assert!(Arc::ptr_eq(&model, &view.snapshot().unwrap().model));
        assert!(Arc::ptr_eq(&original, &engine.session.doc));
        assert_eq!(engine.session.undo.len(), history);
    }
}

#[test]
fn connected_edge_solid_groups_stay_in_the_picked_body_and_exclude_polyline_middles() {
    use solidcraft_engine::geom::{Aabb3, Mesh};
    use solidcraft_render::{Rgb, Scene, SceneMesh};
    let p = |x, y| Vec3::new(x, y, 0.);
    let mesh = Arc::new(Mesh {
        edges: vec![vec![p(0., 0.), p(5., 0.), p(10., 0.)], vec![p(10., 0.), p(10., 10.)], vec![p(5., 0.), p(5., 10.)]],
        ..Mesh::default()
    });
    let scene_mesh = || SceneMesh {
        mesh: mesh.clone(),
        color: Rgb(100, 100, 100),
        face_colors: Vec::new(),
        opacity: 1.,
        closed: true,
        hatch: solidcraft_render::hatch::Hatch(0),
    };
    let scene = Scene { meshes: vec![scene_mesh(), scene_mesh()], ..Scene::default() };
    let picks = crate::picking::PickScene::new(&scene, vec!["One".into(), "Two".into()], Aabb3::default()).unwrap();
    let picked = Sel::Edge { body: "One".into(), index: 0, point: p(2., 0.) };
    let group = picks.connected_edges(&picked).unwrap();
    assert_eq!(group.first(), Some(&picked));
    assert_eq!(group.len(), 2);
    assert!(group.iter().all(|s| matches!(s, Sel::Edge { body, index: 0 | 1, .. } if body == "One")));
}
