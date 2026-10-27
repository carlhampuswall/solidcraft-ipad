//! Host-GPU evidence for the same engine snapshot and shared viewport renderer.
//! This does not launch an iPad application or verify its UIKit drawable.

use solidcraft_ipados_session::SessionAdapter;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let destination = std::env::args().nth(1).ok_or("usage: render_proof OUTPUT.png")?;
    let mut session = SessionAdapter::new();
    let snapshot = session.proof()?;
    let capturer = solidcraft_gpu::capture::Capturer::headless()?;
    let png = capturer.png(&snapshot.scene, &snapshot.camera, 640, 480)?;
    std::fs::write(&destination, png)?;
    let stats = capturer.last_stats();
    if stats.triangles == 0 {
        return Err("the shared viewport renderer drew no engine geometry".into());
    }
    println!(
        "engine: {} bodies, {} triangles, {} mm³; renderer: {} ({}), {} drawn triangles; image: {}",
        snapshot.body_count,
        snapshot.triangle_count,
        snapshot.volume_mm3,
        capturer.renderer(),
        capturer.adapter_name(),
        stats.triangles,
        destination
    );
    Ok(())
}
