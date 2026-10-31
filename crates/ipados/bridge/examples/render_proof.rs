//! Real command → revisioned scene → shared GPU, with before/after update evidence.
//! macOS GPU evidence only: does not launch or emulate the iPad application.
use serde_json::json;
use solidcraft_ipados_session::{InputEvent, InputPhase, InputSource, SessionAdapter, Viewport, ViewportMetrics};
use solidcraft_render::{Scene, SceneLine, SceneMesh};
use std::sync::Arc;

/// Capture the native appearance override with real engine geometry. Dark RGB
/// represents a resolved UIKit semantic surface; UIKit itself needs a device.
fn appearance_proof(destination: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = SessionAdapter::new();
    let mut view = Viewport::default();
    view.set_metrics(ViewportMetrics { width_points: 960., height_points: 640., scale: 1., origin_points: [0.; 2] })?;
    view.begin_update(1);
    view.install(1, engine.fixture()?);
    let capturer = solidcraft_gpu::capture::Capturer::headless()?;
    let mut light = None;
    let mut dark = None;
    for (stage, color, visible) in [
        ("light", json!(null), true),
        ("dark", json!([28, 28, 30]), true),
        ("grid-hidden", json!([28, 28, 30]), false),
        ("grid-restored", json!([28, 28, 30]), true),
        ("restored", json!(null), true),
    ] {
        let grid = if color.is_null() { json!(null) } else { json!([[45, 45, 47], [62, 62, 65]]) };
        view.command("view.background", &json!({"color":color,"grid":grid}))?;
        view.command("view.grid", &json!({"visible":visible}))?;
        let snapshot = view.snapshot().ok_or("missing appearance scene")?;
        let line = |l: &SceneLine| SceneLine { points: l.points.clone(), color: l.color, alpha: l.alpha, width: l.width, on_top: l.on_top };
        let scene = Scene {
            // NativeRenderer clears uniformly to the scene's top colour.
            background: snapshot.background.map(|(top, _)| (top, top)),
            radius: snapshot.model.bounds.diagonal() + snapshot.camera.half_height() * 2.,
            meshes: snapshot
                .model
                .scene
                .meshes
                .iter()
                .map(|m| SceneMesh { mesh: m.mesh.clone(), color: m.color, face_colors: m.face_colors.clone(), opacity: m.opacity })
                .collect(),
            lines: snapshot.model.scene.lines.iter().chain(&snapshot.guides.lines).map(line).collect(),
            grid: snapshot.guides.grid.iter().map(line).collect(),
            ..Scene::default()
        };
        let rgba = capturer.rgba(&scene, &snapshot.camera, 960, 640)?;
        if stage == "light" {
            light = Some(rgba.clone());
        }
        if stage == "dark" {
            dark = Some(rgba.clone());
        }
        if stage == "grid-hidden" && (dark.as_ref() == Some(&rgba) || !snapshot.guides.grid.is_empty()) {
            return Err("Grid toggle did not hide the rendered grid".into());
        }
        if stage == "grid-restored" && dark.as_ref() != Some(&rgba) {
            return Err("Grid toggle did not restore the original dark rendering".into());
        }
        // Grid lines can cross the corners; check a substantial background area.
        let dark_matches = color.is_null() || rgba.chunks_exact(4).filter(|pixel| pixel[..3] == [28, 28, 30]).count() > 960 * 640 / 4;
        if stage == "restored" && light.as_ref() != Some(&rgba) {
            return Err("Light did not restore the original rendering".into());
        }
        let path = destination.replace(".png", &format!("-{stage}.png"));
        let png = solidcraft_render::png(960, 640, rgba).ok_or("PNG encoding failed")?;
        std::fs::write(&path, png)?;
        if !dark_matches {
            return Err(format!("dark background pixels do not match native colour; inspect {path}").into());
        }
        println!("{stage}: {} triangles; {path}", capturer.last_stats().triangles);
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let destination = std::env::args().nth(1).ok_or("usage: render_proof OUTPUT.png")?;
    if std::env::args().nth(2).as_deref() == Some("--home") {
        return home_proof(&destination);
    }
    if std::env::args().nth(2).as_deref() == Some("--appearance") {
        return appearance_proof(&destination);
    }
    if std::env::args().nth(2).as_deref() == Some("--history") {
        return history_proof(&destination);
    }
    if std::env::args().nth(2).as_deref() == Some("--edge-selection") {
        return edge_selection_proof(&destination);
    }
    if std::env::args().nth(2).as_deref() == Some("--selection-taps") {
        return selection_taps_proof(&destination);
    }
    if std::env::args().nth(2).as_deref() == Some("--cylinder-selection") {
        return cylinder_selection_proof(&destination, false);
    }
    if std::env::args().nth(2).as_deref() == Some("--cylinder-edge-selection") {
        return cylinder_selection_proof(&destination, true);
    }
    if std::env::args().nth(2).as_deref() == Some("--continuous-resize") {
        return continuous_resize_proof(&destination);
    }
    if std::env::args().nth(2).as_deref() == Some("--profiles") {
        return profile_proof(&destination);
    }
    if std::env::args().nth(2).as_deref() == Some("--origin") {
        return origin_proof(&destination);
    }
    if std::env::args().nth(2).as_deref() == Some("--grid-axes") {
        return grid_axes_proof(&destination);
    }
    if std::env::args().nth(2).as_deref() == Some("--input") {
        return input_proof(&destination);
    }
    if std::env::args().nth(2).as_deref() == Some("--orbit") {
        return orbit_proof(&destination);
    }
    if std::env::args().nth(2).as_deref() == Some("--sketch") {
        return sketch_proof(&destination);
    }
    if std::env::args().nth(2).as_deref() == Some("--rectangle-dimensions") {
        return rectangle_dimensions_proof(&destination);
    }
    if std::env::args().nth(2).as_deref() == Some("--sketch-drag") {
        return sketch_drag_proof(&destination);
    }
    if std::env::args().nth(2).as_deref() == Some("--modelling") {
        return modelling_proof(&destination);
    }
    if std::env::args().nth(2).as_deref() == Some("--face-extrude") {
        return face_extrude_proof(&destination);
    }
    if std::env::args().nth(2).as_deref() == Some("--subtract-selection") {
        return subtract_selection_proof(&destination);
    }
    if std::env::args().nth(2).as_deref() == Some("--assemblies") {
        return assembly_proof(&destination);
    }
    let mut engine = SessionAdapter::new();
    let model = engine.fixture()?;
    let mut view = Viewport::default();
    view.set_metrics(ViewportMetrics { width_points: 960.0, height_points: 640.0, scale: 1.0, origin_points: [0.0; 2] })?;
    view.begin_update(1);
    view.install(1, model);
    let capturer = solidcraft_gpu::capture::Capturer::headless()?;
    for (generation, path) in [(1, destination.clone()), (2, destination.replace(".png", "-changed.png"))] {
        if generation == 2 {
            engine.execute("timeline.edit", json!({"feature":"Viewport post","set":{"height":"40"}}))?;
            view.begin_update(2);
            view.install(2, engine.snapshot()?);
        }
        let snapshot = view.snapshot().ok_or("no scene")?;
        // Capture the same model/guides; shared meshes are retained, not re-tessellated.
        let line = |l: &SceneLine| SceneLine { points: l.points.clone(), color: l.color, alpha: l.alpha, width: l.width, on_top: l.on_top };
        let mut scene = Scene {
            background: snapshot.background,
            radius: snapshot.model.bounds.diagonal() + snapshot.camera.half_height() * 2.0,
            grid: snapshot.guides.grid.iter().map(line).collect(),
            ..Scene::default()
        };
        scene.lines.extend(snapshot.model.scene.lines.iter().map(line));
        scene.lines.extend(snapshot.guides.lines.iter().map(line));
        scene.meshes.extend(snapshot.model.scene.meshes.iter().map(|m| SceneMesh {
            mesh: Arc::clone(&m.mesh),
            color: m.color,
            face_colors: m.face_colors.clone(),
            opacity: m.opacity,
        }));
        std::fs::write(&path, capturer.png(&scene, &snapshot.camera, 960, 640)?)?;
        let stats = capturer.last_stats();
        if stats.triangles == 0 {
            return Err("GPU drew no engine geometry".into());
        }
        println!(
            "r{}: {} bodies, {} triangles, {:.3} mm³; {} ({}); {} drawn triangles; {}",
            snapshot.geometry_revision,
            snapshot.model.body_count,
            snapshot.model.triangle_count,
            snapshot.model.volume_mm3,
            capturer.renderer(),
            capturer.adapter_name(),
            stats.triangles,
            path
        );
    }
    view.command("view.top", &json!({}))?;
    for (kind, world) in [("body", [-8.0, 0.0, 10.0]), ("face", [-8.0, 0.0, 10.0]), ("edge", [0.0, -15.0, 10.0]), ("vertex", [-20.0, -15.0, 10.0])] {
        view.command("select.filter", &json!({"filter":kind}))?;
        let snapshot = view.snapshot().ok_or("no scene")?;
        let point = solidcraft_render::Camera::to_screen(
            &snapshot.camera,
            solidcraft_ipados_session::WorldPoint::new(world[0], world[1], world[2]),
            960.0,
            640.0,
            100.0,
        )
        .ok_or("point outside projection")?;
        for phase in [InputPhase::Began, InputPhase::Ended] {
            view.input(InputEvent {
                source: InputSource::Pencil,
                phase,
                id: 1,
                x_points: point.0,
                y_points: point.1,
                x_pixels: point.0,
                y_pixels: point.1,
                scale: 1.0,
                timestamp: 1.0,
                azimuth: None,
                predicted: false,
                display_preview: false,
                sequence: 0,
                viewport_epoch: None,
                camera_epoch: None,
                document_id: None,
                pressure: None,
                altitude: None,
                geometry_revision: Some(snapshot.geometry_revision),
                consumed: false,
                sketch_annotation: None,
                shift: false,
                button: 0,
                scroll: 0.0,
            })?;
        }
        if view.selection().len() != 1 {
            return Err(format!("no {kind} selection").into());
        }
        let snapshot = view.snapshot().ok_or("no scene")?;
        let rgba = solidcraft_ipados_bridge::renderer::capture_feedback(&snapshot, &capturer, 960, 640)?;
        let png = solidcraft_render::png(960, 640, rgba).ok_or("PNG encoding failed")?;
        let path = destination.replace(".png", &format!("-{kind}.png"));
        std::fs::write(&path, png)?;
        println!("{kind}: {}; image: {path}", view.diagnostic());
    }
    let mut sphere_engine = SessionAdapter::new();
    sphere_engine.execute("solid.sphere", json!({"radius":10,"body_name":"Sphere"}))?;
    let mut sphere_view = Viewport::default();
    sphere_view.set_metrics(ViewportMetrics { width_points: 640.0, height_points: 480.0, scale: 1.0, origin_points: [0.0; 2] })?;
    sphere_view.begin_update(1);
    sphere_view.install(1, sphere_engine.snapshot()?);
    sphere_view.command("view.top", &json!({}))?;
    sphere_view.command("select.filter", &json!({"filter":"body"}))?;
    let sphere = sphere_view.snapshot().ok_or("no sphere scene")?;
    let (x, y, _) =
        sphere.camera.to_screen(solidcraft_ipados_session::WorldPoint::new(0.0, 0.0, 10.0), 640.0, 480.0, 100.0).ok_or("sphere projection")?;
    for phase in [InputPhase::Began, InputPhase::Ended] {
        sphere_view.input(InputEvent {
            source: InputSource::Pencil,
            phase,
            id: 1,
            x_points: x,
            y_points: y,
            x_pixels: x,
            y_pixels: y,
            scale: 1.0,
            timestamp: 1.0,
            azimuth: None,
            predicted: false,
            display_preview: false,
            sequence: 0,
            viewport_epoch: None,
            camera_epoch: None,
            document_id: None,
            pressure: None,
            altitude: None,
            geometry_revision: Some(sphere.geometry_revision),
            consumed: false,
            sketch_annotation: None,
            shift: false,
            button: 0,
            scroll: 0.0,
        })?;
    }
    let sphere = sphere_view.snapshot().ok_or("no selected sphere scene")?;
    if sphere.selected_bodies.len() != 1 {
        return Err("sphere not selected".into());
    }
    let rgba = solidcraft_ipados_bridge::renderer::capture_feedback(&sphere, &capturer, 640, 480)?;
    let png = solidcraft_render::png(640, 480, rgba).ok_or("sphere PNG encoding")?;
    let path = destination.replace(".png", "-sphere.png");
    std::fs::write(&path, png)?;
    println!("selected smooth sphere uses model GPU vertices: {path}");
    Ok(())
}

/// Fixed home versus model framing, through the real adapter and GPU renderer.
fn home_proof(destination: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = SessionAdapter::new();
    let mut view = Viewport::default();
    view.set_metrics(ViewportMetrics { width_points: 960., height_points: 640., scale: 1., origin_points: [0.; 2] })?;
    view.begin_update(1);
    view.install(1, engine.fixture()?);
    let capturer = solidcraft_gpu::capture::Capturer::headless()?;
    for (label, command) in [("fit", "view.fit"), ("home", "view.home")] {
        view.command(command, &json!({}))?;
        let snapshot = view.snapshot().ok_or("missing home frame")?;
        let rgba = solidcraft_ipados_bridge::renderer::capture_feedback(&snapshot, &capturer, 960, 640)?;
        let path = destination.replace(".png", &format!("-{label}.png"));
        std::fs::write(&path, solidcraft_render::png(960, 640, rgba).ok_or("home PNG")?)?;
        println!("{label}: camera={:?}; {path}", snapshot.camera);
    }
    Ok(())
}

/// Actual Pencil picks and native feedback on the inside and outside of a sleeve.
fn cylinder_selection_proof(destination: &str, edges: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = SessionAdapter::new();
    engine.execute("solid.cylinder", json!({"radius":10,"height":20,"body_name":"Sleeve"}))?;
    engine.execute("solid.cylinder", json!({"radius":6,"height":22,"base":[0,0,-1],"operation":"cut","targets":["Sleeve"]}))?;
    let mut view = Viewport::default();
    view.set_metrics(ViewportMetrics { width_points: 960., height_points: 640., scale: 1., origin_points: [0.; 2] })?;
    view.begin_update(1);
    view.install(1, engine.snapshot()?);
    view.command("select.filter", &json!({"filter":if edges { "edge" } else { "face" }}))?;
    view.command("view.navigate", &json!({"direction":[0.8,0.6,0.8]}))?;
    let frame = view.snapshot().ok_or("cylinder selection frame")?;
    let back = frame.camera.back();
    let radial = solidcraft_ipados_session::WorldPoint::new(back.x, back.y, 0.).normalized().ok_or("cylinder radial direction")?;
    let capturer = solidcraft_gpu::capture::Capturer::headless()?;
    for (stage, location) in [
        ("outer", radial * 10. + solidcraft_ipados_session::WorldPoint::Z * if edges { 20. } else { 10. }),
        ("inner", radial * -6. + solidcraft_ipados_session::WorldPoint::Z * if edges { 20. } else { 17. }),
    ] {
        view.command("select.clear", &json!({}))?;
        let projected = view.command("view.project", &json!({"point":location}))?;
        let x = projected["points"][0].as_f64().ok_or("cylinder pick x")?;
        let y = projected["points"][1].as_f64().ok_or("cylinder pick y")?;
        let diagnostic = view.diagnostic();
        for phase in [InputPhase::Began, InputPhase::Ended] {
            view.input(serde_json::from_value(json!({
                "source":"pencil","phase":phase,"id":1,"x_points":x,"y_points":y,"x_pixels":x,"y_pixels":y,"scale":1.,"timestamp":1.,
                "viewport_epoch":diagnostic["viewport_epoch"],"camera_epoch":diagnostic["camera_epoch"],
                "document_id":diagnostic["document_id"],"geometry_revision":diagnostic["geometry_revision"]
            }))?)?;
        }
        if view.selection().len() != 2 {
            return Err(format!("{stage} selection did not include both kernel pieces").into());
        }
        let snapshot = view.snapshot().ok_or("selected cylinder frame")?;
        let rgba = solidcraft_ipados_bridge::renderer::capture_feedback(&snapshot, &capturer, 960, 640)?;
        let path = destination.replace(".png", &format!("-{stage}.png"));
        std::fs::write(&path, solidcraft_render::png(960, 640, rgba).ok_or("cylinder selection PNG")?)?;
        println!("{stage}: {} selected {}; {path}", view.selection().len(), if edges { "edges" } else { "faces" });
    }
    Ok(())
}

/// Real geometry and highlights after synthetic Pencil taps with small lift motion.
fn selection_taps_proof(destination: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = SessionAdapter::new();
    engine.execute("solid.box", json!({"length":20,"width":20,"height":20,"body_name":"Block"}))?;
    let mut view = Viewport::default();
    view.set_metrics(ViewportMetrics { width_points: 960., height_points: 640., scale: 1., origin_points: [0.; 2] })?;
    view.begin_update(1);
    view.install(1, engine.snapshot()?);
    view.command("view.isometric", &json!({}))?;
    view.command("view.fit", &json!({}))?;
    let projection = view.command("view.project", &json!({"point":[10.,10.,20.]}))?;
    let face = [projection["points"][0].as_f64().ok_or("face pick x")?, projection["points"][1].as_f64().ok_or("face pick y")?];
    let camera = view.snapshot().ok_or("selection tap frame")?.camera;
    let capturer = solidcraft_gpu::capture::Capturer::headless()?;
    let capture = |view: &Viewport, stage: &str| -> Result<(), Box<dyn std::error::Error>> {
        let frame = view.snapshot().ok_or("selection tap frame")?;
        let rgba = solidcraft_ipados_bridge::renderer::capture_feedback(&frame, &capturer, 960, 640)?;
        let path = destination.replace(".png", &format!("-{stage}.png"));
        std::fs::write(&path, solidcraft_render::png(960, 640, rgba).ok_or("selection tap PNG")?)?;
        println!("{stage}: {} selected; {path}", view.selection().len());
        Ok(())
    };
    let input = |view: &mut Viewport, phase: InputPhase, id: u64, position: [f64; 2], timestamp: f64| -> Result<(), Box<dyn std::error::Error>> {
        let diagnostic = view.diagnostic();
        view.input(serde_json::from_value(json!({
            "source":"pencil","phase":phase,"id":id,"x_points":position[0],"y_points":position[1],
            "x_pixels":position[0],"y_pixels":position[1],"scale":1.,"timestamp":timestamp,
            "viewport_epoch":diagnostic["viewport_epoch"],"camera_epoch":diagnostic["camera_epoch"],
            "document_id":diagnostic["document_id"],"geometry_revision":diagnostic["geometry_revision"]
        }))?)?;
        Ok(())
    };
    input(&mut view, InputPhase::Began, 1, face, 1.)?;
    input(&mut view, InputPhase::Ended, 1, [face[0] + 6., face[1]], 1.1)?;
    if view.selection().len() != 1 || view.diagnostic()["selection"][0]["type"] != "face" {
        return Err("Pencil face tap with six-point lift motion did not select".into());
    }
    let selected = serde_json::to_value(view.selection())?;
    capture(&view, "selected")?;

    input(&mut view, InputPhase::Began, 2, [24., 24.], 2.)?;
    input(&mut view, InputPhase::Ended, 2, [32., 24.], 2.1)?;
    if !view.selection().is_empty() {
        return Err("Pencil empty-space tap with eight-point lift motion did not clear".into());
    }
    capture(&view, "cleared")?;

    // Hover is visual feedback. Moving it away and back must not consume a tap.
    input(&mut view, InputPhase::Hover, 0, [24., 24.], 3.)?;
    input(&mut view, InputPhase::Hover, 0, face, 3.1)?;
    input(&mut view, InputPhase::Began, 3, face, 3.2)?;
    input(&mut view, InputPhase::Cancelled, 0, face, 3.21)?;
    input(&mut view, InputPhase::Ended, 3, [face[0] + 8., face[1]], 3.3)?;
    if serde_json::to_value(view.selection())? != selected {
        return Err("Pencil tap after moving hover did not reselect the original face".into());
    }
    if view.snapshot().ok_or("reselected tap frame")?.camera != camera {
        return Err("Pencil selection changed the camera".into());
    }
    capture(&view, "reselected")?;
    println!("PASS: select, clear and reselect with six/eight-point lift motion and moving hover; synthetic Pencil input, macOS GPU only.");
    Ok(())
}

/// Compare the former face-only default with an Auto Pencil edge pick, then
/// consume that exact native edge selection in a real fillet operation.
fn edge_selection_proof(destination: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = SessionAdapter::new();
    engine.execute("solid.box", json!({"length":20,"width":20,"height":20,"body_name":"Block"}))?;
    let mut view = Viewport::default();
    view.set_metrics(ViewportMetrics { width_points: 960., height_points: 640., scale: 1., origin_points: [0.; 2] })?;
    view.begin_update(1);
    view.install(1, engine.snapshot()?);
    view.command("view.isometric", &json!({}))?;
    view.command("view.fit", &json!({}))?;
    if view.diagnostic()["filter"] != "auto" {
        return Err("initial selection filter excludes edges".into());
    }
    let capturer = solidcraft_gpu::capture::Capturer::headless()?;
    let capture = |view: &Viewport, stage: &str| -> Result<(), Box<dyn std::error::Error>> {
        let frame = view.snapshot().ok_or("edge selection frame")?;
        let rgba = solidcraft_ipados_bridge::renderer::capture_feedback(&frame, &capturer, 960, 640)?;
        let path = destination.replace(".png", &format!("-{stage}.png"));
        std::fs::write(&path, solidcraft_render::png(960, 640, rgba).ok_or("edge selection PNG")?)?;
        println!("{stage}: {path}");
        Ok(())
    };
    let tap = |view: &mut Viewport, point: [f64; 3]| -> Result<(), Box<dyn std::error::Error>> {
        let projection = view.command("view.project", &json!({"point":point}))?;
        let x = projection["points"][0].as_f64().ok_or("pick x")?;
        let y = projection["points"][1].as_f64().ok_or("pick y")?;
        let frame = view.diagnostic();
        for (phase, timestamp) in [(InputPhase::Began, 1.), (InputPhase::Ended, 1.1)] {
            view.input(InputEvent {
                source: InputSource::Pencil,
                phase,
                id: 1,
                x_points: x,
                y_points: y,
                x_pixels: x,
                y_pixels: y,
                scale: 1.,
                timestamp,
                azimuth: None,
                predicted: false,
                display_preview: false,
                sequence: 0,
                viewport_epoch: frame["viewport_epoch"].as_u64(),
                camera_epoch: frame["camera_epoch"].as_u64(),
                document_id: frame["document_id"].as_str().map(str::to_string),
                pressure: None,
                altitude: None,
                geometry_revision: frame["geometry_revision"].as_u64(),
                consumed: false,
                sketch_annotation: None,
                shift: false,
                button: 0,
                scroll: 0.,
            })?;
        }
        Ok(())
    };
    // Capture the old replacement behaviour and the default accumulated edge
    // and face highlights using the same engine geometry and actual renderer.
    let edges = [[20., 0., 10.], [20., 20., 10.]];
    for (multiple, filter, kind, points, count, stage) in [
        (false, "auto", "edge", edges, 1, "before"),
        (true, "face", "face", [[20., 10., 10.], [10., 0., 10.]], 2, "faces-selected"),
        (true, "auto", "edge", edges, 2, "selected"),
    ] {
        view.command("select.clear", &json!({}))?;
        view.command("select.multiple", &json!({"enabled":multiple}))?;
        view.command("select.filter", &json!({"filter":filter}))?;
        for point in points {
            tap(&mut view, point)?;
        }
        if view.selection().len() != count
            || view.diagnostic()["selection"].as_array().is_none_or(|items| items.iter().any(|item| item["type"] != kind))
        {
            return Err(format!("{stage} Pencil taps did not retain {count} {kind} selections").into());
        }
        capture(&view, stage)?;
    }
    let edit = engine.begin_model(9001, Some("solid.fillet"), None, view.selection())?;
    view.begin_model(edit.clone())?;
    let prepared = engine.prepare_model(&edit, edit.params.clone(), true)?;
    view.finish_model(edit.token, Some(engine.accept_model(&edit, prepared)?))?;
    // Use native body measurement rather than the coarser display tessellation.
    let measurement = engine.execute("inspect.measure", json!({"bodies":["Block"]}))?.value;
    let volume = measurement["bodies"][0]["volume_mm3"].as_f64().ok_or("fillet volume")?;
    let expected = 8_000. - 2. * (1. - std::f64::consts::PI / 4.) * 20.;
    if (volume - expected).abs() > 0.1 {
        return Err(format!("selected-edge fillet has incorrect volume: {volume}").into());
    }
    capture(&view, "filleted")?;
    println!("PASS: Pencil taps retain two edges or faces; both edges consumed by one Fillet; volume {volume:.3} mm³. macOS GPU only.");
    Ok(())
}

/// Real feature selection through the native viewport and highlight renderer.
/// UIKit chrome is captured separately on a physical iPad.
fn history_proof(destination: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = SessionAdapter::new();
    engine.fixture()?;
    engine.execute("solid.fillet", json!({"edges":[[20,-15,5]],"radius":2,"body":"Base","name":"Base round"}))?;
    let workspace = engine.workspace(&[], false, false);
    let timeline = workspace["modelling"]["timeline"].as_array().ok_or("feature timeline unavailable")?;
    let features: Vec<_> = timeline.iter().map(|f| f["id"].as_u64().ok_or("feature identity unavailable")).collect::<Result<_, _>>()?;
    let mut view = Viewport::default();
    view.set_metrics(ViewportMetrics { width_points: 960., height_points: 640., scale: 1., origin_points: [0.; 2] })?;
    view.begin_update(1);
    view.install(1, engine.snapshot()?);
    let capturer = solidcraft_gpu::capture::Capturer::headless()?;
    let before = view.snapshot().ok_or("history scene unavailable")?;
    let plain = solidcraft_ipados_bridge::renderer::capture_feedback(&before, &capturer, 960, 640)?;
    std::fs::write(destination.replace(".png", "-unselected.png"), solidcraft_render::png(960, 640, plain.clone()).ok_or("PNG")?)?;
    for (index, name) in ["base", "post", "sketch", "fillet"].into_iter().enumerate() {
        let id = features.get(index).ok_or("missing fixture feature")?;
        view.command("select.entity", &json!({"entity":{"type":"feature","id":id},"additive":false}))?;
        let snapshot = view.snapshot().ok_or("selected history scene unavailable")?;
        if !Arc::ptr_eq(&before.model, &snapshot.model) || before.camera != snapshot.camera {
            return Err("feature selection changed model/camera".into());
        }
        let rgba = solidcraft_ipados_bridge::renderer::capture_feedback(&snapshot, &capturer, 960, 640)?;
        if rgba == plain {
            return Err(format!("{name} feature produced no visible highlight").into());
        }
        let path = destination.replace(".png", &format!("-{name}.png"));
        std::fs::write(&path, solidcraft_render::png(960, 640, rgba).ok_or("PNG")?)?;
        println!("history {name}: selected {:?}; {path}", snapshot.selected_bodies);
    }
    view.command("select.clear", &json!({}))?;
    let snapshot = view.snapshot().ok_or("cleared history scene unavailable")?;
    if solidcraft_ipados_bridge::renderer::capture_feedback(&snapshot, &capturer, 960, 640)? != plain {
        return Err("clearing history selection did not restore the original rendering".into());
    }
    Ok(())
}

/// A synthetic Pencil tap selects one of two finished regions and supplies the
/// exact source of an extrusion. Captures shared GPU output, never device input.
fn profile_proof(destination: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = SessionAdapter::new();
    engine.execute("sketch.create", json!({"plane":"XY","name":"Two regions"}))?;
    engine.execute("sketch.rectangle.two_point", json!({"p0":[-30,-10],"p1":[-10,10]}))?;
    engine.execute("sketch.rectangle.two_point", json!({"p0":[10,-5],"p1":[30,5]}))?;
    engine.execute("sketch.finish", json!({}))?;
    let mut view = Viewport::default();
    view.set_metrics(ViewportMetrics { width_points: 960., height_points: 640., scale: 1., origin_points: [0.; 2] })?;
    view.begin_update(1);
    view.install(1, engine.snapshot()?);
    view.command("view.isometric", &json!({}))?;
    view.command("view.fit", &json!({}))?;
    let capturer = solidcraft_gpu::capture::Capturer::headless()?;
    let capture = |view: &Viewport, label: &str| -> Result<(), Box<dyn std::error::Error>> {
        let snapshot = view.snapshot().ok_or("profile frame")?;
        let rgba = solidcraft_ipados_bridge::renderer::capture_feedback(&snapshot, &capturer, 960, 640)?;
        let path = destination.replace(".png", &format!("-{label}.png"));
        std::fs::write(&path, solidcraft_render::png(960, 640, rgba).ok_or("profile PNG")?)?;
        println!("{label}: {path}");
        Ok(())
    };
    capture(&view, "before")?;
    let projection = view.command("view.project", &json!({"point":[20,0,0]}))?;
    let x = projection["points"][0].as_f64().ok_or("profile x")?;
    let y = projection["points"][1].as_f64().ok_or("profile y")?;
    let frame = view.diagnostic();
    for (phase, timestamp) in [(InputPhase::Began, 1.), (InputPhase::Ended, 1.1)] {
        view.input(InputEvent {
            source: InputSource::Pencil,
            phase,
            id: 1,
            x_points: x,
            y_points: y,
            x_pixels: x,
            y_pixels: y,
            scale: 1.,
            timestamp,
            azimuth: None,
            predicted: false,
            display_preview: false,
            sequence: 0,
            viewport_epoch: frame["viewport_epoch"].as_u64(),
            camera_epoch: frame["camera_epoch"].as_u64(),
            document_id: frame["document_id"].as_str().map(str::to_string),
            pressure: None,
            altitude: None,
            geometry_revision: frame["geometry_revision"].as_u64(),
            consumed: false,
            sketch_annotation: None,
            shift: false,
            button: 0,
            scroll: 0.,
        })?;
    }
    if view.selection().len() != 1 || view.diagnostic()["selection"][0]["type"] != "profile" {
        return Err("Pencil tap did not select a finished profile".into());
    }
    capture(&view, "selected")?;
    let edit = engine.begin_model(9000, Some("solid.extrude"), None, view.selection())?;
    view.begin_model(edit.clone())?;
    let prepared = engine.prepare_model(&edit, edit.params.clone(), true)?;
    view.finish_model(edit.token, Some(engine.accept_model(&edit, prepared)?))?;
    let volume = view.snapshot().ok_or("extruded frame")?.model.volume_mm3;
    if (volume - 2000.).abs() > 1e-6 {
        return Err(format!("wrong selected-region extrusion volume: {volume}").into());
    }
    capture(&view, "extruded")?;
    println!("PASS: synthetic Pencil selected the right-hand 200 mm² region; Extrude used it directly and produced 2000 mm³. macOS GPU only.");
    Ok(())
}

/// Real native finger routing after panning a top view. Captures the original and
/// corrected orbit with the shared GPU; no UIKit or device input is simulated.
fn orbit_proof(destination: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = SessionAdapter::new();
    let model = engine.proof()?;
    let pivot = model.bounds.center();
    let capturer = solidcraft_gpu::capture::Capturer::headless()?;
    for (follows_viewpoint, label) in [(false, "previous"), (true, "fixed")] {
        let mut view = Viewport::default();
        view.set_metrics(ViewportMetrics { width_points: 960.0, height_points: 640.0, scale: 1.0, origin_points: [0.0; 2] })?;
        view.begin_update(1);
        view.install(1, model.clone());
        view.command("view.top", &json!({}))?;
        view.command("view.navigate", &json!({"motion":"pan","delta":[100,50],"viewport":[960,640]}))?;
        view.command("view.touch_orbit", &json!({"follows_viewpoint":follows_viewpoint}))?;
        let capture = |view: &Viewport, stage: &str| -> Result<(), Box<dyn std::error::Error>> {
            let frame = view.snapshot().ok_or("no orbit scene")?;
            let rgba = solidcraft_ipados_bridge::renderer::capture_feedback(&frame, &capturer, 960, 640)?;
            let path = destination.replace(".png", &format!("-{stage}.png"));
            std::fs::write(&path, solidcraft_render::png(960, 640, rgba).ok_or("orbit PNG encoding")?)?;
            println!("{stage}: {path}");
            Ok(())
        };
        if !follows_viewpoint {
            capture(&view, "before")?;
        }
        let initial = view.command("view.project", &json!({"point":pivot}))?;
        let x = initial["points"][0].as_f64().ok_or("missing orbit x")?;
        let y = initial["points"][1].as_f64().ok_or("missing orbit y")?;
        for (phase, dx, timestamp) in [(InputPhase::Began, 0.0, 1.0), (InputPhase::Moved, 120.0, 2.0), (InputPhase::Ended, 120.0, 3.0)] {
            let report = view.diagnostic();
            let sample: InputEvent = serde_json::from_value(json!({
                "source":"finger","phase":phase,"id":1,"x_points":x+dx,"y_points":y,
                "x_pixels":x+dx,"y_pixels":y,"scale":1.0,"timestamp":timestamp,
                "document_id":report["document_id"],"geometry_revision":report["geometry_revision"],
                "viewport_epoch":report["viewport_epoch"],"camera_epoch":report["camera_epoch"]
            }))?;
            view.input(sample)?;
        }
        let turned = view.command("view.project", &json!({"point":pivot}))?;
        let tx = turned["points"][0].as_f64().ok_or("missing turned x")?;
        let ty = turned["points"][1].as_f64().ok_or("missing turned y")?;
        let drift = (tx - x).hypot(ty - y);
        if follows_viewpoint && drift > 1e-6 {
            return Err(format!("object centre drifted {drift} points").into());
        }
        println!("{label}: object centre drift {drift:.6} points");
        capture(&view, label)?;
    }
    Ok(())
}

/// Real native grid/origin state through the production guide converter and Mac
/// GPU. Empty geometry leaves the full positive and negative grid axes visible.
fn grid_axes_proof(destination: &str) -> Result<(), Box<dyn std::error::Error>> {
    use solidcraft_ipados_session::WorldPoint as Vec3;

    let mut engine = SessionAdapter::new();
    let mut view = Viewport::default();
    view.set_metrics(ViewportMetrics { width_points: 960., height_points: 640., scale: 1., origin_points: [0.; 2] })?;
    view.begin_update(1);
    view.install(1, engine.snapshot()?);
    let capturer = solidcraft_gpu::capture::Capturer::headless()?;
    let capture = |view: &Viewport, stage: &str, expected: [bool; 3]| -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let diagnostic = view.diagnostic();
        if diagnostic["origin"]["visible"] != json!([false, expected[0], expected[1], expected[2], false, false, false]) {
            return Err(format!("{stage}: unexpected effective origin visibility: {}", diagnostic["origin"]["visible"]).into());
        }
        let frame = view.snapshot().ok_or("missing grid-axis scene")?;
        let line = |l: &SceneLine| SceneLine { points: l.points.clone(), color: l.color, alpha: l.alpha, width: l.width, on_top: l.on_top };
        let mesh = |m: &SceneMesh| SceneMesh { mesh: m.mesh.clone(), color: m.color, face_colors: m.face_colors.clone(), opacity: m.opacity };
        let scene = Scene {
            background: frame.background,
            radius: frame.camera.distance * 2.,
            meshes: frame.model.scene.meshes.iter().map(mesh).collect(),
            lines: frame.model.scene.lines.iter().map(line).collect(),
            grid: frame.guides.grid.iter().map(line).collect(),
            ..Scene::default()
        };
        let rgba =
            capturer.rgba_highlighted(&scene, Some(solidcraft_ipados_bridge::renderer::guide_geometry(&frame.guides)), &frame.camera, 960, 640)?;
        let path = destination.replace(".png", &format!("-{stage}.png"));
        std::fs::write(&path, solidcraft_render::png(960, 640, rgba.clone()).ok_or("grid-axis PNG encoding")?)?;
        println!("{stage}: {path}");
        Ok(rgba)
    };
    let require_short_axis = |view: &Viewport, axis: Vec3| -> Result<(), Box<dyn std::error::Error>> {
        let frame = view.snapshot().ok_or("missing short-axis scene")?;
        let lines: Vec<_> =
            frame.guides.lines.iter().filter(|line| !line.on_top && line.points.iter().all(|point| point.cross(axis).len() < 1e-8)).collect();
        if lines.len() != 1 || lines[0].points != vec![Vec3::ZERO, axis * 30.] {
            return Err(format!("axis {axis:?} must retain its short positive origin marker outside the visible grid").into());
        }
        Ok(())
    };
    view.command("view.grid", &json!({"visible":true,"integrated_axes":false}))?;
    let legacy = capture(&view, "legacy-before", [false; 3])?;
    view.command("view.grid", &json!({"visible":true,"integrated_axes":true}))?;
    let isometric = capture(&view, "isometric", [true, true, false])?;
    if legacy == isometric {
        return Err("integrated XY axes did not change the legacy grid image".into());
    }
    view.command("view.top", &json!({}))?;
    let top = capture(&view, "top", [true, true, false])?;
    view.command("view.front", &json!({}))?;
    let front = capture(&view, "front", [true, false, true])?;
    view.command("view.right", &json!({}))?;
    let right = capture(&view, "right", [false, true, true])?;
    if top == front || front == right || right == top {
        return Err("principal grid images did not show their different axis colours".into());
    }
    view.command("view.isometric", &json!({}))?;
    view.command("view.origin", &json!({"item":"Z","visible":true}))?;
    require_short_axis(&view, Vec3::Z)?;
    let explicit = capture(&view, "explicit-offplane-z", [true; 3])?;
    if explicit == isometric {
        return Err("explicit Show Z did not change the automatic XY grid image".into());
    }
    view.command("view.origin", &json!({"group":"axes","visible":false}))?;
    let hidden = capture(&view, "all-axes-hidden", [false; 3])?;
    if hidden == isometric || hidden != legacy {
        return Err("hiding all axes did not restore the neutral legacy grid".into());
    }
    view.command("view.origin", &json!({"group":"axes","visible":null}))?;
    view.command("view.grid", &json!({"visible":false}))?;
    let grid_hidden = capture(&view, "grid-hidden", [false; 3])?;
    if grid_hidden == hidden || !view.snapshot().ok_or("missing hidden-grid scene")?.guides.grid.is_empty() {
        return Err("hiding the grid did not remove its automatic axes and neutral lines".into());
    }
    view.command("view.origin", &json!({"group":"axes","visible":true}))?;
    for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
        require_short_axis(&view, axis)?;
    }
    let grid_hidden_explicit = capture(&view, "grid-hidden-explicit-axes", [true; 3])?;
    if grid_hidden_explicit == grid_hidden {
        return Err("explicit origin markers disappeared when the grid was hidden".into());
    }
    view.command("view.grid", &json!({"visible":false,"integrated_axes":false}))?;
    if capture(&view, "legacy-hidden-explicit-axes", [true; 3])? != grid_hidden_explicit {
        return Err("hidden-grid origin markers must match their legacy finite appearance".into());
    }
    view.command("view.origin", &json!({"group":"axes","visible":null}))?;
    view.command("view.grid", &json!({"visible":true,"integrated_axes":true}))?;
    if capture(&view, "restored", [true, true, false])? != isometric {
        return Err("restoring the grid did not reproduce its original integrated-axis image".into());
    }
    println!(
        "PASS: native grid-plane axes, bidirectional faded in-plane guides, finite off-plane/hidden-grid origin markers, individual/group visibility and grid restoration; macOS GPU only, no UIKit/device claim"
    );
    Ok(())
}

