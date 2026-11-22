pub mod wayland;

use crate::core::bar::Bar;

pub fn run(bars: Vec<Bar>) {
    wayland::run(bars);
}
