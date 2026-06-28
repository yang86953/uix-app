//! RenderContext — 渲染上下文。
//!
//! 持有 Canvas2D（2D 零成本路径）和 SpatialContext（3D 空间路径），
//! 提供字体服务、设计令牌、调试能力。
//!
//! # 两条绘制路径
//!
//! - **2D 零成本路径**：`ctx.fill_rect(rect, color, radius)` → 直接委托 Canvas2D
//! - **3D 空间路径**：`ctx.spatial().fill_rect(box_3d, color, radius)` → 经 SpatialContext 投影

use uix_platform::{Point, Rect, Size};
use uix_graphics::font_service::FontService;
use uix_graphics::path::{FillRule, Path};
use uix_graphics::spatial::{Orientation, SpatialContext};
use uix_graphics::stroker::StrokeOptions;
use uix_graphics::traits::Canvas2D;
use uix_graphics::{Color, FontHandle, Radius};
use uix_graphics::{GradientDirection, TextLayoutOptions};
use crate::style::Style;
use crate::theme::TokenProvider;

/// 渲染上下文。
pub struct RenderContext<'a> {
    /// 3D 空间上下文（持有 Canvas2D 引用）。
    spatial: SpatialContext<'a>,

    /// 当前字体句柄。
    font: FontHandle,
    /// 字体服务（文本布局 + 光栅化）。
    font_service: &'a FontService,
    /// 文本绘制最大宽度。
    max_text_width: f32,
    /// 设计令牌提供者。
    tokens: &'a dyn TokenProvider,
    /// 调试模式。
    debug_mode: bool,
}

impl<'a> RenderContext<'a> {
    /// 创建渲染上下文。
    ///
    /// `canvas_2d` 用于所有绘制操作。
    /// `dpi` 为屏幕 DPI（96=标准，192=高 DPI）。
    /// `device_pixel_ratio` 为设备像素比（1.0=标准，2.0=2x）。
    /// `orientation` 为平台坐标方向。
    /// `surface_w` / `surface_h` 为表面尺寸。
    pub fn new(
        canvas_2d: &'a mut dyn Canvas2D,
        font: FontHandle,
        font_service: &'a FontService,
        tokens: &'a dyn TokenProvider,
        dpi: f32,
        device_pixel_ratio: f32,
        orientation: Orientation,
        surface_w: i32,
        surface_h: i32,
    ) -> Self {
        Self {
            spatial: SpatialContext::new(
                canvas_2d,
                dpi,
                device_pixel_ratio,
                orientation,
                surface_w,
                surface_h,
            ),
            font,
            font_service,
            max_text_width: f32::MAX,
            tokens,
            debug_mode: false,
        }
    }

    // ════════════════════════════════════════════════════════════════════
    // 空间路径入口
    // ════════════════════════════════════════════════════════════════════

