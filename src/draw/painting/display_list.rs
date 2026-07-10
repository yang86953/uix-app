//! DisplayList — 可录制、可重放的绘制指令序列（Phase 7）。
//!
//! Widget 通过 `PaintContext` 录制绘制指令；Picture 离屏缓存与合成器重放时使用。

use crate::core::{Point, Rect};

use crate::draw::font::text::TextRenderService;
use crate::draw::image::{blit_handle, BitmapHandle, ImageService};
use crate::draw::painting::PaintContext;
use crate::draw::traits::Canvas2D;
use crate::draw::{Color, FontHandle, GradientDirection, Radius};

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
    DrawBoxShadow {
        rect: Rect,
        blur_radius: f32,
        offset_x: f32,
        offset_y: f32,
        color: Color,
        corner_radius: Option<Radius>,
    },
    DrawText {
        text: String,
        pos: Point,
        color: Color,
        font_size: f32,
    },
    TextCenter {
        text: String,
        rect: Rect,
        color: Color,
        font_size: f32,
    },
    DrawTextInFrame {
        text: String,
        rect: Rect,
        color: Color,
        font_size: f32,
    },
    BlitGlyphLayout {
        layout: crate::draw::font::text_backend::TextLayout,
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
    DrawImage {
        handle: BitmapHandle,
        bounds: Rect,
        fit: bool,
    },
    PushClip {
        rect: Rect,
    },
    PopClip,
    Save,
    Restore,
}

/// 可重放的绘制指令列表。
#[derive(Debug, Clone, Default)]
pub struct DisplayList {
    ops: Vec<PaintOp>,
}

impl DisplayList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    pub fn len(&self) -> usize {
        self.ops.len()
    }

    pub fn push(&mut self, op: PaintOp) {
        self.ops.push(op);
    }

    /// 重放到 `PaintContext`（不再二次录制）。
    pub fn replay(&self, ctx: &mut PaintContext<'_>) {
        ctx.set_record_ops(false);
        for op in &self.ops {
            match op {
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
                PaintOp::BlitGlyphLayout {
                    layout,
                    pos,
                    color,
                    font_size,
                } => ctx.blit_glyph_layout(layout, *pos, *color, *font_size),
                PaintOp::SetFont { font } => ctx.set_font(*font),
                PaintOp::FillLinearGradient {
                    rect,
                    color_a,
                    color_b,
                    dir,
                } => ctx.fill_linear_gradient(*rect, *color_a, *color_b, *dir),
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
                PaintOp::PushClip { rect } => ctx.push_clip(*rect),
                PaintOp::PopClip => ctx.pop_clip(),
                PaintOp::Save => ctx.save(),
                PaintOp::Restore => ctx.restore(),
            }
        }
        ctx.set_record_ops(true);
    }

    /// 重放到原始 Canvas（Picture 离屏回放，不含复杂文本布局）。
    pub fn replay_canvas(
        &self,
        canvas: &mut dyn Canvas2D,
        font: FontHandle,
        font_service: &crate::draw::font::font_service::FontService,
        image_service: Option<&ImageService>,
        surface_w: f32,
    ) {
        let mut text = TextRenderService::new(font, font_service, surface_w);
        for op in &self.ops {
            match op {
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
                PaintOp::BlitGlyphLayout {
                    layout,
                    pos,
                    color,
                    font_size,
                } => text.blit_to(canvas, layout, *pos, *color, *font_size),
                PaintOp::SetFont { font } => {
                    text.set_font(*font);
                }
                PaintOp::FillLinearGradient {
                    rect,
                    color_a,
                    color_b,
                    dir,
                } => canvas.fill_linear_gradient(*rect, *color_a, *color_b, *dir),
                PaintOp::DrawImage {
                    handle,
                    bounds,
                    fit,
                } => {
                    if let Some(svc) = image_service {
                        blit_handle(svc, canvas, *handle, *bounds, *fit);
                    }
                }
                PaintOp::PushClip { rect } => canvas.push_clip(*rect),
                PaintOp::PopClip => canvas.pop_clip(),
                PaintOp::Save => canvas.save(),
                PaintOp::Restore => canvas.restore(),
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/draw/painting/display_list.rs"]
mod tests;
