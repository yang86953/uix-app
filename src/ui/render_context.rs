//! RenderContext — bridges Widget tree with GraphicsEngine and token system.
//!
//! Widgets access design tokens via `RenderContext::tokens()` instead of
//! hardcoding a concrete theme preset. This is the single point where the
//! widget tree receives injectable design tokens during rendering.

use crate::graphics::{Color, FontHandle, GraphicsEngine, Point, Radius, Rect, Size};
use crate::graphics::{GradientDirection, TextLayoutOptions};
use crate::ui::theme::TokenProvider;

/// RenderContext wraps a GraphicsEngine reference and a TokenProvider,
/// providing widget-level drawing and token access.
///
/// Widgets **must not** call `DesignTokens::antd_light()` directly —
/// use `ctx.tokens()` to get the active token provider.
pub struct RenderContext<'a> {
    engine: &'a mut dyn GraphicsEngine,
    font: FontHandle,
    #[allow(dead_code)]
    pub(crate) global_opacity: f32,
    max_text_width: f32,
    tokens: &'a dyn TokenProvider,
}

impl<'a> RenderContext<'a> {
    /// Create a new render context with the given engine, font, and token provider.
    ///
    /// `tokens` should carry the active theme's token provider (e.g. from Theme).
    /// When not needed for testing, `DesignTokens::antd_light()` can be passed.
    pub fn new(
        engine: &'a mut dyn GraphicsEngine,
        font: FontHandle,
        tokens: &'a dyn TokenProvider,
    ) -> Self {
        Self {
            engine,
            font,
            global_opacity: 1.0,
            max_text_width: f32::MAX,
            tokens,
        }
    }

    /// Access the active design token provider.
    ///
    /// This is the **only** approved way for widgets to read design tokens.
    /// Do not hardcode `DesignTokens::antd_light()` in widget code.
    pub fn tokens(&self) -> &dyn TokenProvider {
        self.tokens
    }

    pub fn engine(&mut self) -> &mut dyn GraphicsEngine {
        self.engine
    }

    pub fn draw_box_shadow(
        &mut self,
        rect: Rect,
        blur_radius: f32,
        offset_x: f32,
        offset_y: f32,
        color: Color,
        corner_radius: Option<Radius>,
    ) {
        self.engine
            .draw_box_shadow(rect, blur_radius, offset_x, offset_y, color, corner_radius);
    }

    pub fn fill_rect(&mut self, rect: Rect, color: Color, radius: Option<Radius>) {
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

    /// Push a clip rect onto the engine's clip stack.
    /// Drawing outside this rect will be masked out.
    pub fn push_clip_rect(&mut self, rect: Rect) {
        self.engine.push_clip_rect(rect);
    }

    /// Pop the most recent clip rect from the engine's clip stack.
    pub fn pop_clip_rect(&mut self) {
        self.engine.pop_clip_rect();
    }

    /// Save the current engine state (clip, transform, opacity) and push
    /// a clip rect. Use `pop_clip()` after rendering children to restore.
    pub fn push_clip(&mut self, rect: Rect) {
        self.engine.save();
        self.engine.push_clip_rect(rect);
    }

    /// Pop the clip rect pushed by `push_clip`, restoring pre-clip state.
    pub fn pop_clip(&mut self) {
        self.engine.restore();
    }

    /// Save the current engine state (clip, transform, opacity).
    pub fn save(&mut self) {
        self.engine.save();
    }

    /// Restore the most recently saved engine state.
    pub fn restore(&mut self) {
        self.engine.restore();
    }

    /// Set a maximum text width for text layout.
    /// Use `f32::MAX` (default) for unlimited width.
    pub fn set_max_text_width(&mut self, width: f32) {
        self.max_text_width = width;
    }

    pub fn draw_text(&mut self, text: &str, pos: Point, color: Color, font_size: f32) {
        let opts = TextLayoutOptions {
            max_width: self.max_text_width,
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
        let opts = TextLayoutOptions {
            max_width: self.max_text_width,
            line_height: font_size + 2.0,
            word_wrap: false,
            h_align: crate::graphics::HAlign::Left,
            v_align: crate::graphics::VAlign::Top,
            font_size,
        };
        self.engine.measure_text(&self.font, text, &opts)
    }
}