/// Actual native assembly documents and shared-renderer frames. This does not
/// launch UIKit, a simulator or a device, and uses no independent motion model.
fn origin_proof(destination: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = SessionAdapter::new();
    let mut view = Viewport::default();
    view.set_metrics(ViewportMetrics { width_points: 960., height_points: 640., scale: 1., origin_points: [0.; 2] })?;
    view.begin_update(1);
    view.install(1, engine.snapshot()?);
    let capturer = solidcraft_gpu::capture::Capturer::headless()?;
    let capture = |view: &Viewport, stage: &str| -> Result<(), Box<dyn std::error::Error>> {
        let frame = view.snapshot().ok_or("missing origin scene")?;
        let line = |l: &SceneLine| SceneLine { points: l.points.clone(), color: l.color, alpha: l.alpha, width: l.width, on_top: l.on_top };
        let mesh = |m: &SceneMesh| SceneMesh { mesh: m.mesh.clone(), color: m.color, face_colors: m.face_colors.clone(), opacity: m.opacity };
        let scene = Scene {
            background: frame.model.scene.background,
            radius: frame.camera.distance * 2.,
            meshes: frame.model.scene.meshes.iter().map(mesh).collect(),
            lines: frame.model.scene.lines.iter().map(line).collect(),
            grid: frame.guides.grid.iter().map(line).collect(),
            ..Scene::default()
        };
        let path = destination.replace(".png", &format!("-{stage}.png"));
        // Highlight and guide slots share the same depth-tested `trans` pass.
        // Exercise the production converter rather than adding fills to the model.
        let rgba =
            capturer.rgba_highlighted(&scene, Some(solidcraft_ipados_bridge::renderer::guide_geometry(&frame.guides)), &frame.camera, 960, 640)?;
        std::fs::write(&path, solidcraft_render::png(960, 640, rgba).ok_or("origin PNG encoding")?)?;
        println!("{stage}: {path}");
        Ok(())
    };
    capture(&view, "default-hidden")?;
    view.command("view.choose_sketch_plane", &json!({}))?;
    capture(&view, "visible")?;
    view.command("view.origin", &json!({"item":"XZ","visible":false}))?;
    capture(&view, "xz-hidden")?;
    view.command("select.entity", &json!({"entity":{"type":"plane","name":"YZ"}}))?;
    capture(&view, "yz-selected")?;
    let plane = engine.preview_sketch_plane(json!("YZ"))?;
    view.command("view.sketch_plane", &json!({"plane":plane}))?;
    capture(&view, "plane-preview")?;
    view.command("view.cancel_plane", &json!({}))?;
    capture(&view, "automatic-hidden")?;
    view.command("view.origin", &json!({"item":"XY","visible":true}))?;
    capture(&view, "xy-manual-visible")?;
    view.command("view.origin", &json!({"visible":false}))?;
    capture(&view, "hidden")?;
    view.command("view.origin", &json!({"group":"planes","visible":true}))?;
    capture(&view, "all-planes")?;
    view.command("view.origin", &json!({"group":"planes","visible":false}))?;
    view.command("view.origin", &json!({"group":"axes","visible":true}))?;
    capture(&view, "all-axes")?;
    view.command("view.origin", &json!({"group":"axes","visible":false}))?;
    view.command("view.origin", &json!({"item":"O","visible":true}))?;
    capture(&view, "origin-point")?;
    println!(
        "PASS: native origin guide geometry, individual/group visibility, solid origin dot, existing plane selection and reversible preview; macOS GPU only, no UIKit/device claim"
    );
    Ok(())
}

