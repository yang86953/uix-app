//! Timeline widget — 时间线组件，Ant Design 风格。
//!
//! 垂直时间轴展示事件序列，支持节点颜色、标签、描述。

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::api::PaintContext;
use crate::draw::Color;
use crate::ui::core::widget::WidgetTree;
use crate::ui::SnapshotFields;

const ITEM_HEIGHT: f32 = 60.0;
const MIN_ITEM_HEIGHT: f32 = 28.0;
const DOT_RADIUS: f32 = 5.0;

#[derive(Debug, Clone, Copy)]
struct TimelineGeometry {
    frame: Rect,
    row_height: f32,
    line_x: f32,
    content_x: f32,
    content_width: f32,
    dot_radius: f32,
}

/// 时间线节点。
#[derive(Debug, Clone, PartialEq)]
pub struct TimelineItem {
    pub color: Color,
    pub label: String,
    pub description: String,
}

// Timeline — 时间线组件。
component! {
    pub struct Timeline {
        items: Vec<TimelineItem>,
        pending: bool,
        reverse: bool,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let loc = crate::ui::locale::use_locale();
        let row_count = self.row_count();
        let geometry = TimelineGeometry::new(frame, row_count);
        if row_count == 0 || geometry.frame.w <= 0.0 || geometry.frame.h <= 0.0 {
            return;
        }
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let border = ctx.tokens().color_border_secondary();
        let dot_border = ctx.tokens().color_bg_container();
        let visible_rows = ((geometry.frame.h / geometry.row_height).ceil() as usize)
            .max(1)
            .min(row_count);
        let mut previous_dot_y = None;

        ctx.push_clip(geometry.frame);
        for index in 0..visible_rows {
            let row = geometry.row(index);
            let (label, description, dot_color, pending) = if index < self.items.len() {
                let item = self.item_at(index);
                (
                    item.label.as_str(),
                    item.description.as_str(),
                    item.color,
                    false,
                )
            } else {
                (loc.timeline_pending, "", border, true)
            };
            let text_geometry = row_text_geometry(ctx, row, !description.is_empty());
            let dot_y = text_geometry.dot_y;
            if let Some(previous_dot_y) = previous_dot_y {
                draw_connector(
                    ctx,
                    geometry.line_x,
                    previous_dot_y,
                    dot_y,
                    geometry.dot_radius,
                    border,
                );
            }
            if pending {
                ctx.stroke_circle(
                    geometry.line_x,
                    dot_y,
                    geometry.dot_radius,
                    border,
                    geometry.dot_radius.min(2.0),
                );
            } else {
                ctx.fill_circle(
                    geometry.line_x,
                    dot_y,
                    geometry.dot_radius,
                    dot_color,
                );
                ctx.stroke_circle(
                    geometry.line_x,
                    dot_y,
                    geometry.dot_radius,
                    dot_border,
                    geometry.dot_radius.min(2.0),
                );
            }
            paint_single_line(
                ctx,
                label,
                Rect::new(
                    geometry.content_x,
                    text_geometry.label_y,
                    geometry.content_width,
                    text_geometry.label_height,
                ),
                if pending { text_sec } else { text },
                text_geometry.label_font,
            );
            if !description.is_empty() && text_geometry.description_height > 0.0 {
                paint_single_line(
                    ctx,
                    description,
                    Rect::new(
                        geometry.content_x,
                        text_geometry.description_y,
                        geometry.content_width,
                        text_geometry.description_height,
                    ),
                    text_sec,
                    text_geometry.description_font,
                );
            }
            previous_dot_y = Some(dot_y);
        }
        ctx.pop_clip();
    }
}

impl Timeline {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            pending: false,
            reverse: false,
        }
    }
    pub fn items(mut self, items: Vec<TimelineItem>) -> Self {
        self.items = items;
        self
    }
    #[allow(
        clippy::should_implement_trait,
        reason = "add is the established fluent builder API, not arithmetic addition"
    )]
    pub fn add(mut self, item: TimelineItem) -> Self {
        self.items.push(item);
        self
    }
    pub fn pending(mut self, v: bool) -> Self {
        self.pending = v;
        self
    }
    pub fn reverse(mut self, v: bool) -> Self {
        self.reverse = v;
        self
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(400.0, self.row_count().max(1) as f32 * ITEM_HEIGHT)
    }

    fn row_count(&self) -> usize {
        self.items.len() + usize::from(self.pending)
    }

    fn item_at(&self, index: usize) -> &TimelineItem {
        if self.reverse {
            &self.items[self.items.len() - 1 - index]
        } else {
            &self.items[index]
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.items = next.items;
        self.pending = next.pending;
        self.reverse = next.reverse;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Timeline {
            items: self.items.clone(),
            pending: self.pending,
            reverse: self.reverse,
        }
    }
}

