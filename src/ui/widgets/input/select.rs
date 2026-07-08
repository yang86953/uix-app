use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::ui::animation::{presets, TransitionPlayer};
use crate::ui::foundation::virtual_scroll::VirtualListScroll;
use crate::ui::{
    ComponentId, EventResult, KeyCode, SemanticEvent, SnapshotFields, SystemEvent, WidgetTree,
};
use std::cell::{Cell, RefCell};

const DROPDOWN_ROW_HEIGHT: f32 = 28.0;
const DROPDOWN_TRIGGER_HEIGHT: f32 = 32.0;
const MAX_DROPDOWN_VIEWPORT_HEIGHT: f32 = 280.0;

/// 选项组。
#[derive(Debug, Clone, PartialEq)]
pub struct OptGroup {
    pub label: String,
    pub options: Vec<String>,
}

impl OptGroup {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            options: Vec::new(),
        }
    }

    pub fn add(mut self, opt: &str) -> Self {
        self.options.push(opt.to_string());
        self
    }
}

component! {
    pub struct Select {
        options: Vec<String>,
        optgroups: Vec<OptGroup>,
        selected: usize,
        selected_multi: Vec<usize>,
        open: bool,
        disabled: bool,
        hovered: bool,
        focused: bool,
        transition: TransitionPlayer,
        closing: bool,
        transition_dirty: bool,
        placeholder: String,
        hovered_option: Option<usize>,
        pending_change: RefCell<Option<String>>,
        multiple: bool,
        search: bool,
        dropdown_scroll: VirtualListScroll,
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
        if self.disabled {
            return EventResult::NotHandled;
        }

        let all_opts: Vec<&str> = self.all_options();
        match event {
            SystemEvent::PointerDown { pos, .. } => {
                if pos.y >= 0.0 && pos.y <= DROPDOWN_TRIGGER_HEIGHT {
                    if self.open {
                        self.close();
                    } else {
                        self.open();
                    }
                    self.hovered_option = None;
                    return EventResult::Handled;
                }

                if self.is_present() && pos.y > DROPDOWN_TRIGGER_HEIGHT {
                    if let Some(flat_idx) = self.dropdown_row_at_y(pos.y) {
                        if let Some(opt_idx) = self.flat_row_option_index(flat_idx) {
                            if self.multiple {
                                if let Some(multi_idx) =
                                    self.selected_multi.iter().position(|&i| i == opt_idx)
                                {
                                    self.selected_multi.remove(multi_idx);
                                } else {
                                    self.selected_multi.push(opt_idx);
                                }
                                self.pending_change.replace(Some(self.selected_multi_payload()));
                            } else {
                                if self.selected != opt_idx {
                                    self.selected = opt_idx;
                                    self.pending_change.replace(Some(opt_idx.to_string()));
                                }
                                self.close();
                            }
                            return EventResult::Handled;
                        }
                    }
                }

                self.close();
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if self.is_present() && pos.y > DROPDOWN_TRIGGER_HEIGHT {
                    self.hovered_option = self.dropdown_row_at_y(pos.y);
                } else {
                    self.hovered_option = None;
                }
                self.hovered = pos.y >= 0.0 && pos.y <= DROPDOWN_TRIGGER_HEIGHT;
                EventResult::Handled
            }
            SystemEvent::PointerEnter => {
                self.hovered = true;
                EventResult::Handled
            }
            SystemEvent::PointerLeave => {
                self.hovered = false;
                self.hovered_option = None;
                EventResult::Handled
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
            SystemEvent::Wheel { delta, pos, .. } => {
                if self.is_present() && pos.y > DROPDOWN_TRIGGER_HEIGHT {
                    let row_count = self.dropdown_row_count();
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
            SystemEvent::KeyDown { key, .. } => match key {
                KeyCode::Down => {
                    if self.open {
                        let next = self.selected + 1;
                        if next < all_opts.len() {
                            self.selected = next;
                            self.pending_change.replace(Some(next.to_string()));
                        }
                    } else {
                        self.open();
                    }
                    EventResult::Handled
                }
                KeyCode::Up => {
                    if self.open && self.selected > 0 {
                        let prev = self.selected - 1;
                        self.selected = prev;
                        self.pending_change.replace(Some(prev.to_string()));
                    }
                    EventResult::Handled
                }
                KeyCode::Enter | KeyCode::Space => {
                    if self.open {
                        self.close();
                    } else {
                        self.open();
                    }
                    EventResult::Handled
                }
                KeyCode::Escape => {
                    self.close();
                    EventResult::Handled
                }
                KeyCode::Backspace => {
                    if self.multiple && !self.selected_multi.is_empty() {
                        self.selected_multi.pop();
                        self.pending_change.replace(Some(self.selected_multi_payload()));
                    }
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .borrow_mut()
            .take()
            .map(|value| SemanticEvent::change(id, value))
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_bg_elevated();
        let border = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let primary = ctx.tokens().color_primary();
        let fill_quaternary = ctx.tokens().color_fill_quaternary();
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));

        let box_rect = Rect::new(frame.x, frame.y, frame.w, DROPDOWN_TRIGGER_HEIGHT);
        let box_bg = if self.disabled {
            fill_quaternary
        } else if self.hovered || self.open {
            ctx.tokens().color_bg_container()
        } else {
            bg
        };
        ctx.fill_rect(box_rect, box_bg, r);
        let border_color = if self.focused {
            primary
        } else if self.disabled {
            ctx.tokens().color_border_secondary()
        } else {
            border
        };
        ctx.stroke_rect(box_rect, border_color, if self.focused { 2.0 } else { 1.0 }, r);

        let box_rect_v = Rect::new(frame.x, frame.y, frame.w, DROPDOWN_TRIGGER_HEIGHT);
        let draw_y = ctx.visual_center_y(box_rect_v, 13.0);
        if self.multiple && !self.selected_multi.is_empty() {
            let all_opts: Vec<&str> = self.all_options();
            let mut x = frame.x + 8.0;
            for &idx in &self.selected_multi {
                if idx < all_opts.len() {
                    let tag = all_opts[idx];
                    let tag_w = tag.len() as f32 * 7.0 + 16.0;
                    ctx.fill_rect(
                        Rect::new(x, frame.y + 4.0, tag_w, 24.0),
                        ctx.tokens().color_fill_tertiary(),
                        Some(Radius::uniform(4.0)),
                    );
                    ctx.draw_text(tag, Point::new(x + 4.0, draw_y), text_color, 12.0);
                    ctx.draw_text(
                        "✕",
                        Point::new(x + tag_w - 14.0, draw_y),
                        text_secondary,
                        10.0,
                    );
                    x += tag_w + 4.0;
                }
            }
        } else {
            let display_text = if self.selected < self.all_options().len() {
                self.all_options()[self.selected]
            } else {
                ""
            };
            let (disp, color) = if display_text.is_empty() && !self.placeholder.is_empty() {
                (&self.placeholder as &str, text_secondary)
            } else if display_text.is_empty() {
                ("", text_secondary)
            } else {
                (display_text, text_color)
            };
            ctx.draw_text(disp, Point::new(frame.x + 10.0, draw_y), color, 13.0);
        }

        let arrow = if self.is_present() { "▲" } else { "▼" };
        let arrow_y = ctx.visual_center_y(box_rect_v, 10.0);
        ctx.draw_text(
            arrow,
            Point::new(frame.x + frame.w - 18.0, arrow_y),
            text_secondary,
            10.0,
        );

        if !self.is_present() {
            return;
        }

        let all_opts: Vec<&str> = self.all_options();
        if all_opts.is_empty() {
            return;
        }

        let opacity = self.transition.opacity_progress.clamp(0.0, 1.0);
        let bg = fade_color(ctx.tokens().color_bg_elevated(), opacity);
        let border = fade_color(ctx.tokens().color_border(), opacity);
        let text_color = fade_color(ctx.tokens().color_text(), opacity);
        let primary = fade_color(ctx.tokens().color_primary(), opacity);
        let primary_bg = fade_color(ctx.tokens().color_primary_bg(), opacity);
        let fill_tertiary = fade_color(ctx.tokens().color_fill_tertiary(), opacity);
        let text_sec = fade_color(ctx.tokens().color_text_secondary(), opacity);
        let group_header = fade_color(ctx.tokens().color_fill_quaternary(), opacity);

        let row_count = self.dropdown_row_count();
        let list_y = frame.y + DROPDOWN_TRIGGER_HEIGHT;
        let list_h = self.dropdown_viewport_height(row_count);
        let list_rect = Rect::new(frame.x, list_y, frame.w, list_h);
        let shadow = ctx.tokens().box_shadow_secondary();
        ctx.draw_box_shadow(
            list_rect,
            shadow.layer_1.2,
            shadow.layer_1.0,
            shadow.layer_1.1,
            shadow.layer_1.3,
            Some(Radius::uniform(ctx.tokens().border_radius_sm())),
        );
        ctx.fill_rect(list_rect, bg, Some(Radius::uniform(ctx.tokens().border_radius_sm())));
        ctx.stroke_rect(
            list_rect,
            border,
            1.0,
            Some(Radius::uniform(ctx.tokens().border_radius_sm())),
        );

        let scroll_offset = self.dropdown_scroll.scroll_offset();
        let (start, end) = self.dropdown_scroll.scroll_range(
            row_count,
            DROPDOWN_ROW_HEIGHT,
            list_h,
        );
        ctx.canvas_2d().push_clip(list_rect);

        for flat_idx in start..end {
            let item_y = list_y + flat_idx as f32 * DROPDOWN_ROW_HEIGHT - scroll_offset;
            if item_y + DROPDOWN_ROW_HEIGHT < list_y || item_y > list_y + list_h {
                continue;
            }

            if self.optgroups.is_empty() {
                if let Some(opt) = all_opts.get(flat_idx) {
                    self.render_option(
                        frame,
                        item_y,
                        flat_idx,
                        opt,
                        flat_idx,
                        ctx,
                        text_color,
                        primary,
                        primary_bg,
                        fill_tertiary,
                    );
                }
            } else {
                self.render_flat_row(
                    frame,
                    item_y,
                    flat_idx,
                    ctx,
                    text_color,
                    primary,
                    primary_bg,
                    fill_tertiary,
                    text_sec,
                    group_header,
                );
            }
        }

        ctx.canvas_2d().pop_clip();
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        select_dirty_rect(frame, self.dropdown_row_count())
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
            select_dirty_rect(frame, self.dropdown_row_count())
        } else {
            Rect::zero()
        }
    }
}

impl Select {
    fn intrinsic_size(&self) -> Size {
        if self.options.is_empty() && self.optgroups.is_empty() {
            return Size::new(120.0, 32.0);
        }

        let all_opts: Vec<&str> = self.all_options();
        let w = all_opts
            .iter()
            .map(|o| o.len() as f32 * 9.0 + 32.0)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(150.0)
            .max(120.0);
        Size::new(w, DROPDOWN_TRIGGER_HEIGHT)
    }

    fn dropdown_row_count(&self) -> usize {
        if self.optgroups.is_empty() {
            self.options.len()
        } else {
            self.optgroups
                .iter()
                .map(|g| 1 + g.options.len())
                .sum()
        }
    }

    fn dropdown_viewport_height(&self, row_count: usize) -> f32 {
        (row_count as f32 * DROPDOWN_ROW_HEIGHT).min(MAX_DROPDOWN_VIEWPORT_HEIGHT)
    }

    fn dropdown_row_at_y(&self, pos_y: f32) -> Option<usize> {
        if pos_y <= DROPDOWN_TRIGGER_HEIGHT {
            return None;
        }
        let local_y = pos_y - DROPDOWN_TRIGGER_HEIGHT + self.dropdown_scroll.scroll_offset();
        if local_y < 0.0 {
            return None;
        }
        let idx = (local_y / DROPDOWN_ROW_HEIGHT) as usize;
        if idx < self.dropdown_row_count() {
            Some(idx)
        } else {
            None
        }
    }

    fn flat_row_option_index(&self, flat_idx: usize) -> Option<usize> {
        if self.optgroups.is_empty() {
            return if flat_idx < self.options.len() {
                Some(flat_idx)
            } else {
                None
            };
        }
        let mut idx = 0;
        for group in &self.optgroups {
            if idx == flat_idx {
                return None;
            }
            idx += 1;
            for opt in &group.options {
                if idx == flat_idx {
                    return Some(self.option_index(&group.label, opt));
                }
                idx += 1;
            }
        }
        None
    }

    fn render_flat_row(
        &self,
        frame: Rect,
        item_y: f32,
        flat_idx: usize,
        ctx: &mut PaintContext,
        text_color: Color,
        primary: Color,
        primary_bg: Color,
        fill_tertiary: Color,
        text_sec: Color,
        group_header: Color,
    ) {
        let mut idx = 0;
        for group in &self.optgroups {
            if idx == flat_idx {
                let item_rect = Rect::new(frame.x, item_y, frame.w, DROPDOWN_ROW_HEIGHT);
                ctx.fill_rect(item_rect, group_header, None);
                let gy = ctx.visual_center_y(item_rect, 12.0);
                ctx.draw_text(&group.label, Point::new(frame.x + 10.0, gy), text_sec, 12.0);
                return;
            }
            idx += 1;
            for opt in &group.options {
                if idx == flat_idx {
                    let opt_idx = self.option_index(&group.label, opt);
                    self.render_option(
                        frame,
                        item_y,
                        flat_idx,
                        opt,
                        opt_idx,
                        ctx,
                        text_color,
                        primary,
                        primary_bg,
                        fill_tertiary,
                    );
                    return;
                }
                idx += 1;
            }
        }
    }

    fn push_scroll_delta(&self, dx: f32, dy: f32) {
        if dx.abs() <= 0.01 && dy.abs() <= 0.01 {
            return;
        }
        let current = self.scroll_delta_strip.get();
        self.scroll_delta_strip.set((current.0 + dx, current.1 + dy));
    }

    fn render_option(
        &self,
        frame: Rect,
        item_y: f32,
        flat_idx: usize,
        label: &str,
        opt_idx: usize,
        ctx: &mut PaintContext,
        text_color: Color,
        primary: Color,
        primary_bg: Color,
        fill_tertiary: Color,
    ) {
        let item_rect = Rect::new(frame.x, item_y, frame.w, DROPDOWN_ROW_HEIGHT);
        let is_hovered = self.hovered_option == Some(flat_idx);
        let is_selected = if self.multiple {
            self.selected_multi.contains(&opt_idx)
        } else {
            opt_idx == self.selected
        };

        if is_hovered || is_selected {
            let highlight = if is_hovered {
                fill_tertiary
            } else {
                primary_bg
            };
            ctx.fill_rect(item_rect, highlight, None);
        }

        let tc = if is_selected { primary } else { text_color };
        let draw_y = ctx.visual_center_y(item_rect, 13.0);
        if self.multiple {
            let check = if is_selected { "☑ " } else { "☐ " };
            ctx.draw_text(
                &format!("{}{}", check, label),
                Point::new(frame.x + 10.0, draw_y),
                tc,
                13.0,
            );
        } else {
            ctx.draw_text(label, Point::new(frame.x + 10.0, draw_y), tc, 13.0);
        }
    }

    fn all_options(&self) -> Vec<&str> {
        if !self.optgroups.is_empty() {
            self.optgroups
                .iter()
                .flat_map(|g| g.options.iter().map(|s| s.as_str()))
                .collect()
        } else {
            self.options.iter().map(|s| s.as_str()).collect()
        }
    }

    fn option_index(&self, _group_label: &str, opt: &str) -> usize {
        self.all_options()
            .iter()
            .position(|&s| s == opt)
            .unwrap_or(0)
    }

    fn selected_multi_payload(&self) -> String {
        self.selected_multi
            .iter()
            .map(|idx| idx.to_string())
            .collect::<Vec<_>>()
            .join(",")
    }
}

impl Default for Select {
    fn default() -> Self {
        Self::new()
    }
}

impl Select {
    pub fn new() -> Self {
        Self {
            options: Vec::new(),
            optgroups: Vec::new(),
            selected: 0,
            selected_multi: Vec::new(),
            open: false,
            disabled: false,
            hovered: false,
            focused: false,
            transition: TransitionPlayer::new(presets::tooltip_enter()),
            closing: false,
            transition_dirty: false,
            placeholder: String::new(),
            hovered_option: None,
            pending_change: RefCell::new(None),
            multiple: false,
            search: false,
            dropdown_scroll: VirtualListScroll::new(),
            scroll_delta_strip: Cell::new((0.0, 0.0)),
        }
    }

    pub fn options(mut self, opts: Vec<impl Into<String>>) -> Self {
        self.options = opts.into_iter().map(|s| s.into()).collect();
        self
    }

    pub fn optgroups(mut self, groups: Vec<OptGroup>) -> Self {
        self.optgroups = groups;
        self
    }

    pub fn selected(mut self, idx: usize) -> Self {
        self.selected = idx;
        self
    }

    pub fn placeholder(mut self, p: impl Into<String>) -> Self {
        self.placeholder = p.into();
        self
    }

    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }

    pub fn multiple(mut self, v: bool) -> Self {
        self.multiple = v;
        self
    }

    pub fn search(mut self, v: bool) -> Self {
        self.search = v;
        self
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
        SnapshotFields::Select {
            options: self.options.clone(),
            optgroups: self.optgroups.clone(),
            disabled: self.disabled,
            placeholder: self.placeholder.clone(),
            multiple: self.multiple,
            search: self.search,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.options = next.options;
        self.optgroups = next.optgroups;
        self.disabled = next.disabled;
        self.placeholder = next.placeholder;
        self.multiple = next.multiple;
        self.search = next.search;
        let row_count = self.dropdown_row_count();
        self.dropdown_scroll.clamp_to_content(
            row_count,
            DROPDOWN_ROW_HEIGHT,
            self.dropdown_viewport_height(row_count),
        );
    }
}

fn select_dirty_rect(frame: Rect, row_count: usize) -> Rect {
    let list_h = (row_count as f32 * DROPDOWN_ROW_HEIGHT).min(MAX_DROPDOWN_VIEWPORT_HEIGHT);
    let list = Rect::new(frame.x, frame.y + DROPDOWN_TRIGGER_HEIGHT, frame.w, list_h);
    let expanded = frame.union(&list);
    let expand = 8.0;
    Rect::new(
        expanded.x - expand,
        expanded.y - expand,
        expanded.w + expand * 2.0,
        expanded.h + expand * 2.0,
    )
}

fn fade_color(color: Color, opacity: f32) -> Color {
    let alpha = (color.a as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    color.with_alpha(alpha)
}

#[cfg(test)]
#[path = "../../../tests/ui/widgets/input/select.rs"]
mod tests;