fn assembly_proof(destination: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = SessionAdapter::new();
    let mut token = 8_000;
    let mut part = |name: &str, grounded: bool, shape: serde_json::Value| -> Result<u64, Box<dyn std::error::Error>> {
        token += 1;
        let created = assembly_command(&mut engine, token, "component.create", json!({"name":name,"parent":0,"activate":false,"grounded":grounded}))?;
        let component = created["component"].as_u64().ok_or("component ID")?;
        let occurrence = created["occurrence"].as_u64().ok_or("occurrence ID")?;
        engine.execute("component.activate", json!({"component":component,"occurrence":occurrence,"path":[occurrence]}))?;
        engine.execute("solid.box", shape)?;
        engine.execute("component.activate", json!({"component":0}))?;
        Ok(occurrence)
    };
    let base =
        part("Grounded Base", true, json!({"length":16,"width":16,"height":8,"corner":[-8,-8,-4],"name":"Base dimensions","body_name":"Base body"}))?;
    let arm = part("Arm", false, json!({"length":100,"width":4,"height":4,"corner":[0,-2,-2],"name":"Arm dimensions","body_name":"Arm body"}))?;
    let pin = part("Pin", false, json!({"length":8,"width":8,"height":12,"corner":[-4,-4,-6],"name":"Pin dimensions","body_name":"Pin body"}))?;
    let carriage = part(
        "Carriage",
        false,
        json!({"length":10,"width":8,"height":8,"corner":[-5,-4,-4],"name":"Carriage dimensions","body_name":"Carriage body"}),
    )?;
    for (occurrence, translation) in [(pin, [100, 0, 0]), (carriage, [0, 20, 0])] {
        token += 1;
        assembly_command(
            &mut engine,
            token,
            "occurrence.move",
            json!({"occurrence":occurrence,"path":[occurrence],"space":"world","translate":translation,"capture":true}),
        )?;
    }
    token += 1;
    let hinge = assembly_command(&mut engine, token, "joint.create", json!({"type":"revolute","name":"Limited hinge","flip":true,"a":{"occurrence":base,"point":[0,0,0],"z":[0,0,1]},"b":{"occurrence":arm,"point":[0,0,0],"z":[0,0,1]},"limits":[[-15,90]]}))?["joint"].as_u64().ok_or("hinge ID")?;
    token += 1;
    let rigid = assembly_command(
        &mut engine,
        token,
        "joint.as_built",
        json!({"type":"rigid","name":"Pin as built","a":arm,"b":pin,"at":{"occurrence":arm,"point":[100,0,0],"z":[0,0,1]}}),
    )?["joint"]
        .as_u64()
        .ok_or("rigid ID")?;
    token += 1;
    let slider = assembly_command(&mut engine, token, "joint.create", json!({"type":"slider","name":"Limited carriage","flip":true,"a":{"occurrence":base,"point":[0,20,0],"z":[1,0,0],"x":[0,1,0]},"b":{"occurrence":carriage,"point":[0,20,0],"z":[1,0,0],"x":[0,1,0]},"limits":[[0,50]]}))?["joint"].as_u64().ok_or("slider ID")?;
    let check_pose = |engine: &mut SessionAdapter, angle: f64, displacement: f64| -> Result<(), Box<dyn std::error::Error>> {
        let radians = angle.to_radians();
        assembly_centroid(engine, arm, [50. * radians.cos(), 50. * radians.sin(), 0.])?;
        assembly_centroid(engine, pin, [100. * radians.cos(), 100. * radians.sin(), 0.])?;
        assembly_centroid(engine, carriage, [displacement, 20., 0.])?;
        assembly_centroid(engine, base, [0.; 3])?;
        Ok(())
    };
    check_pose(&mut engine, 0., 0.)?;
    let mut view = Viewport::default();
    view.set_metrics(ViewportMetrics { width_points: 960., height_points: 640., scale: 1., origin_points: [0.; 2] })?;
    view.begin_update(1);
    view.install(1, engine.snapshot()?);
    view.command("view.isometric", &json!({}))?;
    let capturer = solidcraft_gpu::capture::Capturer::headless()?;
    assembly_capture(&mut view, &capturer, destination, "before0")?;
    let mut results = Vec::new();
    for (joint, requested, actual, angle, displacement, label) in [
        (hinge, "120 deg", 90., 90., 0., "hinge90"),
        (hinge, "30 deg", 30., 30., 0., "hinge30"),
        (slider, "75 mm", 50., 30., 50., "slider50"),
        (slider, "20 mm", 20., 30., 20., "slider20"),
    ] {
        token += 1;
        let params = json!({"joint":joint,"value":requested});
        let edit = engine.begin_model(token, Some("joint.drive"), None, &[])?;
        let edit = engine.refresh_model(&edit, &[], Some(params))?;
        view.begin_model(edit.clone())?;
        let before = engine.execute("component.list", json!({}))?.value;
        let prepared = engine.prepare_model(&edit, edit.params.clone(), false)?;
        let native = prepared.reply["values"][0]["value"].as_f64().ok_or("driven native value")?;
        if (native - actual).abs() > 1e-9 || prepared.feedback.is_empty() {
            return Err(format!("{requested}: actual {native}, expected {actual}; missing feedback").into());
        }
        println!("{label}: requested {requested}, actual native {native}; {:?}", prepared.feedback);
        results.push(json!({"label":label,"requested":requested,"reply":prepared.reply,"feedback":prepared.feedback}));
        view.publish_model(token, &prepared)?;
        if engine.execute("component.list", json!({}))?.value != before {
            return Err("assembly preview mutated authoritative placements".into());
        }
        assembly_capture(&mut view, &capturer, destination, &format!("{label}-preview"))?;
        let prepared = engine.prepare_model(&edit, edit.params.clone(), true)?;
        view.finish_model(token, Some(engine.accept_model(&edit, prepared)?))?;
        check_pose(&mut engine, angle, displacement)?;
        assembly_capture(&mut view, &capturer, destination, label)?;
    }
    // Cancel a real speculative solve; only the provisional scene disappears.
    token += 1;
    let before_cancel = engine.execute("component.list", json!({}))?.value;
    let edit = engine.begin_model(token, Some("joint.drive"), None, &[])?;
    let edit = engine.refresh_model(&edit, &[], Some(json!({"joint":hinge,"value":"-15 deg"})))?;
    view.begin_model(edit.clone())?;
    let preview = engine.prepare_model(&edit, edit.params.clone(), false)?;
    view.publish_model(token, &preview)?;
    assembly_capture(&mut view, &capturer, destination, "cancel-preview")?;
    view.finish_model(token, None)?;
    if before_cancel != engine.execute("component.list", json!({}))?.value {
        return Err("cancel changed captured placements".into());
    }
    check_pose(&mut engine, 30., 20.)?;
    assembly_capture(&mut view, &capturer, destination, "cancelled")?;
    let joints = engine.execute("joint.list", json!({}))?.value;
    if joints["joints"].as_array().map(Vec::len) != Some(3)
        || joints["conflicts"].as_array().is_some_and(|v| !v.is_empty())
        || joints["errors"].as_array().is_some_and(|v| !v.is_empty())
    {
        return Err("assembly structure or native joint solve is invalid".into());
    }
    let measured = engine.execute("inspect.measure", json!({}))?.value;
    if measured["body_count"] != 4 || (measured["total"]["volume_mm3"].as_f64().ok_or("volume")? - 5056.).abs() > 1e-6 {
        return Err("native assembly body count or volume differs from its four parametric boxes".into());
    }
    let path = destination.replace(".png", ".solidcraft");
    engine.execute("file.save_as", json!({"path":path}))?;
    let properties: Vec<_> = [base, arm, pin, carriage]
        .into_iter()
        .map(|occurrence| engine.execute("occurrence.properties", json!({"occurrence":occurrence,"path":[occurrence]})).map(|reply| reply.value))
        .collect::<Result<_, _>>()?;
    let mut loaded = SessionAdapter::new();
    loaded.execute("doc.open", json!({"path":path,"read_only":true}))?;
    check_pose(&mut loaded, 30., 20.)?;
    if loaded.execute("joint.list", json!({}))?.value != joints || loaded.execute("inspect.measure", json!({}))?.value != measured {
        return Err("native round-trip changed joint relationships or geometry".into());
    }
    let report = json!({"source":"macOS shared GPU; not UIKit or physical iPad","native_file":path,"occurrences":{"base":base,"arm":arm,"pin":pin,"carriage":carriage},"joint_ids":{"hinge":hinge,"rigid":rigid,"slider":slider},"results":results,"components":engine.execute("component.list",json!({}))?.value,"joints":joints,"properties":properties,"measurement":measured,"document":engine.execute("document.inspect",json!({}))?.value});
    std::fs::write(destination.replace(".png", "-native.json"), serde_json::to_vec_pretty(&report)?)?;
    println!(
        "PASS: four parametric box components, hinge/rigid/slider native joints, 120→90 deg and 75→50 mm limits, speculative previews/cancel, analytic centroids, 5056 mm³, native save/reopen; {path}"
    );
    Ok(())
}

