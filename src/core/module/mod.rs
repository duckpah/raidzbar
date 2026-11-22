pub mod battery;
pub mod bluetooth;
pub mod brightness;
pub mod clock;
pub mod cpu;
pub mod custom;
pub mod memory;
pub mod network;
pub mod playback;
pub mod temperature;
pub mod tray;
pub mod volume;
pub mod window;
pub mod workspaces;

use crate::core::config::{ModuleConfig, resolve_length, resolve_optional_color};
use crate::core::event::ClickEvent;
use crate::core::popup::PopupMenu;
use crate::renderer::color::Color;
use crate::renderer::primitives::{Renderer, TextStyle};

pub type ModuleId = String;

#[derive(Debug, Clone)]
pub struct IconData {
    /// ARGB32 pixel data, row-major, length = width * height * 4.
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone)]
pub struct TextSegment {
    pub text: String,
    pub style: TextStyle,
}

#[derive(Debug, Clone)]
pub struct ModuleView {
    pub text: String,
    pub text_segments: Vec<TextSegment>,
    pub style: TextStyle,
    pub background: Option<Color>,
    pub padding: (f64, f64),
    pub icons: Vec<IconData>,
    pub icon_size: Option<u32>,
    pub icon_spacing: f64,
}

impl Default for ModuleView {
    fn default() -> Self {
        Self {
            text: String::new(),
            text_segments: Vec::new(),
            style: TextStyle::default(),
            background: None,
            padding: (8.0, 8.0),
            icons: Vec::new(),
            icon_size: None,
            icon_spacing: 4.0,
        }
    }
}

impl ModuleView {
    pub fn text_width<R: Renderer>(&self, renderer: &R) -> f64 {
        if self.text_segments.is_empty() {
            renderer.measure_text(&self.text, &self.style)
        } else {
            self.text_segments
                .iter()
                .map(|segment| renderer.measure_text(&segment.text, &segment.style))
                .sum()
        }
    }

    pub fn text_height<R: Renderer>(&self, renderer: &R) -> f64 {
        if self.text_segments.is_empty() {
            renderer.measure_text_height(&self.text, &self.style)
        } else {
            self.text_segments
                .iter()
                .map(|segment| renderer.measure_text_height(&segment.text, &segment.style))
                .fold(
                    renderer.measure_text_height(&self.text, &self.style),
                    f64::max,
                )
        }
    }
}

pub fn char_len(text: &str) -> usize {
    text.chars().count()
}

pub fn pad_text_right(text: &str, width: usize) -> String {
    let padding = width.saturating_sub(char_len(text));
    if padding == 0 {
        text.to_string()
    } else {
        format!("{text}{}", " ".repeat(padding))
    }
}

pub fn prefix_text(prefix: &str, text: &str) -> String {
    if prefix.trim().is_empty() {
        text.to_string()
    } else {
        format!("{prefix} {text}")
    }
}

pub(crate) fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

pub(crate) fn resolve_format(format: Option<String>, default: &str) -> String {
    match format {
        Some(format) if !format.trim().is_empty() => format,
        _ => default.to_string(),
    }
}

pub(crate) fn format_bytes(bytes: u64) -> String {
    format_scaled_bytes(bytes, "")
}

pub(crate) fn format_bytes_per_second(bytes: u64) -> String {
    format_scaled_bytes(bytes, "/s")
}

fn format_scaled_bytes(bytes: u64, suffix: &str) -> String {
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];

    let mut value = bytes as f64;
    let mut unit_index = 0usize;

    while value >= 1024.0 && unit_index + 1 < UNITS.len() {
        value /= 1024.0;
        unit_index += 1;
    }

    let number = if unit_index == 0 {
        bytes.to_string()
    } else if value >= 100.0 {
        format!("{value:.0}")
    } else if value >= 10.0 {
        format!("{value:.1}")
    } else {
        format!("{value:.2}")
    };
    format!("{number} {}{suffix}", UNITS[unit_index])
}

#[derive(Debug, Clone)]
pub struct ModuleChrome {
    pub foreground: Option<Color>,
    pub background: Option<Color>,
    pub padding: (f64, f64),
    pub icon_spacing: Option<f64>,
    /// User-set update interval override (`refresh_interval_ms` in TOML). When
    /// `Some`, takes priority over the module's built-in default. Modules
    /// reference this through their own `update_interval()` impl.
    pub update_interval: Option<std::time::Duration>,
}

impl Default for ModuleChrome {
    fn default() -> Self {
        Self {
            foreground: None,
            background: None,
            padding: (8.0, 8.0),
            icon_spacing: None,
            update_interval: None,
        }
    }
}

impl ModuleChrome {
    pub fn from_config(config: &ModuleConfig, default_padding: (f64, f64)) -> Self {
        Self {
            foreground: resolve_optional_color(config.foreground.as_deref(), "module.foreground"),
            background: resolve_optional_color(config.background.as_deref(), "module.background"),
            padding: (
                resolve_length(
                    config.padding_left,
                    default_padding.0,
                    "module.padding_left",
                ),
                resolve_length(
                    config.padding_right,
                    default_padding.1,
                    "module.padding_right",
                ),
            ),
            icon_spacing: config.icon_spacing,
            update_interval: config
                .refresh_interval_ms
                .filter(|ms| *ms > 0)
                .map(|ms| std::time::Duration::from_millis(ms.max(10))),
        }
    }

    pub fn apply(&self, mut view: ModuleView) -> ModuleView {
        if let Some(color) = self.foreground {
            view.style.color = color;
        }
        if let Some(color) = self.background {
            view.background = Some(color);
        }
        view.padding = self.padding;
        if let Some(spacing) = self.icon_spacing {
            view.icon_spacing = spacing;
        }
        view
    }
}

#[derive(Debug, Clone)]
pub struct SliderGlyphs {
    pub left: String,
    pub filled: String,
    pub empty: String,
    pub right: String,
}

impl SliderGlyphs {
    #[cfg(test)]
    pub fn new(
        left: impl Into<String>,
        filled: impl Into<String>,
        empty: impl Into<String>,
        right: impl Into<String>,
    ) -> Self {
        Self {
            left: left.into(),
            filled: filled.into(),
            empty: empty.into(),
            right: right.into(),
        }
    }

    pub fn from_config(config: &ModuleConfig) -> Self {
        Self {
            left: config.glyph_left.clone().unwrap_or_else(|| "▐".to_string()),
            filled: config
                .glyph_filled
                .clone()
                .unwrap_or_else(|| "█".to_string()),
            empty: config
                .glyph_empty
                .clone()
                .unwrap_or_else(|| "░".to_string()),
            right: config
                .glyph_right
                .clone()
                .unwrap_or_else(|| "▌".to_string()),
        }
    }

    pub fn unit_chars(&self) -> usize {
        char_len(&self.filled).max(char_len(&self.empty)).max(1)
    }

    pub fn total_chars(&self, slots: usize) -> usize {
        char_len(&self.left) + char_len(&self.right) + slots * self.unit_chars()
    }
}

pub trait Module: Send {
    fn update(&mut self);

    fn view(&self) -> ModuleView;

    fn update_interval(&self) -> std::time::Duration {
        std::time::Duration::from_secs(1)
    }

    fn click(&mut self, _event: ClickEvent) {}

    fn popup(&self) -> Option<PopupMenu> {
        None
    }

    fn popup_click(&mut self, _item_index: usize, _button: crate::core::event::MouseButton) {}

    fn dismiss_popup(&mut self) {}
}
