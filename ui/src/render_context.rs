//! RenderContext — bridges Widget tree with GraphicsEngine, FontService and token system.
//!
//! Widgets access design tokens via `RenderContext::tokens()` instead of
//! hardcoding a concrete theme preset. This is the single point where the
//! widget tree receives injectable design tokens during rendering.
//!
//! 文本操作委托给 `FontService`（独立于渲染器），引擎只负责 `draw_glyph_raster`。

use uix_core::{Point, Rect, Size};
use uix_graphics::font_service::FontService;
use uix_graphics::path::{FillRule, Path};
use uix_graphics::stroker::StrokeOptions;
use uix_graphics::{Color, FontHandle, GraphicsEngine, HAlign, Radius, VAlign};
use uix_graphics::{GradientDirection, TextLayoutOptions};
use crate::style::Style;
use crate::theme::TokenProvider;

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
    max_text_width: f32,
    tokens: &'a dyn TokenProvider,
    /// 调试模式开关：开启时在 overlay 层绘制调试边框和信息。
    debug_mode: bool,
}

impl<'a> RenderContext<'a> {
    /// Create a new render context.
    ///
    /// `tokens` should carry the active theme's token provider (e.g. from Theme).
    /// FontService 从 engine 获取，不再作为独立参数传入。
    pub fn new(
        engine: &'a mut dyn GraphicsEngine,
        font: FontHandle,
        tokens: &'a dyn TokenProvider,
    ) -> Self {
        Self {
            engine,
            font,
            max_text_width: f32::MAX,
            tokens,
            debug_mode: false,
        }
    }

    /// 设置调试模式。开启时在 overlay 层绘制调试边框和信息。
    pub fn set_debug_mode(&mut self, mode: bool) {
        self.debug_mode = mode;
    }

    /// 是否处于调试模式。
    pub fn debug_mode(&self) -> bool {
        self.debug_mode
    }

    /// Access the active design token provider.
    pub fn tokens(&self) -> &dyn TokenProvider {
        self.tokens
    }

    pub fn engine(&mut self) -> &mut dyn GraphicsEngine {
        self.engine
    }