impl TimelineGeometry {
    fn new(frame: Rect, row_count: usize) -> Self {
        let frame = Rect::new(
            frame.x,
            frame.y,
            if frame.w.is_finite() {
                frame.w.max(0.0)
            } else {
                0.0
            },
            if frame.h.is_finite() {
                frame.h.max(0.0)
            } else {
                0.0
            },
        );
        let fitted_height = if row_count == 0 {
            ITEM_HEIGHT
        } else {
            frame.h / row_count as f32
        };
        let row_height = fitted_height.clamp(MIN_ITEM_HEIGHT, ITEM_HEIGHT);
        let gutter = (frame.w * 0.5).min(40.0);
        let line_x = frame.x + gutter * 0.4;
        let content_x = frame.x + gutter;
        let right_padding = (frame.w * 0.1).min(8.0);
        let content_width = (frame.x + frame.w - right_padding - content_x).max(0.0);
        let dot_radius = DOT_RADIUS
            .min(row_height * 0.18)
            .min((gutter * 0.18).max(0.0));
        Self {
            frame,
            row_height,
            line_x,
            content_x,
            content_width,
            dot_radius,
        }
    }

    fn row(self, index: usize) -> Rect {
        Rect::new(
            self.frame.x,
            self.frame.y + index as f32 * self.row_height,
            self.frame.w,
            self.row_height,
        )
    }
}

#[derive(Debug, Clone, Copy)]
struct RowTextGeometry {
    label_y: f32,
    label_height: f32,
    label_font: f32,
    description_y: f32,
    description_height: f32,
    description_font: f32,
    dot_y: f32,
}

fn row_text_geometry(
    ctx: &mut PaintContext<'_>,
    row: Rect,
    has_description: bool,
) -> RowTextGeometry {
    let scale = (row.h / ITEM_HEIGHT).clamp(0.75, 1.0);
    let label_font = 14.0 * scale;
    let description_font = 12.0 * scale;
    let label_height = ctx.line_box_height(label_font).min(row.h);
    let gap = if has_description { scale } else { 0.0 };
    let description_height = if has_description {
        ctx.line_box_height(description_font)
            .min((row.h - label_height - gap).max(0.0))
    } else {
        0.0
    };
    let block_height = label_height + gap + description_height;
    let label_y = row.y + ((row.h - block_height) * 0.5).max(0.0);
    let description_y = label_y + label_height + gap;
    RowTextGeometry {
        label_y,
        label_height,
        label_font,
        description_y,
        description_height,
        description_font,
        dot_y: (label_y + label_height * 0.5).clamp(row.y, row.y + row.h),
    }
}

fn draw_connector(
    ctx: &mut PaintContext<'_>,
    line_x: f32,
    previous_dot_y: f32,
    dot_y: f32,
    dot_radius: f32,
    color: Color,
) {
    let start_y = previous_dot_y + dot_radius;
    let end_y = dot_y - dot_radius;
    if end_y > start_y {
        ctx.fill_rect(
            Rect::new(line_x - 1.0, start_y, 2.0, end_y - start_y),
            color,
            None,
        );
    }
}

fn paint_single_line(
    ctx: &mut PaintContext<'_>,
    value: &str,
    frame: Rect,
    color: Color,
    font_size: f32,
) {
    if frame.w <= 0.0 || frame.h <= 0.0 {
        return;
    }
    let Some(value) = elide_single_line(ctx, value, font_size, frame.w) else {
        return;
    };
    ctx.push_clip(frame);
    ctx.draw_text_in_frame(&value, frame, color, font_size);
    ctx.pop_clip();
}

fn elide_single_line(
    ctx: &mut PaintContext<'_>,
    value: &str,
    font_size: f32,
    max_width: f32,
) -> Option<String> {
    if !max_width.is_finite() || max_width <= 0.0 {
        return None;
    }
    let value = value.replace(['\r', '\n'], " ");
    if text_width(ctx, &value, font_size) <= max_width {
        return Some(value);
    }
    const ELLIPSIS: &str = "…";
    if text_width(ctx, ELLIPSIS, font_size) > max_width {
        return None;
    }
    let mut visible = String::new();
    for ch in value.chars() {
        visible.push(ch);
        visible.push_str(ELLIPSIS);
        let fits = text_width(ctx, &visible, font_size) <= max_width;
        visible.pop();
        if !fits {
            visible.pop();
            break;
        }
    }
    visible.push_str(ELLIPSIS);
    Some(visible)
}

fn text_width(ctx: &mut PaintContext<'_>, value: &str, font_size: f32) -> f32 {
    ctx.measure_text(value, font_size).w.max(
        crate::draw::resources::font::text_backend::estimate_text_metrics(
            value,
            f32::INFINITY,
            font_size,
        )
        .max_line_width,
    )
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}

impl TimelineItem {
    pub fn new(label: &str) -> Self {
        Self {
            color: Color::from_rgba(22, 119, 255, 255),
            label: label.to_string(),
            description: String::new(),
        }
    }
    pub fn description(mut self, d: &str) -> Self {
        self.description = d.to_string();
        self
    }
    pub fn color(mut self, c: Color) -> Self {
        self.color = c;
        self
    }
}

impl Default for TimelineItem {
    fn default() -> Self {
        Self::new("")
    }
}
