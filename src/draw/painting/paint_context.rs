//! PaintContext — 绘制上下文（Phase 3 迁入 draw）。
//!
//! 持有 Canvas2D（2D 零成本路径）和 SpatialContext（3D 空间路径），
//! 组合 TextRenderService 与 DebugRenderService。

use std::ptr::NonNull;
use std::sync::Arc;

use crate::core::{Point, Rect, Size};
use crate::draw::debug::DebugRenderService;
use crate::draw::font::font_service::FontService;
use crate::draw::font::text::TextRenderService;
use crate::draw::image::{blit_handle, BitmapHandle, ImageService};
use crate::draw::painting::display_list::{DisplayList, PaintOp, PaintPass};
use crate::draw::painting::{ScopedThemeTokens, ThemeTokens, TokenPatch, TokenScope};
use crate::draw::primitives::path::{FillRule, Path, PathBuilder};
use crate::draw::primitives::stroker::StrokeOptions;
use crate::draw::spatial::{Orientation, PhysicalUnit, SpatialContext, Vec3, AABB3D};
use crate::draw::traits::Canvas2D;
use crate::draw::GradientDirection;
use crate::draw::{Color, FontHandle, Radius};

/// 绘制表面配置（组合 dpi/pr/orientation/size 减少参数传递）。
#[derive(Clone, Copy, Debug)]
pub struct PaintSurfaceConfig {
    pub dpi: f32,
    pub device_pixel_ratio: f32,
    pub orientation: Orientation,
    pub surface_w: i32,
    pub surface_h: i32,
}

pub struct PaintContext<'a> {
    /// 3D 空间上下文（持有 Canvas2D 引用）。
    spatial: SpatialContext<'a>,

    /// 文本渲染服务。
    text: TextRenderService<'a>,

    /// 图片资源服务。
    image_service: &'a ImageService,

    /// 调试渲染服务。
    debug: DebugRenderService,

    /// 设计令牌。
    tokens: ScopedThemeTokens<'a>,

    /// 当前绘制阶段（合成器设置）。
    paint_pass: PaintPass,

    /// 可选：录制绘制指令到 DisplayList（独立生命周期，不绑定 canvas）。
    recorder: Option<NonNull<DisplayList>>,

    /// 是否向 recorder 写入（replay 时关闭）。
    record_ops: bool,

    /// 当前录制是否完整；底层直绘一旦绕过 PaintOp 就不可缓存。
    recording_complete: bool,
}

impl<'a> PaintContext<'a> {
    /// 创建渲染上下文。
    ///
    /// `canvas_2d` 用于所有绘制操作。
    /// `surface` 为表面配置（dpi/dpr/orientation/size）。
    pub fn new(
        canvas_2d: &'a mut dyn Canvas2D,
        font: FontHandle,
        font_service: &'a FontService,
        image_service: &'a ImageService,
        tokens: &'a dyn ThemeTokens,
        surface: PaintSurfaceConfig,
    ) -> Self {
        Self {
            spatial: SpatialContext::new(
                canvas_2d,
                surface.dpi,
                surface.device_pixel_ratio,
                surface.orientation,
                surface.surface_w,
                surface.surface_h,
            ),
            text: TextRenderService::new(font, font_service, f32::MAX),
            image_service,
            debug: DebugRenderService::new(false),
            tokens: ScopedThemeTokens::new(tokens),
            paint_pass: PaintPass::Content,
            recorder: None,
            record_ops: true,
            recording_complete: true,
        }
    }

    /// 测试用构造（10 参数兼容旧签名）。
    #[doc(hidden)]
    pub fn new_for_test(
        canvas_2d: &'a mut dyn Canvas2D,
        font: FontHandle,
        font_service: &'a FontService,
        image_service: &'a ImageService,
        tokens: &'a dyn ThemeTokens,
        dpi: f32,
        device_pixel_ratio: f32,
        orientation: Orientation,
        surface_w: i32,
        surface_h: i32,
    ) -> Self {
        Self::new(
            canvas_2d,
            font,
            font_service,
            image_service,
            tokens,
            PaintSurfaceConfig {
                dpi,
                device_pixel_ratio,
                orientation,
                surface_w,
                surface_h,
            },
        )
    }

    /// 当前绘制阶段。
    pub fn paint_pass(&self) -> PaintPass {
        self.paint_pass
    }

    /// 设置绘制阶段（合成器在子节点前后切换）。
    pub fn set_paint_pass(&mut self, pass: PaintPass) {
        self.paint_pass = pass;
    }

