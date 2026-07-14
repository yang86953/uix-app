//! DatePicker 日期选择器 — 弹出日历选择日期。
//!
//! 基于 Calendar 的日期逻辑，增加弹出面板和选中回显。

use std::cell::Cell;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Color;
use crate::ui::state::State;
use crate::ui::{
    ComponentId, EventResult, KeyCode, SemanticEvent, SnapshotFields, SystemEvent, WidgetTree,
};

/// 归一到合法年月日的公历日期。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Date {
    pub year: i32,
    pub month: usize,
    pub day: usize,
}

impl Date {
    pub fn new(year: i32, month: usize, day: usize) -> Self {
        let month = month.clamp(1, 12);
        let day = day.clamp(1, days_in_month(year, month));
        Self { year, month, day }
    }
    pub fn format(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
    /// 返回当前 UTC 日期。
    pub fn today() -> Self {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO);
        let days = elapsed.as_secs() / 86_400;
        let (year, month, day) = crate::core::diagnostic::timestamp::days_to_date(days);
        Self::new(year, month as usize, day as usize)
    }
}

fn days_in_month(year: i32, month: usize) -> usize {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

fn first_weekday(year: i32, month: usize) -> usize {
    let m = if month <= 2 { month + 12 } else { month };
    let y = if month <= 2 {
        (year - 1) as usize
    } else {
        year as usize
    };
    let c = y / 100;
    let y_mod = y % 100;
    let w = (1usize + (13 * (m + 1)) / 5 + y_mod + y_mod / 4 + c / 4).wrapping_sub(2 * c) % 7;
    (w + 6) % 7
}

component! {
    /// DatePicker — 日期选择器。
    pub struct DatePicker {
        value: Cell<Date>,
        value_binding: Option<State<Date>>,
        view_year: Cell<i32>,
        view_month: Cell<usize>,
        placeholder: String,
        open: Cell<bool>,
        focused: bool,
        hover_day: Cell<Option<usize>>,
        last_frame: Cell<Option<Rect>>,
        pending_change: Cell<Option<Date>>,
    }


    tab_index => (&self) -> i32 { 1 }
    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        self.sync_bound_value();
        match event {
            SystemEvent::PointerDown { pos, .. } => {
                self.focused = true;
                if !self.open.get() {
                    self.open.set(true);
                    let val = self.selected_or_today();
                    self.view_year.set(val.year);
                    self.view_month.set(val.month);
                    return EventResult::Handled;
                }

                if let Some(frame) = self.last_frame.get() {
                    let popup_y = frame.y + frame.h + 2.0;
                    let rel = Point::new(pos.x - frame.x, pos.y - popup_y);

                    if rel.y >= 0.0 && rel.y < 32.0 {
                        if rel.x > frame.w * 0.5 && rel.x < frame.w * 0.5 + 40.0 {
                            let (y, m) = next_month(self.view_year.get(), self.view_month.get());
                            self.view_year.set(y); self.view_month.set(m);
                        } else if rel.x > frame.w * 0.5 - 50.0 && rel.x < frame.w * 0.5 - 10.0 {
                            let (y, m) = prev_month(self.view_year.get(), self.view_month.get());
                            self.view_year.set(y); self.view_month.set(m);
                        }
                        return EventResult::Handled;
                    }

                    let grid_y = rel.y - 32.0;
                    if grid_y >= 0.0 {
                        let row = (grid_y / 30.0) as usize;
                        let col = ((rel.x - 8.0) / ((frame.w - 16.0) / 7.0)) as usize;
                        if row < 6 && col < 7 {
                            let day = row * 7 + col + 1;
                            let fwd = first_weekday(self.view_year.get(), self.view_month.get());
                            if day > fwd {
                                let d = day - fwd;
                                if d <= days_in_month(self.view_year.get(), self.view_month.get()) {
                                    let new_val = Date::new(self.view_year.get(), self.view_month.get(), d);
                                    self.commit_value(new_val);
                                    self.open.set(false);
                                }
                            }
                        }
                    }
                }
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if self.open.get() {
                    if let Some(frame) = self.last_frame.get() {
                        let popup_y = frame.y + frame.h + 2.0;
                        let rel = Point::new(pos.x - frame.x, pos.y - popup_y);
                        let grid_y = rel.y - 32.0;
                        if grid_y >= 0.0 {
                            let row = (grid_y / 30.0) as usize;
                            let col = ((rel.x - 8.0) / ((frame.w - 16.0) / 7.0)) as usize;
                            if row < 6 && col < 7 {
                                let day = row * 7 + col + 1;
                                let fwd = first_weekday(self.view_year.get(), self.view_month.get());
                                if day > fwd {
                                    let d = day - fwd;
                                    if d <= days_in_month(self.view_year.get(), self.view_month.get()) {
                                        self.hover_day.set(Some(d));
                                        return EventResult::Handled;
                                    }
                                }
                            }
                        }
                        self.hover_day.set(None);
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerLeave => { self.hover_day.set(None); EventResult::NotHandled }
            SystemEvent::FocusOut => { self.focused = false; self.open.set(false); EventResult::Handled }
            SystemEvent::KeyDown { key, .. } => {
                if self.open.get() {
                    match key {
                        KeyCode::Escape => { self.open.set(false); }
                        KeyCode::Left => {
                            let (y, m) = prev_month(self.view_year.get(), self.view_month.get());
                            self.view_year.set(y); self.view_month.set(m);
                        }
                        KeyCode::Right => {
                            let (y, m) = next_month(self.view_year.get(), self.view_month.get());
                            self.view_year.set(y); self.view_month.set(m);
                        }
                        _ => {}
                    }
                } else {
                    if *key == KeyCode::Space || *key == KeyCode::Enter {
                        self.open.set(true);
                        let val = self.selected_or_today();
                        self.view_year.set(val.year);
                        self.view_month.set(val.month);
                    }
                }
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .take()
            .map(|value| SemanticEvent::change(id, value.format()))
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.sync_bound_value();
        self.last_frame.set(Some(frame));
        let primary = ctx.tokens().color_primary();
        let primary_bg = ctx.tokens().color_primary_bg();
        let border_color = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let text_tertiary = ctx.tokens().color_text_tertiary();
        let bg_elevated = ctx.tokens().color_bg_elevated();
        let fill_tertiary = ctx.tokens().color_fill_tertiary();
        let border_radius_sm = ctx.tokens().border_radius_sm();
        let radius = Some(crate::draw::Radius::uniform(border_radius_sm));

        let val = self.value.get();
        let is_default = val == Date::default();
        ctx.fill_rect(frame, Color::white(), radius);
        ctx.stroke_rect(frame, if self.focused { primary } else { border_color },
            if self.focused { 2.0 } else { 1.0 }, radius);

        let input_text_y = ctx.visual_center_y(frame, 14.0);
        if is_default {
            ctx.draw_text(&self.placeholder,
                Point::new(frame.x + 12.0, input_text_y),
                text_tertiary, 14.0);
        } else {
            let formatted = val.format();
            ctx.draw_text(&formatted,
                Point::new(frame.x + 12.0, input_text_y),
                text_color, 14.0);
        }

        let icon_y = ctx.visual_center_y(frame, 12.0);
        ctx.draw_text("📅", Point::new(frame.x + frame.w - 24.0, icon_y),
            text_secondary, 12.0);

        if self.open.get() {
            let cell_w = (frame.w - 16.0) / 7.0;
            let cell_h = 30.0;
            let popup_h = 32.0 + 7.0 * cell_h + 8.0;
            let popup = Rect::new(frame.x, frame.y + frame.h + 2.0, frame.w, popup_h);

            ctx.fill_rect(popup, bg_elevated, radius);
            ctx.stroke_rect(popup, border_color, 1.0, radius);

            let vy = self.view_year.get();
            let vm = self.view_month.get();
            let title = format!("{}年{:02}月", vy, vm);
            let header_rect = Rect::new(popup.x, popup.y, popup.w, 32.0);
            let arrow_y = ctx.visual_center_y(header_rect, 12.0);
            let title_y = ctx.visual_center_y(header_rect, 14.0);
            ctx.draw_text(&title,
                Point::new(popup.x + popup.w * 0.5 - 28.0, title_y),
                text_color, 14.0);

            ctx.draw_text("◀", Point::new(popup.x + popup.w * 0.5 - 50.0, arrow_y),
                text_secondary, 12.0);
            ctx.draw_text("▶", Point::new(popup.x + popup.w * 0.5 + 30.0, arrow_y),
                text_secondary, 12.0);

            let loc = crate::ui::locale::use_locale();
            let weekdays = loc.weekdays_short;
            for (i, &w) in weekdays.iter().enumerate() {
                let x = popup.x + 8.0 + i as f32 * cell_w;
                ctx.draw_text(w, Point::new(x + cell_w * 0.3, popup.y + 34.0), text_tertiary, 10.0);
            }

            let fwd = first_weekday(vy, vm);
            let dim = days_in_month(vy, vm);
            let sel = self.value.get();
            let hover_d = self.hover_day.get();
            for day in 1..=dim {
                let idx = fwd + day - 1;
                let row = idx / 7;
                let col = idx % 7;
                let x = popup.x + 8.0 + col as f32 * cell_w;
                let y = popup.y + 32.0 + 4.0 + row as f32 * cell_h + 16.0;

                let is_selected = sel.day == day && sel.month == vm && sel.year == vy;
                let is_hovered = hover_d == Some(day);
                if is_selected {
                    ctx.fill_rect(Rect::new(x - 2.0, y - cell_h * 0.5 + 2.0, cell_w, cell_h),
                        primary_bg, None);
                } else if is_hovered {
                    ctx.fill_rect(Rect::new(x - 2.0, y - cell_h * 0.5 + 2.0, cell_w, cell_h),
                        fill_tertiary, None);
                }

                let cell_rect = Rect::new(x - 2.0, y - cell_h * 0.5 + 2.0, cell_w, cell_h);
                let text_y = ctx.visual_center_y(cell_rect, 12.0);
                ctx.draw_text(&day.to_string(),
                    Point::new(x + cell_w * 0.3, text_y),
                    if is_selected { primary } else { text_color }, 12.0);
            }
        }
    }
}

impl DatePicker {
    fn intrinsic_size(&self) -> Size {
        Size::new(160.0, 32.0)
    }

    pub fn new() -> Self {
        let today = Date::today();
        Self {
            value: Cell::new(Date::default()),
            value_binding: None,
            view_year: Cell::new(today.year),
            view_month: Cell::new(today.month),
            placeholder: crate::ui::locale::use_locale().placeholder.to_owned(),
            open: Cell::new(false),
            focused: false,
            hover_day: Cell::new(None),
            last_frame: Cell::new(None),
            pending_change: Cell::new(None),
        }
    }

    /// 将日期绑定到外部 `State<Date>`。
    pub fn value(mut self, state: &State<Date>) -> Self {
        self.value_binding = Some(state.clone());
        self.value.set(state.get());
        self
    }

    /// 设置非受控日期选择器的初始值。
    pub fn default_value(mut self, value: Date) -> Self {
        self.value_binding = None;
        self.value.set(value);
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// 返回组件当前缓存值；controlled 用法应以绑定的 `State` 为真值来源。
    pub fn current_value(&self) -> Date {
        self.value.get()
    }
    pub fn is_open(&self) -> bool {
        self.open.get()
    }
    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        let value = self.current_value();
        SnapshotFields::DatePicker {
            placeholder: self.placeholder.clone(),
            value: (value != Date::default()).then(|| value.format()),
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let controlled_value = next.value_binding.as_ref().map(|_| next.value.get());
        self.value_binding = next.value_binding;
        if let Some(value) = controlled_value {
            self.value.set(value);
        }
        self.placeholder = next.placeholder;
    }

    fn selected_or_today(&self) -> Date {
        let value = self.value.get();
        if value == Date::default() {
            Date::today()
        } else {
            value
        }
    }

    fn sync_bound_value(&self) {
        if let Some(state) = self.value_binding.as_ref() {
            self.value.set(state.get());
        }
    }

    fn commit_value(&self, value: Date) {
        if self.value.get() == value {
            return;
        }
        self.value.set(value);
        if let Some(state) = self.value_binding.as_ref() {
            if state.get() != value {
                state.set(value);
            }
        }
        self.pending_change.set(Some(value));
    }
}

impl Default for DatePicker {
    fn default() -> Self {
        Self::new()
    }
}

fn next_month(y: i32, m: usize) -> (i32, usize) {
    if m >= 12 {
        (y + 1, 1)
    } else {
        (y, m + 1)
    }
}
fn prev_month(y: i32, m: usize) -> (i32, usize) {
    if m <= 1 {
        (y - 1, 12)
    } else {
        (y, m - 1)
    }
}
