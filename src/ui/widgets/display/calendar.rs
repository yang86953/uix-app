//! Calendar widget — 日历组件，Ant Design 风格。
//!
//! 月视图展示日期，支持选中日期、月份切换。

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::ui::widgets::input::date_picker::{days_in_month, first_weekday, Date};
use crate::ui::SnapshotFields;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent, WidgetTree,
};
use std::cell::Cell;

const HEADER_HEIGHT: f32 = 40.0;
const DEFAULT_CELL_SIZE: f32 = 40.0;
const MIN_CELL_SIZE: f32 = 20.0;
const MIN_YEAR: i32 = 1;
const MAX_YEAR: i32 = 9999;

// Calendar — 日历组件。
component! {
    pub struct Calendar {
        year: Cell<i32>,
        month: Cell<usize>,
        selected_date: Cell<Option<Date>>,
        focused_day: Cell<usize>,
        cell_size: f32,
        year_jump: bool,
        focused: bool,
        pending_change: Cell<Option<Date>>,
    }

    tab_index => (&self) -> i32 { 1 }

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
                let width = self.cell_size * 7.0;
                if pos.x < 0.0 || pos.x >= width || pos.y < 0.0 {
                    return EventResult::NotHandled;
                }
                if pos.y < HEADER_HEIGHT {
                    if pos.x < HEADER_HEIGHT {
                        self.shift_months(-self.navigation_month_delta());
                        return EventResult::Handled;
                    }
                    if pos.x >= width - HEADER_HEIGHT {
                        self.shift_months(self.navigation_month_delta());
                        return EventResult::Handled;
                    }
                    return EventResult::NotHandled;
                }
                if let Some(day) = self.day_at(*pos) {
                    self.focused_day.set(day);
                    self.commit_selection();
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
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } => match key {
                KeyCode::Left => {
                    self.shift_focused_day(-1);
                    EventResult::Handled
                }
                KeyCode::Right => {
                    self.shift_focused_day(1);
                    EventResult::Handled
                }
                KeyCode::Up => {
                    self.shift_focused_day(-7);
                    EventResult::Handled
                }
                KeyCode::Down => {
                    self.shift_focused_day(7);
                    EventResult::Handled
                }
                KeyCode::Home => {
                    self.focused_day.set(1);
                    EventResult::Handled
                }
                KeyCode::End => {
                    self.focused_day
                        .set(days_in_month(self.year.get(), self.month.get()));
                    EventResult::Handled
                }
                KeyCode::PageUp => {
                    self.shift_months(-self.navigation_month_delta());
                    EventResult::Handled
                }
                KeyCode::PageDown => {
                    self.shift_months(self.navigation_month_delta());
                    EventResult::Handled
                }
                KeyCode::Enter | KeyCode::Space => {
                    self.commit_selection();
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .take()
            .map(|date| SemanticEvent::change(id, date.format()))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let primary = ctx.tokens().color_primary();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_quaternary();
        let border = ctx.tokens().color_border_secondary();
        let _fill = ctx.tokens().color_fill_tertiary();
        let _bg = ctx.tokens().color_bg_container();
        let cs = self.cell_size;
        let y = frame.y;
        let x = frame.x;
        let selected = self.selected_date.get();
        let focused_day = self.focused_day.get();
        let cur_year = self.year.get();
        let cur_month = self.month.get();

        // 标题行
        let title = format!("{}年{}月", cur_year, cur_month);
        let header_rect = Rect::new(x, y, cs * 7.0, 40.0);
        let arrow_y = ctx.visual_center_y(header_rect, 14.0);
        let title_y = ctx.visual_center_y(header_rect, 15.0);
        ctx.draw_text(&title, Point::new(x + cs * 3.0 - 24.0, title_y), text, 15.0);
        ctx.draw_text("◀", Point::new(x + 10.0, arrow_y), primary, 14.0);
        ctx.draw_text("▶", Point::new(x + cs * 7.0 - 28.0, arrow_y), primary, 14.0);

        // 星期行
        let loc = crate::ui::locale::use_locale();
        let weekdays = loc.weekdays_short;
        for (i, wd) in weekdays.iter().enumerate() {
            ctx.draw_text(wd, Point::new(x + i as f32 * cs + cs * 0.5 - 5.0, y + 28.0), text_sec, 11.0);
        }

        // 日期网格
        let dim = days_in_month(cur_year, cur_month);
        let wd = first_weekday(cur_year, cur_month);
        let grid_y = y + 40.0;
        for d in 1..=dim {
            let col = (wd + d - 1) % 7;
            let row = (wd + d - 1) / 7;
            let cx = x + col as f32 * cs;
            let cy = grid_y + row as f32 * cs;
            let cell_rect = Rect::new(cx, cy, cs, cs);
            let date = Date::new(cur_year, cur_month, d);
            let is_sel = selected == Some(date);
            let is_focused = self.focused && focused_day == d;
            let is_weekend = col >= 5;
            let tc = if is_sel { Color::white() } else if is_weekend { ctx.tokens().color_error() } else { text };
            if is_sel {
                ctx.fill_rect(cell_rect, primary, Some(Radius::uniform(4.0)));
            }
            if is_focused {
                ctx.stroke_rect(
                    cell_rect,
                    primary,
                    1.5,
                    Some(Radius::uniform(4.0)),
                );
            }
            let cell_text_y = ctx.visual_center_y(cell_rect, 13.0);
            ctx.draw_text(&d.to_string(), Point::new(cx + cs * 0.5 - 6.0, cell_text_y), tc, 13.0);
            if row > 0 || col > 0 {
                ctx.stroke_rect(cell_rect, border, 0.5, None);
            }
        }
    }
}

impl Calendar {
    pub fn new() -> Self {
        Self {
            year: Cell::new(2026),
            month: Cell::new(6),
            selected_date: Cell::new(None),
            focused_day: Cell::new(1),
            cell_size: DEFAULT_CELL_SIZE,
            year_jump: false,
            focused: false,
            pending_change: Cell::new(None),
        }
    }
    pub fn cell_size(mut self, s: f32) -> Self {
        self.cell_size = Self::normalize_cell_size(s);
        self
    }
    pub fn selected_day(&self) -> Option<usize> {
        self.selected_date.get().map(|date| date.day)
    }
    pub fn selected_date(&self) -> Option<Date> {
        self.selected_date.get()
    }
    pub fn displayed_month(&self) -> (i32, usize) {
        (self.year.get(), self.month.get())
    }
    /// 设置初始选中日期；后续 reconcile 保留用户运行态选择。
    pub fn default_date(self, date: Date) -> Self {
        let date = Self::normalize_date(date);
        self.year.set(date.year);
        self.month.set(date.month);
        self.focused_day.set(date.day);
        self.selected_date.set(Some(date));
        self
    }
    /// 设置初始展示月份；后续 reconcile 保留用户导航到的月份。
    pub fn default_displayed(self, year: i32, month: usize) -> Self {
        let date = Self::normalize_date(Date::new(year, month, self.focused_day.get()));
        self.year.set(date.year);
        self.month.set(date.month);
        self.focused_day.set(date.day);
        self
    }
    /// 启用后，标题箭头与 PageUp / PageDown 按整年跳转；默认按月。
    pub fn year_jump(mut self, v: bool) -> Self {
        self.year_jump = v;
        self
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(self.cell_size * 7.0, self.cell_size * 6.0 + HEADER_HEIGHT)
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Calendar {
            cell_size: self.cell_size,
            year_jump: self.year_jump,
            year: self.year.get(),
            month: self.month.get(),
            selected: self.selected_date.get(),
            focused_day: self.focused_day.get(),
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.cell_size = Self::normalize_cell_size(next.cell_size);
        self.year_jump = next.year_jump;
    }

    fn normalize_cell_size(value: f32) -> f32 {
        if value.is_finite() {
            value.max(MIN_CELL_SIZE)
        } else {
            DEFAULT_CELL_SIZE
        }
    }

    fn normalize_date(date: Date) -> Date {
        Date::new(date.year.clamp(MIN_YEAR, MAX_YEAR), date.month, date.day)
    }

    fn navigation_month_delta(&self) -> i32 {
        if self.year_jump {
            12
        } else {
            1
        }
    }

    fn day_at(&self, pos: Point) -> Option<usize> {
        let grid_height = self.cell_size * 6.0;
        if pos.y < HEADER_HEIGHT || pos.y >= HEADER_HEIGHT + grid_height {
            return None;
        }
        let column = (pos.x / self.cell_size) as usize;
        let row = ((pos.y - HEADER_HEIGHT) / self.cell_size) as usize;
        if column >= 7 || row >= 6 {
            return None;
        }
        let weekday = first_weekday(self.year.get(), self.month.get());
        let ordinal = row * 7 + column;
        let day = ordinal.checked_sub(weekday)? + 1;
        (day <= days_in_month(self.year.get(), self.month.get())).then_some(day)
    }

    fn shift_months(&self, delta: i32) {
        let year = self.year.get().clamp(MIN_YEAR, MAX_YEAR);
        let month = self.month.get().clamp(1, 12);
        let current = i64::from(year - MIN_YEAR) * 12 + month as i64 - 1;
        let maximum = i64::from(MAX_YEAR - MIN_YEAR + 1) * 12 - 1;
        let shifted = (current + i64::from(delta)).clamp(0, maximum);
        let next_year = MIN_YEAR + (shifted / 12) as i32;
        let next_month = (shifted % 12) as usize + 1;
        self.year.set(next_year);
        self.month.set(next_month);
        self.clamp_focused_day();
    }

    fn shift_focused_day(&self, delta: i32) {
        let mut year = self.year.get();
        let mut month = self.month.get();
        let mut day = self.focused_day.get() as i32 + delta;
        loop {
            if day < 1 {
                let current = i64::from(year - MIN_YEAR) * 12 + month as i64 - 1;
                if current == 0 {
                    day = 1;
                    break;
                }
                let previous = current - 1;
                year = MIN_YEAR + (previous / 12) as i32;
                month = (previous % 12) as usize + 1;
                day += days_in_month(year, month) as i32;
                continue;
            }
            let days = days_in_month(year, month) as i32;
            if day > days {
                let current = i64::from(year - MIN_YEAR) * 12 + month as i64 - 1;
                let maximum = i64::from(MAX_YEAR - MIN_YEAR + 1) * 12 - 1;
                if current == maximum {
                    day = days;
                    break;
                }
                day -= days;
                let next = current + 1;
                year = MIN_YEAR + (next / 12) as i32;
                month = (next % 12) as usize + 1;
                continue;
            }
            break;
        }
        self.year.set(year);
        self.month.set(month);
        self.focused_day.set(day as usize);
    }

    fn clamp_focused_day(&self) {
        self.focused_day.set(
            self.focused_day
                .get()
                .clamp(1, days_in_month(self.year.get(), self.month.get())),
        );
    }

    fn commit_selection(&self) {
        let date = Date::new(self.year.get(), self.month.get(), self.focused_day.get());
        if self.selected_date.get() != Some(date) {
            self.selected_date.set(Some(date));
            self.pending_change.set(Some(date));
        }
    }
}

impl Default for Calendar {
    fn default() -> Self {
        Self::new()
    }
}