    /// 在闭包期间设置录制目标；正常返回或 panic 展开都会恢复上一层作用域。
    pub fn with_recorder<R>(
        &mut self,
        list: &mut DisplayList,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let previous_recorder = self.recorder.replace(NonNull::from(list));
        let previous_complete = self.recording_complete;
        self.recording_complete = true;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(self)));
        let scope_complete = self.recording_complete;
        self.recorder = previous_recorder;
        self.recording_complete = if previous_recorder.is_some() {
            previous_complete && scope_complete
        } else {
            scope_complete
        };
        match result {
            Ok(value) => value,
            Err(payload) => {
                self.recording_complete = false;
                std::panic::resume_unwind(payload)
            }
        }
    }

    /// 当前一次 DisplayList 录制是否覆盖了全部绘制操作。
    pub fn recording_complete(&self) -> bool {
        self.recording_complete
    }

    /// 暂停/恢复指令录制（replay 时使用）。
    pub fn set_record_ops(&mut self, on: bool) {
        self.record_ops = on;
    }

    pub(super) fn with_recording_disabled<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let previous = std::mem::replace(&mut self.record_ops, false);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(self)));
        self.record_ops = previous;
        match result {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    fn record_op(&mut self, op: PaintOp) {
        self.record_op_lazy(|| op);
    }

    fn record_op_lazy(&mut self, build: impl FnOnce() -> PaintOp) {
        if !self.record_ops {
            return;
        }
        if let Some(mut list) = self.recorder {
            // SAFETY: recorder 仅在 with_recorder 闭包内有效，闭包返回前会清除。
            unsafe {
                list.as_mut().push(build());
            }
        }
    }

    fn mark_recording_incomplete(&mut self) {
        if self.record_ops && self.recorder.is_some() {
            self.recording_complete = false;
        }
    }

    // ════════════════════════════════════════════════════════════════════
    // 空间路径入口
    // ════════════════════════════════════════════════════════════════════

    /// 获取空间上下文（3D 变换/物理单位绘制）。
    #[inline(always)]
    pub fn spatial(&mut self) -> &mut SpatialContext<'a> {
        self.mark_recording_incomplete();
        &mut self.spatial
    }

    /// 当前逻辑 DPI；只读查询不会使 DisplayList 录制失效。
    pub fn dpi(&self) -> f32 {
        self.spatial.dpi()
    }

    /// 当前绘制表面的设备像素比；供需要按目标像素生成派生资源的组件使用。
    pub(crate) fn device_pixel_ratio(&self) -> f32 {
        self.spatial.device_pixel_ratio()
    }

    /// 当前绘制表面的逻辑尺寸；只读查询不会使 DisplayList 录制失效。
    pub(crate) fn logical_surface_size(&self) -> Size {
        let (width, height) = self.spatial.surface_size();
        let scale = self.spatial.device_pixel_ratio().max(f32::EPSILON);
        Size::new(width as f32 / scale, height as f32 / scale)
    }

    // ════════════════════════════════════════════════════════════════════
    // 2D 零成本绘制路径（直接委托 Canvas2D）
    // ════════════════════════════════════════════════════════════════════

    /// 填充矩形。
    #[inline(always)]
    pub fn fill_rect(&mut self, rect: Rect, color: Color, radius: Option<Radius>) {
        self.record_op(PaintOp::FillRect {
            rect,
            color,
            radius,
        });
        self.spatial.canvas_2d().fill_rect(rect, color, radius);
    }

    /// 填充圆形。
    #[inline(always)]
    pub fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        self.record_op(PaintOp::FillCircle { cx, cy, r, color });
        self.spatial.canvas_2d().fill_circle(cx, cy, r, color);
    }

    /// 填充椭圆。
    #[inline(always)]
    pub fn fill_ellipse(&mut self, rect: Rect, color: Color) {
        self.record_op(PaintOp::FillEllipse { rect, color });
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
        self.record_op(PaintOp::FillSector {
            cx,
            cy,
            r,
            start_angle,
            end_angle,
            color,
        });
        self.spatial
            .canvas_2d()
            .fill_sector(cx, cy, r, start_angle, end_angle, color);
    }

    /// 填充路径。
    #[inline(always)]
    pub fn fill_path(&mut self, path: &Path, color: Color, fill_rule: FillRule) {
        self.record_op(PaintOp::FillPath {
            path: path.clone(),
            color,
            fill_rule,
        });
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
        self.record_op(PaintOp::StrokeRect {
            rect,
            color,
            line_width,
            radius,
        });
        self.spatial
            .canvas_2d()
            .stroke_rect(rect, color, line_width, radius);
    }

    /// 描边圆形。
    #[inline(always)]
    pub fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color, line_width: f32) {
        self.record_op(PaintOp::StrokeCircle {
            cx,
            cy,
            r,
            color,
            line_width,
        });
        self.spatial
            .canvas_2d()
            .stroke_circle(cx, cy, r, color, line_width);
    }

    /// 描边圆弧；角度使用弧度，几何经共享 Path 描边管线录制与绘制。
    #[inline(always)]
    pub fn stroke_arc(
        &mut self,
        cx: f32,
        cy: f32,
        r: f32,
        start_angle: f32,
        end_angle: f32,
        color: Color,
        line_width: f32,
    ) {
        let path = PathBuilder::new()
            .arc(cx, cy, r, start_angle, end_angle)
            .build();
        if path.is_empty() {
            return;
        }
        self.stroke_path(
            &path,
            color,
            &StrokeOptions {
                width: line_width,
                ..StrokeOptions::default()
            },
        );
    }

    /// 描边路径。
    #[inline(always)]
    pub fn stroke_path(&mut self, path: &Path, color: Color, opts: &StrokeOptions) {
        self.record_op(PaintOp::StrokePath {
            path: path.clone(),
            color,
            options: *opts,
        });
        self.spatial.canvas_2d().stroke_path(path, color, opts);
    }

    /// 画直线。
    #[inline(always)]
    pub fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, width: f32) {
        self.record_op(PaintOp::DrawLine {
            x1,
            y1,
            x2,
            y2,
            color,
            width,
        });
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
        self.record_op(PaintOp::FillLinearGradient {
            rect,
            color_a,
            color_b,
            dir,
        });
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
        self.record_op(PaintOp::FillRadialGradient {
            cx,
            cy,
            inner_r,
            outer_r,
            inner_color,
            outer_color,
        });
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
        self.record_op(PaintOp::DrawBoxShadow {
            rect,
            blur_radius,
            offset_x,
            offset_y,
            color,
            corner_radius,
        });
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
        self.record_op(PaintOp::DrawBoxShadowAmbient {
            rect,
            blur_radius,
            offset_x,
            offset_y,
            color,
            corner_radius,
        });
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
        self.record_op(PaintOp::Save);
        self.spatial.canvas_2d().save();
    }

    /// 恢复渲染状态。
    #[inline(always)]
    pub fn restore(&mut self) {
        self.record_op(PaintOp::Restore);
        self.spatial.canvas_2d().restore();
    }

    /// 推入局部裁剪，并保持 DisplayList 的状态栈顺序。
    pub fn push_clip(&mut self, rect: Rect) {
        self.record_op(PaintOp::PushClip { rect });
        self.spatial.canvas_2d().push_clip(rect);
    }

    /// 弹出最近一次局部裁剪。
    pub fn pop_clip(&mut self) {
        self.record_op(PaintOp::PopClip);
        self.spatial.canvas_2d().pop_clip();
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
        self.mark_recording_incomplete();
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
        self.mark_recording_incomplete();
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
        self.record_op_lazy(|| PaintOp::DrawText {
            text: Arc::from(text),
            pos,
            color,
            font_size,
        });
        let text_t0 = std::time::Instant::now();
        self.text
            .draw_text(self.spatial.canvas_2d(), text, pos, color, font_size);
        crate::core::perf_probe::add_text_draw(text_t0.elapsed().as_micros());
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
        self.record_op_lazy(|| PaintOp::DrawTextBaseline {
            text: Arc::from(text),
            x,
            baseline_y,
            color,
            font_size,
        });
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
        self.record_op_lazy(|| PaintOp::TextCenter {
            text: Arc::from(text),
            rect,
            color,
            font_size,
        });
        self.text
            .text_center(self.spatial.canvas_2d(), text, rect, color, font_size);
    }

    /// 左对齐、垂直居中的文本绘制。
    pub fn draw_text_in_frame(&mut self, text: &str, rect: Rect, color: Color, font_size: f32) {
        if !text.is_empty() {
            self.record_op_lazy(|| PaintOp::DrawTextInFrame {
                text: Arc::from(text),
                rect,
                color,
                font_size,
            });
        }
        self.text
            .draw_text_in_frame(self.spatial.canvas_2d(), text, rect, color, font_size);
    }

    /// 在矩形内绘制自动换行文本。
    pub fn draw_text_wrapped(&mut self, text: &str, rect: Rect, color: Color, font_size: f32) {
        self.record_op_lazy(|| PaintOp::DrawTextWrapped {
            text: Arc::from(text),
            rect,
            color,
            font_size,
        });
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
        self.record_op_lazy(|| PaintOp::DrawTextWithSelection {
            text: Arc::from(text),
            pos,
            color,
            font_size,
            selection,
            selection_bg,
        });
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

    /// 单行行盒高度（ascent + descent）。
    pub fn line_box_height(&mut self, font_size: f32) -> f32 {
        self.text.line_box_height(font_size)
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

    /// 行盒在 rect 内几何居中时的 layout 原点 y（过渡；优先布局算盒再 draw_text）。
    pub fn visual_center_y(&mut self, rect: Rect, font_size: f32) -> f32 {
        self.text.visual_center_y(rect, font_size)
    }

    /// 设置文本绘制最大宽度。
    pub fn set_max_text_width(&mut self, width: f32) {
        self.text.set_max_text_width(width);
    }

    /// 将 glyph layout 绘制到引擎上。
    pub fn blit_glyph_layout(
        &mut self,
        layout: &crate::draw::font::text_backend::TextLayout,
        pos: Point,
        color: Color,
        font_size: f32,
    ) {
        self.record_op(PaintOp::BlitGlyphLayout {
            layout: layout.clone(),
            pos,
            color,
            font_size,
        });
        self.text
            .blit_to(self.spatial.canvas_2d(), layout, pos, color, font_size);
    }

    // ── 访问器 ──

    /// 获取设计令牌。
    #[inline(always)]
    pub fn tokens(&self) -> &dyn ThemeTokens {
        &self.tokens
    }

    pub(crate) fn replace_token_scope(
        &mut self,
        theme: Option<Arc<dyn ThemeTokens>>,
        patch: Option<Arc<TokenPatch>>,
    ) -> TokenScope {
        self.tokens.replace_scope(theme, patch)
    }

    pub(crate) fn restore_token_scope(&mut self, scope: TokenScope) {
        self.tokens.restore_scope(scope);
    }

    /// 获取字体服务。
    #[inline(always)]
    pub fn font_service(&mut self) -> &FontService {
        self.text.font_service
    }

    /// 获取图片服务。
    #[inline(always)]
    pub fn image_service(&self) -> &ImageService {
        self.image_service
    }

    /// 绘制解码位图（按目标区域 fit 居中）。
    pub fn draw_image(&mut self, handle: BitmapHandle, bounds: Rect) {
        self.blit_bitmap(handle, bounds, true);
    }

    /// 绘制解码位图（拉伸填满 bounds）。
    pub fn draw_image_fill(&mut self, handle: BitmapHandle, bounds: Rect) {
        self.blit_bitmap(handle, bounds, false);
    }

    fn blit_bitmap(&mut self, handle: BitmapHandle, bounds: Rect, fit: bool) {
        self.record_op(PaintOp::DrawImage {
            handle,
            bounds,
            fit,
        });
        blit_handle(
            self.image_service,
            self.spatial.canvas_2d(),
            handle,
            bounds,
            fit,
        );
    }

    /// 获取当前字体句柄。
    #[inline(always)]
    pub fn font(&self) -> &FontHandle {
        self.text.font()
    }

    /// 设置字体句柄。
    #[inline(always)]
    pub fn set_font(&mut self, font: FontHandle) {
        self.record_op(PaintOp::SetFont { font });
        self.text.set_font(font);
    }

    /// 获取底面 Canvas2D 引用（用于底层操作）。
    #[inline(always)]
    pub fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        self.mark_recording_incomplete();
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

    /// 绘制节点调试边框。
    pub fn draw_debug_border(&mut self, rect: Rect, depth: usize, hovered: bool) {
        self.debug
            .draw_debug_border(self.spatial.canvas_2d(), rect, depth, hovered);
    }

    /// 在节点左上角显示调试标签。
    pub fn draw_debug_label(&mut self, node_slot: usize, depth: usize, rect: Rect) {
        if !self.debug.debug_mode {
            return;
        }
        let color =
            DebugRenderService::DEBUG_COLORS[depth % DebugRenderService::DEBUG_COLORS.len()];
        let label = format!("#{} d{}", node_slot, depth);
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
    pub fn draw_debug_frame_info(&mut self, node_slot: usize, rect: Rect) {
        if !self.debug.debug_mode {
            return;
        }
        let info = format!(
            "#{} ({:.0},{:.0}) {:.0}×{:.0}",
            node_slot, rect.x, rect.y, rect.w, rect.h
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
