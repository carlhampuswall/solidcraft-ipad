//! Compare native axis opacity in the actual renderer, without launching an iPad.
use serde_json::json;
use solidcraft_ipados_session::{SessionAdapter, Viewport, ViewportMetrics};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let prefix = std::env::args().nth(1).ok_or("usage: axis_appearance OUTPUT_PREFIX")?;
    let mut engine = SessionAdapter::new();
    let mut view = Viewport::default();
    view.set_metrics(ViewportMetrics { width_points: 960., height_points: 640., scale: 1., origin_points: [0.; 2] })?;
    view.begin_update(1);
    view.install(1, engine.fixture()?);
    view.command("view.origin", &json!({"group":"axes","visible":true}))?;
    let capturer = solidcraft_gpu::capture::Capturer::headless()?;
    for (appearance, color, grid) in [("light", json!(null), json!(null)), ("dark", json!([28, 28, 30]), json!([[45, 45, 47], [62, 62, 65]]))] {
        view.command("view.background", &json!({"color":color,"grid":grid}))?;
        let mut original = None;
        for (style, vibrant) in [("vibrant", true), ("muted", false)] {
            view.command("view.grid", &json!({"visible":true,"vibrant_axes":vibrant}))?;
            let frame = view.snapshot().ok_or("missing axis appearance scene")?;
            let rgba = solidcraft_ipados_bridge::renderer::capture_appearance(&frame, &capturer, 960, 640)?;
            if vibrant {
                original = Some(rgba.clone());
            } else if original.as_ref() == Some(&rgba) {
                return Err("muting axes did not change the rendered image".into());
            }
            let path = format!("{prefix}-{appearance}-{style}.png");
            std::fs::write(&path, solidcraft_render::png(960, 640, rgba).ok_or("axis appearance PNG encoding failed")?)?;
            println!("{path}");
        }
    }
    Ok(())
}