fn assembly_command(
    engine: &mut SessionAdapter,
    token: u64,
    operation: &str,
    params: serde_json::Value,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let edit = engine.begin_model(token, Some(operation), None, &[])?;
    let prepared = engine.prepare_model(&edit, params, true)?;
    let reply = prepared.reply.clone();
    engine.accept_model(&edit, prepared)?;
    Ok(reply)
}

fn assembly_centroid(engine: &mut SessionAdapter, occurrence: u64, expected: [f64; 3]) -> Result<(), Box<dyn std::error::Error>> {
    let properties = engine.execute("occurrence.properties", json!({"occurrence":occurrence,"path":[occurrence],"descendants":false}))?.value;
    let centroid = &properties["total"]["center_of_mass"];
    for (axis, expected) in expected.into_iter().enumerate() {
        let actual = centroid[axis].as_f64().ok_or("world center of mass")?;
        if (actual - expected).abs() > 1e-6 {
            return Err(format!("occurrence {occurrence} centroid {axis}: {actual} != {expected}").into());
        }
    }
    println!("occurrence {occurrence}: native world centroid {centroid}");
    Ok(())
}

fn assembly_capture(
    view: &mut Viewport,
    capturer: &solidcraft_gpu::capture::Capturer,
    destination: &str,
    label: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    view.command("view.fit", &json!({}))?;
    let snapshot = view.snapshot().ok_or("assembly frame")?;
    let rgba = solidcraft_ipados_bridge::renderer::capture_feedback(&snapshot, capturer, 960, 640)?;
    let path = destination.replace(".png", &format!("-{label}.png"));
    std::fs::write(&path, solidcraft_render::png(960, 640, rgba).ok_or("assembly PNG")?)?;
    if capturer.last_stats().triangles == 0 {
        return Err("GPU drew no native assembly geometry".into());
    }
    println!(
        "{label}: {} bodies; {} triangles; {}; {} ({})",
        snapshot.model.body_count,
        snapshot.model.triangle_count,
        path,
        capturer.renderer(),
        capturer.adapter_name()
    );
    Ok(())
}

