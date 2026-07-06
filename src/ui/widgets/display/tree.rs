use crate::core::{Point, Rect, Size};
use crate::define_widget;
use crate::draw::painting::PaintContext;
use crate::ui::{EventResult, SystemEvent, WidgetTree};

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub title: String,
    pub key: String,
    pub icon: String,
    pub children: Vec<TreeNode>,
    pub disabled: bool,
    pub checkable: bool,
    pub checked: bool,
    pub draggable: bool,
    pub is_leaf: bool,
}

struct FlatNode {
    title: String,
    key: String,
    icon: String,
    depth: usize,
    has_children: bool,
    expanded: bool,
    disabled: bool,
    checkable: bool,
    checked: bool,
}

define_widget! {
    pub struct Tree {
        nodes: Vec<TreeNode>,
        flat: Vec<FlatNode>,
        selected_key: String,
        selected_keys: Vec<String>,
        expanded_keys: Vec<String>,
        multiple: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::draw::traits::GraphicsEngine>) -> Size {
        let h = self.flat.len() as f32 * 28.0;
        Size::new(200.0, h.max(28.0))
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if let SystemEvent::PointerDown { pos, .. } = event {
            let idx = (pos.y / 28.0) as usize;
            if idx < self.flat.len() {
                let node_key = self.flat[idx].key.clone();
                let node_disabled = self.flat[idx].disabled;

                if node_disabled {
                    return EventResult::NotHandled;
                }

                let indent = self.flat[idx].depth as f32 * 20.0;

                let check_x = indent;
                if self.flat[idx].checkable && pos.x >= check_x && pos.x < check_x + 20.0 {
                    self.toggle_check(&node_key);
                    return EventResult::Handled;
                }

                let arrow_x = indent + 20.0;
                if pos.x >= arrow_x && pos.x < arrow_x + 20.0 && self.flat[idx].has_children {
                    if let Some(ek_idx) = self.expanded_keys.iter().position(|k| *k == node_key) {
                        self.expanded_keys.remove(ek_idx);
                    } else {
                        self.expanded_keys.push(node_key.clone());
                    }
                    self.flatten();
                    return EventResult::Handled;
                }

                if self.multiple {
                    if let Some(ex_idx) = self.selected_keys.iter().position(|k| *k == node_key) {
                        self.selected_keys.remove(ex_idx);
                    } else {
                        self.selected_keys.push(node_key.clone());
                    }
                } else {
                    self.selected_key = node_key.clone();
                    self.selected_keys.clear();
                    self.selected_keys.push(node_key);
                }
                return EventResult::Handled;
            }
        }
        EventResult::NotHandled
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let primary = ctx.tokens().color_primary();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let fill = ctx.tokens().color_fill_tertiary();

        for (i, node) in self.flat.iter().enumerate() {
            let y = frame.y + i as f32 * 28.0;
            let indent = node.depth as f32 * 20.0;
            let is_selected = self.multiple && self.selected_keys.contains(&node.key)
                || (!self.multiple && node.key == self.selected_key);

            if is_selected {
                ctx.fill_rect(Rect::new(frame.x, y, frame.w, 28.0), fill, None);
            }

            let row_rect = Rect::new(frame.x, y, frame.w, 28.0);
            let mut cursor = frame.x + indent;

            if node.checkable {
                let check_str = if node.checked { "[x]" } else { "[ ]" };
                let check_y = ctx.visual_center_y(row_rect, 12.0);
                ctx.draw_text(
                    check_str,
                    Point::new(cursor + 2.0, check_y),
                    if node.checked { primary } else { text_sec },
                    12.0,
                );
                cursor += 28.0;
            }

            let row_y = ctx.visual_center_y(row_rect, 10.0);
            if node.has_children {
                let arrow = if node.expanded { "v" } else { ">" };
                ctx.draw_text(arrow, Point::new(cursor + 4.0, row_y), text_sec, 10.0);
            }
            cursor += 20.0;

            if !node.icon.is_empty() {
                let icon_str = crate::ui::widgets::icon::icon_char(&node.icon);
                let saved = *ctx.font();
                if let Some(fh) = crate::ui::widgets::icon::lucide_handle() {
                    ctx.set_font(fh);
                }
                let icon_y = ctx.visual_center_y(row_rect, 12.0);
                ctx.draw_text(icon_str, Point::new(cursor, icon_y), text_sec, 12.0);
                ctx.set_font(saved);
                cursor += 20.0;
            }

            let tc = if node.disabled {
                text_sec
            } else if is_selected {
                primary
            } else {
                text
            };
            let title_y = ctx.visual_center_y(row_rect, 13.0);
            ctx.draw_text(&node.title, Point::new(cursor, title_y), tc, 13.0);
        }
    }
}

impl Tree {
    pub fn new(nodes: Vec<TreeNode>) -> Self {
        let mut tree = Self {
            nodes,
            flat: Vec::new(),
            selected_key: String::new(),
            selected_keys: Vec::new(),
            expanded_keys: Vec::new(),
            multiple: false,
        };
        tree.flatten();
        tree
    }

    pub fn selected_key(&self) -> &str {
        &self.selected_key
    }

    pub fn selected_keys(&self) -> &[String] {
        &self.selected_keys
    }

    pub fn set_selected_key(&mut self, key: &str) {
        self.selected_key = key.to_string();
    }

    pub fn multiple(mut self, v: bool) -> Self {
        self.multiple = v;
        self
    }

    fn toggle_check(&mut self, key: &str) {
        if let Some(node) = self.find_node_mut(key) {
            node.checked = !node.checked;
            self.flatten();
        }
    }

    fn find_node_mut(&mut self, key: &str) -> Option<&mut TreeNode> {
        Self::find_in_nodes(&mut self.nodes, key)
    }

    fn find_in_nodes<'a>(nodes: &'a mut [TreeNode], key: &str) -> Option<&'a mut TreeNode> {
        for node in nodes.iter_mut() {
            if node.key == key {
                return Some(node);
            }
            if let Some(found) = Self::find_in_nodes(&mut node.children, key) {
                return Some(found);
            }
        }
        None
    }

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
        self.flat.push(FlatNode {
            title: node.title.clone(),
            key: node.key.clone(),
            icon: node.icon.clone(),
            depth,
            has_children,
            expanded: is_expanded,
            disabled: node.disabled,
            checkable: node.checkable,
            checked: node.checked,
        });
        if is_expanded {
            for child in &node.children {
                self.flatten_node(child, depth + 1);
            }
        }
    }
}

impl TreeNode {
    pub fn new(title: &str, key: &str) -> Self {
        Self {
            title: title.to_string(),
            key: key.to_string(),
            icon: String::new(),
            children: Vec::new(),
            disabled: false,
            checkable: false,
            checked: false,
            draggable: false,
            is_leaf: true,
        }
    }

    pub fn icon(mut self, i: &str) -> Self {
        self.icon = i.to_string();
        self
    }

    pub fn children(mut self, c: Vec<TreeNode>) -> Self {
        self.children = c;
        self.is_leaf = false;
        self
    }

    pub fn add(mut self, child: TreeNode) -> Self {
        self.children.push(child);
        self.is_leaf = false;
        self
    }

    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }

    pub fn checkable(mut self, v: bool) -> Self {
        self.checkable = v;
        self
    }

    pub fn draggable(mut self, v: bool) -> Self {
        self.draggable = v;
        self
    }
}
