use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::api::PaintContext;
use crate::draw::{Color, Radius};
use crate::ui::animation::{presets, TransitionPlayer};
use crate::ui::foundation::virtual_scroll::VirtualListScroll;
use crate::ui::widgets::display::tree::TreeNode;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields,
    SnapshotTreeNode, SystemEvent, WidgetTree,
};
use std::cell::{Cell, RefCell};

const DROPDOWN_ROW_HEIGHT: f32 = 28.0;
const DROPDOWN_TRIGGER_HEIGHT: f32 = 32.0;
const MAX_DROPDOWN_VIEWPORT_HEIGHT: f32 = 280.0;
const MIN_DROPDOWN_WIDTH: f32 = 200.0;

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
        focused: bool,
        hovered_option: Option<String>,
        highlighted_option: Option<String>,
        pending_change: RefCell<Option<String>>,
        pub(crate) dropdown_scroll: VirtualListScroll,
        scroll_delta_strip: Cell<(f32, f32)>,
        last_frame: Cell<Option<Rect>>,
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
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                self.focused = true;
                let frame = self.interaction_frame();
                if point_in_half_open_rect(frame, *pos) {
                    if self.open {
                        self.close();
                    } else {
                        self.open();
                    }
                    return EventResult::Handled;
                }
                if self.open && point_in_half_open_rect(self.dropdown_local_rect(), *pos) {
                    if let Some(idx) = self.dropdown_row_at_y(pos.y) {
                        let flat = self.flatten_nodes();
                        if flat.get(idx).is_some_and(|(_, _, _, disabled)| *disabled) {
                            return EventResult::Handled;
                        }
                        if self.select_flat_index(idx) {
                            return EventResult::Handled;
                        }
                    }
                    return EventResult::Handled;
                }
                self.close();
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if !self.open {
                    return EventResult::NotHandled;
                }
                let next = if point_in_half_open_rect(self.dropdown_local_rect(), *pos) {
                    let idx = self.dropdown_row_at_y(pos.y);
                    let flat = self.flatten_nodes();
                    idx.and_then(|i| {
                        flat.get(i).and_then(|(key, _, _, disabled)| {
                            (!disabled).then(|| key.clone())
                        })
                    })
                } else {
                    None
                };
                if self.hovered_option != next {
                    self.hovered_option = next;
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerLeave => {
                if self.hovered_option.take().is_some() {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::Wheel { delta, pos, .. } => {
                if self.open && point_in_half_open_rect(self.dropdown_local_rect(), *pos) {
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
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.close();
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } => {
                if !self.open {
                    return match key {
                        KeyCode::Down | KeyCode::Enter | KeyCode::Space => {
                            self.open();
                            EventResult::Handled
                        }
                        _ => EventResult::NotHandled,
                    };
                }
                match key {
                    KeyCode::Down => {
                        self.move_highlight(true);
                        EventResult::Handled
                    }
                    KeyCode::Up => {
                        self.move_highlight(false);
                        EventResult::Handled
                    }
                    KeyCode::Home => {
                        self.move_to_edge(true);
                        EventResult::Handled
                    }
                    KeyCode::End => {
                        self.move_to_edge(false);
                        EventResult::Handled
                    }
                    KeyCode::Enter | KeyCode::Space => {
                        self.select_highlighted();
                        EventResult::Handled
                    }
                    KeyCode::Escape => {
                        self.close();
                        EventResult::Handled
                    }
                    _ => EventResult::NotHandled,
                }
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

    hit_test_frame => (&self, frame: Rect) -> Rect {
        if self.is_present() {
            tree_select_dirty_rect(frame, self.flatten_nodes().len())
        } else {
            frame
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w.max(0.0), frame.h.max(0.0))));
        let bg = ctx.tokens().color_bg_container();
        let border = ctx.tokens().color_border();
        let primary = ctx.tokens().color_primary();
        let text = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let text_tertiary = ctx.tokens().color_text_quaternary();
        let fill = ctx.tokens().color_fill_tertiary();
        let scale = (frame.h / DROPDOWN_TRIGGER_HEIGHT).clamp(0.0, 1.0);
        let font_size = 13.0 * scale;
        let left_padding = 10.0 * scale;
        let arrow_slot = 28.0 * scale;
        let radius = Some(Radius::uniform(
            (ctx.tokens().border_radius_sm() * scale).min(frame.h.max(0.0) * 0.5),
        ));
        let input_rect = Rect::new(frame.x, frame.y, frame.w.max(0.0), frame.h.max(0.0));
        let bc = if self.open || self.focused { primary } else { border };
        ctx.push_clip(input_rect);
        ctx.fill_rect(input_rect, bg, radius);
        ctx.stroke_rect(
            input_rect,
            bc,
            if self.open || self.focused { 2.0 } else { 1.0 },
            radius,
        );
        if font_size > 0.0 && input_rect.w > 0.0 {
            let arrow_rect = Rect::new(
                input_rect.x + (input_rect.w - arrow_slot).max(0.0),
                input_rect.y,
                arrow_slot.min(input_rect.w),
                input_rect.h,
            );
            let text_area = Rect::new(
                input_rect.x + left_padding,
                input_rect.y,
                (arrow_rect.x - input_rect.x - left_padding).max(0.0),
                input_rect.h,
            );
            if text_area.w > 0.0 {
                let display = if self.value.is_empty() {
                    &self.placeholder
                } else {
                    &self.value
                };
                let display_color = if self.value.is_empty() {
                    text_tertiary
                } else {
                    text
                };
                let input_y = ctx.visual_center_y(text_area, font_size);
                ctx.push_clip(text_area);
                ctx.draw_text(
                    display,
                    Point::new(text_area.x, input_y),
                    display_color,
                    font_size,
                );
                ctx.pop_clip();
            }
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                if self.is_present() {
                    "chevron-up"
                } else {
                    "chevron-down"
                },
                arrow_rect,
                text_secondary,
                12.0 * scale,
            );
        }
        ctx.pop_clip();

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
        let text_tertiary = fade_color(text_tertiary, opacity);
        let flat = self.flatten_nodes();
        let display_row_count = flat.len().max(1);
        let list_rect = tree_select_popup_rect(frame, flat.len());
        let panel_radius = Some(Radius::uniform(ctx.tokens().border_radius_sm()));
        ctx.push_clip(list_rect);
        ctx.fill_rect(list_rect, bg, panel_radius);
        ctx.stroke_rect(list_rect, border, 1.0, panel_radius);

        if flat.is_empty() {
            let text_area = Rect::new(
                list_rect.x + 10.0,
                list_rect.y,
                (list_rect.w - 20.0).max(0.0),
                list_rect.h,
            );
            let row_y = ctx.visual_center_y(text_area, 13.0);
            ctx.push_clip(text_area);
            ctx.draw_text(
                crate::ui::locale::use_locale().no_data,
                Point::new(text_area.x, row_y),
                text_tertiary,
                13.0,
            );
            ctx.pop_clip();
            ctx.pop_clip();
            return;
        }

        let scroll_offset = self.dropdown_scroll.scroll_offset();
        let (start, end) = self.dropdown_scroll.scroll_range(
            display_row_count,
            DROPDOWN_ROW_HEIGHT,
            list_rect.h,
        );

        for (i, (key, title, depth, disabled)) in flat.iter().enumerate().take(end).skip(start) {
            let item_y = list_rect.y + i as f32 * DROPDOWN_ROW_HEIGHT - scroll_offset;
            if item_y + DROPDOWN_ROW_HEIGHT <= list_rect.y
                || item_y >= list_rect.y + list_rect.h
            {
                continue;
            }
            let item_rect = Rect::new(
                list_rect.x,
                item_y,
                list_rect.w,
                DROPDOWN_ROW_HEIGHT,
            );
            let indent = (*depth as f32 * 20.0 + 8.0)
                .min((item_rect.w - 34.0).max(8.0));
            let is_hovered = !disabled && self.hovered_option.as_ref() == Some(key);
            let is_highlighted = !disabled && self.highlighted_option.as_ref() == Some(key);
            let is_selected = *key == self.value_key;

            if is_hovered || is_highlighted {
                ctx.fill_rect(item_rect, fill, None);
            }
            if is_selected {
                ctx.fill_rect(item_rect, primary_bg, None);
            }

            let row_y = ctx.visual_center_y(item_rect, 13.0);
            let tc = if *disabled {
                text_tertiary
            } else if is_selected {
                primary
            } else {
                text
            };
            let text_area = Rect::new(
                item_rect.x + indent,
                item_rect.y,
                (item_rect.w - indent - 10.0).max(0.0),
                item_rect.h,
            );
            if text_area.w > 0.0 {
                ctx.push_clip(text_area);
                ctx.draw_text(title, Point::new(text_area.x, row_y), tc, 13.0);
                ctx.pop_clip();
            }
        }

        ctx.pop_clip();
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        tree_select_dirty_rect(frame, self.flatten_nodes().len())
    }

    overlay_entry => (&self, id: ComponentId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        self.is_present().then(|| {
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Popover)
                .bounds(tree_select_dirty_rect(frame, self.flatten_nodes().len()))
                .z_index(900)
        })
    }

    update_animation => (&mut self, dt: f64) -> bool {
        if !self.is_present() {
            self.transition_dirty = false;
            return false;
        }

        if self.transition.finished {
            if self.closing {
                self.closing = false;
                self.hovered_option = None;
                self.highlighted_option = None;
                self.dropdown_scroll.set_scroll_offset(0.0);
            }
            self.transition_dirty = false;
            return false;
        }

        self.transition.update(dt);
        self.transition_dirty = true;

        if self.closing && self.transition.finished {
            self.closing = false;
            self.hovered_option = None;
            self.highlighted_option = None;
            self.dropdown_scroll.set_scroll_offset(0.0);
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
        (row_count.max(1) as f32 * DROPDOWN_ROW_HEIGHT).min(MAX_DROPDOWN_VIEWPORT_HEIGHT)
    }

    pub(crate) fn dropdown_row_at_y(&self, pos_y: f32) -> Option<usize> {
        let popup = self.dropdown_local_rect();
        if pos_y < popup.y || pos_y >= popup.y + popup.h {
            return None;
        }
        let local_y = pos_y - popup.y + self.dropdown_scroll.scroll_offset();
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

    fn interaction_frame(&self) -> Rect {
        self.last_frame.get().unwrap_or_else(|| {
            let size = self.intrinsic_size();
            Rect::new(0.0, 0.0, size.w, size.h)
        })
    }

    fn dropdown_local_rect(&self) -> Rect {
        let frame = self.interaction_frame();
        Rect::new(
            0.0,
            frame.h,
            frame.w.max(MIN_DROPDOWN_WIDTH),
            self.dropdown_viewport_height(self.flatten_nodes().len()),
        )
    }

    fn push_scroll_delta(&self, dx: f32, dy: f32) {
        if dx.abs() <= 0.01 && dy.abs() <= 0.01 {
            return;
        }
        let current = self.scroll_delta_strip.get();
        self.scroll_delta_strip
            .set((current.0 + dx, current.1 + dy));
    }

    pub(crate) fn flatten_nodes(&self) -> Vec<(String, String, usize, bool)> {
        let mut result = Vec::new();
        self.flatten(&self.nodes, 0, &mut result);
        result
    }

    fn flatten(
        &self,
        nodes: &[TreeNode],
        depth: usize,
        result: &mut Vec<(String, String, usize, bool)>,
    ) {
        for node in nodes {
            result.push((node.key.clone(), node.title.clone(), depth, node.disabled));
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
            focused: false,
            hovered_option: None,
            highlighted_option: None,
            pending_change: RefCell::new(None),
            dropdown_scroll: VirtualListScroll::new(),
            scroll_delta_strip: Cell::new((0.0, 0.0)),
            last_frame: Cell::new(None),
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
        let flat = self.flatten_nodes();
        self.hovered_option = None;
        let highlighted_index = flat
            .iter()
            .position(|(key, _, _, disabled)| key == &self.value_key && !disabled)
            .or_else(|| flat.iter().position(|(_, _, _, disabled)| !disabled));
        self.highlighted_option = highlighted_index.map(|index| flat[index].0.clone());
        if let Some(index) = highlighted_index {
            self.reveal_index(index, flat.len());
        }
        self.scroll_delta_strip.set((0.0, 0.0));
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
        self.hovered_option = None;
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
            value: self.value.clone(),
            value_key: self.value_key.clone(),
            open: self.open,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let nodes_changed = self.nodes != next.nodes;
        self.placeholder = next.placeholder;
        self.nodes = next.nodes;
        let row_count = self.flatten_nodes().len();
        self.dropdown_scroll.clamp_to_content(
            row_count,
            DROPDOWN_ROW_HEIGHT,
            self.dropdown_viewport_height(row_count),
        );
        if self.is_present() && nodes_changed {
            let flat = self.flatten_nodes();
            self.hovered_option = None;
            self.highlighted_option = flat
                .iter()
                .find(|(key, _, _, disabled)| key == &self.value_key && !disabled)
                .or_else(|| flat.iter().find(|(_, _, _, disabled)| !disabled))
                .map(|(key, _, _, _)| key.clone());
        }
    }

    fn select_highlighted(&mut self) {
        let Some(key) = self.highlighted_option.as_deref() else {
            return;
        };
        let flat = self.flatten_nodes();
        if let Some(index) = flat
            .iter()
            .position(|(candidate, _, _, _)| candidate == key)
        {
            self.select_flat_index(index);
        }
    }

    fn select_flat_index(&mut self, index: usize) -> bool {
        let flat = self.flatten_nodes();
        let Some((key, title, _, disabled)) = flat.get(index) else {
            return false;
        };
        if *disabled {
            return false;
        }
        self.value.clone_from(title);
        self.value_key.clone_from(key);
        self.pending_change.replace(Some(key.clone()));
        self.close();
        true
    }

    fn move_highlight(&mut self, forward: bool) {
        let flat = self.flatten_nodes();
        let enabled: Vec<usize> = flat
            .iter()
            .enumerate()
            .filter_map(|(index, (_, _, _, disabled))| (!disabled).then_some(index))
            .collect();
        if enabled.is_empty() {
            return;
        }
        let current = self
            .highlighted_option
            .as_ref()
            .and_then(|key| enabled.iter().position(|index| flat[*index].0 == *key));
        let position = match (current, forward) {
            (Some(position), true) => (position + 1) % enabled.len(),
            (Some(position), false) => (position + enabled.len() - 1) % enabled.len(),
            (None, true) => 0,
            (None, false) => enabled.len() - 1,
        };
        let index = enabled[position];
        self.highlighted_option = Some(flat[index].0.clone());
        self.reveal_index(index, flat.len());
    }

    fn move_to_edge(&mut self, first: bool) {
        let flat = self.flatten_nodes();
        let index = if first {
            flat.iter().position(|(_, _, _, disabled)| !disabled)
        } else {
            flat.iter().rposition(|(_, _, _, disabled)| !disabled)
        };
        let Some(index) = index else {
            return;
        };
        self.highlighted_option = Some(flat[index].0.clone());
        self.reveal_index(index, flat.len());
    }

    fn reveal_index(&mut self, index: usize, row_count: usize) {
        let viewport_height = self.dropdown_viewport_height(row_count);
        let old_offset = self.dropdown_scroll.scroll_offset();
        let row_top = index as f32 * DROPDOWN_ROW_HEIGHT;
        let row_bottom = row_top + DROPDOWN_ROW_HEIGHT;
        let new_offset = if row_top < old_offset {
            row_top
        } else if row_bottom > old_offset + viewport_height {
            row_bottom - viewport_height
        } else {
            old_offset
        };
        self.dropdown_scroll.set_scroll_offset(new_offset);
        self.dropdown_scroll
            .clamp_to_content(row_count, DROPDOWN_ROW_HEIGHT, viewport_height);
        let applied = self.dropdown_scroll.scroll_offset() - old_offset;
        if applied.abs() > 0.01 {
            self.push_scroll_delta(0.0, applied);
        }
    }
}

fn tree_select_dirty_rect(frame: Rect, item_count: usize) -> Rect {
    frame.union(&tree_select_popup_rect(frame, item_count))
}

fn tree_select_popup_rect(frame: Rect, item_count: usize) -> Rect {
    let list_h = (item_count.max(1) as f32 * DROPDOWN_ROW_HEIGHT).min(MAX_DROPDOWN_VIEWPORT_HEIGHT);
    Rect::new(
        frame.x,
        frame.y + frame.h,
        frame.w.max(MIN_DROPDOWN_WIDTH),
        list_h,
    )
}

fn point_in_half_open_rect(rect: Rect, point: Point) -> bool {
    point.x >= rect.x && point.x < rect.x + rect.w && point.y >= rect.y && point.y < rect.y + rect.h
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