fn continuous_resize_proof(destination: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = SessionAdapter::new();
    let sketch = engine.execute("sketch.create", json!({"plane":"XY"}))?.value["sketch"].as_u64().ok_or("sketch")?;
    engine.execute("sketch.rectangle.two_point", json!({"p0":[0,0],"p1":[40,30]}))?;
    engine.execute("sketch.finish", json!({}))?;
    let selection = [serde_json::from_value(json!({"type":"feature","id":sketch}))?];
    let mut view = Viewport::default();
    view.set_metrics(ViewportMetrics { width_points: 960., height_points: 640., scale: 1., origin_points: [0.; 2] })?;
    view.begin_update(1);
    view.install(1, engine.snapshot()?);
    view.command("view.exit_planar", &json!({}))?;
    view.command("view.home", &json!({}))?;
    let edit = engine.begin_model(901, Some("solid.extrude"), None, &selection)?;
    view.begin_model(edit.clone())?;
    let initial = engine.prepare_model(&edit, edit.params.clone(), false)?;
    view.publish_model(901, &initial)?;
    view.command("view.fit", &json!({}))?;
    view.command("view.navigate", &json!({"motion":"zoom","factor":1.5}))?;
    let start = view.diagnostic()["handle_point"].as_array().ok_or("handle")?.clone();
    let capturer = solidcraft_gpu::capture::Capturer::headless()?;
    let capture = |view: &Viewport, label: &str| -> Result<(), Box<dyn std::error::Error>> {
        let frame = view.snapshot().ok_or("resize frame")?;
        let rgba = solidcraft_ipados_bridge::renderer::capture_feedback(&frame, &capturer, 960, 640)?;
        std::fs::write(destination.replace(".png", &format!("-{label}.png")), solidcraft_render::png(960, 640, rgba).ok_or("resize PNG")?)?;
        println!("{label}: {:.3} mm³, contact {}", frame.model.volume_mm3, view.diagnostic()["input"]["edit"]);
        Ok(())
    };
    let send = |view: &mut Viewport, phase: &str, depth: f64, time: f64| -> Result<(), Box<dyn std::error::Error>> {
        let mut world = start.clone();
        world[2] = json!(depth);
        let projected = view.command("view.project", &json!({"point":world}))?;
        let context = view.diagnostic();
        view.input(serde_json::from_value(json!({"source":"pencil","phase":phase,"id":91,"timestamp":time,
            "x_points":projected["points"][0],"y_points":projected["points"][1],"x_pixels":projected["pixels"][0],"y_pixels":projected["pixels"][1],"scale":1,
            "geometry_revision":context["geometry_revision"],"document_id":context["document_id"],"viewport_epoch":context["viewport_epoch"],
            "camera_epoch":context["camera_epoch"],"display_preview":context["preview"]}))?)?;
        Ok(())
    };
    capture(&view, "depth10")?;
    send(&mut view, "began", 10., 1.)?;
    send(&mut view, "ended", 10., 2.)?;
    send(&mut view, "began", 10., 3.)?;
    for (index, depth) in [15., 20., 12.].into_iter().enumerate() {
        let time = 4. + index as f64;
        send(&mut view, "moved", depth, time)?;
        let work = view.take_tool_work().ok_or("lost continuous resize contact")?;
        let prepared = engine.prepare_tool(work.clone())?;
        // New actual input arrives before scratch geometry completes. Display
        // that completed pose without rewinding/validating the newer proposal.
        send(&mut view, "moved", depth + 1., time + 0.1)?;
        if !view.publish_model_progress(&work, prepared.model) || view.diagnostic()["model_value"]["valid"] != false {
            return Err("continuous movement lost preview progress or enabled Apply".into());
        }
        let edit = view.model_edit(901)?;
        view.update_model(engine.refresh_model(&edit, &selection, None)?)?;
        capture(&view, &format!("progress{depth}"))?;
        send(&mut view, "moved", depth, time + 0.2)?;
        let work = view.take_tool_work().ok_or("inspection released the new extrusion handle")?;
        let prepared = engine.prepare_tool(work.clone())?;
        view.publish_model_tool_details(&work, prepared.reply, prepared.feedback, prepared.handle);
        if !view.publish_preview(&work, prepared.model) {
            return Err("resize preview rejected".into());
        }
        let edit = view.model_edit(901)?;
        view.update_model(engine.refresh_model(&edit, &selection, None)?)?;
        capture(&view, &format!("depth{depth}"))?;
    }
    send(&mut view, "ended", 12., 8.)?;
    let work = view.take_tool_work().ok_or("final lift")?;
    let prepared = engine.prepare_tool(work.clone())?;
    if !view.publish_preview(&work, prepared.model) {
        return Err("final resize preview rejected".into());
    }
    if engine.snapshot()?.volume_mm3.abs() > 1e-6 {
        return Err("a handle sample changed the authoritative document".into());
    }
    let edit = view.model_edit(901)?;
    let prepared = engine.prepare_model(&edit, edit.params.clone(), true)?;
    view.finish_model(901, Some(engine.accept_model(&edit, prepared)?))?;
    if (engine.snapshot()?.volume_mm3 - 14_400.).abs() > 1e-5 {
        return Err("wrong final extrusion volume".into());
    }
    capture(&view, "applied12")?;
    println!(
        "PASS: new extrusion, tap/re-grab, one continuous Pencil contact 10→15→20→12 mm with in-flight motion, displayed progress/inspection, explicit Apply. Synthetic input, macOS GPU; physical Pencil unverified."
    );
    Ok(())
}

