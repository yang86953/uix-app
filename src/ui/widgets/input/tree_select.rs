use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::ui::animation::{presets, TransitionPlayer};
use crate::ui::foundation::virtual_scroll::VirtualListScroll;
use crate::ui::widgets::display::tree::TreeNode;
use crate::ui::{
    ComponentId, EventResult, SemanticEvent, SnapshotFields, SnapshotTreeNode, SystemEvent,
    WidgetTree,
};
use std::cell::{Cell, RefCell};

const DROPDOWN_ROW_HEIGHT: f32 = 28.0;
const DROPDOWN_TRIGGER_HEIGHT: f32 = 32.0;
const MAX_DROPDOWN_VIEWPORT_HEIGHT: f32 = 280.0;

component! {
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
        pub(crate) dropdown_scroll: VirtualListScroll,
        scroll_delta_strip: Cell<(f32, f32)>,
    }


    tab_index => (&self) -> i32 { 1 }
    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    scroll_delta_for_dirty => (&self) -> Option<(f32, f32)> {
        let delta = self.scroll_delta_strip.get();
        if delta.0.abs() > 0.01 || delta.1.abs() > 0.01 {
            self.scroll_delta_strip.set((0.0, 0.0));
            Some(delta)
        } else {
            None
        }
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown { pos, .. } => {
                if pos.y >= 0.0 && pos.y <= DROPDOWN_TRIGGER_HEIGHT {
                    if self.open {
                        self.close();
                    } else {
                        self.open();
                    }
                    return EventResult::Handled;
                }
                if self.is_present() && pos.y > DROPDOWN_TRIGGER_HEIGHT {
                    if let Some(idx) = self.dropdown_row_at_y(pos.y) {
                        let flat = self.flatten_nodes();
                        if let Some((key, title, _)) = flat.get(idx) {
                            self.value = title.clone();
                            self.value_key = key.clone();
                            self.close();
                            self.pending_change.replace(Some(key.clone()));
                            return EventResult::Handled;
                        }
                    }
                }
                self.close();
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if self.is_present() && pos.y > DROPDOWN_TRIGGER_HEIGHT {
                    let idx = self.dropdown_row_at_y(pos.y);
                    let flat = self.flatten_nodes();
                    self.hovered_option = idx.and_then(|i| flat.get(i).map(|(k, _, _)| k.clone()));
                } else {
                    self.hovered_option = None;
                }
                EventResult::Handled
            }
            SystemEvent::Wheel { delta, pos, .. } => {
                if self.is_present() && pos.y > DROPDOWN_TRIGGER_HEIGHT {
                    let row_count = self.flatten_nodes().len();
                    let viewport_h = self.dropdown_viewport_height(row_count);
                    let dy = self.dropdown_scroll.scroll_by_wheel(
                        delta.y,
                        row_count,
                        DROPDOWN_ROW_HEIGHT,
                        viewport_h,
                    );
                    if dy.abs() > 0.01 {
                        self.push_scroll_delta(0.0, dy);
                        return EventResult::Handled;
                    }
                }
                EventResult::NotHandled
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
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
        let input_rect = Rect::new(frame.x, frame.y, frame.w, DROPDOWN_TRIGGER_HEIGHT);
        let bc = if self.open { primary } else { border };
        ctx.fill_rect(input_rect, bg, r);
        ctx.stroke_rect(input_rect, bc, if self.open { 2.0 } else { 1.0 }, r);
        let display = if self.value.is_empty() { &self.placeholder } else { &self.value };
        let input_y = ctx.visual_center_y(input_rect, 13.0);
        let disp_color = if self.value.is_empty() { text_sec } else { text };
        ctx.draw_text(display, Point::new(frame.x + 10.0, input_y), disp_color, 13.0);
        let arrow_y = ctx.visual_center_y(input_rect, 10.0);
        ctx.draw_text(if self.open { "▲" } else { "▼" }, Point::new(frame.x + frame.w - 18.0, arrow_y), text_sec, 10.0);

        if !self.is_present() {
            return;
        }

        let opacity = self.transition.opacity_progress.clamp(0.0, 1.0);
        let bg = fade_color(bg, opacity);
        let border = fade_color(border, opacity);
        let primary = fade_color(primary, opacity);
        let text = fade_color(text, opacity);
        let fill = fade_color(fill, opacity);
        let primary_bg = fade_color(ctx.tokens().color_primary_bg(), opacity);
        let flat = self.flatten_nodes();
        let row_count = flat.len();
        if row_count == 0 {
            return;
        }

        let list_h = self.dropdown_viewport_height(row_count);
        let list_y = frame.y + DROPDOWN_TRIGGER_HEIGHT;
        let list_rect = Rect::new(frame.x, list_y, frame.w, list_h);
        ctx.fill_rect(list_rect, bg, Some(Radius::uniform(ctx.tokens().border_radius_sm())));
        ctx.stroke_rect(list_rect, border, 1.0, Some(Radius::uniform(ctx.tokens().border_radius_sm())));

        let scroll_offset = self.dropdown_scroll.scroll_offset();
        let (start, end) = self.dropdown_scroll.scroll_range(
            row_count,
            DROPDOWN_ROW_HEIGHT,
            list_h,
        );
        ctx.canvas_2d().push_clip(list_rect);

        for i in start..end {
            let (key, title, depth) = &flat[i];
            let item_y = list_y + i as f32 * DROPDOWN_ROW_HEIGHT - scroll_offset;
            if item_y + DROPDOWN_ROW_HEIGHT < list_y || item_y > list_y + list_h {
                continue;
            }
            let item_rect = Rect::new(frame.x, item_y, frame.w, DROPDOWN_ROW_HEIGHT);
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

        ctx.canvas_2d().pop_clip();
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
        Size::new(200.0, DROPDOWN_TRIGGER_HEIGHT)
    }

    pub(crate) fn dropdown_viewport_height(&self, row_count: usize) -> f32 {
        (row_count as f32 * DROPDOWN_ROW_HEIGHT).min(MAX_DROPDOWN_VIEWPORT_HEIGHT)
    }

    pub(crate) fn dropdown_row_at_y(&self, pos_y: f32) -> Option<usize> {
        if pos_y <= DROPDOWN_TRIGGER_HEIGHT {
            return None;
        }
        let local_y = pos_y - DROPDOWN_TRIGGER_HEIGHT + self.dropdown_scroll.scroll_offset();
        if local_y < 0.0 {
            return None;
        }
        let idx = (local_y / DROPDOWN_ROW_HEIGHT) as usize;
        let flat_len = self.flatten_nodes().len();
        if idx < flat_len {
            Some(idx)
        } else {
            None
        }
    }

    fn push_scroll_delta(&self, dx: f32, dy: f32) {
        if dx.abs() <= 0.01 && dy.abs() <= 0.01 {
            return;
        }
        let current = self.scroll_delta_strip.get();
        self.scroll_delta_strip
            .set((current.0 + dx, current.1 + dy));
    }

    pub(crate) fn flatten_nodes(&self) -> Vec<(String, String, usize)> {
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
            dropdown_scroll: VirtualListScroll::new(),
            scroll_delta_strip: Cell::new((0.0, 0.0)),
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
        self.dropdown_scroll.set_scroll_offset(0.0);
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
        self.dropdown_scroll.set_scroll_offset(0.0);
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

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.placeholder = next.placeholder;
        self.nodes = next.nodes;
        let row_count = self.flatten_nodes().len();
        self.dropdown_scroll.clamp_to_content(
            row_count,
            DROPDOWN_ROW_HEIGHT,
            self.dropdown_viewport_height(row_count),
        );
    }
}

fn tree_select_dirty_rect(frame: Rect, item_count: usize) -> Rect {
    let list_h = (item_count as f32 * DROPDOWN_ROW_HEIGHT).min(MAX_DROPDOWN_VIEWPORT_HEIGHT);
    let list = Rect::new(frame.x, frame.y + DROPDOWN_TRIGGER_HEIGHT, frame.w, list_h);
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
