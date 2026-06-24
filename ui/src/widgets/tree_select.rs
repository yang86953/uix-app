//! TreeSelect widget — 树选择器，Ant Design 风格。
//!
//! 结合 Tree 与 Select，弹出树形面板选择。

use uix_core::{Point, Rect, Size};
use crate::define_widget;
use uix_graphics::Radius;
use crate::render_context::RenderContext;
use crate::widget::{EventResult, WidgetEvent, WidgetTree};
use super::tree::TreeNode;

/// TreeSelect — 树选择器。
define_widget! {
    pub struct TreeSelect {
        placeholder: String,
        value: String,
        nodes: Vec<TreeNode>,
        open: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        Size::new(200.0, 32.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if let WidgetEvent::MouseDown { pos, .. } = event {
            if pos.y >= 0.0 && pos.y <= 32.0 {
                self.open = !self.open;
                return EventResult::Handled;
            }
            if self.open && pos.y > 32.0 {
                // 简化：点击面板区域选择
                self.open = false;
                return EventResult::Handled;
            }
        }
        EventResult::NotHandled
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_bg_container();
        let border = ctx.tokens().color_border();
        let primary = ctx.tokens().color_primary();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_quaternary();
        let r = Some(Radius::uniform(ctx.tokens().border_radius()));
        let input_rect = Rect::new(frame.x, frame.y, frame.w, 32.0);
        let bc = if self.open { primary } else { border };
        ctx.fill_rect(input_rect, bg, r);
        ctx.stroke_rect(input_rect, bc, if self.open { 2.0 } else { 1.0 }, r);
        let display = if self.value.is_empty() { &self.placeholder } else { &self.value };
        let input_y = ctx.visual_center_y(input_rect, 13.0);
        ctx.draw_text(display, Point::new(frame.x + 10.0, input_y),
            if self.value.is_empty() { text_sec } else { text }, 13.0);
        let arrow_y = ctx.visual_center_y(input_rect, 10.0);
        ctx.draw_text(if self.open { "▲" } else { "▼" }, Point::new(frame.x + frame.w - 18.0, arrow_y), text_sec, 10.0);
        if self.open {
            ctx.draw_text("(树形面板 - 选择节点)", Point::new(frame.x, frame.y + 40.0), text_sec, 11.0);
        }
    }
}

impl TreeSelect {
    pub fn new() -> Self { Self { placeholder: "请选择".into(), value: String::new(), nodes: Vec::new(), open: false } }
    pub fn placeholder(mut self, p: &str) -> Self { self.placeholder = p.to_string(); self }
    pub fn nodes(mut self, n: Vec<TreeNode>) -> Self { self.nodes = n; self }
    pub fn value(&self) -> &str { &self.value }
}
impl Default for TreeSelect { fn default() -> Self { Self::new() } }