fn subtract_selection_proof(destination: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = SessionAdapter::new();
    engine.execute("solid.box", json!({"length":40,"width":30,"height":20,"body_name":"Stock"}))?;
    engine.execute("solid.box", json!({"corner":[55,0,0],"length":20,"width":30,"height":20,"body_name":"Other"}))?;
    let sketch = engine.execute("sketch.create", json!({"plane":"XY","offset":20}))?.value["sketch"].as_u64().ok_or("sketch")?;
    engine.execute("sketch.rectangle.two_point", json!({"p0":[10,8],"p1":[30,22]}))?;
    engine.execute("sketch.finish", json!({}))?;
    let mut view = Viewport::default();
    view.set_metrics(ViewportMetrics { width_points: 960., height_points: 640., scale: 1., origin_points: [0.; 2] })?;
    view.begin_update(1);
    view.install(1, engine.snapshot()?);
    view.command("view.isometric", &json!({}))?;
    view.command("view.fit", &json!({}))?;
    let capturer = solidcraft_gpu::capture::Capturer::headless()?;
    let capture = |view: &Viewport, stage: &str| -> Result<(), Box<dyn std::error::Error>> {
        let snapshot = view.snapshot().ok_or("subtract frame")?;
        let rgba = solidcraft_ipados_bridge::renderer::capture_feedback(&snapshot, &capturer, 960, 640)?;
        let path = destination.replace(".png", &format!("-{stage}.png"));
        std::fs::write(&path, solidcraft_render::png(960, 640, rgba).ok_or("PNG")?)?;
        println!("{stage}: {:.3} mm³; {path}", snapshot.model.volume_mm3);
        Ok(())
    };
    let send = |view: &mut Viewport, point: serde_json::Value, phase: &str, time: f64| -> Result<(), Box<dyn std::error::Error>> {
        let projected = view.command("view.project", &json!({"point":point}))?;
        let context = view.diagnostic();
        view.input(serde_json::from_value(json!({"source":"pencil","phase":phase,"id":91,"timestamp":time,
            "x_points":projected["points"][0],"y_points":projected["points"][1],"x_pixels":projected["pixels"][0],"y_pixels":projected["pixels"][1],"scale":1,
            "geometry_revision":context["geometry_revision"],"document_id":context["document_id"],"viewport_epoch":context["viewport_epoch"],
            "camera_epoch":context["camera_epoch"],"display_preview":context["preview"]}))?)?;
        Ok(())
    };
    capture(&view, "before")?;
    let edit = engine.begin_model(788, Some("solid.extrude"), None, &[serde_json::from_value(json!({"type":"feature","id":sketch}))?])?;
    view.begin_model(edit)?;
    view.select_model_inputs(788, Some("targets"))?;
    send(&mut view, json!([5, 5, 20]), "began", 1.)?;
    send(&mut view, json!([5, 5, 20]), "ended", 1.1)?;
    let edit = engine.refresh_model(&view.model_edit(788)?, view.selection(), None)?;
    if edit.params["targets"] != json!(["Stock"]) || edit.params["profile"]["sketch"] != sketch {
        return Err("target pick replaced extrusion source".into());
    }
    view.update_model(edit.clone())?;
    capture(&view, "target-picked")?;
    let preview = engine.prepare_model(&edit, edit.params.clone(), false)?;
    view.publish_model(788, &preview)?;
    let start = view.diagnostic()["handle_point"].clone();
    send(&mut view, start.clone(), "began", 2.)?;
    let mut inward = start;
    inward[2] = json!(15.);
    send(&mut view, inward.clone(), "moved", 3.)?;
    let work = view.take_tool_work().ok_or("inward drag work")?;
    let prepared = engine.prepare_tool(work.clone())?;
    view.publish_tool_preview(prepared);
    capture(&view, "automatic-subtract")?;
    send(&mut view, inward, "ended", 4.)?;
    let edit = view.model_edit(788)?;
    if edit.params["operation"] != "cut" {
        return Err("inward drag did not select Subtract".into());
    }
    let prepared = engine.prepare_model(&edit, edit.params.clone(), true)?;
    let model = engine.accept_model(&edit, prepared)?;
    if (model.volume_mm3 - 34600.).abs() > 1e-5 {
        return Err("automatic cut volume".into());
    }
    view.finish_model(788, Some(model))?;
    capture(&view, "applied")?;
    Ok(())
}

fn face_extrude_proof(destination: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = SessionAdapter::new();
    engine.execute("solid.box", json!({"length":40,"width":30,"height":10,"body_name":"Stock"}))?;
    let mut view = Viewport::default();
    view.set_metrics(ViewportMetrics { width_points: 960., height_points: 640., scale: 1., origin_points: [0.; 2] })?;
    view.begin_update(1);
    view.install(1, engine.snapshot()?);
    view.command("view.isometric", &json!({}))?;
    view.command("view.fit", &json!({}))?;
    let faces = engine.execute("model.faces", json!({"body":"Stock"}))?.value;
    let top = faces["faces"]
        .as_array()
        .ok_or("faces")?
        .iter()
        .find(|face| face["plane_normal"][2].as_f64().is_some_and(|normal| normal > 0.999))
        .ok_or("top face")?;
    let selection = [serde_json::from_value(json!({"type":"face","body":"Stock","index":top["index"],"point":top["point"]}))?];
    let capturer = solidcraft_gpu::capture::Capturer::headless()?;
    let capture = |view: &Viewport, stage: &str| -> Result<(), Box<dyn std::error::Error>> {
        let snapshot = view.snapshot().ok_or("face extrusion frame")?;
        let rgba = solidcraft_ipados_bridge::renderer::capture_feedback(&snapshot, &capturer, 960, 640)?;
        let path = destination.replace(".png", &format!("-{stage}.png"));
        std::fs::write(&path, solidcraft_render::png(960, 640, rgba).ok_or("PNG")?)?;
        println!("{stage}: {:.3} mm³; {path}", snapshot.model.volume_mm3);
        Ok(())
    };
    capture(&view, "before")?;
    let edit = engine.begin_model(771, Some("solid.extrude"), None, &selection)?;
    let mut params = edit.params.clone();
    params["distance"] = json!("5 mm");
    params["operation"] = json!("join");
    let edit = engine.refresh_model(&edit, &selection, Some(params))?;
    view.begin_model(edit.clone())?;
    let preview = engine.prepare_model(&edit, edit.params.clone(), false)?;
    if (preview.model.volume_mm3 - 18_000.).abs() > 1e-5 {
        return Err("face extrusion preview did not add 5 mm".into());
    }
    view.publish_model(771, &preview)?;
    if view.diagnostic()["model_value"].is_null() {
        return Err("face extrusion preview has no distance handle".into());
    }
    capture(&view, "preview")?;
    let prepared = engine.prepare_model(&edit, edit.params.clone(), true)?;
    view.finish_model(771, Some(engine.accept_model(&edit, prepared)?))?;
    capture(&view, "applied")?;
    for (token, operation, direction, volume) in
        [(772, "new", "positive", 18_000.), (773, "intersect", "negative", 6000.), (774, "cut", "negative", 6000.)]
    {
        let mut engine = SessionAdapter::new();
        engine.execute("solid.box", json!({"length":40,"width":30,"height":10,"body_name":"Stock"}))?;
        view.begin_update(token);
        view.install(token, engine.snapshot()?);
        let edit = engine.begin_model(token, Some("solid.extrude"), None, &selection)?;
        let mut params = edit.params.clone();
        params["distance"] = json!("5 mm");
        params["operation"] = json!(operation);
        params["direction"] = json!(direction);
        let edit = engine.refresh_model(&edit, &selection, Some(params))?;
        view.begin_model(edit.clone())?;
        let preview = engine.prepare_model(&edit, edit.params.clone(), false)?;
        if (preview.model.volume_mm3 - volume).abs() > 1e-5 {
            return Err(format!("{operation} preview produced the wrong volume").into());
        }
        view.publish_model(token, &preview)?;
        capture(&view, operation)?;
        let prepared = engine.prepare_model(&edit, edit.params.clone(), true)?;
        view.finish_model(token, Some(engine.accept_model(&edit, prepared)?))?;
    }
    Ok(())
}

