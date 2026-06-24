//! Timeline widget — 时间线组件，Ant Design 风格。
//!
//! 垂直时间轴展示事件序列，支持节点颜色、标签、描述。

use uix_core::{Point, Rect, Size};
use crate::define_widget;
use uix_graphics::{Color, Radius};
use crate::render_context::RenderContext;
use crate::widget::{EventResult, WidgetEvent, WidgetTree};

/// 时间线节点。
#[derive(Debug, Clone)]
pub struct TimelineItem {
    pub color: Color,
    pub label: String,
    pub description: String,
}

/// Timeline — 时间线组件。
define_widget! {
    pub struct Timeline {
        items: Vec<TimelineItem>,
        pending: bool,
        reverse: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        let h = self.items.len() as f32 * 60.0;
        Size::new(400.0, h.max(60.0))
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let loc = crate::locale::use_locale();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let border = ctx.tokens().color_border_secondary();
        let dot_r = 5.0;
        let line_x = frame.x + 16.0;
        let content_x = frame.x + 40.0;
        let items: Vec<&TimelineItem> = if self.reverse { self.items.iter().rev().collect() } else { self.items.iter().collect() };

        for (i, item) in items.iter().enumerate() {
            let y = frame.y + i as f32 * 60.0;
            // 垂直连接线
            if i > 0 {
                ctx.fill_rect(Rect::new(line_x - 1.0, y - 30.0, 2.0, 30.0), border, None);
            }
            if i < items.len() - 1 {
                ctx.fill_rect(Rect::new(line_x - 1.0, y + dot_r, 2.0, 30.0 - dot_r), border, None);
            }
            // 节点圆点
            let dot_color = item.color;
            ctx.fill_circle(line_x, y + 15.0, dot_r, dot_color);
            ctx.engine().stroke_circle(line_x, y + 15.0, dot_r, Color::white(), 2.0);
            // 标签
            ctx.draw_text(&item.label, Point::new(content_x, y + 5.0), text, 14.0);
            // 描述
            if !item.description.is_empty() {
                ctx.draw_text(&item.description, Point::new(content_x, y + 24.0), text_sec, 12.0);
            }
        }

        // Pending 节点
        if self.pending {
            let y = frame.y + items.len() as f32 * 60.0;
            ctx.engine().stroke_circle(line_x, y + 15.0, dot_r, border, 2.0);
            ctx.draw_text(loc.timeline_pending, Point::new(content_x, y + 5.0), text_sec, 14.0);
        }
    }
}

impl Timeline {
    pub fn new() -> Self { Self { items: Vec::new(), pending: false, reverse: false } }
    pub fn items(mut self, items: Vec<TimelineItem>) -> Self { self.items = items; self }
    pub fn add(mut self, item: TimelineItem) -> Self { self.items.push(item); self }
    pub fn pending(mut self, v: bool) -> Self { self.pending = v; self }
    pub fn reverse(mut self, v: bool) -> Self { self.reverse = v; self }
}

impl Default for Timeline { fn default() -> Self { Self::new() } }

impl TimelineItem {
    pub fn new(label: &str) -> Self {
        Self { color: Color::from_rgba(22, 119, 255, 255), label: label.to_string(), description: String::new() }
    }
    pub fn description(mut self, d: &str) -> Self { self.description = d.to_string(); self }
    pub fn color(mut self, c: Color) -> Self { self.color = c; self }
}

impl Default for TimelineItem {
    fn default() -> Self { Self::new("") }
}
