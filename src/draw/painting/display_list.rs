//! DisplayList — 可录制、可重放的绘制指令序列（Phase 7）。
//!
//! Widget 通过 `PaintContext` 录制绘制指令；Picture 离屏缓存与合成器重放时使用。

use crate::core::{Point, Rect};

use crate::draw::Canvas2D;
use crate::draw::geometry::path::{FillRule, Path};
use crate::draw::geometry::stroker::StrokeOptions;
use crate::draw::painting::PaintContext;
use crate::draw::resources::font::text::TextRenderService;
use crate::draw::resources::image::{BitmapHandle, ImageService, blit_handle};
use crate::draw::{BlendMode, Color, FontHandle, GradientDirection, Radius, Transform};
use std::sync::Arc;

/// 绘制阶段：合成器在子节点前后分别调用 `paint`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaintPass {
    /// 默认：widget 主体内容（背景、chrome）。
    #[default]
    Content,
    /// 子节点绘制完成后：滚动条等覆盖层。
    AfterChildren,
}

/// 单条绘制指令（与 `PaintContext` 常用 API 对齐）。
#[derive(Debug, Clone)]
pub enum PaintOp {
    /// 填充矩形或圆角矩形。
    FillRect {
        /// 目标逻辑矩形。
        rect: Rect,
        /// 填充颜色。
        color: Color,
        /// 可选的四角半径。
        radius: Option<Radius>,
    },
    /// 描边矩形或圆角矩形。
    StrokeRect {
        /// 目标逻辑矩形。
        rect: Rect,
        /// 描边颜色。
        color: Color,
        /// 描边线宽。
        line_width: f32,
        /// 可选的四角半径。
        radius: Option<Radius>,
    },
    /// 填充圆形。
    FillCircle {
        /// 圆心水平坐标。
        cx: f32,
        /// 圆心垂直坐标。
        cy: f32,
        /// 圆形半径。
        r: f32,
        /// 填充颜色。
        color: Color,
    },
    /// 填充椭圆。
    FillEllipse {
        /// 椭圆的外接矩形。
        rect: Rect,
        /// 填充颜色。
        color: Color,
    },
    /// 填充圆扇形。
    FillSector {
        /// 圆心水平坐标。
        cx: f32,
        /// 圆心垂直坐标。
        cy: f32,
        /// 圆形半径。
        r: f32,
        /// 起始角度。
        start_angle: f32,
        /// 结束角度。
        end_angle: f32,
        /// 填充颜色。
        color: Color,
    },
    /// 按规则填充路径。
    FillPath {
        /// 要填充的路径。
        path: Path,
        /// 填充颜色。
        color: Color,
        /// 路径填充规则。
        fill_rule: FillRule,
    },
    /// 描边圆形。
    StrokeCircle {
        /// 圆心水平坐标。
        cx: f32,
        /// 圆心垂直坐标。
        cy: f32,
        /// 圆形半径。
        r: f32,
        /// 描边颜色。
        color: Color,
        /// 描边线宽。
        line_width: f32,
    },
    /// 按描边选项绘制路径。
    StrokePath {
        /// 要描边的路径。
        path: Path,
        /// 描边颜色。
        color: Color,
        /// 描边样式选项。
        options: StrokeOptions,
    },
    /// 绘制一条线段。
    DrawLine {
        /// 起点水平坐标。
        x1: f32,
        /// 起点垂直坐标。
        y1: f32,
        /// 终点水平坐标。
        x2: f32,
        /// 终点垂直坐标。
        y2: f32,
        /// 线段颜色。
        color: Color,
        /// 线段宽度。
        width: f32,
    },
    /// 绘制方向性矩形阴影。
    DrawBoxShadow {
        /// 产生阴影的逻辑矩形。
        rect: Rect,
        /// 阴影模糊半径。
        blur_radius: f32,
        /// 阴影水平偏移。
        offset_x: f32,
        /// 阴影垂直偏移。
        offset_y: f32,
        /// 阴影颜色。
        color: Color,
        /// 可选的四角半径。
        corner_radius: Option<Radius>,
    },
    /// 绘制环境光矩形阴影。
    DrawBoxShadowAmbient {
        /// 产生阴影的逻辑矩形。
        rect: Rect,
        /// 阴影模糊半径。
        blur_radius: f32,
        /// 阴影水平偏移。
        offset_x: f32,
        /// 阴影垂直偏移。
        offset_y: f32,
        /// 阴影颜色。
        color: Color,
        /// 可选的四角半径。
        corner_radius: Option<Radius>,
    },
    /// 从指定位置绘制单行文本。
    DrawText {
        /// 共享文本内容。
        text: Arc<str>,
        /// 文本起始位置。
        pos: Point,
        /// 文本颜色。
        color: Color,
        /// 字体大小。
        font_size: f32,
    },
    /// 在矩形内居中绘制文本。
    TextCenter {
        /// 共享文本内容。
        text: Arc<str>,
        /// 文本布局矩形。
        rect: Rect,
        /// 文本颜色。
        color: Color,
        /// 字体大小。
        font_size: f32,
    },
    /// 在指定框内绘制文本。
    DrawTextInFrame {
        /// 共享文本内容。
        text: Arc<str>,
        /// 文本布局矩形。
        rect: Rect,
        /// 文本颜色。
        color: Color,
        /// 字体大小。
        font_size: f32,
    },
    /// 按基线位置绘制单行文本。
    DrawTextBaseline {
        /// 共享文本内容。
        text: Arc<str>,
        /// 文本起点水平坐标。
        x: f32,
        /// 文本基线垂直坐标。
        baseline_y: f32,
        /// 文本颜色。
        color: Color,
        /// 字体大小。
        font_size: f32,
    },
    /// 在矩形内自动换行绘制文本。
    DrawTextWrapped {
        /// 共享文本内容。
        text: Arc<str>,
        /// 文本布局矩形。
        rect: Rect,
        /// 文本颜色。
        color: Color,
        /// 字体大小。
        font_size: f32,
    },
    /// 绘制文本及可选的选择范围背景。
    DrawTextWithSelection {
        /// 共享文本内容。
        text: Arc<str>,
        /// 文本起始位置。
        pos: Point,
        /// 文本颜色。
        color: Color,
        /// 字体大小。
        font_size: f32,
        /// 选择范围的字节索引区间。
        selection: Option<(usize, usize)>,
        /// 选择范围背景色。
        selection_bg: Color,
    },
    /// 重放预布局的字形序列。
    BlitGlyphLayout {
        /// 共享已完成塑形与布局的文本结果。
        layout: Arc<crate::draw::resources::font::text_backend::TextLayout>,
        /// 字形布局起始位置。
        pos: Point,
        /// 字形颜色。
        color: Color,
        /// 请求的字体大小。
        font_size: f32,
    },
    /// 设置后续文本指令使用的字体。
    SetFont {
        /// 后端字体句柄。
        font: FontHandle,
    },
    /// 填充线性渐变矩形。
    FillLinearGradient {
        /// 目标逻辑矩形。
        rect: Rect,
        /// 渐变起始颜色。
        color_a: Color,
        /// 渐变结束颜色。
        color_b: Color,
        /// 渐变方向。
        dir: GradientDirection,
    },
    /// 填充径向渐变。
    FillRadialGradient {
        /// 渐变圆心水平坐标。
        cx: f32,
        /// 渐变圆心垂直坐标。
        cy: f32,
        /// 渐变起始半径。
        inner_r: f32,
        /// 渐变结束半径。
        outer_r: f32,
        /// 内侧颜色。
        inner_color: Color,
        /// 外侧颜色。
        outer_color: Color,
    },
    /// 绘制已加载的位图资源。
    DrawImage {
        /// 图像服务中的位图句柄。
        handle: BitmapHandle,
        /// 图像目标边界。
        bounds: Rect,
        /// 是否保持比例适配目标边界。
        fit: bool,
    },
    /// 压入矩形裁剪。
    PushClip {
        /// 新增的逻辑裁剪矩形。
        rect: Rect,
    },
    /// 压入路径裁剪。
    PushClipPath {
        /// 新增的裁剪路径。
        path: Path,
    },
    /// 弹出最近压入的裁剪。
    PopClip,
    /// 平移后续绘制坐标。
    Translate {
        /// 水平平移量。
        dx: f32,
        /// 垂直平移量。
        dy: f32,
    },
    /// 替换后续绘制使用的仿射变换。
    SetTransform {
        /// 新的仿射变换。
        transform: Transform,
    },
    /// 设置后续绘制使用的全局透明度。
    SetOpacity {
        /// 新的透明度值。
        opacity: f32,
    },
    /// 设置后续绘制使用的混合模式。
    SetBlendMode {
        /// 新的混合模式。
        mode: BlendMode,
    },
    /// 保存当前画布状态。
    Save,
    /// 恢复最近保存的画布状态。
    Restore,
}

