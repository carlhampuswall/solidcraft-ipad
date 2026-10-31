//! Native construction strokes: keep picking paths whole and dash only display geometry.
use solidcraft_engine::view::Palette;
use solidcraft_render::{Scene, SceneLine};

pub(crate) fn construction_stroke(line: &SceneLine) -> bool {
    // The shared scene assigns this palette entry and width to construction
    // curves and virtual-corner extensions. Hover feedback has a wider stroke.
    line.color == Palette::DEFAULT.sketch_construction && line.width == 1.0
}

/// Carry dash phase across tessellation vertices, so circles and splines have
/// the same pattern as straight lines. Bound expansion for hostile coordinates.
pub(crate) fn dash_line(scene: &mut Scene, line: &SceneLine, unit: f64) {
    let total: f64 = line.points.windows(2).map(|p| (p[1] - p[0]).len()).sum();
    if !total.is_finite() || total <= 0.0 || !unit.is_finite() || unit <= 0.0 {
        return;
    }
    let unit = unit.max(total / 32_768.0);
    let mut visible = true;
    let mut remaining = 7.0 * unit;
    for pair in line.points.windows(2) {
        let delta = pair[1] - pair[0];
        let length = delta.len();
        if length <= 0.0 {
            continue;
        }
        let mut at = 0.0;
        for _ in 0..8192 {
            if at >= length {
                break;
            }
            let step = remaining.min(length - at);
            let end = at + step;
            if end <= at {
                break;
            }
            if visible {
                scene.lines.push(SceneLine {
                    points: vec![pair[0] + delta * (at / length), pair[0] + delta * (end / length)],
                    color: line.color,
                    alpha: line.alpha,
                    width: line.width,
                    on_top: line.on_top,
                });
            }
            at = end;
            remaining -= step;
            if remaining <= unit * 1e-9 {
                visible = !visible;
                remaining = if visible { 7.0 * unit } else { 4.0 * unit };
            }
        }
    }
}
