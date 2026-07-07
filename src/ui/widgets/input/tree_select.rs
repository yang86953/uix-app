use crate::core::{Constraints, Point, Rect, Size};
use crate::define_widget;
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::ui::animation::{presets, TransitionPlayer};
use crate::ui::widgets::display::tree::TreeNode;
use crate::ui::{
    EventResult, SemanticEvent, SnapshotFields, SnapshotTreeNode, SystemEvent, WidgetId, WidgetTree,
};
use std::cell::RefCell;

define_widget! {
    pub struct TreeSelect {
        placeholder: String,
        value: String,
        value_key: String,
        nodes: Vec<TreeNode>,
        open: bool,
        transition: TransitionPlayer,
        closing: bool,
        transition_dirty: bool,
        hovered_option: Option<String>,
        pending_change: RefCell<Option<String>>,
    }


    tab_index => (&self) -> i32 { 1 }
    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    preferred_size => (&self, _engine: Option<&dyn crate::draw::traits::GraphicsEngine>) -> Size {
        self.intrinsic_size()
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown { pos, .. } => {
                if pos.y >= 0.0 && pos.y <= 32.0 {
                    if self.open {
                        self.close();
                    } else {
                        self.open();
                    }
                    return EventResult::Handled;
                }
                if self.is_present() && pos.y > 32.0 {
                    let flat = self.flatten_nodes();
                    let idx = ((pos.y - 32.0) / 28.0) as usize;
                    if idx < flat.len() {
                        let (key, title, _) = &flat[idx];
                        self.value = title.clone();
                        self.value_key = key.clone();
                        self.close();
                        self.pending_change.replace(Some(key.clone()));
                        return EventResult::Handled;
                    }
                }
                self.close();
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if self.is_present() && pos.y > 32.0 {
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

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .borrow_mut()
            .take()
            .map(|key| SemanticEvent::change(id, key))
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
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
        if self.is_present() {
            let opacity = self.transition.opacity_progress.clamp(0.0, 1.0);
            let bg = fade_color(bg, opacity);
            let border = fade_color(border, opacity);
            let primary = fade_color(primary, opacity);
            let text = fade_color(text, opacity);
            let fill = fade_color(fill, opacity);
            let primary_bg = fade_color(ctx.tokens().color_primary_bg(), opacity);
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
                    ctx.fill_rect(item_rect, primary_bg, None);
                }

                let row_y = ctx.visual_center_y(item_rect, 13.0);
                let tc = if is_selected { primary } else { text };
                ctx.draw_text(title, Point::new(frame.x + indent, row_y), tc, 13.0);
            }
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        tree_select_dirty_rect(frame, self.flatten_nodes().len())
    }

    update_animation => (&mut self, dt: f64) -> bool {
        if !self.is_present() || self.transition.finished {
            self.transition_dirty = false;
            return false;
        }

        self.transition.update(dt);
        self.transition_dirty = true;

        if self.closing && self.transition.finished {
            self.open = false;
            self.closing = false;
        }

        self.is_present() && !self.transition.finished
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.transition_dirty {
            tree_select_dirty_rect(frame, self.flatten_nodes().len())
        } else {
            Rect::zero()
        }
    }
}

impl TreeSelect {
    fn intrinsic_size(&self) -> Size {
        Size::new(200.0, 32.0)
    }

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
            transition: TransitionPlayer::new(presets::tooltip_enter()),
            closing: false,
            transition_dirty: false,
            hovered_option: None,
            pending_change: RefCell::new(None),
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

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn is_present(&self) -> bool {
        self.open || self.closing
    }

    pub fn open(&mut self) {
        self.open = true;
        self.closing = false;
        self.transition = TransitionPlayer::new(presets::tooltip_enter());
        self.transition_dirty = true;
    }

    pub fn close(&mut self) {
        if !self.is_present() {
            self.open = false;
            self.closing = false;
            self.transition_dirty = false;
            return;
        }

        self.open = false;
        self.closing = true;
        self.transition = TransitionPlayer::new(presets::tooltip_exit());
        self.transition_dirty = true;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::TreeSelect {
            placeholder: self.placeholder.clone(),
            nodes: self
                .nodes
                .iter()
                .map(SnapshotTreeNode::from_tree_node)
                .collect(),
        }
    }
}

fn tree_select_dirty_rect(frame: Rect, item_count: usize) -> Rect {
    let list_h = item_count as f32 * 28.0;
    let list = Rect::new(frame.x, frame.y + 32.0, frame.w, list_h);
    frame.union(&list)
}

fn fade_color(color: Color, opacity: f32) -> Color {
    let alpha = (color.a as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    color.with_alpha(alpha)
}

impl Default for TreeSelect {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "../../../tests/ui/widgets/input/tree_select.rs"]
mod tests;
