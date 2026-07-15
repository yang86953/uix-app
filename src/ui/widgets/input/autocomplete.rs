//! AutoComplete widget — 自动完成输入框，Ant Design 风格。
//!
//! 输入时弹出匹配选项列表，支持键盘导航选择。

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::ui::animation::{presets, TransitionPlayer};
use crate::ui::{EventResult, KeyCode, SnapshotFields, SystemEvent, WidgetTree};
use std::cell::Cell;

// AutoComplete — 自动完成输入框。
component! {
    pub struct AutoComplete {
        placeholder: String,
        value: String,
        options: Vec<String>,
        filtered: Vec<String>,
        open: bool,
        transition: TransitionPlayer,
        closing: bool,
        transition_dirty: bool,
        focus: bool,
        hovered: bool,
        selected_idx: usize,
        cursor_rect: Cell<Rect>,
    }

    tab_index => (&self) -> i32 { 1 }

    accepts_text_input => (&self) -> bool { true }

    text_input_cursor_rect => (&self) -> Rect { self.cursor_rect.get() }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown { pos, .. } => {
                if pos.y >= 0.0 && pos.y <= 32.0 {
                    self.focus = true;
                    self.open();
                    return EventResult::Handled;
                }
                // 点击选项
                if self.is_present() && pos.y > 32.0 {
                    let idx = ((pos.y - 32.0) / 28.0) as usize;
                    if idx < self.filtered.len() {
                        self.value = self.filtered[idx].clone();
                        self.close();
                        return EventResult::Handled;
                    }
                }
                self.close();
                EventResult::NotHandled
            }
            SystemEvent::PointerEnter => { self.hovered = true; EventResult::Handled }
            SystemEvent::PointerLeave => { self.hovered = false; EventResult::Handled }
            SystemEvent::FocusIn => {
                self.focus = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focus = false;
                self.close();
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } => {
                match key {
                    KeyCode::Down => {
                        if !self.open {
                            self.open();
                        } else if !self.filtered.is_empty() {
                            self.selected_idx = (self.selected_idx + 1).min(self.filtered.len() - 1);
                        }
                    }
                    KeyCode::Up if self.open => {
                        self.selected_idx = self.selected_idx.saturating_sub(1);
                    }
                    KeyCode::Enter if self.open => {
                        if self.selected_idx < self.filtered.len() {
                            self.value = self.filtered[self.selected_idx].clone();
                            self.close();
                        }
                    }
                    KeyCode::Escape => { self.close(); }
                    KeyCode::Backspace => {
                        if self.value.pop().is_none() {
                            return EventResult::NotHandled;
                        }
                        self.filter();
                        self.open();
                    }
                    _ => return EventResult::NotHandled,
                }
                EventResult::Handled
            }
            SystemEvent::TextInput { text } | SystemEvent::Paste { text } => {
                if text.is_empty() || text.chars().any(char::is_control) {
                    return EventResult::NotHandled;
                }
                self.value.push_str(text);
                self.filter();
                self.open();
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_bg_container();
        let border = ctx.tokens().color_border();
        let primary = ctx.tokens().color_primary();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_quaternary();
        let r = Some(Radius::uniform(ctx.tokens().border_radius()));
        let input_rect = Rect::new(frame.x, frame.y, frame.w, 32.0);
        let bc = if self.focus { primary } else { border };
        ctx.fill_rect(input_rect, bg, r);
        ctx.stroke_rect(input_rect, bc, if self.focus { 2.0 } else { 1.0 }, r);
        let display = if self.value.is_empty() { &self.placeholder } else { &self.value };
        let disp_c = if self.value.is_empty() { text_sec } else { text };
        let draw_y = ctx.visual_center_y(input_rect, 13.0);
        ctx.draw_text(display, Point::new(frame.x + 10.0, draw_y), disp_c, 13.0);
        let cursor_x = (frame.x + 10.0 + self.value.chars().count() as f32 * 7.0)
            .min(frame.x + frame.w - 10.0);
        let cursor_rect = Rect::new(cursor_x, frame.y + 7.0, 1.0, 18.0);
        self.cursor_rect.set(cursor_rect);
        if self.focus {
            ctx.fill_rect(cursor_rect, primary, None);
        }

        // 下拉选项
        if self.is_present() && !self.filtered.is_empty() {
            let opacity = self.transition.opacity_progress.clamp(0.0, 1.0);
            let menu_h = self.filtered.len() as f32 * 28.0;
            let menu_rect = Rect::new(frame.x, frame.y + 32.0, frame.w, menu_h);
            let fill = fade_color(ctx.tokens().color_fill_tertiary(), opacity);
            let bg_elev = fade_color(ctx.tokens().color_bg_elevated(), opacity);
            let border = fade_color(border, opacity);
            let text = fade_color(text, opacity);
            ctx.fill_rect(menu_rect, bg_elev, Some(Radius::uniform(ctx.tokens().border_radius_sm())));
            ctx.stroke_rect(menu_rect, border, 1.0, Some(Radius::uniform(ctx.tokens().border_radius_sm())));
            for (i, opt) in self.filtered.iter().enumerate() {
                let opt_y = frame.y + 32.0 + i as f32 * 28.0;
                let item_rect = Rect::new(frame.x, opt_y, frame.w, 28.0);
                if i == self.selected_idx {
                    ctx.fill_rect(item_rect, fill, None);
                }
                let py = ctx.visual_center_y(item_rect, 13.0);
                ctx.draw_text(opt, Point::new(frame.x + 10.0, py), text, 13.0);
            }
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        autocomplete_dirty_rect(frame, self.dropdown_damage_rows())
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
            autocomplete_dirty_rect(frame, self.dropdown_damage_rows())
        } else {
            Rect::zero()
        }
    }
}

