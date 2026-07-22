//! Cascader widget - linked multi-level popup selection.

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::ui::animation::{presets, TransitionPlayer};
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent,
    WidgetTree,
};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;

const TRIGGER_HEIGHT: f32 = 32.0;
const POPUP_GAP: f32 = 2.0;
const POPUP_COLUMN_MIN_WIDTH: f32 = 200.0;
const POPUP_HEIGHT: f32 = 200.0;
const ITEM_HEIGHT: f32 = 32.0;
const WHEEL_STEP: f32 = 40.0;

#[derive(Debug, Clone, PartialEq)]
pub struct CascaderOption {
    pub label: String,
    pub value: String,
    pub children: Vec<CascaderOption>,
    pub disabled: bool,
}

impl CascaderOption {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            children: vec![],
            disabled: false,
        }
    }

    pub fn children(mut self, children: Vec<CascaderOption>) -> Self {
        self.children = children;
        self
    }

    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CascaderValue {
    pub labels: Vec<String>,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct CascaderSearchResult {
    value: CascaderValue,
    disabled: bool,
    loading: bool,
}

component! {
    pub struct Cascader {
        options: Vec<CascaderOption>,
        selected: CascaderValue,
        current_levels: Vec<Vec<CascaderOption>>,
        level_indices: Vec<usize>,
        scroll_offsets: Vec<f32>,
        hovered_option: Option<(usize, usize)>,
        open: bool,
        transition: TransitionPlayer,
        closing: bool,
        transition_dirty: bool,
        loading_children: HashSet<String>,
        loading_phase: f32,
        loading_dirty: bool,
        searchable: bool,
        search_query: String,
        search_results: Vec<CascaderSearchResult>,
        search_index: usize,
        search_scroll_offset: f32,
        search_cursor_char: usize,
        search_cursor_rect: Cell<Rect>,
        search_glyph_xs: RefCell<Vec<f32>>,
        search_text_scroll_x: Cell<f32>,
        placeholder: String,
        focused: bool,
        last_frame: Cell<Option<Rect>>,
        pending_change: RefCell<Option<String>>,
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .borrow_mut()
            .take()
            .map(|value| SemanticEvent::change(id, value))
    }

    tab_index => (&self) -> i32 { 1 }

    accepts_text_input => (&self) -> bool { self.searchable }

    text_input_cursor_rect => (&self) -> Rect { self.search_cursor_rect.get() }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
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
                if frame.contains(*pos) {
                    if self.searchable {
                        self.set_search_cursor_from_x(pos.x);
                        if self.open {
                            return EventResult::Handled;
                        }
                    }
                    if self.open {
                        self.close();
                    } else {
                        self.open();
                    }
                    return EventResult::Handled;
                }

                if self.open {
                    if self.search_active() {
                        if let Some(index) = self.search_result_at(frame, *pos) {
                            self.hovered_option = Some((0, index));
                            self.select_search_result(index);
                            return EventResult::Handled;
                        }
                    } else if let Some((level, index)) = self.option_at(frame, *pos) {
                        self.hovered_option = Some((level, index));
                        self.select_option(level, index);
                        return EventResult::Handled;
                    }
                    if point_in_half_open_rect(
                        cascader_popup_rect(frame, self.visible_column_count()),
                        *pos,
                    ) {
                        return EventResult::Handled;
                    }
                }

                if self.is_present() {
                    self.close();
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if !self.open {
                    return EventResult::NotHandled;
                }
                let frame = self.interaction_frame();
                let next = if self.search_active() {
                    self.search_result_at(frame, *pos).map(|index| (0, index))
                } else {
                    self.option_at(frame, *pos)
                };
                if self.hovered_option != next {
                    self.hovered_option = next;
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerLeave => {
                if self.hovered_option.take().is_some() {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
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
            SystemEvent::Wheel { pos, delta } => {
                if !self.open || !delta.y.is_finite() {
                    return EventResult::NotHandled;
                }
                let frame = self.interaction_frame();
                let popup = cascader_popup_rect(frame, self.visible_column_count());
                if !point_in_half_open_rect(popup, *pos) {
                    return EventResult::NotHandled;
                }
                if self.search_active() {
                    return if self.scroll_search_results(delta.y * WHEEL_STEP) {
                        EventResult::Handled
                    } else {
                        EventResult::NotHandled
                    };
                }
                let column_width = cascader_column_width(frame);
                let level = ((pos.x - popup.x) / column_width).floor() as usize;
                if self.scroll_level(level, delta.y * WHEEL_STEP) {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
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
                    KeyCode::Escape => {
                        self.close();
                        EventResult::Handled
                    }
                    KeyCode::Down => {
                        if self.search_active() {
                            self.move_search_highlight(true);
                        } else {
                            self.move_highlight(true);
                        }
                        EventResult::Handled
                    }
                    KeyCode::Up => {
                        if self.search_active() {
                            self.move_search_highlight(false);
                        } else {
                            self.move_highlight(false);
                        }
                        EventResult::Handled
                    }
                    KeyCode::Enter if self.search_active() => {
                        self.select_search_result(self.search_index);
                        EventResult::Handled
                    }
                    KeyCode::Backspace if self.searchable => {
                        if self.delete_previous_search_char() {
                            EventResult::Handled
                        } else {
                            EventResult::NotHandled
                        }
                    }
                    KeyCode::Delete if self.searchable => {
                        if self.delete_next_search_char() {
                            EventResult::Handled
                        } else {
                            EventResult::NotHandled
                        }
                    }
                    KeyCode::Left if self.search_active() => {
                        self.search_cursor_char = self.search_cursor_char.saturating_sub(1);
                        EventResult::Handled
                    }
                    KeyCode::Right if self.search_active() => {
                        self.search_cursor_char = (self.search_cursor_char + 1)
                            .min(self.search_query.chars().count());
                        EventResult::Handled
                    }
                    KeyCode::Home if self.search_active() => {
                        self.search_cursor_char = 0;
                        EventResult::Handled
                    }
                    KeyCode::End if self.search_active() => {
                        self.search_cursor_char = self.search_query.chars().count();
                        EventResult::Handled
                    }
                    KeyCode::Right | KeyCode::Enter | KeyCode::Space
                        if !self.search_active() =>
                    {
                        self.activate_highlight();
                        EventResult::Handled
                    }
                    KeyCode::Left => {
                        self.return_to_parent();
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
                    _ => EventResult::NotHandled,
                }
            }
            SystemEvent::TextInput { text } | SystemEvent::Paste { text }
                if self.searchable =>
            {
                if self.insert_search_text(text) {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            _ => EventResult::NotHandled,
        }
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        let loc = crate::ui::locale::use_locale();
        let primary = ctx.tokens().color_primary();
        let border_color = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let text_tertiary = ctx.tokens().color_text_tertiary();
        let bg_elevated = ctx.tokens().color_bg_elevated();
        let nominal_height = TRIGGER_HEIGHT;
        let scale = if nominal_height > 0.0 {
            (frame.h / nominal_height).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let font_size = 14.0 * scale;
        let horizontal_padding = 12.0 * scale;
        let arrow_gap = 4.0 * scale;
        let arrow_slot_width = 24.0 * scale;
        let arrow_right_inset = 4.0 * scale;
        let trigger_radius = Some(Radius::uniform(
            (ctx.tokens().border_radius_sm() * scale).min(frame.h.max(0.0) * 0.5),
        ));

        ctx.push_clip(frame);
        ctx.fill_rect(frame, ctx.tokens().color_bg_container(), trigger_radius);
        ctx.stroke_rect(
            frame,
            if self.focused { primary } else { border_color },
            if self.focused { 2.0 } else { 1.0 },
            trigger_radius,
        );

        if font_size > 0.0 && frame.w > 0.0 {
            let arrow_frame = Rect::new(
                frame.x + frame.w - arrow_right_inset - arrow_slot_width,
                frame.y,
                arrow_slot_width,
                frame.h,
            );
            let text_left = frame.x + horizontal_padding;
            let text_right = (arrow_frame.x - arrow_gap).max(text_left);
            let text_area = Rect::new(text_left, frame.y, text_right - text_left, frame.h);
            if text_area.w > 0.0 {
                let mut glyph_xs = Vec::with_capacity(self.search_query.chars().count() + 1);
                let mut prefix = String::new();
                glyph_xs.push(0.0);
                for ch in self.search_query.chars() {
                    prefix.push(ch);
                    glyph_xs.push(ctx.measure_text(&prefix, font_size).w);
                }
                let cursor_index = self
                    .search_cursor_char
                    .min(glyph_xs.len().saturating_sub(1));
                let total_width = glyph_xs.last().copied().unwrap_or(0.0);
                let cursor_offset = glyph_xs.get(cursor_index).copied().unwrap_or(0.0);
                let max_scroll = (total_width - text_area.w).max(0.0);
                let mut scroll = self.search_text_scroll_x.get().clamp(0.0, max_scroll);
                if cursor_offset < scroll {
                    scroll = cursor_offset;
                } else if cursor_offset > scroll + text_area.w {
                    scroll = cursor_offset - text_area.w;
                }
                scroll = scroll.clamp(0.0, max_scroll);
                self.search_text_scroll_x.set(scroll);
                *self.search_glyph_xs.borrow_mut() = glyph_xs;

                ctx.push_clip(text_area);
                let draw_y = ctx.visual_center_y(frame, font_size);
                let showing_query = self.search_active();
                if showing_query {
                    ctx.draw_text(
                        &self.search_query,
                        Point::new(text_left - scroll, draw_y),
                        text_color,
                        font_size,
                    );
                } else if self.selected.labels.is_empty() {
                    ctx.draw_text(
                        &self.placeholder,
                        Point::new(text_left, draw_y),
                        text_tertiary,
                        font_size,
                    );
                } else {
                    let display_text = self.selected.labels.join(loc.cascader_separator);
                    ctx.draw_text(
                        &display_text,
                        Point::new(text_left, draw_y),
                        text_color,
                        font_size,
                    );
                }
                let caret_height = (18.0 * scale).min(text_area.h);
                let caret_x = (text_area.x + cursor_offset - scroll)
                    .clamp(text_area.x, text_area.x + text_area.w);
                let caret = Rect::new(
                    caret_x,
                    text_area.y + (text_area.h - caret_height) * 0.5,
                    1.0,
                    caret_height,
                );
                self.search_cursor_rect.set(caret);
                if self.searchable && self.focused && self.open {
                    ctx.fill_rect(caret, primary, None);
                }
                ctx.pop_clip();
            } else {
                self.search_glyph_xs.replace(vec![0.0]);
                self.search_text_scroll_x.set(0.0);
                self.search_cursor_rect
                    .set(Rect::new(text_area.x, text_area.y, 0.0, text_area.h));
            }
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                if self.is_present() {
                    "chevron-up"
                } else {
                    "chevron-down"
                },
                arrow_frame,
                text_secondary,
                font_size,
            );
        }
        ctx.pop_clip();

        if !self.is_present() {
            return;
        }

        let opacity = self.transition.opacity_progress.clamp(0.0, 1.0);
        let popup = cascader_popup_rect(frame, self.visible_column_count());
        let column_width = cascader_column_width(frame);
        let bg_elevated = fade_color(bg_elevated, opacity);
        let border_color = fade_color(border_color, opacity);
        let text_color = fade_color(text_color, opacity);
        let text_secondary = fade_color(text_secondary, opacity);
        let text_tertiary = fade_color(text_tertiary, opacity);
        let primary_bg = fade_color(ctx.tokens().color_primary_bg(), opacity);
        let panel_radius = Some(Radius::uniform(ctx.tokens().border_radius_sm()));

        ctx.push_clip(popup);
        ctx.fill_rect(popup, bg_elevated, panel_radius);

        if self.search_active() {
            let column = Rect::new(popup.x, popup.y, column_width, popup.h);
            ctx.push_clip(column);
            if self.search_results.is_empty() {
                ctx.text_center(loc.no_data, column, text_tertiary, 14.0);
            }
            for (index, result) in self.search_results.iter().enumerate() {
                let y = column.y + index as f32 * ITEM_HEIGHT - self.search_scroll_offset;
                if y + ITEM_HEIGHT <= column.y || y >= column.y + column.h {
                    continue;
                }
                let row = Rect::new(column.x, y, column.w, ITEM_HEIGHT);
                if self.hovered_option == Some((0, index)) || self.search_index == index {
                    ctx.fill_rect(row, primary_bg, None);
                }
                let loading_width = if result.loading { 24.0 } else { 0.0 };
                let text_area = Rect::new(
                    row.x + 12.0,
                    row.y,
                    (row.w - 24.0 - loading_width).max(0.0),
                    row.h,
                );
                if text_area.w > 0.0 {
                    let label = result.value.labels.join(loc.cascader_separator);
                    ctx.push_clip(text_area);
                    let text_y = ctx.visual_center_y(row, 14.0);
                    ctx.draw_text(
                        &label,
                        Point::new(text_area.x, text_y),
                        if result.disabled { text_tertiary } else { text_color },
                        14.0,
                    );
                    ctx.pop_clip();
                }
                if result.loading {
                    paint_loading_spinner(ctx, row, self.loading_phase, text_secondary);
                }
            }
            ctx.pop_clip();
        } else {
        for (level, options) in self.current_levels.iter().enumerate() {
            let column = Rect::new(
                popup.x + level as f32 * column_width,
                popup.y,
                column_width,
                popup.h,
            );
            ctx.push_clip(column);
            if options.is_empty() {
                ctx.text_center(loc.no_data, column, text_tertiary, 14.0);
            }
            let scroll = self.scroll_offsets.get(level).copied().unwrap_or(0.0);
            for (index, option) in options.iter().enumerate() {
                let y = column.y + index as f32 * ITEM_HEIGHT - scroll;
                if y + ITEM_HEIGHT <= column.y || y >= column.y + column.h {
                    continue;
                }
                let row = Rect::new(column.x, y, column.w, ITEM_HEIGHT);
                let highlighted = self.hovered_option == Some((level, index))
                    || self.level_indices.get(level) == Some(&index);
                if highlighted {
                    ctx.fill_rect(row, primary_bg, None);
                }

                let loading = self.loading_children.contains(&option.value);
                let arrow_width = if option.children.is_empty() && !loading {
                    0.0
                } else {
                    24.0
                };
                let text_area = Rect::new(
                    row.x + 12.0,
                    row.y,
                    (row.w - 24.0 - arrow_width).max(0.0),
                    row.h,
                );
                if text_area.w > 0.0 {
                    ctx.push_clip(text_area);
                    let text_y = ctx.visual_center_y(row, 14.0);
                    ctx.draw_text(
                        &option.label,
                        Point::new(text_area.x, text_y),
                        if option.disabled { text_tertiary } else { text_color },
                        14.0,
                    );
                    ctx.pop_clip();
                }
                if loading {
                    paint_loading_spinner(ctx, row, self.loading_phase, text_secondary);
                } else if !option.children.is_empty() {
                    let arrow = Rect::new(row.x + row.w - 24.0, row.y, 24.0, row.h);
                    crate::ui::widgets::icon::Icon::paint_in_frame(
                        ctx,
                        "chevron-right",
                        arrow,
                        text_secondary,
                        14.0,
                    );
                }
            }
            ctx.pop_clip();
            if level > 0 {
                ctx.fill_rect(
                    Rect::new(column.x, column.y, 1.0, column.h),
                    border_color,
                    None,
                );
            }
        }
        }
        ctx.stroke_rect(popup, border_color, 1.0, panel_radius);
        ctx.pop_clip();
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        cascader_dirty_rect(frame, self.damage_column_count())
    }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        if self.is_present() {
            cascader_dirty_rect(frame, self.visible_column_count())
        } else {
            frame
        }
    }

    overlay_entry => (&self, id: crate::ui::ComponentId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        self.is_present().then(|| {
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Popover)
                .bounds(cascader_dirty_rect(frame, self.visible_column_count()))
                .z_index(900)
        })
    }

    update_animation => (&mut self, dt: f64) -> bool {
        if !self.is_present() {
            self.transition_dirty = false;
            self.loading_dirty = false;
            return false;
        }

        let transition_active = if self.transition.finished {
            self.transition_dirty = false;
            false
        } else {
            self.transition.update(dt);
            self.transition_dirty = true;

            if self.closing && self.transition.finished {
                self.open = false;
                self.closing = false;
            }

            self.is_present() && !self.transition.finished
        };

        self.loading_dirty = false;
        let loading_active = self.is_present() && self.has_visible_loading_child();
        if loading_active {
            let before = self.loading_phase;
            self.loading_phase = (self.loading_phase
                + dt.max(0.0) as f32 * std::f32::consts::TAU / 0.8)
                .rem_euclid(std::f32::consts::TAU);
            self.loading_dirty = (self.loading_phase - before).abs() > f32::EPSILON;
        }

        transition_active || loading_active
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.transition_dirty || self.loading_dirty {
            cascader_dirty_rect(frame, self.visible_column_count())
        } else {
            Rect::zero()
        }
    }
}

impl Cascader {
    fn intrinsic_size(&self) -> Size {
        Size::new(120.0, 32.0)
    }

    pub fn new(options: Vec<CascaderOption>, placeholder: impl Into<String>) -> Self {
        Self {
            options: options.clone(),
            selected: CascaderValue {
                labels: vec![],
                values: vec![],
            },
            current_levels: vec![options],
            level_indices: vec![0],
            scroll_offsets: vec![0.0],
            hovered_option: None,
            open: false,
            transition: TransitionPlayer::new(presets::tooltip_enter()),
            closing: false,
            transition_dirty: false,
            loading_children: HashSet::new(),
            loading_phase: 0.0,
            loading_dirty: false,
            searchable: false,
            search_query: String::new(),
            search_results: Vec::new(),
            search_index: 0,
            search_scroll_offset: 0.0,
            search_cursor_char: 0,
            search_cursor_rect: Cell::new(Rect::zero()),
            search_glyph_xs: RefCell::new(vec![0.0]),
            search_text_scroll_x: Cell::new(0.0),
            placeholder: placeholder.into(),
            focused: false,
            last_frame: Cell::new(None),
            pending_change: RefCell::new(None),
        }
    }

    fn init_levels(&mut self) {
        self.current_levels = vec![self.options.clone()];
        self.level_indices = vec![first_enabled_index(&self.options).unwrap_or(0)];
        self.scroll_offsets = vec![0.0];
        self.hovered_option = None;
    }

    pub fn select_option(&mut self, level: usize, index: usize) {
        if level >= self.current_levels.len() {
            return;
        }
        let opt = match self.current_levels[level].get(index) {
            Some(o) => o.clone(),
            None => return,
        };
        if opt.disabled {
            return;
        }

        self.level_indices.truncate(level + 1);
        self.scroll_offsets.truncate(level + 1);
        if let Some(highlighted) = self.level_indices.get_mut(level) {
            *highlighted = index;
        } else {
            self.level_indices.push(index);
        }
        self.selected.labels.truncate(level);
        self.selected.values.truncate(level);
        self.selected.labels.push(opt.label.clone());
        self.selected.values.push(opt.value.clone());

        self.current_levels.truncate(level + 1);
        if self.loading_children.contains(&opt.value) {
            return;
        }
        if !opt.children.is_empty() {
            let child_highlight = first_enabled_index(&opt.children).unwrap_or(0);
            self.current_levels.push(opt.children);
            self.level_indices.push(child_highlight);
            self.scroll_offsets.push(0.0);
        } else {
            self.pending_change
                .replace(Some(self.selected.values.join("/")));
            self.close();
        }
    }

    fn activate_highlight(&mut self) {
        let Some(level) = self.current_levels.len().checked_sub(1) else {
            return;
        };
        let Some(index) = self.level_indices.get(level).copied() else {
            return;
        };
        self.select_option(level, index);
    }

    fn move_highlight(&mut self, forward: bool) {
        let Some(level) = self.current_levels.len().checked_sub(1) else {
            return;
        };
        let Some(options) = self.current_levels.get(level) else {
            return;
        };
        let current = self.level_indices.get(level).copied().unwrap_or(0);
        if let Some(next) = next_enabled_index(options, current, forward) {
            if let Some(highlighted) = self.level_indices.get_mut(level) {
                *highlighted = next;
            }
            self.ensure_highlight_visible(level);
        }
    }

    fn return_to_parent(&mut self) {
        if self.current_levels.len() <= 1 {
            return;
        }
        self.current_levels.pop();
        self.level_indices.pop();
        self.scroll_offsets.pop();
        self.hovered_option = None;
    }

    pub fn selected(&self) -> &CascaderValue {
        &self.selected
    }

    pub fn placeholder(mut self, p: impl Into<String>) -> Self {
        self.placeholder = p.into();
        self
    }

    /// 标记指定 value 的子级正在加载；加载项保留高亮与路径，但不进入或提交。
    pub fn loading_child(mut self, value: impl Into<String>, loading: bool) -> Self {
        let value = value.into();
        if loading {
            self.loading_children.insert(value);
        } else {
            self.loading_children.remove(&value);
            if self.loading_children.is_empty() {
                self.loading_phase = 0.0;
                self.loading_dirty = false;
            }
        }
        self
    }

    /// 启用跨层级路径搜索；查询只过滤候选，提交时仍返回完整 value 路径。
    pub fn searchable(mut self, searchable: bool) -> Self {
        self.searchable = searchable;
        if !searchable {
            self.clear_search();
        }
        self
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn is_present(&self) -> bool {
        self.open || self.closing
    }

    pub fn open(&mut self) {
        self.init_levels();
        self.refresh_search_results();
        self.open = true;
        self.closing = false;
        self.hovered_option = None;
        self.transition = TransitionPlayer::new(presets::tooltip_enter());
        self.transition_dirty = true;
    }

    pub fn close(&mut self) {
        if !self.is_present() {
            self.open = false;
            self.closing = false;
            self.transition_dirty = false;
            self.clear_search();
            return;
        }

        self.open = false;
        self.closing = true;
        self.hovered_option = None;
        self.clear_search();
        self.transition = TransitionPlayer::new(presets::tooltip_exit());
        self.transition_dirty = true;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        let mut loading_children = self.loading_children.iter().cloned().collect::<Vec<_>>();
        loading_children.sort();
        SnapshotFields::Cascader {
            options: self.options.clone(),
            placeholder: self.placeholder.clone(),
            selected_labels: self.selected.labels.clone(),
            selected_values: self.selected.values.clone(),
            open: self.open,
            loading_children,
            searchable: self.searchable,
            search_query: self.search_query.clone(),
            search_results: self
                .search_results
                .iter()
                .map(|result| result.value.clone())
                .collect(),
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let options_changed = self.options != next.options;
        let searchable_changed = self.searchable != next.searchable;
        let loading_changed = self.loading_children != next.loading_children;
        self.options = next.options;
        self.placeholder = next.placeholder;
        self.loading_children = next.loading_children;
        self.searchable = next.searchable;
        if self.loading_children.is_empty() {
            self.loading_phase = 0.0;
            self.loading_dirty = false;
        }
        if self.is_present() && options_changed {
            self.init_levels();
        }
        if !self.searchable {
            self.clear_search();
        } else if self.is_present() && (options_changed || searchable_changed || loading_changed) {
            self.refresh_search_results();
        }
    }

    fn has_visible_loading_child(&self) -> bool {
        if self.search_active() {
            return self.search_results.iter().any(|result| result.loading);
        }
        self.current_levels
            .iter()
            .flatten()
            .any(|option| self.loading_children.contains(&option.value))
    }

    fn search_active(&self) -> bool {
        self.searchable && !self.search_query.is_empty()
    }

    fn visible_column_count(&self) -> usize {
        if self.search_active() {
            1
        } else {
            self.current_levels.len()
        }
    }

    fn damage_column_count(&self) -> usize {
        self.visible_column_count().max(self.current_levels.len())
    }

    fn refresh_search_results(&mut self) {
        self.search_results.clear();
        self.search_index = 0;
        self.search_scroll_offset = 0.0;
        self.hovered_option = None;
        if !self.search_active() {
            return;
        }

        let query = self.search_query.to_lowercase();
        let mut path = CascaderValue {
            labels: Vec::new(),
            values: Vec::new(),
        };
        collect_search_results(
            &self.options,
            &self.loading_children,
            &query,
            &mut path,
            false,
            &mut self.search_results,
        );
        self.search_index = self
            .search_results
            .iter()
            .position(|result| !result.disabled)
            .unwrap_or(0);
    }

    fn clear_search(&mut self) {
        self.search_query.clear();
        self.search_results.clear();
        self.search_index = 0;
        self.search_scroll_offset = 0.0;
        self.search_cursor_char = 0;
        self.search_glyph_xs.replace(vec![0.0]);
        self.search_text_scroll_x.set(0.0);
        self.search_cursor_rect.set(Rect::zero());
    }

    fn select_search_result(&mut self, index: usize) -> bool {
        let Some(result) = self.search_results.get(index).cloned() else {
            return false;
        };
        if result.disabled {
            return false;
        }

        self.search_index = index;
        self.selected = result.value;
        if result.loading {
            return true;
        }
        self.pending_change
            .replace(Some(self.selected.values.join("/")));
        self.close();
        true
    }

    fn move_search_highlight(&mut self, forward: bool) {
        let len = self.search_results.len();
        if len == 0 {
            return;
        }
        for step in 1..=len {
            let index = if forward {
                (self.search_index + step) % len
            } else {
                (self.search_index + len - (step % len)) % len
            };
            if !self.search_results[index].disabled {
                self.search_index = index;
                self.ensure_search_highlight_visible();
                return;
            }
        }
    }

    fn ensure_search_highlight_visible(&mut self) {
        if self.search_index >= self.search_results.len() {
            return;
        }
        let row_top = self.search_index as f32 * ITEM_HEIGHT;
        let row_bottom = row_top + ITEM_HEIGHT;
        if row_top < self.search_scroll_offset {
            self.search_scroll_offset = row_top;
        } else if row_bottom > self.search_scroll_offset + POPUP_HEIGHT {
            self.search_scroll_offset = row_bottom - POPUP_HEIGHT;
        }
        let max_scroll = (self.search_results.len() as f32 * ITEM_HEIGHT - POPUP_HEIGHT).max(0.0);
        self.search_scroll_offset = self.search_scroll_offset.clamp(0.0, max_scroll);
    }

    fn scroll_search_results(&mut self, delta: f32) -> bool {
        if !delta.is_finite() {
            return false;
        }
        let max_scroll = (self.search_results.len() as f32 * ITEM_HEIGHT - POPUP_HEIGHT).max(0.0);
        let next = (self.search_scroll_offset + delta).clamp(0.0, max_scroll);
        if (next - self.search_scroll_offset).abs() <= f32::EPSILON {
            false
        } else {
            self.search_scroll_offset = next;
            self.hovered_option = None;
            true
        }
    }

    fn search_result_at(&self, frame: Rect, pos: Point) -> Option<usize> {
        let popup = cascader_popup_rect(frame, 1);
        if !point_in_half_open_rect(popup, pos) {
            return None;
        }
        let index = ((pos.y - popup.y + self.search_scroll_offset) / ITEM_HEIGHT).floor() as usize;
        (index < self.search_results.len()).then_some(index)
    }

    fn set_search_cursor_from_x(&mut self, x: f32) {
        let glyph_xs = self.search_glyph_xs.borrow();
        if glyph_xs.len() != self.search_query.chars().count() + 1 {
            self.search_cursor_char = self.search_query.chars().count();
            return;
        }
        let scale = (self.interaction_frame().h / TRIGGER_HEIGHT).clamp(0.0, 1.0);
        let target = (x - 12.0 * scale + self.search_text_scroll_x.get()).max(0.0);
        let mut index = glyph_xs.len().saturating_sub(1);
        for candidate in 0..glyph_xs.len().saturating_sub(1) {
            let midpoint = (glyph_xs[candidate] + glyph_xs[candidate + 1]) * 0.5;
            if target < midpoint {
                index = candidate;
                break;
            }
        }
        self.search_cursor_char = index;
    }

    fn insert_search_text(&mut self, text: &str) -> bool {
        if text.is_empty() || text.chars().any(char::is_control) {
            return false;
        }
        let byte_index = byte_index_for_char(&self.search_query, self.search_cursor_char);
        self.search_query.insert_str(byte_index, text);
        self.search_cursor_char += text.chars().count();
        if self.open {
            self.refresh_search_results();
        } else {
            self.open();
        }
        true
    }

    fn delete_previous_search_char(&mut self) -> bool {
        if self.search_cursor_char == 0 || self.search_query.is_empty() {
            return false;
        }
        let end = byte_index_for_char(&self.search_query, self.search_cursor_char);
        let start = byte_index_for_char(&self.search_query, self.search_cursor_char - 1);
        self.search_query.replace_range(start..end, "");
        self.search_cursor_char -= 1;
        self.refresh_search_results();
        true
    }

    fn delete_next_search_char(&mut self) -> bool {
        let char_count = self.search_query.chars().count();
        if self.search_cursor_char >= char_count {
            return false;
        }
        let start = byte_index_for_char(&self.search_query, self.search_cursor_char);
        let end = byte_index_for_char(&self.search_query, self.search_cursor_char + 1);
        self.search_query.replace_range(start..end, "");
        self.refresh_search_results();
        true
    }

    fn interaction_frame(&self) -> Rect {
        self.last_frame.get().unwrap_or_else(|| {
            Rect::new(0.0, 0.0, self.intrinsic_size().w, self.intrinsic_size().h)
        })
    }

    fn option_at(&self, frame: Rect, pos: Point) -> Option<(usize, usize)> {
        let popup = cascader_popup_rect(frame, self.current_levels.len());
        if !point_in_half_open_rect(popup, pos) {
            return None;
        }
        let column_width = cascader_column_width(frame);
        let level = ((pos.x - popup.x) / column_width).floor() as usize;
        let options = self.current_levels.get(level)?;
        let scroll = self.scroll_offsets.get(level).copied().unwrap_or(0.0);
        let index = ((pos.y - popup.y + scroll) / ITEM_HEIGHT).floor() as usize;
        (index < options.len()).then_some((level, index))
    }

    fn scroll_level(&mut self, level: usize, delta: f32) -> bool {
        if !delta.is_finite() {
            return false;
        }
        let Some(options) = self.current_levels.get(level) else {
            return false;
        };
        let Some(offset) = self.scroll_offsets.get_mut(level) else {
            return false;
        };
        let max_scroll = (options.len() as f32 * ITEM_HEIGHT - POPUP_HEIGHT).max(0.0);
        let next = (*offset + delta).clamp(0.0, max_scroll);
        if (next - *offset).abs() <= f32::EPSILON {
            false
        } else {
            *offset = next;
            self.hovered_option = None;
            true
        }
    }

    fn ensure_highlight_visible(&mut self, level: usize) {
        let Some(index) = self.level_indices.get(level).copied() else {
            return;
        };
        let Some(options) = self.current_levels.get(level) else {
            return;
        };
        let Some(offset) = self.scroll_offsets.get_mut(level) else {
            return;
        };
        let row_top = index as f32 * ITEM_HEIGHT;
        let row_bottom = row_top + ITEM_HEIGHT;
        if row_top < *offset {
            *offset = row_top;
        } else if row_bottom > *offset + POPUP_HEIGHT {
            *offset = row_bottom - POPUP_HEIGHT;
        }
        let max_scroll = (options.len() as f32 * ITEM_HEIGHT - POPUP_HEIGHT).max(0.0);
        *offset = (*offset).clamp(0.0, max_scroll);
    }

    fn move_to_edge(&mut self, first: bool) {
        let Some(level) = self.current_levels.len().checked_sub(1) else {
            return;
        };
        let Some(options) = self.current_levels.get(level) else {
            return;
        };
        let next = if first {
            first_enabled_index(options)
        } else {
            options.iter().rposition(|option| !option.disabled)
        };
        if let Some(next) = next {
            if let Some(highlighted) = self.level_indices.get_mut(level) {
                *highlighted = next;
            }
            self.ensure_highlight_visible(level);
        }
    }
}

fn collect_search_results(
    options: &[CascaderOption],
    loading_children: &HashSet<String>,
    query: &str,
    path: &mut CascaderValue,
    ancestor_disabled: bool,
    results: &mut Vec<CascaderSearchResult>,
) {
    for option in options {
        path.labels.push(option.label.clone());
        path.values.push(option.value.clone());
        let disabled = ancestor_disabled || option.disabled;
        let loading = loading_children.contains(&option.value);
        if loading || option.children.is_empty() {
            let searchable_path = path.labels.join(" / ").to_lowercase();
            if searchable_path.contains(query) {
                results.push(CascaderSearchResult {
                    value: path.clone(),
                    disabled,
                    loading,
                });
            }
        } else {
            collect_search_results(
                &option.children,
                loading_children,
                query,
                path,
                disabled,
                results,
            );
        }
        path.labels.pop();
        path.values.pop();
    }
}

fn byte_index_for_char(text: &str, char_index: usize) -> usize {
    text.char_indices()
        .nth(char_index)
        .map(|(index, _)| index)
        .unwrap_or(text.len())
}

fn paint_loading_spinner(ctx: &mut PaintContext<'_>, row: Rect, phase: f32, color: Color) {
    let slot = Rect::new(row.x + row.w - 24.0, row.y, 24.0, row.h);
    let radius = 4.5_f32.min(slot.w.min(slot.h) * 0.25);
    if radius > 0.0 {
        ctx.stroke_arc(
            slot.x + slot.w * 0.5,
            slot.y + slot.h * 0.5,
            radius,
            phase,
            phase + std::f32::consts::PI * 1.45,
            color,
            1.6,
        );
    }
}

fn first_enabled_index(options: &[CascaderOption]) -> Option<usize> {
    options.iter().position(|option| !option.disabled)
}

fn next_enabled_index(options: &[CascaderOption], current: usize, forward: bool) -> Option<usize> {
    let len = options.len();
    if len == 0 {
        return None;
    }

    (1..=len)
        .map(|step| {
            if forward {
                (current + step) % len
            } else {
                (current + len - (step % len)) % len
            }
        })
        .find(|index| !options[*index].disabled)
}

fn cascader_column_width(frame: Rect) -> f32 {
    frame.w.max(POPUP_COLUMN_MIN_WIDTH)
}

fn cascader_popup_rect(frame: Rect, level_count: usize) -> Rect {
    Rect::new(
        frame.x,
        frame.y + frame.h + POPUP_GAP,
        cascader_column_width(frame) * level_count.max(1) as f32,
        POPUP_HEIGHT,
    )
}

fn cascader_dirty_rect(frame: Rect, level_count: usize) -> Rect {
    frame.union(&cascader_popup_rect(frame, level_count))
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
