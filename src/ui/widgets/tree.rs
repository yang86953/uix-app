//! Tree widget — 树形控件，Ant Design 风格。
//!
//! 支持展开/折叠、选中、多级嵌套、前缀图标。

use crate::base::{Point, Rect, Size};
use crate::define_widget;
use crate::graphics::Color;
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, WidgetEvent, WidgetTree};

/// 树节点。
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub title: String,
    pub key: String,
    pub icon: String,
    pub children: Vec<TreeNode>,
    pub disabled: bool,
}

// 扁平化节点用于渲染
struct FlatNode {
    title: String,
    key: String,
    icon: String,
    depth: usize,
    has_children: bool,
    expanded: bool,
    disabled: bool,
}

/// Tree — 树形控件。
define_widget! {
    pub struct Tree {
        nodes: Vec<TreeNode>,
        flat: Vec<FlatNode>,
        selected_key: String,
        expanded_keys: Vec<String>,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        let h = self.flat.len() as f32 * 28.0;
        Size::new(200.0, h.max(28.0))
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if let WidgetEvent::MouseDown { pos, .. } = event {
            let idx = (pos.y / 28.0) as usize;
            if idx < self.flat.len() {
                let node = &self.flat[idx];
                if node.disabled { return EventResult::NotHandled; }
                // 点击展开/折叠切换区域（缩进处）
                let indent = node.depth as f32 * 20.0;
                if pos.x >= indent && pos.x < indent + 20.0 && node.has_children {
                    if let Some(ek_idx) = self.expanded_keys.iter().position(|k| *k == node.key) {
                        self.expanded_keys.remove(ek_idx);
                    } else {
                        self.expanded_keys.push(node.key.clone());
                    }
                    self.flatten();
                    return EventResult::Handled;
                }
                // 选中
                self.selected_key = node.key.clone();
                return EventResult::Handled;
            }
        }
        EventResult::NotHandled
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let primary = ctx.tokens().color_primary();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let fill = ctx.tokens().color_fill_tertiary();
        let sel = &self.selected_key;

        for (i, node) in self.flat.iter().enumerate() {
            let y = frame.y + i as f32 * 28.0;
            let indent = node.depth as f32 * 20.0;
            let is_selected = node.key == *sel;

            // 选中高亮
            if is_selected {
                ctx.fill_rect(Rect::new(frame.x + indent, y, frame.w - indent, 28.0), fill, None);
            }

            // 展开/折叠箭头
            let arrow_x = frame.x + indent;
            if node.has_children {
                let arrow = if node.expanded { "▼" } else { "▶" };
                ctx.draw_text(arrow, Point::new(arrow_x + 4.0, y + 5.0), text_sec, 10.0);
            }

            // 图标
            let mut cursor = frame.x + indent + 20.0;
            if !node.icon.is_empty() {
                ctx.draw_text(&node.icon, Point::new(cursor, y + 5.0), text_sec, 12.0);
                cursor += 20.0;
            }

            // 标题
            let tc = if node.disabled { text_sec } else if is_selected { primary } else { text };
            ctx.draw_text(&node.title, Point::new(cursor, y + 5.0), tc, 13.0);
        }
    }
}

impl Tree {
    pub fn new(nodes: Vec<TreeNode>) -> Self {
        let mut tree = Self { nodes, flat: Vec::new(), selected_key: String::new(), expanded_keys: Vec::new() };
        tree.flatten();
        tree
    }
    pub fn selected_key(&self) -> &str { &self.selected_key }
    pub fn set_selected_key(&mut self, key: &str) { self.selected_key = key.to_string(); }

    fn flatten(&mut self) {
        self.flat.clear();
        let nodes = self.nodes.clone();
        for node in &nodes {
            self.flatten_node(node, 0);
        }
    }

    fn flatten_node(&mut self, node: &TreeNode, depth: usize) {
        let is_expanded = self.expanded_keys.contains(&node.key);
        let has_children = !node.children.is_empty();
        let children = node.children.clone();
        self.flat.push(FlatNode {
            title: node.title.clone(),
            key: node.key.clone(),
            icon: node.icon.clone(),
            depth,
            has_children,
            expanded: is_expanded,
            disabled: node.disabled,
        });
        if is_expanded {
            for child in &children {
                self.flatten_node(child, depth + 1);
            }
        }
    }
}

impl TreeNode {
    pub fn new(title: &str, key: &str) -> Self {
        Self { title: title.to_string(), key: key.to_string(), icon: String::new(), children: Vec::new(), disabled: false }
    }
    pub fn icon(mut self, i: &str) -> Self { self.icon = i.to_string(); self }
    pub fn children(mut self, c: Vec<TreeNode>) -> Self { self.children = c; self }
    pub fn add(mut self, child: TreeNode) -> Self { self.children.push(child); self }
    pub fn disabled(mut self, v: bool) -> Self { self.disabled = v; self }
}
