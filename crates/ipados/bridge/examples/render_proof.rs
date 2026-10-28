//! Real command → revisioned scene → shared GPU, with before/after update evidence.
//! macOS GPU evidence only: does not launch or emulate the iPad application.
use serde_json::json;
use solidcraft_ipados_session::{InputEvent, InputPhase, InputSource, SessionAdapter, Viewport, ViewportMetrics};
use solidcraft_render::{Scene, SceneLine, SceneMesh};
use std::sync::Arc;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let destination = std::env::args().nth(1).ok_or("usage: render_proof OUTPUT.png")?;
    if std::env::args().nth(2).as_deref() == Some("--input") {
        return input_proof(&destination);
    }
    if std::env::args().nth(2).as_deref() == Some("--sketch") {
        return sketch_proof(&destination);
    }
    if std::env::args().nth(2).as_deref() == Some("--modelling") {
        return modelling_proof(&destination);
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

/// Actual native assembly documents and shared-renderer frames. This does not
/// launch UIKit, a simulator or a device, and uses no independent motion model.
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

/// Actual adapter input, solver, history and GPU capture on macOS. These are
/// synthetic samples, not UIKit screenshots or evidence of Pencil hardware.
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
    println!(
        "PASS: synthetic Pencil rectangle; actual driving dimensions, reopen, width preview/commit, native closed profile area1800, undo/redo. macOS GPU only; UIKit/Pencil hardware unverified."
    );
    Ok(())
}