    /// 返回 FontService 引用（通过 engine 获取），供需要直接操作字体的场景使用。
    pub fn font_service(&mut self) -> &FontService {
        self.engine.font_service()
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

    /// 计算文字视觉中心与 rect 中心对齐时的 y 位置。
    ///
    /// 文字在布局中 y=0 处开始，glyph 通过 `pos.y + gp.y + bearing_y` 渲染，
    /// 视觉上从 y 延伸到 y + (ascent + descent)，视觉中心在 y + (ascent + descent) * 0.5 处。
    /// 令该值与 rect 中心对齐，得：
    ///   y = rect.y + (rect.h - ascent - descent) * 0.5
    pub fn visual_center_y(&mut self, rect: Rect, font_size: f32) -> f32 {
        let fs = font_size.max(1.0);
        let fh = self.font;
        match self.font_service().horizontal_line_metrics(&fh, fs) {
            Some(m) => rect.y + (rect.h - m.ascent - m.descent) * 0.5,
            None => rect.y + rect.h * 0.5,
        }
    }

    // ── 文本绘制（委托给 FontService 布局/光栅化，引擎只绘制像素）──

    /// 绘制文本（左对齐，顶部对齐）。标准行高 = font_size × 1.5。
    pub fn draw_text(&mut self, text: &str, pos: Point, color: Color, font_size: f32) {
        if text.is_empty() {
            return;
        }
        let opts = TextLayoutOptions {
            max_width: self.max_text_width,
            max_height: 0.0,
            line_height: font_size * 1.5,
            word_wrap: false,
            h_align: uix_graphics::HAlign::Left,
            v_align: uix_graphics::VAlign::Top,
            font_size,
        };
        let backend_opts = uix_graphics::text_backend::TextLayoutOptions::from(opts);
        let fh = self.font;
        let layout = self
            .font_service()
            .layout_text(&fh, text, &backend_opts);
        self.blit_glyph_layout(&layout, pos, color, font_size);
    }

    /// 基于基线绘制文本（四线三格法对齐）。
    ///
    /// 四线三格对应的字体度量：
    /// - 顶线（Top）     = baseline - ascent
    /// - 上基线（Mean）  = baseline - x_height（小写字母 x 顶部）
    /// - 基线（Baseline）= baseline_y（参数）
    /// - 下基线（Bottom）= baseline + descent
    ///
    /// `pos.x` 为文字左边缘 x 坐标，`baseline_y` 为基线 y 坐标。
    pub fn draw_text_baseline(
        &mut self,
        text: &str,
        x: f32,
        baseline_y: f32,
        color: Color,
        font_size: f32,
    ) {
        if text.is_empty() {
            return;
        }
        let fs = font_size.max(1.0);
        let fh = self.font; // FontHandle 是 Copy 的
        let metrics = self.font_service().horizontal_line_metrics(&fh, fs);
        let ascent = metrics.map(|m| m.ascent).unwrap_or(fs * 0.8);
        // 文字顶部 = 基线 - ascent
        let top_y = baseline_y - ascent;
        self.draw_text(text, Point::new(x, top_y), color, fs);
    }

    /// 在 rect 内居中绘制文本。水平用文本框宽度居中；垂直用字体度量
    /// (ascent + descent) 计算视觉中心，使文字视觉中位线与 rect 中心对齐。
    pub fn text_center(&mut self, text: &str, rect: Rect, color: Color, font_size: f32) {
        if text.is_empty() {
            return;
        }
        let cnt = rect.x + rect.w * 0.5;
        let opts = TextLayoutOptions {
            max_width: self.max_text_width,
            max_height: 0.0,
            line_height: font_size * 1.5,
            word_wrap: false,
            h_align: HAlign::Left,
            v_align: VAlign::Top,
            font_size,
        };
        let backend_opts = uix_graphics::text_backend::TextLayoutOptions::from(opts);
        let fh = self.font;
        let layout = self
            .font_service()
            .layout_text(&fh, text, &backend_opts);
        let x = cnt - layout.width * 0.5;
        let y = self.visual_center_y(rect, font_size);
        self.blit_glyph_layout(&layout, Point::new(x, y), color, font_size);
    }

    /// 左对齐、垂直居中的文本绘制。
    pub fn draw_text_in_frame(&mut self, text: &str, rect: Rect, color: Color, font_size: f32) {
        if text.is_empty() {
            return;
        }
        let opts = TextLayoutOptions {
            max_width: rect.w.max(1.0),
            max_height: 0.0,
            line_height: font_size * 1.5,
            word_wrap: false,
            h_align: HAlign::Left,
            v_align: VAlign::Top,
            font_size,
        };
        let backend_opts = uix_graphics::text_backend::TextLayoutOptions::from(opts);
        let fh = self.font;
        let layout = self
            .font_service()
            .layout_text(&fh, text, &backend_opts);
        let x = rect.x;
        let y = self.visual_center_y(rect, font_size);
        self.blit_glyph_layout(&layout, Point::new(x, y), color, font_size);
    }

    /// Draw text with word wrap enabled within the given rect。标准行高 = font_size × 1.5。
    pub fn draw_text_wrapped(&mut self, text: &str, rect: Rect, color: Color, font_size: f32) {
        if text.is_empty() {
            return;
        }
        let opts = TextLayoutOptions {
            max_width: rect.w.max(1.0),
            max_height: rect.h.max(0.0),
            line_height: font_size * 1.5,
            word_wrap: true,
            h_align: uix_graphics::HAlign::Left,
            v_align: uix_graphics::VAlign::Top,
            font_size,
        };
        let backend_opts = uix_graphics::text_backend::TextLayoutOptions::from(opts);
        let fh = self.font;
        let layout = self
            .font_service()
            .layout_text(&fh, text, &backend_opts);
        self.blit_glyph_layout(&layout, Point::new(rect.x, rect.y), color, font_size);
    }

    /// 将 glyph layout 绘制到引擎上。
    pub(crate) fn blit_glyph_layout(
        &mut self,
        layout: &uix_graphics::text_backend::TextLayout,
        pos: Point,
        color: Color,
        font_size: f32,
    ) {
        let fs = font_size.max(1.0);

        // layout 中的 glyph 位置已由 text_backend 正确计算（包含 v_align 偏移），
        // 直接按 pos 偏移绘制即可，无需额外 y_off 修正。
        for gp in &layout.glyphs {
            // 优先使用 glyph 自带的 font handle（支持多字体回退），
            // 当 font 为默认值 (u32::MAX) 时回退到上下文当前字体
            let fh = if gp.font.0 != u32::MAX { gp.font } else { self.font };
            let raster = self
                .font_service()
                .rasterize_glyph(&fh, gp.glyph_id, fs);
            if raster.width == 0 || raster.height == 0 {
                continue;
            }
            let gx = (pos.x + gp.x + raster.bearing_x) as i32;
            let gy = (pos.y + gp.y + raster.bearing_y) as i32;
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

    // ── 文本选中渲染 ──

    /// 获取选中文本的矩形区域列表（用于绘制选中背景）。
    /// `pos` 为文字绘制起点（与 `draw_text` 的 `pos` 一致）。
    /// `start..end` 为待选中的字符索引范围。
    pub fn selection_rects(
        &mut self,
        text: &str,
        font_size: f32,
        pos: Point,
        start: usize,
        end: usize,
    ) -> Vec<Rect> {
        if text.is_empty() || start >= end {
            return Vec::new();
        }
        let opts = TextLayoutOptions {
            max_width: self.max_text_width,
            max_height: 0.0,
            line_height: font_size * 1.5,
            word_wrap: false,
            h_align: HAlign::Left,
            v_align: VAlign::Top,
            font_size,
        };
        let backend_opts = uix_graphics::text_backend::TextLayoutOptions::from(opts);
        let fh = self.font;
        let layout = self
            .font_service()
            .layout_text(&fh, text, &backend_opts);
        if layout.glyphs.is_empty() {
            return Vec::new();
        }
        let end = end.min(layout.glyphs.len());
        let start = start.min(end);

        // 文字实际视觉高度（ascent + descent），而非行间距 (line.height)
        let visual_h = self.font_service()
            .horizontal_line_metrics(&fh, font_size)
            .map(|m| m.ascent + m.descent)
            .unwrap_or(font_size * 1.2);

        let mut rects = Vec::new();
        for line in &layout.lines {
            let gs = line.glyph_start;
            let gc = line.glyph_count;
            let ge = gs + gc;
            // 只处理与选中范围有交集的行的字形
            let sel_start = start.max(gs);
            let sel_end = end.min(ge);
            if sel_start >= sel_end {
                continue;
            }
            let glyphs = &layout.glyphs[sel_start..sel_end];
            let x0 = pos.x + glyphs[0].x;
            let last = glyphs[glyphs.len() - 1];
            let x1 = pos.x + last.x + last.width.max(0.0);
            let y0 = pos.y + line.y;
            rects.push(Rect::new(x0, y0, (x1 - x0).max(0.0), visual_h));
        }
        rects
    }

    /// 绘制文本选中背景 + 文本。便捷方法，等同于先 fill rects 再 draw_text。
    pub fn draw_text_with_selection(
        &mut self,
        text: &str,
        pos: Point,
        color: Color,
        font_size: f32,
        selection: Option<(usize, usize)>,
        selection_bg: Color,
    ) {
        if let Some((s, e)) = selection {
            let rects = self.selection_rects(text, font_size, pos, s, e);
            for r in rects {
                self.fill_rect(r, selection_bg, None);
            }
        }
        self.draw_text(text, pos, color, font_size);
    }

    // ── 文本测量（委托给 FontService）──

    pub fn measure_text(&mut self, text: &str, font_size: f32) -> Size {
        let opts = TextLayoutOptions {
            max_width: self.max_text_width,
            max_height: 0.0,
            line_height: font_size * 1.5,
            word_wrap: false,
            h_align: uix_graphics::HAlign::Left,
            v_align: uix_graphics::VAlign::Top,
            font_size,
        };
        let backend_opts = uix_graphics::text_backend::TextLayoutOptions::from(opts);
        let fh = self.font;
        self.font_service()
            .measure_text(&fh, text, &backend_opts)
    }

    /// Measure text with word wrap enabled at the given max width.
    pub fn measure_text_wrapped(&mut self, text: &str, font_size: f32, max_width: f32) -> Size {
        let opts = TextLayoutOptions {
            max_width,
            max_height: 0.0,
            line_height: font_size * 1.5,
            word_wrap: true,
            h_align: uix_graphics::HAlign::Left,
            v_align: uix_graphics::VAlign::Top,
            font_size,
        };
        let backend_opts = uix_graphics::text_backend::TextLayoutOptions::from(opts);
        let fh = self.font;
        self.font_service()
            .measure_text(&fh, text, &backend_opts)
    }

    /// 命中测试：返回点击位置对应的字符索引。
    pub fn hit_test(&mut self, text: &str, font_size: f32, point: Point) -> Option<usize> {
        let opts = TextLayoutOptions {
            max_width: self.max_text_width,
            max_height: 0.0,
            line_height: font_size * 1.5,
            word_wrap: false,
            h_align: uix_graphics::HAlign::Left,
            v_align: uix_graphics::VAlign::Top,
            font_size,
        };
        let backend_opts = uix_graphics::text_backend::TextLayoutOptions::from(opts);
        let fh = self.font;
        self.font_service()
            .hit_test_text(&fh, text, &backend_opts, point)
    }

    /// 获取指定字符的光标 x 位置（相对文本起始点）。
    pub fn cursor_x(&mut self, text: &str, font_size: f32, char_index: usize) -> f32 {
        let opts = TextLayoutOptions {
            max_width: self.max_text_width,
            max_height: 0.0,
            line_height: font_size * 1.5,
            word_wrap: false,
            h_align: uix_graphics::HAlign::Left,
            v_align: uix_graphics::VAlign::Top,
            font_size,
        };
        let backend_opts = uix_graphics::text_backend::TextLayoutOptions::from(opts);
        let fh = self.font;
        self.font_service()
            .text_cursor_x(&fh, text, &backend_opts, char_index)
    }

    // ── 调试模式绘制 ──

    /// 调试颜色调色板：按 widget 深度层级循环使用。
    const DEBUG_COLORS: [Color; 8] = [
        Color::from_rgba(220, 60, 60, 200),    // 红
        Color::from_rgba(60, 140, 220, 200),   // 蓝
        Color::from_rgba(60, 180, 80, 200),    // 绿
        Color::from_rgba(220, 160, 40, 200),   // 橙
        Color::from_rgba(160, 60, 220, 200),   // 紫
        Color::from_rgba(220, 80, 140, 200),   // 粉
        Color::from_rgba(40, 200, 200, 200),   // 青
        Color::from_rgba(180, 180, 60, 200),   // 黄
    ];

    /// 绘制 widget 调试边框（仅在调试模式下生效）。
    /// `depth` 为 widget 在树中的深度，用于循环选择调试颜色。
    /// `hovered` 指示该 widget 是否位于光标链上：悬浮时边框加粗亮色，否则极淡。
    pub fn draw_debug_border(&mut self, rect: Rect, depth: usize, hovered: bool) {
        if !self.debug_mode {
            return;
        }
        let base = Self::DEBUG_COLORS[depth % Self::DEBUG_COLORS.len()];
        let color = if hovered {
            base
        } else {
            Color::from_rgba(base.r, base.g, base.b, 30)
        };
        self.engine.stroke_rect(rect, color, if hovered { 1.5 } else { 0.5 }, None);
    }

    /// 在 widget 左上角显示调试标签（ID + 深度），仅在调试模式下对悬浮链 widget 绘制。
    pub fn draw_debug_label(&mut self, widget_id: usize, depth: usize, rect: Rect) {
        let color = Self::DEBUG_COLORS[depth % Self::DEBUG_COLORS.len()];
        let label = format!("#{} d{}", widget_id, depth);
        let font_size = 12.0;
        let label_w = label.len() as f32 * 7.0 + 6.0;
        let label_h = 16.0;
        self.engine.fill_rect(
            Rect::new(rect.x, rect.y, label_w, label_h),
            Color::from_rgba(0, 0, 0, 180),
            None,
        );
        // 补偿 bearing_y（约 -0.75*font_size），使文字顶与背景顶对齐
        self.draw_text(&label, Point::new(rect.x + 2.0, rect.y + font_size * 0.75), color, font_size);
    }

    /// 在 widget 下方显示 frame 坐标和尺寸，仅在调试模式下对悬浮链 widget 绘制。
    pub fn draw_debug_frame_info(&mut self, widget_id: usize, rect: Rect) {
        let info = format!(
            "#{} ({:.0},{:.0}) {:.0}×{:.0}",
            widget_id, rect.x, rect.y, rect.w, rect.h
        );
        let font_size = 11.0;
        let info_w = info.len() as f32 * 6.5 + 6.0;
        let info_h = 15.0;
        let info_y = rect.y + rect.h;
        self.engine.fill_rect(
            Rect::new(rect.x, info_y, info_w, info_h),
            Color::from_rgba(0, 0, 0, 160),
            None,
        );
        self.draw_text(
            &info,
            Point::new(rect.x + 2.0, info_y + font_size * 0.75),
            Color::from_rgba(200, 200, 200, 220),
            font_size,
        );
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
        rect: uix_core::Rect,
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
