use super::color::Color;

#[derive(Debug, Clone, Copy)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone)]
pub struct TextStyle {
    pub font_family: String,
    pub font_size: f64,
    pub color: Color,
    /// Render at Pango Weight::Bold. Defaults to false; modules inherit the
    /// bar's `bold` setting via `Bar::apply_text_style`.
    pub bold: bool,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_family: "monospace".to_string(),
            font_size: 14.0,
            color: Color::WHITE,
            bold: false,
        }
    }
}

pub trait Renderer {
    fn begin(&mut self, width: u32, height: u32);

    fn draw_rect(&mut self, rect: Rect, color: Color);

    fn draw_rounded_rect(&mut self, rect: Rect, radius: f64, color: Color) {
        let _ = radius;
        self.draw_rect(rect, color);
    }

    fn draw_text(&mut self, pos: Point, text: &str, style: &TextStyle) -> f64;

    fn draw_text_ellipsized(&mut self, rect: Rect, text: &str, style: &TextStyle) -> f64 {
        if rect.width <= 0.0 || rect.height <= 0.0 {
            return 0.0;
        }
        self.draw_text(
            Point {
                x: rect.x,
                y: rect.y,
            },
            text,
            style,
        )
    }

    fn push_clip(&mut self, rect: Rect) {
        let _ = rect;
    }

    fn pop_clip(&mut self) {}

    fn measure_text(&self, text: &str, style: &TextStyle) -> f64;

    fn measure_text_height(&self, text: &str, style: &TextStyle) -> f64;

    fn end(&mut self);

    /// Get the underlying pixel data (ARGB32, stride = width * 4).
    fn data(&self) -> &[u8];

    fn draw_icon(&mut self, pos: Point, pixels: &[u8], src_width: u32, src_height: u32, size: u32) {
        let _ = (pos, pixels, src_width, src_height, size);
    }
}
