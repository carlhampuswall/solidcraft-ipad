//! Native sketch tokens; the shared scene builder still decides constraint state.
use solidcraft_engine::view::Palette;
use solidcraft_render::Rgb;

// Emerald rather than the axes' yellow-green (60, 170, 60). Keep determined
// curves distinct from finished sketches, which share a colour upstream.
pub(crate) const SKETCH_PALETTE: Palette = Palette { sketch_fixed: Rgb(0, 125, 85), ..Palette::DEFAULT };
const CONSTRAINED_DARK: Rgb = Rgb(70, 225, 155);

pub(crate) fn sketch_line_color(color: Rgb, free: Option<Rgb>, constrained_green: bool) -> Rgb {
    if color == SKETCH_PALETTE.sketch_fixed {
        if constrained_green { if free.is_some() { CONSTRAINED_DARK } else { color } } else { free.unwrap_or(Palette::DEFAULT.sketch_fixed) }
    } else if [SKETCH_PALETTE.sketch, SKETCH_PALETTE.sketch_done].contains(&color) {
        free.unwrap_or(color)
    } else {
        color
    }
}
