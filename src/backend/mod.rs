pub mod wayland;

use crate::core::bar::Bar;

/// Create and run all bars on niri's Wayland compositor.
pub fn run(bars: Vec<Bar>) {
    wayland::run(bars);
}