/// 可重放的绘制指令列表。
#[derive(Debug, Clone, Default)]
pub struct DisplayList {
    ops: Arc<Vec<PaintOp>>,
}

impl DisplayList {
    /// 创建空显示列表。
    pub fn new() -> Self {
        Self::default()
    }

    // 按上一份显示列表的指令数量预留录制空间。
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        // 新列表仍拥有独立 Arc，容量提示只减少后续 Vec 扩容。
        Self {
            ops: Arc::new(Vec::with_capacity(capacity)),
        }
    }

    /// 列表是否不含任何绘制指令。
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// 指令数量。
    pub fn len(&self) -> usize {
        self.ops.len()
    }

    // 测试目标观测容量提示是否进入新列表，不暴露为公共绘制契约。
    #[cfg(test)]
    pub(crate) fn capacity(&self) -> usize {
        self.ops.capacity()
    }

    // 测试目标保留绘制操作只读观测入口，供 display-list 语义测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn ops(&self) -> &[PaintOp] {
        &self.ops
    }

    /// 追加一条绘制指令（共享存储上执行写时复制）。
    pub fn push(&mut self, op: PaintOp) {
        Arc::make_mut(&mut self.ops).push(op);
    }

    // 测试目标保留操作存储共享性观测入口，供写时复制测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn shares_operation_storage_with(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.ops, &other.ops)
    }

    /// 重放到 `PaintContext`（不再二次录制）。
    pub fn replay(&self, ctx: &mut PaintContext<'_>) {
        ctx.with_recording_disabled(|ctx| self.replay_unrecorded(ctx));
    }

    /// 把指令序列逐条应用到 `PaintContext`（调用方已禁用二次录制）。
    fn replay_unrecorded(&self, ctx: &mut PaintContext<'_>) {
        for op in self.ops.iter() {
            match op {
                // 基础图元：矩形、圆、椭圆、扇形的填充与描边，参数与 PaintContext API 一一对应。
                PaintOp::FillRect {
                    rect,
                    color,
                    radius,
                } => ctx.fill_rect(*rect, *color, *radius),
                PaintOp::StrokeRect {
                    rect,
                    color,
                    line_width,
                    radius,
                } => ctx.stroke_rect(*rect, *color, *line_width, *radius),
                PaintOp::FillCircle { cx, cy, r, color } => ctx.fill_circle(*cx, *cy, *r, *color),
                PaintOp::FillEllipse { rect, color } => ctx.fill_ellipse(*rect, *color),
                PaintOp::FillSector {
                    cx,
                    cy,
                    r,
                    start_angle,
                    end_angle,
                    color,
                } => ctx.fill_sector(*cx, *cy, *r, *start_angle, *end_angle, *color),
                // 路径类：按填充规则或描边选项执行。
                PaintOp::FillPath {
                    path,
                    color,
                    fill_rule,
                } => ctx.fill_path(path, *color, *fill_rule),
                PaintOp::StrokeCircle {
                    cx,
                    cy,
                    r,
                    color,
                    line_width,
                } => ctx.stroke_circle(*cx, *cy, *r, *color, *line_width),
                PaintOp::StrokePath {
                    path,
                    color,
                    options,
                } => ctx.stroke_path(path, *color, options),
                // 直线与阴影（含环境阴影）。
                PaintOp::DrawLine {
                    x1,
                    y1,
                    x2,
                    y2,
                    color,
                    width,
                } => ctx.draw_line(*x1, *y1, *x2, *y2, *color, *width),
                PaintOp::DrawBoxShadow {
                    rect,
                    blur_radius,
                    offset_x,
                    offset_y,
                    color,
                    corner_radius,
                } => ctx.draw_box_shadow(
                    *rect,
                    *blur_radius,
                    *offset_x,
                    *offset_y,
                    *color,
                    *corner_radius,
                ),
                PaintOp::DrawBoxShadowAmbient {
                    rect,
                    blur_radius,
                    offset_x,
                    offset_y,
                    color,
                    corner_radius,
                } => ctx.draw_box_shadow_ambient(
                    *rect,
                    *blur_radius,
                    *offset_x,
                    *offset_y,
                    *color,
                    *corner_radius,
                ),
                // 文本类：文字、居中、帧内、基线、换行、选区与字形布局，直接复用 PaintContext 排版。
                PaintOp::DrawText {
                    text,
                    pos,
                    color,
                    font_size,
                } => ctx.draw_text(text, *pos, *color, *font_size),
                PaintOp::TextCenter {
                    text,
                    rect,
                    color,
                    font_size,
                } => ctx.text_center(text, *rect, *color, *font_size),
                PaintOp::DrawTextInFrame {
                    text,
                    rect,
                    color,
                    font_size,
                } => ctx.draw_text_in_frame(text, *rect, *color, *font_size),
                PaintOp::DrawTextBaseline {
                    text,
                    x,
                    baseline_y,
                    color,
                    font_size,
                } => ctx.draw_text_baseline(text, *x, *baseline_y, *color, *font_size),
                PaintOp::DrawTextWrapped {
                    text,
                    rect,
                    color,
                    font_size,
                } => ctx.draw_text_wrapped(text, *rect, *color, *font_size),
                PaintOp::DrawTextWithSelection {
                    text,
                    pos,
                    color,
                    font_size,
                    selection,
                    selection_bg,
                } => ctx.draw_text_with_selection(
                    text,
                    *pos,
                    *color,
                    *font_size,
                    *selection,
                    *selection_bg,
                ),
                PaintOp::BlitGlyphLayout {
                    layout,
                    pos,
                    color,
                    font_size,
                } => ctx.blit_shared_glyph_layout(Arc::clone(layout), *pos, *color, *font_size),
                // 字体状态切换。
                PaintOp::SetFont { font } => ctx.set_font(*font),
                // 渐变：线性与径向。
                PaintOp::FillLinearGradient {
                    rect,
                    color_a,
                    color_b,
                    dir,
                } => ctx.fill_linear_gradient(*rect, *color_a, *color_b, *dir),
                PaintOp::FillRadialGradient {
                    cx,
                    cy,
                    inner_r,
                    outer_r,
                    inner_color,
                    outer_color,
                } => ctx.fill_radial_gradient(
                    *cx,
                    *cy,
                    *inner_r,
                    *outer_r,
                    *inner_color,
                    *outer_color,
                ),
                // 图片：fit 时保持比例绘制，否则拉伸铺满。
                PaintOp::DrawImage {
                    handle,
                    bounds,
                    fit,
                } => {
                    if *fit {
                        ctx.draw_image(*handle, *bounds);
                    } else {
                        ctx.draw_image_fill(*handle, *bounds);
                    }
                }
                // 状态类：裁剪、变换、透明度与混合模式。
                PaintOp::PushClip { rect } => ctx.push_clip(*rect),
                PaintOp::PushClipPath { path } => ctx.push_clip_path(path),
                PaintOp::PopClip => ctx.pop_clip(),
                PaintOp::Translate { dx, dy } => ctx.translate(*dx, *dy),
                PaintOp::SetTransform { transform } => ctx.set_transform(*transform),
                PaintOp::SetOpacity { opacity } => ctx.set_opacity(*opacity),
                PaintOp::SetBlendMode { mode } => ctx.set_blend_mode(*mode),
                // 画布状态栈。
                PaintOp::Save => ctx.save(),
                PaintOp::Restore => ctx.restore(),
            }
        }
    }

    /// 重放到原始 Canvas（Picture 离屏回放，不含复杂文本布局）。
    pub fn replay_canvas(
        &self,
        canvas: &mut dyn Canvas2D,
        font: FontHandle,
        font_service: &crate::draw::resources::font::font_service::FontService,
        image_service: Option<&ImageService>,
        surface_w: f32,
    ) {
        let mut text = TextRenderService::new(font, font_service, surface_w);
        for op in self.ops.iter() {
            match op {
                // 基础图元：矩形、圆、椭圆、扇形的填充与描边，直接落到目标 Canvas。
                PaintOp::FillRect {
                    rect,
                    color,
                    radius,
                } => canvas.fill_rect(*rect, *color, *radius),
                PaintOp::StrokeRect {
                    rect,
                    color,
                    line_width,
                    radius,
                } => canvas.stroke_rect(*rect, *color, *line_width, *radius),
                PaintOp::FillCircle { cx, cy, r, color } => {
                    canvas.fill_circle(*cx, *cy, *r, *color)
                }
                PaintOp::FillEllipse { rect, color } => canvas.fill_ellipse(*rect, *color),
                PaintOp::FillSector {
                    cx,
                    cy,
                    r,
                    start_angle,
                    end_angle,
                    color,
                } => canvas.fill_sector(*cx, *cy, *r, *start_angle, *end_angle, *color),
                // 路径类：按填充规则或描边选项执行。
                PaintOp::FillPath {
                    path,
                    color,
                    fill_rule,
                } => canvas.fill_path(path, *color, *fill_rule),
                PaintOp::StrokeCircle {
                    cx,
                    cy,
                    r,
                    color,
                    line_width,
                } => canvas.stroke_circle(*cx, *cy, *r, *color, *line_width),
                PaintOp::StrokePath {
                    path,
                    color,
                    options,
                } => canvas.stroke_path(path, *color, options),
                // 直线与阴影（含环境阴影）。
                PaintOp::DrawLine {
                    x1,
                    y1,
                    x2,
                    y2,
                    color,
                    width,
                } => canvas.draw_line(*x1, *y1, *x2, *y2, *color, *width),
                PaintOp::DrawBoxShadow {
                    rect,
                    blur_radius,
                    offset_x,
                    offset_y,
                    color,
                    corner_radius,
                } => canvas.draw_box_shadow(
                    *rect,
                    *blur_radius,
                    *offset_x,
                    *offset_y,
                    *color,
                    *corner_radius,
                ),
                PaintOp::DrawBoxShadowAmbient {
                    rect,
                    blur_radius,
                    offset_x,
                    offset_y,
                    color,
                    corner_radius,
                } => canvas.draw_box_shadow_ambient(
                    *rect,
                    *blur_radius,
                    *offset_x,
                    *offset_y,
                    *color,
                    *corner_radius,
                ),
                // 文本类：全部经 TextRenderService 排版后绘制（Picture 离屏回放不含复杂布局状态）。
                PaintOp::DrawText {
                    text: s,
                    pos,
                    color,
                    font_size,
                } => {
                    text.draw_text(canvas, s, *pos, *color, *font_size);
                }
                PaintOp::TextCenter {
                    text: s,
                    rect,
                    color,
                    font_size,
                } => {
                    text.text_center(canvas, s, *rect, *color, *font_size);
                }
                PaintOp::DrawTextInFrame {
                    text: s,
                    rect,
                    color,
                    font_size,
                } => {
                    text.draw_text_in_frame(canvas, s, *rect, *color, *font_size);
                }
                PaintOp::DrawTextBaseline {
                    text: s,
                    x,
                    baseline_y,
                    color,
                    font_size,
                } => text.draw_text_baseline(canvas, s, *x, *baseline_y, *color, *font_size),
                PaintOp::DrawTextWrapped {
                    text: s,
                    rect,
                    color,
                    font_size,
                } => text.draw_text_wrapped(canvas, s, *rect, *color, *font_size),
                PaintOp::DrawTextWithSelection {
                    text: s,
                    pos,
                    color,
                    font_size,
                    selection,
                    selection_bg,
                } => text.draw_text_with_selection(
                    canvas,
                    s,
                    *pos,
                    *color,
                    *font_size,
                    *selection,
                    *selection_bg,
                ),
                PaintOp::BlitGlyphLayout {
                    layout,
                    pos,
                    color,
                    font_size,
                } => text.blit_to(canvas, layout, *pos, *color, *font_size),
                // 字体切换：只影响本函数内创建的 TextRenderService。
                PaintOp::SetFont { font } => {
                    text.set_font(*font);
                }
                // 渐变：线性与径向。
                PaintOp::FillLinearGradient {
                    rect,
                    color_a,
                    color_b,
                    dir,
                } => canvas.fill_linear_gradient(*rect, *color_a, *color_b, *dir),
                PaintOp::FillRadialGradient {
                    cx,
                    cy,
                    inner_r,
                    outer_r,
                    inner_color,
                    outer_color,
                } => canvas.fill_radial_gradient(
                    *cx,
                    *cy,
                    *inner_r,
                    *outer_r,
                    *inner_color,
                    *outer_color,
                ),
                // 图片：需要调用方提供 ImageService；缺失时跳过（与录制路径的 no-op 语义一致）。
                PaintOp::DrawImage {
                    handle,
                    bounds,
                    fit,
                } => {
                    if let Some(svc) = image_service {
                        blit_handle(svc, canvas, *handle, *bounds, *fit);
                    }
                }
                // 状态类：裁剪、变换、透明度与混合模式。
                PaintOp::PushClip { rect } => canvas.push_clip(*rect),
                PaintOp::PushClipPath { path } => canvas.push_clip_path(path),
                PaintOp::PopClip => canvas.pop_clip(),
                PaintOp::Translate { dx, dy } => canvas.translate(*dx, *dy),
                PaintOp::SetTransform { transform } => canvas.set_transform(*transform),
                PaintOp::SetOpacity { opacity } => canvas.set_opacity(*opacity),
                PaintOp::SetBlendMode { mode } => canvas.set_blend_mode(*mode),
                // 画布状态栈。
                PaintOp::Save => canvas.save(),
                PaintOp::Restore => canvas.restore(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::resources::font::text_backend::TextLayout;

    // DisplayList 写时复制不得深拷贝已经共享的字形与行缓冲。
    #[test]
    fn cloned_glyph_op_shares_layout_storage() {
        let layout = Arc::new(TextLayout {
            glyphs: Vec::new(),
            lines: Vec::new(),
            width: 0.0,
            height: 0.0,
        });
        let operation = PaintOp::BlitGlyphLayout {
            layout: Arc::clone(&layout),
            pos: Point::new(1.0, 2.0),
            color: Color::black(),
            font_size: 14.0,
        };

        let cloned = operation.clone();

        let PaintOp::BlitGlyphLayout {
            layout: cloned_layout,
            ..
        } = cloned
        else {
            unreachable!("克隆后的操作类型必须保持不变");
        };
        assert!(Arc::ptr_eq(&layout, &cloned_layout));
    }
}
