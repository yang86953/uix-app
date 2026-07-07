//! DatePicker 日期选择器 — 弹出日历选择日期。
//!
//! 基于 Calendar 的日期逻辑，增加弹出面板和选中回显。

use std::cell::Cell;

use crate::core::{Constraints, Point, Rect, Size};
use crate::define_widget;
use crate::draw::painting::PaintContext;
use crate::draw::{traits::GraphicsEngine, Color};
use crate::ui::{
    EventResult, KeyCode, SemanticEvent, SnapshotFields, SystemEvent, WidgetId, WidgetTree,
};

// 日期结构（复用 Calendar 中的日期逻辑）
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DateValue {
    pub year: i32,
    pub month: usize,
    pub day: usize,
}

impl DateValue {
    pub fn new(year: i32, month: usize, day: usize) -> Self {
        let d = day.min(days_in_month(year, month));
        Self {
            year,
            month: month.clamp(1, 12),
            day: d.max(1),
        }
    }
    pub fn format(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
    pub fn today() -> Self {
        Self {
            year: 2026,
            month: 6,
            day: 20,
        }
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

define_widget! {
    /// DatePicker — 日期选择器。
    pub struct DatePicker {
        value: Cell<DateValue>,
        view_year: Cell<i32>,
        view_month: Cell<usize>,
        placeholder: String,
        open: Cell<bool>,
        focused: bool,
        hover_day: Cell<Option<usize>>,
        last_frame: Cell<Option<Rect>>,
        pending_change: Cell<Option<DateValue>>,
    }


    tab_index => (&self) -> i32 { 1 }
    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        self.intrinsic_size()
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown { pos, .. } => {
                self.focused = true;
                if !self.open.get() {
                    self.open.set(true);
                    let val = self.value.get();
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
                                    let new_val = DateValue::new(self.view_year.get(), self.view_month.get(), d);
                                    if self.value.get() != new_val {
                                        self.value.set(new_val);
                                        self.pending_change.set(Some(new_val));
                                    }
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
                        let val = self.value.get();
                        self.view_year.set(val.year);
                        self.view_month.set(val.month);
                    }
                }
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .take()
            .map(|value| SemanticEvent::change(id, value.format()))
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
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
        let is_default = val == DateValue::default();
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

    pub fn new(placeholder: impl Into<String>) -> Self {
        Self {
            value: Cell::new(DateValue::default()),
            view_year: Cell::new(2026),
            view_month: Cell::new(6),
            placeholder: placeholder.into(),
            open: Cell::new(false),
            focused: false,
            hover_day: Cell::new(None),
            last_frame: Cell::new(None),
            pending_change: Cell::new(None),
        }
    }

    pub fn value(self, v: DateValue) -> Self {
        self.value.set(v);
        self
    }
    pub fn selected(&self) -> DateValue {
        self.value.get()
    }
    pub fn set_value(&mut self, v: DateValue) {
        self.value.set(v);
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::DatePicker {
            placeholder: self.placeholder.clone(),
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::traits::WidgetLayout;

    #[test]
    fn measure_clamps_date_picker_size() {
        let measured =
            DatePicker::new("Pick date").measure(Constraints::loose(Size::new(100.0, 20.0)));

        assert_eq!(measured, Size::new(100.0, 20.0));
    }
}
