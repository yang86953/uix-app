//! RenderContext — bridges Widget tree with GraphicsEngine for high-level rendering.

use crate::graphics::{Color, Point, Size};
use crate::graphics::{FontHandle, GradientDirection, GraphicsEngine, Radius};

/// RenderContext wraps a GraphicsEngine reference and provides widget-level drawing.
pub struct RenderContext<'a> {
    engine: &'a mut dyn GraphicsEngine,
    font: FontHandle,
    #[allow(dead_code)]
    pub(crate) global_opacity: f32,
}

impl<'a> RenderContext<'a> {
    /// Create a new render context.
    /// `font` should be obtained from `GraphicsEngine::load_font()` or similar.
    /// Until a real font system is integrated, pass `FontHandle` (the unit struct).
    pub fn new(engine: &'a mut dyn GraphicsEngine, font: FontHandle) -> Self {
        Self {
            engine,
            font,
            global_opacity: 1.0,
        }
    }

    pub fn engine(&mut self) -> &mut dyn GraphicsEngine {
        self.engine
    }

    pub fn fill_rect(&mut self, rect: crate::graphics::Rect, color: Color, radius: Option<Radius>) {
        self.engine.fill_rect(rect, color, radius);
    }

    pub fn stroke_rect(
        &mut self,
        rect: crate::graphics::Rect,
        color: Color,
        lw: f32,
        radius: Option<Radius>,
    ) {
        self.engine.stroke_rect(rect, color, lw, radius);
    }

    pub fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        self.engine.fill_circle(cx, cy, r, color);
    }

    pub fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color, lw: f32) {
        self.engine.stroke_circle(cx, cy, r, color, lw);
    }

    pub fn draw_text(&mut self, text: &str, pos: Point, color: Color, font_size: f32) {
        let opts = crate::graphics::TextLayoutOptions {
            max_width: 2000.0,
            line_height: font_size + 2.0,
            word_wrap: false,
            h_align: crate::graphics::HAlign::Left,
            v_align: crate::graphics::VAlign::Top,
            font_size,
        };
        self.engine.draw_text(&self.font, text, pos, color, &opts);
    }

    pub fn set_supersample_level(&mut self, level: u8) {
        self.engine.set_supersample_level(level);
    }

    pub fn supersample_level(&self) -> u8 {
        self.engine.supersample_level()
    }

    pub fn fill_linear_gradient(
        &mut self,
        rect: crate::graphics::Rect,
        ca: Color,
        cb: Color,
        dir: GradientDirection,
    ) {
        self.engine.fill_linear_gradient(rect, ca, cb, dir);
    }

    pub fn fill_radial_gradient(
        &mut self,
        cx: f32,
        cy: f32,
        ir: f32,
        or: f32,
        ic: Color,
        oc: Color,
    ) {
        self.engine.fill_radial_gradient(cx, cy, ir, or, ic, oc);
    }

    pub fn measure_text(&self, text: &str, font_size: f32) -> Size {
        let opts = crate::graphics::TextLayoutOptions {
            max_width: 2000.0,
            line_height: font_size + 2.0,
            word_wrap: false,
            h_align: crate::graphics::HAlign::Left,
            v_align: crate::graphics::VAlign::Top,
            font_size,
        };
        self.engine.measure_text(&self.font, text, &opts)
    }
}
