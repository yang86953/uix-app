//! Timeline widget — 时间线组件，Ant Design 风格。
//!
//! 垂直时间轴展示事件序列，支持节点颜色、标签、描述。

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Color;
use crate::ui::core::widget::WidgetTree;
use crate::ui::SnapshotFields;

const ITEM_HEIGHT: f32 = 60.0;
const DOT_CENTER_Y: f32 = 15.0;
const DOT_RADIUS: f32 = 5.0;

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

    picture_policy => (&self) -> crate::draw::compositor::PicturePolicy {
        crate::draw::compositor::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let loc = crate::ui::locale::use_locale();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let border = ctx.tokens().color_border_secondary();
        let line_x = frame.x + 16.0;
        let content_x = frame.x + 40.0;
        let items: Vec<&TimelineItem> = if self.reverse { self.items.iter().rev().collect() } else { self.items.iter().collect() };

        for (i, item) in items.iter().enumerate() {
            let y = frame.y + i as f32 * ITEM_HEIGHT;
            // 垂直连接线
            if i > 0 {
                draw_incoming_connector(ctx, line_x, y, border);
            }
            // 节点圆点
            let dot_color = item.color;
            ctx.fill_circle(line_x, y + DOT_CENTER_Y, DOT_RADIUS, dot_color);
            ctx.canvas_2d().stroke_circle(
                line_x,
                y + DOT_CENTER_Y,
                DOT_RADIUS,
                Color::white(),
                2.0,
            );
            // 标签
            ctx.draw_text(&item.label, Point::new(content_x, y + 5.0), text, 14.0);
            // 描述
            if !item.description.is_empty() {
                ctx.draw_text(&item.description, Point::new(content_x, y + 24.0), text_sec, 12.0);
            }
        }

        // Pending 节点
        if self.pending {
            let y = frame.y + items.len() as f32 * ITEM_HEIGHT;
            if !items.is_empty() {
                draw_incoming_connector(ctx, line_x, y, border);
            }
            ctx.canvas_2d().stroke_circle(
                line_x,
                y + DOT_CENTER_Y,
                DOT_RADIUS,
                border,
                2.0,
            );
            ctx.draw_text(loc.timeline_pending, Point::new(content_x, y + 5.0), text_sec, 14.0);
        }
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
        let row_count = self.items.len() + usize::from(self.pending);
        Size::new(400.0, row_count.max(1) as f32 * ITEM_HEIGHT)
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

fn draw_incoming_connector(ctx: &mut PaintContext<'_>, line_x: f32, row_y: f32, color: Color) {
    let start_y = row_y - ITEM_HEIGHT + DOT_CENTER_Y + DOT_RADIUS;
    let end_y = row_y + DOT_CENTER_Y - DOT_RADIUS;
    ctx.fill_rect(
        Rect::new(line_x - 1.0, start_y, 2.0, end_y - start_y),
        color,
        None,
    );
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
