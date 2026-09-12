//! 2D 绘制上下文协议。

use crate::core::{Point, Rect, Size};
use crate::draw::geometry::color::Color;
use crate::draw::geometry::path::{FillRule, Path, PathBuilder};
use crate::draw::geometry::stroker::StrokeOptions;
use crate::draw::geometry::types::{BlendMode, GradientDirection, Radius, Transform};

/// 后端实现二维绘制、状态栈与只读呈现数据的统一契约。
pub trait Canvas2D {
    // ── 仿射变换 ──

    /// 返回当前仿射变换。
    fn current_transform(&self) -> Transform;

    /// 替换当前仿射变换。
    fn set_transform(&mut self, transform: Transform);

    /// Concatenates `transform` after the current local coordinates.
    fn concat_transform(&mut self, transform: Transform) {
        self.set_transform(self.current_transform().concat(transform));
    }

    // ── 画布偏移（像素空间平移）──

    /// 当前像素偏移量（影响所有绘制操作的坐标）。
    fn offset(&self) -> (f32, f32) {
        (0.0, 0.0)
    }

    /// 设置像素偏移量。
    fn set_offset(&mut self, _dx: f32, _dy: f32) {}

    /// 累加像素偏移。
    fn translate(&mut self, dx: f32, dy: f32) {
        let (ox, oy) = self.offset();
        self.set_offset(ox + dx, oy + dy);
    }

