//! Real source-anchored surface-tool handles and scratch geometry, without an emulator.
use serde_json::json;
use solidcraft_ipados_session::{SessionAdapter, Viewport, ViewportMetrics};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let destination = std::env::args().nth(1).ok_or("usage: surface_handles OUTPUT_PREFIX")?;
    let capturer = solidcraft_gpu::capture::Capturer::headless()?;
    for (operation, parameter) in [("fillet", "radius"), ("chamfer", "distance"), ("shell", "thickness")] {
        let mut engine = SessionAdapter::new();
        engine.execute("solid.box", json!({"length":20,"width":20,"height":20,"body_name":"Block"}))?;
        let selection = if operation == "shell" {
            let faces = engine.execute("model.faces", json!({"body":"Block"}))?.value;
            let face = faces["faces"]
                .as_array()
                .and_then(|faces| faces.iter().find(|f| f["plane_normal"][2].as_f64().is_some_and(|z| z > 0.99)))
                .ok_or("top face")?;
            serde_json::from_value(json!({"type":"face","body":"Block","index":face["index"],"point":face["point"]}))?
        } else {
            let edges = engine.execute("model.edges", json!({"body":"Block"}))?.value;
            let edge = edges["edges"].as_array().and_then(|edges| edges.iter().find(|e| e["mid"] == json!([20., 0., 10.]))).ok_or("front edge")?;
            serde_json::from_value(json!({"type":"edge","body":"Block","index":edge["index"],"point":edge["mid"]}))?
        };
        let mut view = Viewport::default();
        view.set_metrics(ViewportMetrics { width_points: 960., height_points: 640., scale: 1., origin_points: [0.; 2] })?;
        view.begin_update(1);
        view.install(1, engine.snapshot()?);
        view.command("view.isometric", &json!({}))?;
        view.command("view.fit", &json!({}))?;
        let edit = engine.begin_model(1, Some(&format!("solid.{operation}")), None, &[selection])?;
        let original = engine.model_descriptor(&edit)["handle"].clone();
        if original["parameter"] != parameter {
            return Err("surface handle missing".into());
        }
        view.begin_model(edit.clone())?;
        for size in [1, 3] {
            let mut params = edit.params.clone();
            params[parameter] = json!(format!("{size} mm"));
            let preview = engine.prepare_model(&edit, params, false)?;
            view.publish_model(1, &preview)?;
            let value = view.diagnostic()["model_value"].clone();
            if value["value"].as_f64() != Some(f64::from(size))
                || engine.workspace(&[], false, false)["modelling"]["timeline"].as_array().is_none_or(|timeline| timeline.len() != 1)
            {
                return Err("preview changed history or lost the numeric value".into());
            }
            let snapshot = view.snapshot().ok_or("surface frame")?;
            let rgba = solidcraft_ipados_bridge::renderer::capture_feedback(&snapshot, &capturer, 960, 640)?;
            std::fs::write(format!("{destination}-{operation}-{size}.png"), solidcraft_render::png(960, 640, rgba).ok_or("PNG")?)?;
            std::fs::write(format!("{destination}-{operation}-{size}.json"), serde_json::to_vec_pretty(&value)?)?;
        }
        view.cancel_model();
        println!("PASS: {operation} source handle, 1/3 mm live previews and numeric readout; cancel preserves history. macOS GPU only.");
    }
    Ok(())
}