impl AutoComplete {
    fn intrinsic_size(&self) -> Size {
        Size::new(200.0, 32.0)
    }

    pub fn new() -> Self {
        Self {
            placeholder: String::new(),
            value: String::new(),
            options: Vec::new(),
            filtered: Vec::new(),
            open: false,
            transition: TransitionPlayer::new(presets::tooltip_enter()),
            closing: false,
            transition_dirty: false,
            focus: false,
            hovered: false,
            selected_idx: 0,
            cursor_rect: Cell::new(Rect::zero()),
        }
    }
    pub fn placeholder(mut self, p: &str) -> Self {
        self.placeholder = p.to_string();
        self
    }
    pub fn options(mut self, opts: Vec<impl Into<String>>) -> Self {
        self.options = opts.into_iter().map(|s| s.into()).collect();
        self
    }
    pub fn value(&self) -> &str {
        &self.value
    }
    pub fn set_value(&mut self, v: &str) {
        self.value = v.to_string();
        if self.is_present() || self.focus {
            self.filter();
        }
    }
    fn filter(&mut self) {
        if self.value.is_empty() {
            self.filtered = self.options.clone();
        } else {
            let query = self.value.to_lowercase();
            self.filtered = self
                .options
                .iter()
                .filter(|option| option.to_lowercase().contains(&query))
                .cloned()
                .collect();
        }
        self.selected_idx = 0;
    }

    fn dropdown_damage_rows(&self) -> usize {
        self.options.len().max(self.filtered.len())
    }

    #[cfg(test)]
    pub(crate) fn filtered_options(&self) -> &[String] {
        &self.filtered
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn is_present(&self) -> bool {
        self.open || self.closing
    }

    pub fn open(&mut self) {
        self.filter();
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
        SnapshotFields::AutoComplete {
            placeholder: self.placeholder.clone(),
            options: self.options.clone(),
            value: self.value.clone(),
            open: self.open,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.placeholder = next.placeholder;
        self.options = next.options;
        if self.is_present() || self.focus {
            self.filter();
        }
    }
}

impl Default for AutoComplete {
    fn default() -> Self {
        Self::new()
    }
}

fn autocomplete_dirty_rect(frame: Rect, item_count: usize) -> Rect {
    let menu_h = item_count as f32 * 28.0;
    let menu = Rect::new(frame.x, frame.y + 32.0, frame.w, menu_h);
    frame.union(&menu)
}

fn fade_color(color: Color, opacity: f32) -> Color {
    let alpha = (color.a as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    color.with_alpha(alpha)
}