fn modelling_proof(destination: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = SessionAdapter::new();
    let sketch = engine.execute("sketch.create", json!({"plane":"XY","name":"Plate sketch"}))?.value["sketch"].as_u64().ok_or("sketch")?;
    let rectangle = engine.execute("sketch.rectangle.two_point", json!({"p0":[0,0],"p1":[40,30]}))?.value;
    for (side, value) in [("bottom", "40 mm"), ("right", "30 mm")] {
        engine.execute("sketch.dimension", json!({"entities":[rectangle["sides"][side]],"type":"length","value":value}))?;
    }
    engine.execute("sketch.finish", json!({}))?;
    let mut view = Viewport::default();
    view.set_metrics(ViewportMetrics { width_points: 960., height_points: 640., scale: 1., origin_points: [0.; 2] })?;
    view.begin_update(1);
    view.install(1, engine.snapshot()?);
    let edit = engine.begin_model(71, Some("solid.extrude"), None, &[serde_json::from_value(json!({"type":"feature","id":sketch}))?])?;
    view.begin_model(edit.clone())?;
    let preview = engine.prepare_model(&edit, edit.params.clone(), false)?;
    view.publish_model(71, &preview)?;
    view.command("view.isometric", &json!({}))?;
    view.command("view.fit", &json!({}))?;
    let capturer = solidcraft_gpu::capture::Capturer::headless()?;
    let capture = |view: &Viewport, label: &str| -> Result<(), Box<dyn std::error::Error>> {
        let snapshot = view.snapshot().ok_or("model frame")?;
        let rgba = solidcraft_ipados_bridge::renderer::capture_feedback(&snapshot, &capturer, 960, 640)?;
        let path = destination.replace(".png", &format!("-{label}.png"));
        std::fs::write(&path, solidcraft_render::png(960, 640, rgba).ok_or("PNG")?)?;
        println!("{label}: {path}");
        Ok(())
    };
    capture(&view, "preview10")?;
    let prepared = engine.prepare_model(&edit, edit.params.clone(), true)?;
    view.finish_model(71, Some(engine.accept_model(&edit, prepared)?))?;
    capture(&view, "depth10")?;
    let feature = engine.workspace(&[], false, false)["modelling"]["timeline"]
        .as_array()
        .and_then(|r| r.last())
        .and_then(|r| r["id"].as_u64())
        .ok_or("extrusion ID")?;
    let edit = engine.begin_model(72, None, Some(feature), &[])?;
    let mut params = edit.params.clone();
    params["extent"]["distance"] = json!("15 mm");
    view.begin_model(edit.clone())?;
    let prepared = engine.prepare_model(&edit, params, true)?;
    view.finish_model(72, Some(engine.accept_model(&edit, prepared)?))?;
    capture(&view, "depth15")?;
    for (revision, command, label) in [(2, "edit.undo", "undo"), (3, "edit.redo", "redo")] {
        engine.execute(command, json!({}))?;
        view.begin_update(revision);
        view.install(revision, engine.snapshot()?);
        capture(&view, label)?;
    }
    let faces = engine.execute("model.faces", json!({"body":"Body1"}))?.value;
    let face = faces["faces"]
        .as_array()
        .and_then(|faces| faces.iter().find(|face| face["plane_normal"][2].as_f64().is_some_and(|z| z > 0.999)))
        .ok_or("planar push/pull face")?;
    let selected = serde_json::from_value(json!({"type":"face","body":"Body1","index":face["index"],"point":face["point"]}))?;
    let edit = engine.begin_model(74, Some("solid.offset_face"), None, &[selected])?;
    view.begin_model(edit.clone())?;
    let mut params = edit.params.clone();
    params["distance"] = json!("5 mm");
    let preview = engine.prepare_model(&edit, params, false)?;
    view.publish_model(74, &preview)?;
    capture(&view, "push-pull5")?;
    std::fs::write(destination.replace(".png", "-handle.json"), serde_json::to_vec_pretty(&view.diagnostic()["model_value"])?)?;
    view.finish_model(74, None)?;
    engine.execute("parameters.add", json!({"name":"bore_radius","expression":"3 mm"}))?;
    let bore = engine.execute("sketch.create", json!({"plane":"XY","offset":"15 mm","name":"Bore sketch"}))?.value["sketch"]
        .as_u64()
        .ok_or("bore sketch")?;
    let circle = engine.execute("sketch.circle.center", json!({"center":[20,15],"radius":3}))?.value;
    engine.execute("sketch.dimension", json!({"entities":[circle["curves"][0]],"type":"radius","value":"bore_radius"}))?;
    engine.execute("sketch.finish", json!({}))?;
    view.begin_update(4);
    view.install(4, engine.snapshot()?);
    let cut = engine.begin_model(73, Some("solid.extrude"), None, &[serde_json::from_value(json!({"type":"feature","id":bore}))?])?;
    let mut params = cut.params.clone();
    params["distance"] = json!("15 mm");
    params["direction"] = json!("negative");
    params["operation"] = json!("cut");
    params["targets"] = json!(["Body1"]);
    view.begin_model(cut.clone())?;
    let prepared = engine.prepare_model(&cut, params, true)?;
    view.finish_model(73, Some(engine.accept_model(&cut, prepared)?))?;
    capture(&view, "cut3")?;
    let measured = engine.execute("inspect.measure", json!({}))?.value["total"]["volume_mm3"].as_f64().ok_or("native measurement")?;
    if (measured - (18_000. - 135. * std::f64::consts::PI)).abs() > 0.25 {
        return Err("cut mass does not match analytic volume".into());
    }
    engine.execute("file.save_as", json!({"path":destination.replace(".png",".solidcraft")}))?;
    println!("PASS: native modelling controller preview, creation, original-feature edit, undo/redo and native save. macOS GPU only.");
    Ok(())
}

/// Repeatable Step 4 evidence from the real source-aware adapter. These samples
/// are synthetic Pencil/finger events; this is macOS GPU evidence, not hardware.
fn input_proof(destination: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = SessionAdapter::new();
    let model = engine.fixture()?;
    let mut view = Viewport::default();
    view.set_metrics(ViewportMetrics { width_points: 960.0, height_points: 640.0, scale: 1.0, origin_points: [0.0; 2] })?;
    view.begin_update(1);
    view.install(1, model);
    view.command("tool.set", &json!({"tool":"post_radius"}))?;
    let capturer = solidcraft_gpu::capture::Capturer::headless()?;
    let capture = |view: &Viewport, label: &str| -> Result<(), Box<dyn std::error::Error>> {
        let frame = view.snapshot().ok_or("no input scene")?;
        let rgba = solidcraft_ipados_bridge::renderer::capture_feedback(&frame, &capturer, 960, 640)?;
        let path = destination.replace(".png", &format!("-{label}.png"));
        std::fs::write(&path, solidcraft_render::png(960, 640, rgba).ok_or("input PNG encoding")?)?;
        println!("{label}: {:.6} mm³; {path}", frame.model.volume_mm3);
        Ok(())
    };
    capture(&view, "before")?;
    let volume = engine.snapshot()?.volume_mm3;
    let h = engine.snapshot()?.radius_handle.clone().ok_or("missing radius handle")?;
    let send = |view: &mut Viewport,
                source: InputSource,
                phase: InputPhase,
                id: u64,
                world: solidcraft_ipados_session::WorldPoint,
                time: f64|
     -> Result<(), Box<dyn std::error::Error>> {
        let projected = view.command("view.project", &json!({"point":[world.x,world.y,world.z]}))?;
        let x = projected["points"][0].as_f64().ok_or("x projection")?;
        let y = projected["points"][1].as_f64().ok_or("y projection")?;
        let report = view.diagnostic();
        view.input(InputEvent {
            source,
            phase,
            id,
            x_points: x,
            y_points: y,
            x_pixels: x,
            y_pixels: y,
            scale: 1.0,
            timestamp: time,
            pressure: None,
            altitude: None,
            azimuth: None,
            predicted: false,
            display_preview: false,
            sequence: 0,
            viewport_epoch: report["viewport_epoch"].as_u64(),
            camera_epoch: report["camera_epoch"].as_u64(),
            document_id: report["document_id"].as_str().map(str::to_string),
            geometry_revision: projected["geometry_revision"].as_u64(),
            consumed: false,
            sketch_annotation: None,
            shift: false,
            button: 0,
            scroll: 0.0,
        })?;
        Ok(())
    };
    send(&mut view, InputSource::Pencil, InputPhase::Began, 1, h.point(7.0), 1.0)?;
    send(&mut view, InputSource::Pencil, InputPhase::Moved, 1, h.point(12.0), 2.0)?;
    let work = view.take_tool_work().ok_or("no preview work")?;
    let preview = engine.prepare_tool(work.clone())?;
    if !view.publish_preview(&work, preview.model) {
        return Err("preview rejected".into());
    }
    if (engine.snapshot()?.volume_mm3 - volume).abs() > 1e-6 {
        return Err("preview changed document".into());
    }
    capture(&view, "preview")?;
    // Actual navigation through the shared path while the Pencil remains down.
    use solidcraft_ipados_session::WorldPoint as P;
    for (phase, id, world, time) in [
        (InputPhase::Began, 2, P::new(-10.0, 0.0, 10.0), 3.0),
        (InputPhase::Began, 3, P::new(30.0, 0.0, 10.0), 3.0),
        (InputPhase::Moved, 3, P::new(34.0, 3.0, 10.0), 4.0),
    ] {
        send(&mut view, InputSource::Finger, phase, id, world, time)?;
    }
    send(&mut view, InputSource::Pencil, InputPhase::Moved, 1, h.point(20.0), 5.0)?;
    if view.take_tool_work().is_some() {
        return Err("suspended Pencil produced geometry work".into());
    }
    for id in [2, 3] {
        send(&mut view, InputSource::Finger, InputPhase::Ended, id, P::new(0.0, 0.0, 10.0), 6.0)?;
    }
    send(&mut view, InputSource::Pencil, InputPhase::Moved, 1, h.point(12.0), 7.0)?; // rebase only
    send(&mut view, InputSource::Pencil, InputPhase::Ended, 1, h.point(12.0), 8.0)?;
    let work = view.take_tool_work().ok_or("no commit work")?;
    if !work.commit || !view.tool_is_current(&work) {
        return Err("invalid commit ticket".into());
    }
    let prepared = engine.prepare_tool(work.clone())?;
    let committed = engine.accept_tool(prepared)?;
    let edited_volume = committed.volume_mm3;
    if !view.complete_tool(&work, committed) {
        return Err("commit publication failed".into());
    }
    capture(&view, "committed")?;
    for (generation, command, label, expected) in [(2, "edit.undo", "undo", volume), (3, "edit.redo", "redo", edited_volume)] {
        engine.execute(command, json!({}))?;
        let model = engine.snapshot()?;
        if (model.volume_mm3 - expected).abs() > 1e-6 {
            return Err("history geometry mismatch".into());
        }
        view.begin_update(generation);
        view.install(generation, model);
        capture(&view, label)?;
    }
    send(&mut view, InputSource::Pencil, InputPhase::Began, 1, h.point(12.0), 9.0)?;
    send(&mut view, InputSource::Pencil, InputPhase::Moved, 1, h.point(16.0), 10.0)?;
    let work = view.take_tool_work().ok_or("no cancellation preview")?;
    let preview = engine.prepare_tool(work.clone())?;
    view.publish_preview(&work, preview.model);
    send(&mut view, InputSource::Pencil, InputPhase::Cancelled, 1, h.point(16.0), 11.0)?;
    if (engine.snapshot()?.volume_mm3 - edited_volume).abs() > 1e-6 || view.take_tool_work().is_some() {
        return Err("cancel changed document".into());
    }
    capture(&view, "cancelled")?;
    println!(
        "PASS: synthetic Pencil selects Post; actual engine preview; finger suspension/rebase; one timeline.edit; geometry changes; undo/redo restore; cancellation preserves committed geometry."
    );
    Ok(())
}

/// Actual shared-GPU dimension lines/arrows. UIKit text is a separate native layer.
fn rectangle_dimensions_proof(destination: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = SessionAdapter::new();
    engine.execute("sketch.create", json!({"plane":"XY"}))?;
    let rectangle = engine.execute("sketch.rectangle.two_point", json!({"p0":[10,10],"p1":[50,40],"connect":false}))?.value;
    let mut view = Viewport::default();
    view.set_metrics(ViewportMetrics { width_points: 960., height_points: 640., scale: 1., origin_points: [0.; 2] })?;
    view.begin_update(1);
    view.install(1, engine.snapshot()?);
    view.command("view.fit", &json!({}))?;
    view.command("view.navigate", &json!({"motion":"zoom","factor":0.8}))?;
    let capturer = solidcraft_gpu::capture::Capturer::headless()?;
    for (stage, visible) in [("default", true), ("hidden", false), ("restored", true)] {
        view.command("tool.free_dimensions", &json!({"enabled":visible}))?;
        let annotations = view.diagnostic()["annotations"].clone();
        if annotations.as_array().map(Vec::len) != Some(if visible { 2 } else { 0 }) {
            return Err("rectangle helper count mismatch".into());
        }
        if visible {
            for (side, midpoint, axis, sign) in [("top", [30., 40., 0.], 1, -1.), ("right", [50., 25., 0.], 0, 1.)] {
                let curve = rectangle["sides"][side].as_str().ok_or("missing rectangle side")?;
                let annotation = annotations
                    .as_array()
                    .and_then(|rows| rows.iter().find(|row| row["id"] == format!("edge:{curve}")))
                    .ok_or("missing preferred helper")?;
                let edge = view.command("view.project", &json!({"point":midpoint}))?;
                let separation = sign
                    * (annotation["points"][axis].as_f64().ok_or("missing label position")?
                        - edge["points"][axis].as_f64().ok_or("missing edge position")?);
                if (separation - 36.).abs() > 1e-6 {
                    return Err("helper is not outside its preferred edge".into());
                }
            }
        }
        let snapshot = view.snapshot().ok_or("no rectangle frame")?;
        let rgba = solidcraft_ipados_bridge::renderer::capture_feedback(&snapshot, &capturer, 960, 640)?;
        let path = destination.replace(".png", &format!("-{stage}.png"));
        std::fs::write(&path, solidcraft_render::png(960, 640, rgba).ok_or("PNG encoding")?)?;
        println!("{stage}: {annotations}; {path}");
    }
    println!(
        "PASS: two exterior top/right helpers; visibility toggle and restoration. Actual macOS GPU lines/arrows; UIKit labels and physical input unverified."
    );
    Ok(())
}

