//! RenderContext — bridges Widget tree with GraphicsEngine, FontService and token system.
//!
//! Widgets access design tokens via `RenderContext::tokens()` instead of
//! hardcoding a concrete theme preset. This is the single point where the
//! widget tree receives injectable design tokens during rendering.
//!
//! 文本操作委托给 `FontService`（独立于渲染器），引擎只负责 `draw_glyph_raster`。

use crate::base::{Point, Rect, Size};
use crate::graphics::font_service::FontService;
use crate::graphics::path::{FillRule, Path};
use crate::graphics::stroker::StrokeOptions;
use crate::graphics::{Color, FontHandle, GraphicsEngine, HAlign, Radius, VAlign};
use crate::graphics::{GradientDirection, TextLayoutOptions};
use crate::ui::style::Style;
use crate::ui::theme::TokenProvider;

/// RenderContext wraps a GraphicsEngine reference, a FontService reference,
/// and a TokenProvider, providing widget-level drawing and token access.
///
/// Widgets **must not** call `DesignTokens::antd_light()` directly —
/// use `ctx.tokens()` to get the active token provider.
///
/// FontService 是与渲染器无关的独立字体系统。渲染器无需实现字体逻辑。
pub struct RenderContext<'a> {
    engine: &'a mut dyn GraphicsEngine,
    font: FontHandle,
    font_service: &'a FontService,
    max_text_width: f32,
    tokens: &'a dyn TokenProvider,
}