    // ── 矢量填充（后端必须显式实现，禁止从公开 trait 直写像素）──
    /// 使用纯色填充矩形或圆角矩形。
    fn fill_rect(&mut self, rect: Rect, color: Color, radius: Option<Radius>);
    /// 使用纯色填充圆形。
    fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color);
    /// 使用纯色填充椭圆。
    fn fill_ellipse(&mut self, rect: Rect, color: Color);
    /// 使用纯色填充圆扇形。
    fn fill_sector(
        &mut self,
        cx: f32,
        cy: f32,
        r: f32,
        start_angle: f32,
        end_angle: f32,
        color: Color,
    );
    /// 按填充规则使用纯色填充路径。
    fn fill_path(&mut self, path: &Path, color: Color, fill_rule: FillRule);

    // ── 矢量描边 ──
    /// 使用指定线宽描边矩形或圆角矩形。
    fn stroke_rect(&mut self, rect: Rect, color: Color, line_width: f32, radius: Option<Radius>);
    /// 使用指定线宽描边圆形。
    fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color, line_width: f32);
    /// 按描边选项绘制路径。
    fn stroke_path(&mut self, path: &Path, color: Color, opts: &StrokeOptions);
    /// 绘制两个坐标之间的线段。
    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, width: f32);

    // ── 完整几何 API（统一下沉到现有填充/描边执行器）──

    /// 绘制一个圆形点标记；`diameter` 为逻辑像素直径。
    fn draw_point(&mut self, point: Point, color: Color, diameter: f32) {
        if diameter.is_finite() && diameter > 0.0 {
            self.fill_circle(point.x, point.y, diameter * 0.5, color);
        }
    }

    /// 显式填充圆角矩形。
    fn fill_rounded_rect(&mut self, rect: Rect, color: Color, radius: Radius) {
        self.fill_rect(rect, color, Some(radius));
    }

    /// 显式描边圆角矩形。
    fn stroke_rounded_rect(&mut self, rect: Rect, color: Color, line_width: f32, radius: Radius) {
        self.stroke_rect(rect, color, line_width, Some(radius));
    }

    /// 填充任意多边形。
    fn fill_polygon(&mut self, points: &[Point], color: Color, fill_rule: FillRule) {
        let path = PathBuilder::new().polygon(points).build();
        if !path.is_empty() {
            self.fill_path(&path, color, fill_rule);
        }
    }

    /// 描边开放折线；整条折线只进入一次共享描边管线。
    fn stroke_polyline(&mut self, points: &[Point], color: Color, options: &StrokeOptions) {
        let path = PathBuilder::new().polyline(points).build();
        if !path.is_empty() {
            self.stroke_path(&path, color, options);
        }
    }

    /// 描边闭合多边形。
    fn stroke_polygon(&mut self, points: &[Point], color: Color, options: &StrokeOptions) {
        let path = PathBuilder::new().polygon(points).build();
        if !path.is_empty() {
            self.stroke_path(&path, color, options);
        }
    }

    /// 描边椭圆。
    fn stroke_ellipse(&mut self, rect: Rect, color: Color, options: &StrokeOptions) {
        let path = PathBuilder::new().ellipse(rect).build();
        if !path.is_empty() {
            self.stroke_path(&path, color, options);
        }
    }

    // ── 渐变 ──
    /// 使用两种颜色填充线性渐变矩形。
    fn fill_linear_gradient(
        &mut self,
        rect: Rect,
        color_a: Color,
        color_b: Color,
        dir: GradientDirection,
    );
    /// 使用内外颜色填充径向渐变。
    fn fill_radial_gradient(
        &mut self,
        cx: f32,
        cy: f32,
        inner_r: f32,
        outer_r: f32,
        inner_color: Color,
        outer_color: Color,
    );
    /// 填充带四角圆角裁剪的线性渐变（S4）。
    ///
    /// 半径按相邻和规则归一化后与渐变在同一局部空间掩码；零半径等价
    /// [`Canvas2D::fill_linear_gradient`]。默认实现退回无圆角路径，仅
    /// 供不参与 UIX 渲染的外部画布保持可编译。
    fn fill_linear_gradient_rounded(
        &mut self,
        rect: Rect,
        color_a: Color,
        color_b: Color,
        dir: GradientDirection,
        radius: Radius,
    ) {
        let _ = radius;
        self.fill_linear_gradient(rect, color_a, color_b, dir);
    }
    /// 填充带四角圆角裁剪的径向渐变（S4）。
    ///
    /// `clip_rect` 是圆角掩码的参考矩形（调用方的背景盒）；零半径等价
    /// [`Canvas2D::fill_radial_gradient`]。默认实现退回无圆角路径。
    fn fill_radial_gradient_rounded(
        &mut self,
        cx: f32,
        cy: f32,
        inner_r: f32,
        outer_r: f32,
        inner_color: Color,
        outer_color: Color,
        clip_rect: Rect,
        radius: Radius,
    ) {
        let _ = (clip_rect, radius);
        self.fill_radial_gradient(cx, cy, inner_r, outer_r, inner_color, outer_color);
    }

    // ── 阴影 ──
    /// 绘制单个矩形投影。
    fn draw_box_shadow(
        &mut self,
        rect: Rect,
        blur_radius: f32,
        offset_x: f32,
        offset_y: f32,
        color: Color,
        corner_radius: Option<Radius>,
    );
    /// 绘制环境光风格的矩形投影。
    fn draw_box_shadow_ambient(
        &mut self,
        rect: Rect,
        blur_radius: f32,
        offset_x: f32,
        offset_y: f32,
        color: Color,
        corner_radius: Option<Radius>,
    );

    // ── 图像/字形混合 ──
    /// 将源像素区域复制并缩放到目标矩形。
    fn blit_image(&mut self, src: &[u32], src_w: i32, src_rect: Rect, dst_rect: Rect);

    /// 共享不可变图片像素；同步软件后端默认借用，帧录制与 GPU 后端可保留所有权。
    fn blit_image_shared(
        &mut self,
        src: std::sync::Arc<Vec<u32>>,
        src_w: i32,
        src_rect: Rect,
        dst_rect: Rect,
    ) {
        self.blit_image(src.as_slice(), src_w, src_rect, dst_rect);
    }

    /// 使用覆盖率蒙版绘制一个字形。
    fn blit_glyph(
        &mut self,
        x: i32,
        y: i32,
        coverage: &[u8],
        width: usize,
        height: usize,
        color: Color,
    );

    /// 共享 coverage 的字形复制；默认转发到 [`Self::blit_glyph`]。
    /// 原生 GPU backend 可保留 `Arc`，无需复制 coverage 字节（#105）。
    fn blit_glyph_shared(
        &mut self,
        x: i32,
        y: i32,
        coverage: std::sync::Arc<[u8]>,
        width: usize,
        height: usize,
        color: Color,
    ) {
        self.blit_glyph(x, y, coverage.as_ref(), width, height, color);
    }

    /// GPU 优先：轮廓边列表 → coverage atlas。
    /// soft / 严格 GPU 物理 1:1 优先用字体光栅器的面积 coverage（R8）；
    /// 明显缩放、仿射或高 DPR 走 RGBA8 MSDF。
    fn blit_glyph_outline(
        &mut self,
        x: i32,
        y: i32,
        mesh: std::sync::Arc<[f32]>,
        width: usize,
        height: usize,
        color: Color,
    ) {
        self.blit_glyph_outline_shared(x, y, mesh, None, width, height, color);
    }

    /// 与 [`Self::blit_glyph_outline`] 相同，可携带缓存的面积 coverage（物理 1:1 时复用，
    /// 稳定 atlas 键）。
    #[allow(
        clippy::too_many_arguments,
        reason = "glyph atlas geometry is an established canvas backend boundary"
    )]
    fn blit_glyph_outline_shared(
        &mut self,
        x: i32,
        y: i32,
        mesh: std::sync::Arc<[f32]>,
        area_coverage: Option<std::sync::Arc<[u8]>>,
        width: usize,
        height: usize,
        color: Color,
    ) {
        if width == 0 || height == 0 {
            return;
        }
        let coverage = area_coverage.filter(|c| c.len() >= width.saturating_mul(height));
        let coverage = match coverage {
            Some(c) => c,
            None => {
                let Some(generated) =
                    crate::draw::resources::font::glyph_outline::coverage_from_edges(
                        mesh.as_ref(),
                        width,
                        height,
                    )
                else {
                    return;
                };
                std::sync::Arc::<[u8]>::from(generated)
            }
        };
        self.blit_glyph_shared(x, y, coverage, width, height, color);
    }

    // ── 渲染状态栈（必须自行实现）──
    /// 保存当前画布状态。
    fn save(&mut self);
    /// 恢复最近保存的画布状态。
    fn restore(&mut self);
    /// 将矩形裁剪压入裁剪栈。
    fn push_clip(&mut self, rect: Rect);
    /// 弹出最近压入的裁剪。
    fn pop_clip(&mut self);
    /// 设置后续绘制使用的全局透明度。
    fn set_opacity(&mut self, opacity: f32);
    /// 返回当前全局透明度。
    fn opacity(&self) -> f32;
    /// 设置后续绘制使用的混合模式。
    fn set_blend_mode(&mut self, mode: BlendMode);
    /// 将路径裁剪压入裁剪栈。
    fn push_clip_path(&mut self, path: &Path);

    // ── 只读呈现数据；可变像素仅存在于引擎内部表面 ──
    /// 借用当前表面的只读像素。
    fn pixels(&self) -> &[u32];
    #[cfg(test)]
    canvas2d_pixels_mut_decl!();
    /// 返回当前表面的物理尺寸。
    fn surface_size(&self) -> Size;
    /// 返回当前表面的物理宽度。
    fn width(&self) -> i32 {
        self.surface_size().w as i32
    }
    /// 返回当前表面的物理高度。
    fn height(&self) -> i32 {
        self.surface_size().h as i32
    }
    /// 返回当前生效的矩形裁剪边界。
    fn current_clip(&self) -> Rect;

    // ── 像素移动（滚动优化）──

    /// 在画布上移动一个矩形区域内的像素（scroll/pan 优化）。
    /// 将 viewport 区域内的像素从 `(viewport.x-dx, viewport.y-dy)` 复制到
    /// `(viewport.x, viewport.y)`，避免全帧重绘。dx/dy 应是整数像素偏移。
    /// 调用此方法后，viewport 中非 strip 区域的内容已正确偏移，
    /// 调用者只需重绘新暴露的 strip 区域（与滚动方向相反的一侧）。
    fn scroll_region(&mut self, viewport: Rect, dx: f32, dy: f32);
}

// pixels_mut 测试声明宏位于 tests-src，仅测试构建编译。
#[cfg(test)]
#[path = "../../../tests-src/draw/painting/canvas2d_test_macros.rs"]
pub(crate) mod canvas2d_test_macros;
#[cfg(test)]
use canvas2d_test_macros::canvas2d_pixels_mut_decl;
