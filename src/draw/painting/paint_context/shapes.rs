//! 图元绘制。

use super::*;

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
            paint_pass: PaintPass::Content,
            recorder: None,
            record_ops: true,
            recording_complete: true,
        }
    }

    /// crate 内部测试的紧凑构造。
    #[cfg(test)]
    // 测试目标保留显式依赖注入构造器，供形状绘制契约测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[allow(
        clippy::too_many_arguments,
        reason = "test fixtures keep surface values inline for readability"
    )]
    pub(crate) fn new_for_test(
        canvas_2d: &'a mut dyn Canvas2D,
        font: FontHandle,
        font_service: &'a FontService,
        image_service: &'a ImageService,
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
    pub(crate) fn with_recorder<R>(
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

    pub(crate) fn with_recording_disabled<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        let previous = std::mem::replace(&mut self.record_ops, false);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| f(self)));
        self.record_ops = previous;
        match result {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    pub(super) fn record_op(&mut self, op: PaintOp) {
        self.record_op_lazy(|| op);
    }

    pub(super) fn record_op_lazy(&mut self, build: impl FnOnce() -> PaintOp) {
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

    /// 当前逻辑 DPI；只读查询不会使 DisplayList 录制失效。
    pub fn dpi(&self) -> f32 {
        self.spatial.dpi()
    }

    /// 当前绘制表面的设备像素比；供需要按目标像素生成派生资源的组件使用。
    pub(crate) fn device_pixel_ratio(&self) -> f32 {
        self.spatial.device_pixel_ratio()
    }

    /// 当前绘制表面的逻辑尺寸；只读查询不会使 DisplayList 录制失效。
    pub fn surface_size(&self) -> Size {
        let (width, height) = self.spatial.surface_size();
        let scale = self.spatial.device_pixel_ratio().max(f32::EPSILON);
        Size::new(width as f32 / scale, height as f32 / scale)
    }

    pub(crate) fn logical_surface_size(&self) -> Size {
        self.surface_size()
    }

    /// 返回映射到当前画布局部坐标的逻辑表面矩形。
    pub(crate) fn logical_surface_rect(&mut self) -> Rect {
        let size = self.logical_surface_size();
        let surface = Rect::new(0.0, 0.0, size.w, size.h);
        let canvas = self.spatial.canvas_2d();
        let mut transform = canvas.current_transform();
        let (offset_x, offset_y) = canvas.offset();
        if offset_x != 0.0 || offset_y != 0.0 {
            // 平移快路径保存在 offset 中，合并后才能得到真实画布变换。
            transform = transform.concat(Transform::translate(offset_x, offset_y));
        }
        transform
            .inverse()
            .map(|inverse| inverse.transform_rect(surface))
            .unwrap_or(surface)
    }

    /// 判断逻辑矩形经当前变换与偏移后是否和有效裁剪区相交。
    pub fn is_rect_visible(&mut self, rect: Rect) -> bool {
        if rect.w <= 0.0 || rect.h <= 0.0 {
            return false;
        }
        let canvas = self.spatial.canvas_2d();
        let (offset_x, offset_y) = canvas.offset();
        let surface_rect = canvas.current_transform().transform_rect(Rect::new(
            rect.x + offset_x,
            rect.y + offset_y,
            rect.w,
            rect.h,
        ));
        surface_rect
            .intersect(&canvas.current_clip())
            .is_some_and(|visible| visible.w > 0.0 && visible.h > 0.0)
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

    /// 显式填充圆角矩形。
    #[inline(always)]
    pub fn fill_rounded_rect(&mut self, rect: Rect, color: Color, radius: Radius) {
        self.fill_rect(rect, color, Some(radius));
    }

    /// 填充圆形。
    #[inline(always)]
    pub fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        self.record_op(PaintOp::FillCircle { cx, cy, r, color });
        self.spatial.canvas_2d().fill_circle(cx, cy, r, color);
    }

    /// 绘制圆形点标记；`diameter` 为逻辑像素直径。
    #[inline(always)]
    pub fn draw_point(&mut self, point: Point, color: Color, diameter: f32) {
        if diameter.is_finite() && diameter > 0.0 {
            self.fill_circle(point.x, point.y, diameter * 0.5, color);
        }
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

    /// 填充任意多边形。
    #[inline(always)]
    pub fn fill_polygon(&mut self, points: &[Point], color: Color, fill_rule: FillRule) {
        let path = PathBuilder::new().polygon(points).build();
        if !path.is_empty() {
            self.fill_path(&path, color, fill_rule);
        }
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

    /// 显式描边圆角矩形。
    #[inline(always)]
    pub fn stroke_rounded_rect(
        &mut self,
        rect: Rect,
        color: Color,
        line_width: f32,
        radius: Radius,
    ) {
        self.stroke_rect(rect, color, line_width, Some(radius));
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
    #[allow(
        clippy::too_many_arguments,
        reason = "arc geometry mirrors the canvas drawing contract"
    )]
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

    /// 描边开放折线；整条折线只进入一次共享描边管线。
    #[inline(always)]
    pub fn stroke_polyline(&mut self, points: &[Point], color: Color, options: &StrokeOptions) {
        let path = PathBuilder::new().polyline(points).build();
        if !path.is_empty() {
            self.stroke_path(&path, color, options);
        }
    }

    /// 描边闭合多边形。
    #[inline(always)]
    pub fn stroke_polygon(&mut self, points: &[Point], color: Color, options: &StrokeOptions) {
        let path = PathBuilder::new().polygon(points).build();
        if !path.is_empty() {
            self.stroke_path(&path, color, options);
        }
    }

    /// 描边椭圆。
    #[inline(always)]
    pub fn stroke_ellipse(&mut self, rect: Rect, color: Color, options: &StrokeOptions) {
        let path = PathBuilder::new().ellipse(rect).build();
        if !path.is_empty() {
            self.stroke_path(&path, color, options);
        }
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
}