impl<'a> RenderContext<'a> {
    /// Create a new render context.
    ///
    /// `tokens` should carry the active theme's token provider (e.g. from Theme).
    /// When not needed for testing, `DesignTokens::antd_light()` can be passed.
    /// `font_service` provides text layout and glyph rasterization.
    pub fn new(
        engine: &'a mut dyn GraphicsEngine,
        font: FontHandle,
        font_service: &'a FontService,
        tokens: &'a dyn TokenProvider,
    ) -> Self {
        Self {
            engine,
            font,
            font_service,
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

    /// 返回 FontService 引用，供需要直接操作字体的场景使用。
    pub fn font_service(&self) -> &FontService {
        self.font_service
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

    /// Ambient box shadow — wider, softer falloff for ambient layers.
    pub fn draw_box_shadow_ambient(
        &mut self,
        rect: Rect,
        blur_radius: f32,
        offset_x: f32,
        offset_y: f32,
        color: Color,
        corner_radius: Option<Radius>,
    ) {
        self.engine.draw_box_shadow_ambient(
            rect,
            blur_radius,
            offset_x,
            offset_y,
            color,
            corner_radius,
        );
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

    /// 临时替换字体句柄（用于 Icon widget 切换到图标字体渲染）。
    pub fn set_font(&mut self, font: FontHandle) {
        self.font = font;
    }

    /// 返回当前字体句柄（供需要手动 draw_text 的场景使用）。
    pub fn font(&self) -> &FontHandle {
        &self.font
    }

    pub fn set_max_text_width(&mut self, width: f32) {
        self.max_text_width = width;
    }

    // ── 文本绘制（委托给 FontService 布局/光栅化，引擎只绘制像素）──

    /// 绘制文本（左对齐，顶部对齐）。
    pub fn draw_text(&mut self, text: &str, pos: Point, color: Color, font_size: f32) {
        if text.is_empty() {
            return;
        }
        let opts = TextLayoutOptions {
            max_width: self.max_text_width,
            max_height: 0.0,
            line_height: font_size + 2.0,
            word_wrap: false,
            h_align: crate::graphics::HAlign::Left,
            v_align: crate::graphics::VAlign::Top,
            font_size,
        };
        let backend_opts = crate::graphics::text_backend::TextLayoutOptions::from(opts);
        let layout = self
            .font_service
            .layout_text(&self.font, text, &backend_opts);
        self.blit_glyph_layout(&layout, pos, color, font_size);
    }

    /// 在 rect 内居中绘制文本（水平+竖直居中）。
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
        let backend_opts = crate::graphics::text_backend::TextLayoutOptions::from(opts);
        let layout = self
            .font_service
            .layout_text(&self.font, text, &backend_opts);
        self.blit_glyph_layout(&layout, Point::new(x, rect.y), color, font_size);
    }

    /// Draw text with word wrap enabled within the given rect.
    pub fn draw_text_wrapped(&mut self, text: &str, rect: Rect, color: Color, font_size: f32) {
        if text.is_empty() {
            return;
        }
        let opts = TextLayoutOptions {
            max_width: rect.w.max(1.0),
            max_height: rect.h.max(0.0),
            line_height: font_size + 2.0,
            word_wrap: true,
            h_align: crate::graphics::HAlign::Left,
            v_align: crate::graphics::VAlign::Top,
            font_size,
        };
        let backend_opts = crate::graphics::text_backend::TextLayoutOptions::from(opts);
        let layout = self
            .font_service
            .layout_text(&self.font, text, &backend_opts);
        self.blit_glyph_layout(&layout, Point::new(rect.x, rect.y), color, font_size);
    }

    /// 将 glyph layout 绘制到引擎上。
    fn blit_glyph_layout(
        &mut self,
        layout: &crate::graphics::text_backend::TextLayout,
        pos: Point,
        color: Color,
        font_size: f32,
    ) {
        let fs = font_size.max(1.0);
        let is_top = true; // layout 使用 Top 或 Middle 对齐
        let y_off = if is_top && !layout.glyphs.is_empty() {
            layout
                .glyphs
                .iter()
                .map(|g| g.y)
                .fold(f32::MAX, f32::min)
                .min(0.0) as i32
        } else {
            0
        };

        for gp in &layout.glyphs {
            let raster = self
                .font_service
                .rasterize_glyph(&self.font, gp.glyph_id, fs);
            if raster.width == 0 || raster.height == 0 {
                continue;
            }
            let gx = (pos.x + gp.x) as i32;
            let gy = (pos.y + gp.y) as i32 - y_off;
            self.engine.draw_glyph_raster(
                gx,
                gy,
                &raster.coverage,
                raster.width,
                raster.height,
                color,
            );
        }
    }

    // ── 文本测量（委托给 FontService）──

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
        let backend_opts = crate::graphics::text_backend::TextLayoutOptions::from(opts);
        self.font_service
            .measure_text(&self.font, text, &backend_opts)
    }

    /// Measure text with word wrap enabled at the given max width.
    pub fn measure_text_wrapped(&self, text: &str, font_size: f32, max_width: f32) -> Size {
        let opts = TextLayoutOptions {
            max_width,
            max_height: 0.0,
            line_height: font_size + 2.0,
            word_wrap: true,
            h_align: crate::graphics::HAlign::Left,
            v_align: crate::graphics::VAlign::Top,
            font_size,
        };
        let backend_opts = crate::graphics::text_backend::TextLayoutOptions::from(opts);
        self.font_service
            .measure_text(&self.font, text, &backend_opts)
    }

    /// 命中测试：返回点击位置对应的字符索引。
    pub fn hit_test(&self, text: &str, font_size: f32, point: Point) -> Option<usize> {
        let opts = TextLayoutOptions {
            max_width: self.max_text_width,
            max_height: 0.0,
            line_height: font_size + 2.0,
            word_wrap: false,
            h_align: crate::graphics::HAlign::Left,
            v_align: crate::graphics::VAlign::Top,
            font_size,
        };
        let backend_opts = crate::graphics::text_backend::TextLayoutOptions::from(opts);
        self.font_service
            .hit_test_text(&self.font, text, &backend_opts, point)
    }

    /// 获取指定字符的光标 x 位置（相对文本起始点）。
    pub fn cursor_x(&self, text: &str, font_size: f32, char_index: usize) -> f32 {
        let opts = TextLayoutOptions {
            max_width: self.max_text_width,
            max_height: 0.0,
            line_height: font_size + 2.0,
            word_wrap: false,
            h_align: crate::graphics::HAlign::Left,
            v_align: crate::graphics::VAlign::Top,
            font_size,
        };
        let backend_opts = crate::graphics::text_backend::TextLayoutOptions::from(opts);
        self.font_service
            .text_cursor_x(&self.font, text, &backend_opts, char_index)
    }

    // ── 其他绘制操作 ──

    pub fn set_supersample_level(&mut self, level: u8) {
        self.engine.set_supersample_level(level);
    }

    pub fn supersample_level(&self) -> u8 {
        self.engine.supersample_level()
    }

    pub fn fill_linear_gradient(
        &mut self,
        rect: crate::base::Rect,
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

    /// 用任意路径填充。
    pub fn fill_path(&mut self, path: &Path, color: Color, fill_rule: FillRule) {
        self.engine.fill_path(path, color, fill_rule);
    }

    /// 用任意路径描边。
    pub fn stroke_path(&mut self, path: &Path, color: Color, options: &StrokeOptions) {
        self.engine.stroke_path(path, color, options);
    }
}
