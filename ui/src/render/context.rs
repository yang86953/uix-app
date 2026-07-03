//! RenderContext — 渲染上下文。
//!
//! 持有 Canvas2D（2D 零成本路径）和 SpatialContext（3D 空间路径），
//! 组合 TextRenderService（文本布局/光栅化）和 DebugRenderService（调试绘制）。
//!
//! # 两条绘制路径
//!
//! - **2D 零成本路径**：`ctx.fill_rect(rect, color, radius)` → 直接委托 Canvas2D
//! - **3D 空间路径**：`ctx.spatial().fill_rect(box_3d, color, radius)` → 经 SpatialContext 投影

use crate::api::traits::TokenProvider;
use crate::debug_render::DebugRenderService;
use crate::style::Style;
use crate::text_render::TextRenderService;
use uix_graphics::font_service::FontService;
use uix_graphics::path::{FillRule, Path};
use uix_graphics::spatial::{Orientation, PhysicalUnit, SpatialContext, Vec3, AABB3D};
use uix_graphics::stroker::StrokeOptions;
use uix_graphics::traits::Canvas2D;
use uix_graphics::GradientDirection;
use uix_graphics::{Color, FontHandle, Radius};
use uix_platform::{Point, Rect, Size};

/// 渲染上下文。
pub struct RenderContext<'a> {
    /// 3D 空间上下文（持有 Canvas2D 引用）。
    spatial: SpatialContext<'a>,

    /// 文本渲染服务。
    text: TextRenderService<'a>,

    /// 调试渲染服务。
    debug: DebugRenderService,

    /// 设计令牌提供者。
    tokens: &'a dyn TokenProvider,
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
            text: TextRenderService::new(font, font_service, f32::MAX),
            debug: DebugRenderService::new(false),
            tokens,
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
    pub fn fill_sector(
        &mut self,
        cx: f32,
        cy: f32,
        r: f32,
        start_angle: f32,
        end_angle: f32,
        color: Color,
    ) {
        self.spatial
            .canvas_2d()
            .fill_sector(cx, cy, r, start_angle, end_angle, color);
    }

    /// 填充路径。
    #[inline(always)]
    pub fn fill_path(&mut self, path: &Path, color: Color, fill_rule: FillRule) {
        self.spatial.canvas_2d().fill_path(path, color, fill_rule);
    }

    /// 描边矩形。
    #[inline(always)]
    pub fn stroke_rect(
        &mut self,
        rect: Rect,
        color: Color,
        line_width: f32,
        radius: Option<Radius>,
    ) {
        self.spatial
            .canvas_2d()
            .stroke_rect(rect, color, line_width, radius);
    }

    /// 描边圆形。
    #[inline(always)]
    pub fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color, line_width: f32) {
        self.spatial
            .canvas_2d()
            .stroke_circle(cx, cy, r, color, line_width);
    }

    /// 描边路径。
    #[inline(always)]
    pub fn stroke_path(&mut self, path: &Path, color: Color, opts: &StrokeOptions) {
        self.spatial.canvas_2d().stroke_path(path, color, opts);
    }

    /// 画直线。
    #[inline(always)]
    pub fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, width: f32) {
        self.spatial
            .canvas_2d()
            .draw_line(x1, y1, x2, y2, color, width);
    }

    // ── 渐变 ──

    /// 线性渐变填充。
    #[inline(always)]
    pub fn fill_linear_gradient(
        &mut self,
        rect: Rect,
        color_a: Color,
        color_b: Color,
        dir: GradientDirection,
    ) {
        self.spatial
            .canvas_2d()
            .fill_linear_gradient(rect, color_a, color_b, dir);
    }

    /// 径向渐变填充。
    #[inline(always)]
    pub fn fill_radial_gradient(
        &mut self,
        cx: f32,
        cy: f32,
        inner_r: f32,
        outer_r: f32,
        inner_color: Color,
        outer_color: Color,
    ) {
        self.spatial.canvas_2d().fill_radial_gradient(
            cx,
            cy,
            inner_r,
            outer_r,
            inner_color,
            outer_color,
        );
    }

    // ── 阴影 ──

    /// 绘制盒阴影（定向光阴影）。
    #[inline(always)]
    pub fn draw_box_shadow(
        &mut self,
        rect: Rect,
        blur_radius: f32,
        offset_x: f32,
        offset_y: f32,
        color: Color,
        corner_radius: Option<Radius>,
    ) {
        self.spatial.canvas_2d().draw_box_shadow(
            rect,
            blur_radius,
            offset_x,
            offset_y,
            color,
            corner_radius,
        );
    }

    /// 绘制环境阴影。
    #[inline(always)]
    pub fn draw_box_shadow_ambient(
        &mut self,
        rect: Rect,
        blur_radius: f32,
        offset_x: f32,
        offset_y: f32,
        color: Color,
        corner_radius: Option<Radius>,
    ) {
        self.spatial.canvas_2d().draw_box_shadow_ambient(
            rect,
            blur_radius,
            offset_x,
            offset_y,
            color,
            corner_radius,
        );
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
    // 文本绘制（委托给 TextRenderService）
    // ════════════════════════════════════════════════════════════════════

    /// 在 3D 空间中绘制文本。
    pub fn draw_text_spatial(
        &mut self,
        text: &str,
        pos: Vec3,
        color: Color,
        font_size: PhysicalUnit,
    ) {
        if text.is_empty() {
            return;
        }
        let (sx, sy) = self.spatial.project(&pos);
        let fs = font_size.to_dip(self.spatial.dpi());
        let canvas = self.spatial.canvas_2d();
        self.text
            .draw_text(canvas, text, Point::new(sx, sy), color, fs);
    }

    /// 在 3D 空间中的矩形区域内居中绘制文本。
    pub fn text_center_spatial(
        &mut self,
        text: &str,
        box_3d: AABB3D,
        color: Color,
        font_size: PhysicalUnit,
    ) {
        if text.is_empty() {
            return;
        }
        let fs = font_size.to_dip(self.spatial.dpi());
        let quad = self.spatial.project_aabb(&box_3d);
        let bounds = quad.bounds();
        let sz = self.text.measure_text(text, fs);
        let x = bounds.x + (bounds.w - sz.w) * 0.5;
        let y = bounds.y + (bounds.h - fs * 1.5) * 0.5;
        let canvas = self.spatial.canvas_2d();
        self.text
            .draw_text(canvas, text, Point::new(x, y), color, fs);
    }

    /// 绘制文本（左对齐，顶部对齐）。
    pub fn draw_text(&mut self, text: &str, pos: Point, color: Color, font_size: f32) {
        self.text
            .draw_text(self.spatial.canvas_2d(), text, pos, color, font_size);
    }

    /// 基于基线绘制文本。
    pub fn draw_text_baseline(
        &mut self,
        text: &str,
        x: f32,
        baseline_y: f32,
        color: Color,
        font_size: f32,
    ) {
        self.text.draw_text_baseline(
            self.spatial.canvas_2d(),
            text,
            x,
            baseline_y,
            color,
            font_size,
        );
    }

    /// 在矩形内居中绘制文本。
    pub fn text_center(&mut self, text: &str, rect: Rect, color: Color, font_size: f32) {
        self.text
            .text_center(self.spatial.canvas_2d(), text, rect, color, font_size);
    }

    /// 左对齐、垂直居中的文本绘制。
    pub fn draw_text_in_frame(&mut self, text: &str, rect: Rect, color: Color, font_size: f32) {
        self.text
            .draw_text_in_frame(self.spatial.canvas_2d(), text, rect, color, font_size);
    }

    /// 在矩形内绘制自动换行文本。
    pub fn draw_text_wrapped(&mut self, text: &str, rect: Rect, color: Color, font_size: f32) {
        self.text
            .draw_text_wrapped(self.spatial.canvas_2d(), text, rect, color, font_size);
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
        self.text.draw_text_with_selection(
            self.spatial.canvas_2d(),
            text,
            pos,
            color,
            font_size,
            selection,
            selection_bg,
        );
    }

    /// 获取选中文本的矩形区域列表。
    pub fn selection_rects(
        &mut self,
        text: &str,
        font_size: f32,
        pos: Point,
        start: usize,
        end: usize,
    ) -> Vec<Rect> {
        self.text
            .selection_rects(self.spatial.canvas_2d(), text, font_size, pos, start, end)
    }

    /// 测量文本尺寸（不换行）。
    pub fn measure_text(&mut self, text: &str, font_size: f32) -> Size {
        self.text.measure_text(text, font_size)
    }

    /// 测量文本尺寸（换行模式）。
    pub fn measure_text_wrapped(&mut self, text: &str, font_size: f32, max_width: f32) -> Size {
        self.text.measure_text_wrapped(text, font_size, max_width)
    }

    /// 文本命中测试。
    pub fn text_hit_test(&mut self, text: &str, font_size: f32, point: Point) -> Option<usize> {
        self.text.text_hit_test(text, font_size, point)
    }

    /// 获取指定字符的光标 x 位置。
    pub fn text_cursor_x(&mut self, text: &str, font_size: f32, char_index: usize) -> f32 {
        self.text.text_cursor_x(text, font_size, char_index)
    }

    /// 计算文字视觉中心与 rect 中心对齐时的 y 位置。
    pub fn visual_center_y(&mut self, rect: Rect, font_size: f32) -> f32 {
        self.text.visual_center_y(rect, font_size)
    }

    /// 设置文本绘制最大宽度。
    pub fn set_max_text_width(&mut self, width: f32) {
        self.text.set_max_text_width(width);
    }

    /// 将 glyph layout 绘制到引擎上。
    pub(crate) fn blit_glyph_layout(
        &mut self,
        layout: &uix_graphics::text_backend::TextLayout,
        pos: Point,
        color: Color,
        font_size: f32,
    ) {
        self.text
            .blit_to(self.spatial.canvas_2d(), layout, pos, color, font_size);
    }

    // ── 辅助 ──

    /// 应用 Style 到矩形区域（背景 + 边框 + 阴影 + 透明度）。
    pub fn apply_style(&mut self, rect: Rect, style: &Style) {
        let r = if style.border_radius > 0.0 {
            Some(Radius::uniform(style.border_radius))
        } else {
            None
        };
        if let Some(shadow) = style.box_shadow.as_ref() {
            self.draw_box_shadow(
                rect,
                shadow.blur,
                shadow.offset_x,
                shadow.offset_y,
                shadow.color,
                r,
            );
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
        self.text.font_service
    }

    /// 获取当前字体句柄。
    #[inline(always)]
    pub fn font(&self) -> &FontHandle {
        self.text.font()
    }

    /// 设置字体句柄。
    #[inline(always)]
    pub fn set_font(&mut self, font: FontHandle) {
        self.text.set_font(font);
    }

    /// 获取底面 Canvas2D 引用（用于底层操作）。
    #[inline(always)]
    pub fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        self.spatial.canvas_2d()
    }

    /// 设置调试模式。
    #[inline(always)]
    pub fn set_debug_mode(&mut self, mode: bool) {
        self.debug.set_debug_mode(mode);
    }

    /// 是否处于调试模式。
    #[inline(always)]
    pub fn debug_mode(&self) -> bool {
        self.debug.debug_mode
    }

    // ════════════════════════════════════════════════════════════════════
    // 调试绘制（委托给 DebugRenderService）
    // ════════════════════════════════════════════════════════════════════

    /// 绘制 widget 调试边框。
    pub fn draw_debug_border(&mut self, rect: Rect, depth: usize, hovered: bool) {
        self.debug
            .draw_debug_border(self.spatial.canvas_2d(), rect, depth, hovered);
    }

    /// 在 widget 左上角显示调试标签。
    pub fn draw_debug_label(&mut self, widget_id: usize, depth: usize, rect: Rect) {
        if !self.debug.debug_mode {
            return;
        }
        let color =
            DebugRenderService::DEBUG_COLORS[depth % DebugRenderService::DEBUG_COLORS.len()];
        let label = format!("#{} d{}", widget_id, depth);
        let font_size = 12.0;
        let label_w = label.len() as f32 * 7.0 + 6.0;
        let label_h = 16.0;
        let canvas = self.spatial.canvas_2d();
        canvas.fill_rect(
            Rect::new(rect.x, rect.y, label_w, label_h),
            Color::from_rgba(0, 0, 0, 180),
            None,
        );
        self.draw_text(
            &label,
            Point::new(rect.x + 2.0, rect.y + font_size * 0.75),
            color,
            font_size,
        );
    }

    /// 在 widget 下方显示 frame 坐标和尺寸。
    pub fn draw_debug_frame_info(&mut self, widget_id: usize, rect: Rect) {
        if !self.debug.debug_mode {
            return;
        }
        let info = format!(
            "#{} ({:.0},{:.0}) {:.0}×{:.0}",
            widget_id, rect.x, rect.y, rect.w, rect.h
        );
        let font_size = 11.0;
        let info_w = info.len() as f32 * 6.5 + 6.0;
        let info_h = 15.0;
        let info_y = rect.y + rect.h;
        let canvas = self.spatial.canvas_2d();
        canvas.fill_rect(
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
}

// ════════════════════════════════════════════════════════════════════════════
// 全局辅助函数
// ════════════════════════════════════════════════════════════════════════════

/// 解析字号：如果指定了物理单位则通过当前 DPI 转换，否则返回 base。
///
/// widget render 中的标准用法：
/// ```ignore
/// let fs = resolve_font_size(self.font_size, self.font_size_unit, ctx.spatial().dpi());
/// ```
pub fn resolve_font_size(base: f32, unit: Option<PhysicalUnit>, dpi: f32) -> f32 {
    unit.map(|u| u.to_dip(dpi)).unwrap_or(base)
}

#[cfg(test)]
mod tests {
    use super::*;
    use uix_graphics::spatial::PhysicalUnit;

    #[test]
    fn resolve_font_size_no_unit_returns_base() {
        assert_eq!(resolve_font_size(14.0, None, 96.0), 14.0);
    }

    #[test]
    fn resolve_font_size_zero_base() {
        assert_eq!(resolve_font_size(0.0, None, 96.0), 0.0);
    }

    #[test]
    fn resolve_font_size_with_dip_unit_returns_base() {
        let unit = PhysicalUnit::Px(16.0);
        let result = resolve_font_size(14.0, Some(unit), 96.0);
        // Dip 直接返回其值
        assert_eq!(result, 16.0);
    }

    #[test]
    fn resolve_font_size_with_mm_unit() {
        let unit = PhysicalUnit::Mm(10.0);
        // 10mm @ 96 DPI ≈ 37.795
        let result = resolve_font_size(14.0, Some(unit), 96.0);
        assert!((result - 37.795).abs() < 0.01);
    }

    #[test]
    fn resolve_font_size_with_pt_unit() {
        let unit = PhysicalUnit::Pt(12.0);
        // 12pt @ 96 DPI = 12 * 96/72 = 16
        let result = resolve_font_size(14.0, Some(unit), 96.0);
        assert!((result - 16.0).abs() < 0.01);
    }

    #[test]
    fn resolve_font_size_with_pt_unit_high_dpi() {
        let unit = PhysicalUnit::Pt(12.0);
        let result = resolve_font_size(14.0, Some(unit), 192.0);
        // 12pt @ 192 DPI = 12 * 192/72 = 32
        assert!((result - 32.0).abs() < 0.01);
    }

    #[test]
    fn resolve_font_size_with_px_unit() {
        let unit = PhysicalUnit::Px(20.0);
        let result = resolve_font_size(14.0, Some(unit), 96.0);
        // px 直接返回其值
        assert_eq!(result, 20.0);
    }
}
