//! TreeSelect widget — 树选择器，Ant Design 风格。
//!
//! 结合 Tree 与 Select，弹出树形面板选择。

use crate::base::{Point, Rect, Size};
use crate::define_widget;
use crate::graphics::Radius;
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, WidgetEvent, WidgetTree};
use super::tree::TreeNode;

/// TreeSelect — 树选择器。
define_widget! {
    pub struct TreeSelect {
        placeholder: String,
        value: String,
        nodes: Vec<TreeNode>,
        open: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
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
        ctx.draw_text(display, Point::new(frame.x + 10.0, frame.y + 8.0),
            if self.value.is_empty() { text_sec } else { text }, 13.0);
        ctx.draw_text(if self.open { "▲" } else { "▼" }, Point::new(frame.x + frame.w - 18.0, frame.y + 8.0), text_sec, 10.0);
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

/// Anchor — 锚点组件，页面内快速定位。
define_widget! {
    pub struct Anchor {
        items: Vec<(String, String)>, // (title, anchor_id)
        affix: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        Size::new(160.0, self.items.len() as f32 * 28.0)
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let primary = ctx.tokens().color_primary();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        for (i, (title, _id)) in self.items.iter().enumerate() {
            let y = frame.y + i as f32 * 28.0;
            ctx.draw_text(title, Point::new(frame.x + 12.0, y + 5.0), if i == 0 { primary } else { text_sec }, 13.0);
            if i == 0 {
                ctx.fill_rect(Rect::new(frame.x, y, 3.0, 28.0), primary, None);
            }
        }
    }
}
impl Anchor {
    pub fn new() -> Self { Self { items: Vec::new(), affix: true } }
    pub fn items(mut self, items: Vec<(&str, &str)>) -> Self {
        self.items = items.into_iter().map(|(t, i)| (t.to_string(), i.to_string())).collect(); self
    }
    pub fn add(mut self, title: &str, id: &str) -> Self { self.items.push((title.to_string(), id.to_string())); self }
}
impl Default for Anchor { fn default() -> Self { Self::new() } }
