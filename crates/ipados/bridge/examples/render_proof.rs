//! Real command → revisioned scene → shared GPU, with before/after update evidence.
//! macOS GPU evidence only: does not launch or emulate the iPad application.
use serde_json::json;
use solidcraft_ipados_session::{InputEvent, InputPhase, InputSource, SessionAdapter, Viewport, ViewportMetrics};
use solidcraft_render::{Scene, SceneLine, SceneMesh};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let destination = std::env::args().nth(1).ok_or("usage: render_proof OUTPUT.png")?;
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
            background: snapshot.model.scene.background,
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
                pressure: None,
                altitude: None,
                geometry_revision: Some(snapshot.geometry_revision),
                consumed: false,
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
            pressure: None,
            altitude: None,
            geometry_revision: Some(sphere.geometry_revision),
            consumed: false,
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
