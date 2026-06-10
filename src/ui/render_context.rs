//! RenderContext — bridges Widget tree with GraphicsEngine and token system.
//!
//! Widgets access design tokens via `RenderContext::tokens()` instead of
//! hardcoding a concrete theme preset. This is the single point where the
//! widget tree receives injectable design tokens during rendering.

use crate::graphics::{Color, FontHandle, GraphicsEngine, HAlign, Point, Radius, Rect, Size, VAlign};
use crate::graphics::{GradientDirection, TextLayoutOptions};
use crate::ui::style::Style;
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

    /// 应用 Style 到矩形区域（背景 + 边框）。
    pub fn apply_style(&mut self, rect: Rect, style: &Style) {
        let r = if style.border_radius > 0.0 {
            Some(Radius::uniform(style.border_radius))
        } else {
            None
        };
        if let Some(bg) = style.background {
            self.fill_rect(rect, bg, r);
        }
        if style.border_width > 0.0 {
            if let Some(bc) = style.border_color {
                self.stroke_rect(rect, bc, style.border_width, r);
            }
        }
    }

    pub fn stroke_rect(
        &mut self,
        rect: Rect,
        color: Color,
        line_width: f32,
        radius: Option<Radius>,
    ) {
        self.engine.stroke_rect(rect, color, line_width, radius);
    }

    pub fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        self.engine.fill_circle(cx, cy, r, color);
    }

    pub fn save(&mut self) {
        self.engine.save();
    }

    pub fn restore(&mut self) {
        self.engine.restore();
    }

    pub fn set_max_text_width(&mut self, width: f32) {
        self.max_text_width = width;
    }

    pub fn draw_text(&mut self, text: &str, pos: Point, color: Color, font_size: f32) {
        let opts = TextLayoutOptions {
            max_width: self.max_text_width,
            max_height: 0.0,
            line_height: font_size + 2.0,
            word_wrap: false,
            h_align: crate::graphics::HAlign::Left,
            v_align: crate::graphics::VAlign::Top,
            font_size,
        };
        self.engine.draw_text(&self.font, text, pos, color, &opts);
    }

    /// 在 rect 内竖直居中绘制文本（水平左对齐）。
    ///
    /// widget 只需说"把这个文本放在这个框里"——定位由布局系统处理。
    /// 在 rect 内居中绘制文本（水平 + 竖直居中）。
    ///
    /// 水平用 `measure_text` 取实际文本宽度计算居中 x。
    /// 竖直由 fontdue 的 `VAlign::Middle` + `max_height` 处理。
    pub fn text_center(&mut self, text: &str, rect: Rect, color: Color, font_size: f32) {
        if text.is_empty() {
            return;
        }
        // 水平居中
        let text_w = self.measure_text(text, font_size).w.min(rect.w);
        let x = rect.x + (rect.w - text_w) * 0.5;
        // 竖直居中：fontdue 在 max_height 框内用 Middle 对齐
        let opts = TextLayoutOptions {
            max_width: self.max_text_width,
            max_height: rect.h,
            line_height: font_size + 2.0,
            word_wrap: false,
            h_align: HAlign::Left,
            v_align: VAlign::Middle,
            font_size,
        };
        self.engine.draw_text(&self.font, text, Point::new(x, rect.y), color, &opts);
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
            max_height: 0.0,
            line_height: font_size + 2.0,
            word_wrap: false,
            h_align: crate::graphics::HAlign::Left,
            v_align: crate::graphics::VAlign::Top,
            font_size,
        };
        self.engine.measure_text(&self.font, text, &opts)
    }
}
