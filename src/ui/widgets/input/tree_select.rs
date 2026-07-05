use crate::ui::widgets::display::tree::TreeNode;
use crate::define_widget;
use crate::draw::painting::RenderContext;
use crate::ui::{EventResult, WidgetEvent, WidgetTree};
use crate::draw::Radius;
use crate::native::{Point, Rect, Size};

define_widget! {
    pub struct TreeSelect {
        placeholder: String,
        value: String,
        value_key: String,
        nodes: Vec<TreeNode>,
        open: bool,
        hovered_option: Option<String>,
        on_change: Option<Box<dyn FnMut(String, String) + 'static>>,
    }


    tab_index => (&self) -> i32 { 1 }
    preferred_size => (&self, _engine: Option<&dyn crate::draw::traits::GraphicsEngine>) -> Size {
        Size::new(200.0, 32.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                if pos.y >= 0.0 && pos.y <= 32.0 {
                    self.open = !self.open;
                    return EventResult::Handled;
                }
                if self.open && pos.y > 32.0 {
                    let flat = self.flatten_nodes();
                    let idx = ((pos.y - 32.0) / 28.0) as usize;
                    if idx < flat.len() {
                        let (key, title, _) = &flat[idx];
                        self.value = title.clone();
                        self.value_key = key.clone();
                        self.open = false;
                        if let Some(ref mut cb) = self.on_change {
                            cb(key.clone(), title.clone());
                        }
                        return EventResult::Handled;
                    }
                }
                self.open = false;
                EventResult::NotHandled
            }
            WidgetEvent::MouseMove { pos, .. } => {
                if self.open && pos.y > 32.0 {
                    let flat = self.flatten_nodes();
                    let idx = ((pos.y - 32.0) / 28.0) as usize;
                    self.hovered_option = flat.get(idx).map(|(k, _, _)| k.clone());
                } else {
                    self.hovered_option = None;
                }
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_bg_container();
        let border = ctx.tokens().color_border();
        let primary = ctx.tokens().color_primary();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_quaternary();
        let fill = ctx.tokens().color_fill_tertiary();
        let r = Some(Radius::uniform(ctx.tokens().border_radius()));
        let input_rect = Rect::new(frame.x, frame.y, frame.w, 32.0);
        let bc = if self.open { primary } else { border };
        ctx.fill_rect(input_rect, bg, r);
        ctx.stroke_rect(input_rect, bc, if self.open { 2.0 } else { 1.0 }, r);
        let display = if self.value.is_empty() { &self.placeholder } else { &self.value };
        let input_y = ctx.visual_center_y(input_rect, 13.0);
        let disp_color = if self.value.is_empty() { text_sec } else { text };
        ctx.draw_text(display, Point::new(frame.x + 10.0, input_y), disp_color, 13.0);
        let arrow_y = ctx.visual_center_y(input_rect, 10.0);
        ctx.draw_text(if self.open { "▲" } else { "▼" }, Point::new(frame.x + frame.w - 18.0, arrow_y), text_sec, 10.0);

        // 下拉树面板
        if self.open {
            let flat = self.flatten_nodes();
            let list_h = flat.len() as f32 * 28.0;
            let list_y = frame.y + 32.0;
            let list_rect = Rect::new(frame.x, list_y, frame.w, list_h);
            ctx.fill_rect(list_rect, bg, Some(Radius::uniform(ctx.tokens().border_radius_sm())));
            ctx.stroke_rect(list_rect, border, 1.0, Some(Radius::uniform(ctx.tokens().border_radius_sm())));

            for (i, (key, title, depth)) in flat.iter().enumerate() {
                let item_y = list_y + i as f32 * 28.0;
                let item_rect = Rect::new(frame.x, item_y, frame.w, 28.0);
                let indent = *depth as f32 * 20.0 + 8.0;
                let is_hovered = self.hovered_option.as_ref() == Some(key);
                let is_selected = *key == self.value_key;

                if is_hovered || is_selected {
                    ctx.fill_rect(item_rect, fill, None);
                }
                if is_selected {
                    ctx.fill_rect(item_rect, ctx.tokens().color_primary_bg(), None);
                }

                let row_y = ctx.visual_center_y(item_rect, 13.0);
                let tc = if is_selected { primary } else { text };
                ctx.draw_text(title, Point::new(frame.x + indent, row_y), tc, 13.0);
            }
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        let flat = self.flatten_nodes();
        let list_h = flat.len() as f32 * 28.0;
        let list = Rect::new(frame.x, frame.y + 32.0, frame.w, list_h);
        frame.union(&list)
    }
}

impl TreeSelect {
    fn flatten_nodes(&self) -> Vec<(String, String, usize)> {
        let mut result = Vec::new();
        self.flatten(&self.nodes, 0, &mut result);
        result
    }

    fn flatten(&self, nodes: &[TreeNode], depth: usize, result: &mut Vec<(String, String, usize)>) {
        for node in nodes {
            result.push((node.key.clone(), node.title.clone(), depth));
            if !node.children.is_empty() {
                self.flatten(&node.children, depth + 1, result);
            }
        }
    }

    pub fn new() -> Self {
        Self {
            placeholder: "Please select".into(),
            value: String::new(),
            value_key: String::new(),
            nodes: Vec::new(),
            open: false,
            hovered_option: None,
            on_change: None,
        }
    }
    pub fn placeholder(mut self, p: &str) -> Self {
        self.placeholder = p.to_string();
        self
    }
    pub fn nodes(mut self, n: Vec<TreeNode>) -> Self {
        self.nodes = n;
        self
    }
    pub fn value(&self) -> &str {
        &self.value
    }
    pub fn value_key(&self) -> &str {
        &self.value_key
    }
    pub fn on_change<F: FnMut(String, String) + 'static>(mut self, f: F) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }
}
impl Default for TreeSelect {
    fn default() -> Self {
        Self::new()
    }
}
