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
    FillRect {
        rect: Rect,
        color: Color,
        radius: Option<Radius>,
    },
    StrokeRect {
        rect: Rect,
        color: Color,
        line_width: f32,
        radius: Option<Radius>,
    },
    FillCircle {
        cx: f32,
        cy: f32,
        r: f32,
        color: Color,
    },
    FillEllipse {
        rect: Rect,
        color: Color,
    },
    FillSector {
        cx: f32,
        cy: f32,
        r: f32,
        start_angle: f32,
        end_angle: f32,
        color: Color,
    },
    FillPath {
        path: Path,
        color: Color,
        fill_rule: FillRule,
    },
    StrokeCircle {
        cx: f32,
        cy: f32,
        r: f32,
        color: Color,
        line_width: f32,
    },
    StrokePath {
        path: Path,
        color: Color,
        options: StrokeOptions,
    },
    DrawLine {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: Color,
        width: f32,
    },
    DrawBoxShadow {
        rect: Rect,
        blur_radius: f32,
        offset_x: f32,
        offset_y: f32,
        color: Color,
        corner_radius: Option<Radius>,
    },
    DrawBoxShadowAmbient {
        rect: Rect,
        blur_radius: f32,
        offset_x: f32,
        offset_y: f32,
        color: Color,
        corner_radius: Option<Radius>,
    },
    DrawText {
        text: Arc<str>,
        pos: Point,
        color: Color,
        font_size: f32,
    },
    TextCenter {
        text: Arc<str>,
        rect: Rect,
        color: Color,
        font_size: f32,
    },
    DrawTextInFrame {
        text: Arc<str>,
        rect: Rect,
        color: Color,
        font_size: f32,
    },
    DrawTextBaseline {
        text: Arc<str>,
        x: f32,
        baseline_y: f32,
        color: Color,
        font_size: f32,
    },
    DrawTextWrapped {
        text: Arc<str>,
        rect: Rect,
        color: Color,
        font_size: f32,
    },
    DrawTextWithSelection {
        text: Arc<str>,
        pos: Point,
        color: Color,
        font_size: f32,
        selection: Option<(usize, usize)>,
        selection_bg: Color,
    },
    BlitGlyphLayout {
        layout: crate::draw::resources::font::text_backend::TextLayout,
        pos: Point,
        color: Color,
        font_size: f32,
    },
    SetFont {
        font: FontHandle,
    },
    FillLinearGradient {
        rect: Rect,
        color_a: Color,
        color_b: Color,
        dir: GradientDirection,
    },
    FillRadialGradient {
        cx: f32,
        cy: f32,
        inner_r: f32,
        outer_r: f32,
        inner_color: Color,
        outer_color: Color,
    },
    DrawImage {
        handle: BitmapHandle,
        bounds: Rect,
        fit: bool,
    },
    PushClip {
        rect: Rect,
    },
    PushClipPath {
        path: Path,
    },
    PopClip,
    Translate {
        dx: f32,
        dy: f32,
    },
    SetTransform {
        transform: Transform,
    },
    SetOpacity {
        opacity: f32,
    },
    SetBlendMode {
        mode: BlendMode,
    },
    Save,
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

    /// 列表是否不含任何绘制指令。
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// 指令数量。
    pub fn len(&self) -> usize {
        self.ops.len()
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
                } => ctx.blit_glyph_layout(layout, *pos, *color, *font_size),
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