/// Actual adapter input, solver, history and GPU capture on macOS. These are
/// synthetic samples, not UIKit screenshots or evidence of Pencil hardware.
fn sketch_drag_proof(destination: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = SessionAdapter::new();
    engine.execute("sketch.create", json!({"plane":"XY"}))?;
    engine.execute("sketch.rectangle.two_point", json!({"p0":[10,20],"p1":[50,50],"connect":false}))?;
    let mut view = Viewport::default();
    view.set_metrics(ViewportMetrics { width_points: 960., height_points: 640., scale: 1., origin_points: [0.; 2] })?;
    view.begin_update(1);
    view.install(1, engine.snapshot()?);
    view.command("view.fit", &json!({}))?;
    view.command("view.navigate", &json!({"motion":"zoom","factor":2.0}))?;
    let original_model = engine.snapshot()?;
    let original = original_model.sketch.as_ref().ok_or("no sketch")?;
    for curve in &original.sketch.curves {
        view.command("select.entity", &json!({"entity":{"type":"sketch_curve","id":curve.id,"sketch":original.id},"additive":true}))?;
    }
    let capturer = solidcraft_gpu::capture::Capturer::headless()?;
    let capture = |view: &Viewport, label: &str| -> Result<(), Box<dyn std::error::Error>> {
        let snapshot = view.snapshot().ok_or("no sketch frame")?;
        let rgba = solidcraft_ipados_bridge::renderer::capture_feedback(&snapshot, &capturer, 960, 640)?;
        let path = destination.replace(".png", &format!("-{label}.png"));
        std::fs::write(&path, solidcraft_render::png(960, 640, rgba).ok_or("PNG encoding")?)?;
        println!("{label}: {path}");
        Ok(())
    };
    capture(&view, "before")?;
    for (enabled, contact) in [(false, 1), (true, 2)] {
        view.command("view.tool_handles", &json!({"enabled":enabled}))?;
        for (phase, point) in [("began", [10., 20., 0.]), ("moved", [30., 35., 0.]), ("ended", [30., 35., 0.])] {
            let p = view.command("view.project", &json!({"point":point}))?;
            view.input(serde_json::from_value(json!({"source":"pencil","phase":phase,"id":contact,
                "x_points":p["points"][0],"y_points":p["points"][1],"x_pixels":p["pixels"][0],"y_pixels":p["pixels"][1],
                "scale":1,"timestamp":contact as f64,"geometry_revision":p["geometry_revision"]}))?)?;
            if let Some(work) = view.take_tool_work() {
                if !enabled {
                    return Err("legacy Select unexpectedly moved geometry".into());
                }
                let prepared = engine.prepare_tool(work.clone())?;
                if work.commit {
                    if !view.complete_tool(&work, engine.accept_tool(prepared)?) {
                        return Err("sketch drag commit rejected".into());
                    }
                } else {
                    if !view.publish_preview(&work, prepared.model) {
                        return Err("sketch drag preview rejected".into());
                    }
                    capture(&view, "preview")?;
                }
            }
        }
        capture(&view, if enabled { "committed" } else { "previous" })?;
    }
    let moved_model = engine.snapshot()?;
    let moved = moved_model.sketch.as_ref().ok_or("no moved sketch")?;
    for point in original.sketch.points.iter().filter(|p| p.id != "origin") {
        let index = moved.sketch.resolve_point(&point.id).ok_or("lost sketch point")?;
        let at = moved.sketch.point(index).ok_or("lost sketch position")?;
        if (at.x - point.pos.x - 20.).hypot(at.y - point.pos.y - 15.) > 1e-6 {
            return Err("selected rectangle did not translate together".into());
        }
    }
    if !moved.solved || moved.profiles != 1 || moved.sketch.constraints != original.sketch.constraints {
        return Err("drag did not preserve the solved profile and constraints".into());
    }
    engine.execute("edit.undo", json!({}))?;
    if engine.snapshot()?.sketch.as_ref().ok_or("no undo sketch")?.sketch != original.sketch {
        return Err("one undo did not restore the sketch".into());
    }
    println!(
        "PASS: Select drags a selected rectangle by (20,15) mm, preserves constraints/profile, and undoes once. Synthetic Pencil and macOS GPU; physical iPad input unverified."
    );
    Ok(())
}

fn sketch_proof(destination: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut engine = SessionAdapter::new();
    engine.execute("sketch.create", json!({"plane":"XY","name":"Sketch proof"}))?;
    let mut view = Viewport::default();
    view.set_metrics(ViewportMetrics { width_points: 960.0, height_points: 640.0, scale: 1.0, origin_points: [0.0; 2] })?;
    view.begin_update(1);
    view.install(1, engine.snapshot()?);
    view.command("tool.set", &json!({"tool":"sketch.rectangle.two_point"}))?;
    for (id, point) in [(1, [0.0, 0.0, 0.0]), (2, [40.0, 30.0, 0.0])] {
        let p = view.command("view.project", &json!({"point":point}))?;
        for phase in ["began", "ended"] {
            view.input(serde_json::from_value(json!({"source":"pencil","phase":phase,"id":id,
                "x_points":p["points"][0],"y_points":p["points"][1],"x_pixels":p["pixels"][0],"y_pixels":p["pixels"][1],
                "scale":1,"timestamp":id as f64,"geometry_revision":p["geometry_revision"]}))?)?;
        }
    }
    let work = view.take_tool_work().ok_or("rectangle input did not produce a command")?;
    let prepared = engine.prepare_tool(work.clone())?;
    if !view.complete_tool(&work, engine.accept_tool(prepared)?) {
        return Err("rectangle commit publication failed".into());
    }
    let sketch = engine.workspace(&[], false, false)["sketch"].clone();
    let id = sketch["id"].as_u64().ok_or("missing sketch ID")?;
    for (line, value) in [("l1", "40 mm"), ("l2", "30 mm")] {
        engine.execute("sketch.dimension", json!({"entities":[line],"value":value,"type":"length"}))?;
    }
    engine.execute("sketch.finish", json!({}))?;
    engine.execute("sketch.edit", json!({"sketch":id}))?;
    view.begin_update(2);
    view.install(2, engine.snapshot()?);
    view.command("view.fit", &json!({}))?;
    view.command("view.navigate", &json!({"motion":"zoom","factor":1.1}))?;
    let capturer = solidcraft_gpu::capture::Capturer::headless()?;
    let capture = |view: &Viewport, label: &str| -> Result<(), Box<dyn std::error::Error>> {
        let snapshot = view.snapshot().ok_or("no sketch frame")?;
        let rgba = solidcraft_ipados_bridge::renderer::capture_feedback(&snapshot, &capturer, 960, 640)?;
        let path = destination.replace(".png", &format!("-{label}.png"));
        std::fs::write(&path, solidcraft_render::png(960, 640, rgba).ok_or("PNG encoding")?)?;
        println!("{label}: {path}");
        Ok(())
    };
    let workspace = engine.workspace(&[], false, false);
    let dimension = workspace["sketch"]["constraints"]
        .as_array()
        .ok_or("missing constraints")?
        .iter()
        .find(|c| c["expression"] == "40 mm")
        .and_then(|c| c["id"].as_str())
        .ok_or("missing width dimension")?;
    view.command("select.entity", &json!({"entity":{"type":"sketch_constraint","id":dimension}}))?;
    capture(&view, "40x30")?;
    let edit = engine.begin_numeric(1, view.selection())?;
    view.begin_numeric(edit.clone())?;
    let preview = engine.prepare_numeric(&edit, "60 mm", false)?;
    view.publish_numeric(1, &preview)?;
    capture(&view, "60x30-preview")?;
    let prepared = engine.prepare_numeric(&edit, "60 mm", true)?;
    view.finish_numeric(1, Some(engine.accept_numeric(&edit, prepared)?))?;
    capture(&view, "60x30")?;
    let after = engine.workspace(&[], false, false);
    let area = after["sketch"]["profiles"][0]["area_mm2"].as_f64().ok_or("missing native profile area")?;
    if (area - 1800.0).abs() > 1e-6 {
        return Err("native profile area mismatch".into());
    }
    for (generation, command, label) in [(3, "edit.undo", "undo"), (4, "edit.redo", "redo")] {
        engine.execute(command, json!({}))?;
        view.begin_update(generation);
        view.install(generation, engine.snapshot()?);
        capture(&view, label)?;
    }
    view.command("tool.set", &json!({"tool":"select"}))?;
    view.command("select.entity", &json!({"entity":{"type":"sketch_constraint","id":dimension}}))?;
    capture(&view, "dimension-before")?;
    let annotations = view.diagnostic()["annotations"].clone();
    let annotation = annotations.as_array().and_then(|rows| rows.iter().find(|row| row["id"] == dimension)).ok_or("missing dimension annotation")?;
    let x = annotation["points"][0].as_f64().ok_or("missing annotation x")?;
    let y = annotation["points"][1].as_f64().ok_or("missing annotation y")?;
    for (phase, dx, dy, time) in [("began", 5.0, 4.0, 10.0), ("moved", 45.0, 64.0, 10.1), ("ended", 45.0, 64.0, 10.2)] {
        view.input(serde_json::from_value(json!({"source":"finger","phase":phase,"id":40,
            "x_points":x+dx,"y_points":y+dy,"x_pixels":x+dx,"y_pixels":y+dy,"scale":1,"timestamp":time,
            "geometry_revision":view.snapshot().ok_or("missing drag frame")?.geometry_revision}))?)?;
        if let Some(work) = view.take_tool_work() {
            let prepared = engine.prepare_tool(work.clone())?;
            if work.commit {
                if !view.complete_tool(&work, engine.accept_tool(prepared)?) {
                    return Err("dimension drag commit failed".into());
                }
            } else if !view.publish_preview(&work, prepared.model) {
                return Err("dimension drag preview failed".into());
            }
            capture(&view, if work.commit { "dimension-moved" } else { "dimension-preview" })?;
        }
    }
    let moved = view.diagnostic()["annotations"].clone();
    let annotation = moved.as_array().and_then(|rows| rows.iter().find(|row| row["id"] == dimension)).ok_or("missing moved annotation")?;
    if (annotation["points"][0].as_f64().ok_or("missing moved x")? - x - 40.0).abs() > 1e-6
        || (annotation["points"][1].as_f64().ok_or("missing moved y")? - y - 60.0).abs() > 1e-6
        || (annotation["value"].as_f64().ok_or("missing moved value")? - 60.0).abs() > 1e-6
    {
        return Err("dimension drag changed its grab offset or measured value".into());
    }
    println!(
        "PASS: synthetic Pencil rectangle; actual driving dimensions, reopen, width preview/commit, native closed profile area1800, undo/redo, direct touch dimension drag with arrows and unchanged length. macOS GPU only; UIKit/Pencil hardware unverified."
    );
    Ok(())
}