    /// 获取空间上下文（3D 变换/物理单位绘制）。
    #[inline(always)]
    pub fn spatial(&mut self) -> &mut SpatialContext<'a> {
        &mut self.spatial
    }

    // ════════════════════════════════════════════════════════════════════
    // 2D 零成本绘制路径（直接委托 Canvas2D）
    // ════════════════════════════════════════════════════════════════════

    /// 填充矩形。
    #[inline(always)]
    pub fn fill_rect(&mut self, rect: Rect, color: Color, radius: Option<Radius>) {
        self.spatial.canvas_2d().fill_rect(rect, color, radius);
    }

    /// 填充圆形。
    #[inline(always)]
    pub fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        self.spatial.canvas_2d().fill_circle(cx, cy, r, color);
    }

    /// 填充椭圆。
    #[inline(always)]
    pub fn fill_ellipse(&mut self, rect: Rect, color: Color) {
        self.spatial.canvas_2d().fill_ellipse(rect, color);
    }

    /// 填充扇形。
    #[inline(always)]
    pub fn fill_sector(&mut self, cx: f32, cy: f32, r: f32, start_angle: f32, end_angle: f32, color: Color) {
        self.spatial.canvas_2d().fill_sector(cx, cy, r, start_angle, end_angle, color);
    }

    /// 填充路径。
    #[inline(always)]
    pub fn fill_path(&mut self, path: &Path, color: Color, fill_rule: FillRule) {
        self.spatial.canvas_2d().fill_path(path, color, fill_rule);
    }

    /// 描边矩形。
    #[inline(always)]
    pub fn stroke_rect(&mut self, rect: Rect, color: Color, line_width: f32, radius: Option<Radius>) {
        self.spatial.canvas_2d().stroke_rect(rect, color, line_width, radius);
    }

    /// 描边圆形。
    #[inline(always)]
    pub fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color, line_width: f32) {
        self.spatial.canvas_2d().stroke_circle(cx, cy, r, color, line_width);
    }

    /// 描边路径。
    #[inline(always)]
    pub fn stroke_path(&mut self, path: &Path, color: Color, opts: &StrokeOptions) {
        self.spatial.canvas_2d().stroke_path(path, color, opts);
    }

    /// 画直线。
    #[inline(always)]
    pub fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, width: f32) {
        self.spatial.canvas_2d().draw_line(x1, y1, x2, y2, color, width);
    }

    // ── 渐变 ──

    /// 线性渐变填充。
    #[inline(always)]
    pub fn fill_linear_gradient(&mut self, rect: Rect, color_a: Color, color_b: Color, dir: GradientDirection) {
        self.spatial.canvas_2d().fill_linear_gradient(rect, color_a, color_b, dir);
    }

    /// 径向渐变填充。
    #[inline(always)]
    pub fn fill_radial_gradient(&mut self, cx: f32, cy: f32, inner_r: f32, outer_r: f32, inner_color: Color, outer_color: Color) {
        self.spatial.canvas_2d().fill_radial_gradient(cx, cy, inner_r, outer_r, inner_color, outer_color);
    }

    // ── 阴影 ──

    /// 绘制盒阴影（定向光阴影）。
    #[inline(always)]
    pub fn draw_box_shadow(&mut self, rect: Rect, blur_radius: f32, offset_x: f32, offset_y: f32, color: Color, corner_radius: Option<Radius>) {
        self.spatial.canvas_2d().draw_box_shadow(rect, blur_radius, offset_x, offset_y, color, corner_radius);
    }

    /// 绘制环境阴影。
    #[inline(always)]
    pub fn draw_box_shadow_ambient(&mut self, rect: Rect, blur_radius: f32, offset_x: f32, offset_y: f32, color: Color, corner_radius: Option<Radius>) {
        self.spatial.canvas_2d().draw_box_shadow_ambient(rect, blur_radius, offset_x, offset_y, color, corner_radius);
    }

    // ── 渲染状态 ──

    /// 保存渲染状态。
    #[inline(always)]
    pub fn save(&mut self) {
        self.spatial.canvas_2d().save();
    }

    /// 恢复渲染状态。
    #[inline(always)]
    pub fn restore(&mut self) {
        self.spatial.canvas_2d().restore();
    }

    // ════════════════════════════════════════════════════════════════════
    // 文本绘制（委托给 FontService 布局/光栅化）
    // ════════════════════════════════════════════════════════════════════

    /// 创建文本布局选项。
    fn text_opts(
        &self,
        font_size: f32,
        max_width: f32,
        max_height: f32,
        word_wrap: bool,
        h_align: uix_graphics::HAlign,
        v_align: uix_graphics::VAlign,
    ) -> uix_graphics::text_backend::TextLayoutOptions {
        let opts = TextLayoutOptions {
            max_width,
            max_height,
            line_height: font_size * 1.5,
            word_wrap,
            h_align,
            v_align,
            font_size,
        };
        uix_graphics::text_backend::TextLayoutOptions::from(opts)
    }

    /// 绘制文本（左对齐，顶部对齐）。
    pub fn draw_text(&mut self, text: &str, pos: Point, color: Color, font_size: f32) {
        if text.is_empty() {
            return;
        }
        let backend_opts = self.text_opts(font_size, self.max_text_width, 0.0, false, uix_graphics::HAlign::Left, uix_graphics::VAlign::Top);
        let fh = self.font;
        let layout = self
            .font_service
            .layout_text(&fh, text, &backend_opts);
        self.blit_glyph_layout(&layout, pos, color, font_size);
    }

    /// 基于基线绘制文本。
    pub fn draw_text_baseline(&mut self, text: &str, x: f32, baseline_y: f32, color: Color, font_size: f32) {
        if text.is_empty() {
            return;
        }
        let fs = font_size.max(1.0);
        let fh = self.font;
        let metrics = self.font_service.horizontal_line_metrics(&fh, fs);
        let ascent = metrics.map(|m| m.ascent).unwrap_or(fs * 0.8);
        let top_y = baseline_y - ascent;
        self.draw_text(text, Point::new(x, top_y), color, fs);
    }

    /// 在矩形内居中绘制文本。
    pub fn text_center(&mut self, text: &str, rect: Rect, color: Color, font_size: f32) {
        if text.is_empty() {
            return;
        }
        let cnt = rect.x + rect.w * 0.5;
        let backend_opts = self.text_opts(font_size, self.max_text_width, 0.0, false, uix_graphics::HAlign::Left, uix_graphics::VAlign::Top);
        let fh = self.font;
        let layout = self
            .font_service
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
        let backend_opts = self.text_opts(font_size, rect.w.max(1.0), 0.0, false, uix_graphics::HAlign::Left, uix_graphics::VAlign::Top);
        let fh = self.font;
        let layout = self
            .font_service
            .layout_text(&fh, text, &backend_opts);
        let x = rect.x;
        let y = self.visual_center_y(rect, font_size);
        self.blit_glyph_layout(&layout, Point::new(x, y), color, font_size);
    }

    /// 在矩形内绘制自动换行文本。
    pub fn draw_text_wrapped(&mut self, text: &str, rect: Rect, color: Color, font_size: f32) {
        if text.is_empty() {
            return;
        }
        let backend_opts = self.text_opts(font_size, rect.w.max(1.0), rect.h.max(0.0), true, uix_graphics::HAlign::Left, uix_graphics::VAlign::Top);
        let fh = self.font;
        let layout = self
            .font_service
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
        let canvas = self.spatial.canvas_2d();

        for gp in &layout.glyphs {
            let fh = if gp.font.0 != u32::MAX { gp.font } else { self.font };
            let raster = self
                .font_service
                .rasterize_glyph(&fh, gp.glyph_id, fs);
            if raster.width == 0 || raster.height == 0 {
                continue;
            }
            let gx = (pos.x + gp.x + raster.bearing_x) as i32;
            let gy = (pos.y + gp.y + raster.bearing_y) as i32;
            canvas.blit_glyph(gx, gy, &raster.coverage, raster.width, raster.height, color);
        }
    }

    // ── 文本选中渲染 ──

    /// 获取选中文本的矩形区域列表。
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
        let backend_opts = self.text_opts(font_size, self.max_text_width, 0.0, false, uix_graphics::HAlign::Left, uix_graphics::VAlign::Top);
        let fh = self.font;
        let layout = self
            .font_service
            .layout_text(&fh, text, &backend_opts);
        if layout.glyphs.is_empty() {
            return Vec::new();
        }
        let end = end.min(layout.glyphs.len());
        let start = start.min(end);

        let visual_h = self.font_service
            .horizontal_line_metrics(&fh, font_size)
            .map(|m| m.ascent + m.descent)
            .unwrap_or(font_size * 1.2);

        let mut rects = Vec::new();
        for line in &layout.lines {
            let gs = line.glyph_start;
            let gc = line.glyph_count;
            let ge = gs + gc;
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

    /// 绘制文本选中背景 + 文本。
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

    // ── 文本测量 ──

    /// 测量文本尺寸（不换行）。
    pub fn measure_text(&mut self, text: &str, font_size: f32) -> Size {
        let backend_opts = self.text_opts(font_size, self.max_text_width, 0.0, false, uix_graphics::HAlign::Left, uix_graphics::VAlign::Top);
        let fh = self.font;
        self.font_service
            .measure_text(&fh, text, &backend_opts)
    }

    /// 测量文本尺寸（换行模式）。
    pub fn measure_text_wrapped(&mut self, text: &str, font_size: f32, max_width: f32) -> Size {
        let backend_opts = self.text_opts(font_size, max_width, 0.0, true, uix_graphics::HAlign::Left, uix_graphics::VAlign::Top);
        let fh = self.font;
        self.font_service
            .measure_text(&fh, text, &backend_opts)
    }

    /// 文本命中测试。
    pub fn text_hit_test(&mut self, text: &str, font_size: f32, point: Point) -> Option<usize> {
        let backend_opts = self.text_opts(font_size, self.max_text_width, 0.0, false, uix_graphics::HAlign::Left, uix_graphics::VAlign::Top);
        let fh = self.font;
        self.font_service
            .hit_test_text(&fh, text, &backend_opts, point)
    }

    /// 获取指定字符的光标 x 位置。
    pub fn text_cursor_x(&mut self, text: &str, font_size: f32, char_index: usize) -> f32 {
        let backend_opts = self.text_opts(font_size, self.max_text_width, 0.0, false, uix_graphics::HAlign::Left, uix_graphics::VAlign::Top);
        let fh = self.font;
        self.font_service
            .text_cursor_x(&fh, text, &backend_opts, char_index)
    }

    // ── 辅助方法 ──

    /// 计算文字视觉中心与 rect 中心对齐时的 y 位置。
    pub fn visual_center_y(&mut self, rect: Rect, font_size: f32) -> f32 {
        let fs = font_size.max(1.0);
        let fh = self.font;
        match self.font_service.horizontal_line_metrics(&fh, fs) {
            Some(m) => rect.y + (rect.h - m.ascent - m.descent) * 0.5,
            None => rect.y + rect.h * 0.5,
        }
    }

    /// 设置文本绘制最大宽度。
    pub fn set_max_text_width(&mut self, width: f32) {
        self.max_text_width = width;
    }

    /// 应用 Style 到矩形区域（背景 + 边框 + 阴影 + 透明度）。
    pub fn apply_style(&mut self, rect: Rect, style: &Style) {
        let r = if style.border_radius > 0.0 {
            Some(Radius::uniform(style.border_radius))
        } else {
            None
        };
        if style.shadow_blur > 0.0 && style.shadow_color.a > 0 {
            self.draw_box_shadow(rect, style.shadow_blur, style.shadow_offset_x, style.shadow_offset_y, style.shadow_color, r);
        }
        if style.opacity < 1.0 {
            self.spatial.canvas_2d().set_opacity(style.opacity);
        }
        if let Some(bg) = style.background {
            self.fill_rect(rect, bg, r);
        }
        if style.border_width > 0.0 {
            if let Some(bc) = style.border_color {
                self.stroke_rect(rect, bc, style.border_width, r);
            }
        }
        if style.opacity < 1.0 {
            self.spatial.canvas_2d().set_opacity(1.0);
        }
    }

    // ════════════════════════════════════════════════════════════════════
    // 访问器
    // ════════════════════════════════════════════════════════════════════

    /// 获取设计令牌。
    #[inline(always)]
    pub fn tokens(&self) -> &dyn TokenProvider {
        self.tokens
    }

    /// 获取字体服务。
    #[inline(always)]
    pub fn font_service(&mut self) -> &FontService {
        self.font_service
    }

    /// 获取当前字体句柄。
    #[inline(always)]
    pub fn font(&self) -> &FontHandle {
        &self.font
    }

    /// 设置字体句柄。
    #[inline(always)]
    pub fn set_font(&mut self, font: FontHandle) {
        self.font = font;
    }

    /// 获取底面 Canvas2D 引用（用于底层操作）。
    #[inline(always)]
    pub fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        self.spatial.canvas_2d()
    }

    /// 设置调试模式。
    #[inline(always)]
    pub fn set_debug_mode(&mut self, mode: bool) {
        self.debug_mode = mode;
    }

    /// 是否处于调试模式。
    #[inline(always)]
    pub fn debug_mode(&self) -> bool {
        self.debug_mode
    }

    // ════════════════════════════════════════════════════════════════════
    // 调试绘制
    // ════════════════════════════════════════════════════════════════════

    /// 调试颜色调色板。
    const DEBUG_COLORS: [Color; 8] = [
        Color::from_rgba(220, 60, 60, 200),
        Color::from_rgba(60, 140, 220, 200),
        Color::from_rgba(60, 180, 80, 200),
        Color::from_rgba(220, 160, 40, 200),
        Color::from_rgba(160, 60, 220, 200),
        Color::from_rgba(220, 80, 140, 200),
        Color::from_rgba(40, 200, 200, 200),
        Color::from_rgba(180, 180, 60, 200),
    ];

    /// 绘制 widget 调试边框。
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
        self.spatial.canvas_2d().stroke_rect(rect, color, if hovered { 1.5 } else { 0.5 }, None);
    }

    /// 在 widget 左上角显示调试标签。
    pub fn draw_debug_label(&mut self, widget_id: usize, depth: usize, rect: Rect) {
        if !self.debug_mode { return; }
        let color = Self::DEBUG_COLORS[depth % Self::DEBUG_COLORS.len()];
        let label = format!("#{} d{}", widget_id, depth);
        let font_size = 12.0;
        let label_w = label.len() as f32 * 7.0 + 6.0;
        let label_h = 16.0;
        let canvas = self.spatial.canvas_2d();
        canvas.fill_rect(Rect::new(rect.x, rect.y, label_w, label_h), Color::from_rgba(0, 0, 0, 180), None);
        self.draw_text(&label, Point::new(rect.x + 2.0, rect.y + font_size * 0.75), color, font_size);
    }

    /// 在 widget 下方显示 frame 坐标和尺寸。
    pub fn draw_debug_frame_info(&mut self, widget_id: usize, rect: Rect) {
        if !self.debug_mode { return; }
        let info = format!("#{} ({:.0},{:.0}) {:.0}×{:.0}", widget_id, rect.x, rect.y, rect.w, rect.h);
        let font_size = 11.0;
        let info_w = info.len() as f32 * 6.5 + 6.0;
        let info_h = 15.0;
        let info_y = rect.y + rect.h;
        let canvas = self.spatial.canvas_2d();
        canvas.fill_rect(Rect::new(rect.x, info_y, info_w, info_h), Color::from_rgba(0, 0, 0, 160), None);
        self.draw_text(&info, Point::new(rect.x + 2.0, info_y + font_size * 0.75), Color::from_rgba(200, 200, 200, 220), font_size);
    }
}
