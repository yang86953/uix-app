//! DatePicker 日期选择器 — 弹出日历选择日期。
//!
//! 基于 Calendar 的日期逻辑，增加弹出面板和选中回显。

use std::cell::Cell;

use uix_core::{Point, Rect, Size};
use crate::define_widget;
use uix_graphics::{Color, GraphicsEngine};
use crate::render_context::RenderContext;
use crate::widget::{EventResult, KeyCode, WidgetEvent, WidgetTree};

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
        Self { year, month: month.max(1).min(12), day: d.max(1) }
    }
    pub fn format(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
    pub fn today() -> Self {
        // 默认 2026-06-20（当前日期）
        Self { year: 2026, month: 6, day: 20 }
    }
}

fn days_in_month(year: i32, month: usize) -> usize {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 { 29 } else { 28 },
        _ => 30,
    }
}

fn first_weekday(year: i32, month: usize) -> usize {
    let m = if month <= 2 { month + 12 } else { month };
    let y = if month <= 2 { (year - 1) as usize } else { year as usize };
    let c = y / 100;
    let y_mod = y % 100;
    let w = (1usize + (13 * (m + 1)) / 5 + y_mod + y_mod / 4 + c / 4).wrapping_sub(2 * c) % 7;
    (w + 6) % 7
}

define_widget! {
    /// DatePicker — 日期选择器。
    pub struct DatePicker {
        /// 当前选中日期
        value: Cell<DateValue>,
        /// 浏览中的月份（可能不同于选中月份）
        view_year: Cell<i32>,
        view_month: Cell<usize>,
        /// 占位文本
        placeholder: String,
        /// 弹出
        open: Cell<bool>,
        /// 焦点
        focused: bool,
        /// hover 日期
        hover_day: Cell<Option<usize>>,
        /// 当前 frame
        last_frame: Cell<Option<Rect>>,
    }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        Size::new(160.0, 32.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                self.focused = true;
                if !self.open.get() {
                    self.open.set(true);
                    let val = self.value.get();
                    self.view_year.set(val.year);
                    self.view_month.set(val.month);
                    return EventResult::Handled;
                }

                // 点击弹出层
                if let Some(frame) = self.last_frame.get() {
                    let popup_y = frame.y + frame.h + 2.0;
                    let rel = Point::new(pos.x - frame.x, pos.y - popup_y);

                    // 月份切换
                    if rel.y >= 0.0 && rel.y < 32.0 {
                        if rel.x > frame.w * 0.5 && rel.x < frame.w * 0.5 + 40.0 {
                            // 下月
                            let (y, m) = next_month(self.view_year.get(), self.view_month.get());
                            self.view_year.set(y); self.view_month.set(m);
                        } else if rel.x > frame.w * 0.5 - 50.0 && rel.x < frame.w * 0.5 - 10.0 {
                            // 上月
                            let (y, m) = prev_month(self.view_year.get(), self.view_month.get());
                            self.view_year.set(y); self.view_month.set(m);
                        }
                        return EventResult::Handled;
                    }

                    // 日期选择
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
                                    self.value.set(DateValue::new(self.view_year.get(), self.view_month.get(), d));
                                    self.open.set(false);
                                }
                            }
                        }
                    }
                }
                EventResult::Handled
            }
            WidgetEvent::FocusOut => { self.focused = false; self.open.set(false); EventResult::Handled }
            WidgetEvent::KeyDown { key, .. } => {
                if *key == KeyCode::Escape { self.open.set(false); EventResult::Handled }
                else { EventResult::NotHandled }
            }
            _ => EventResult::NotHandled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        self.last_frame.set(Some(frame));
        let primary = ctx.tokens().color_primary();
        let primary_bg = ctx.tokens().color_primary_bg();
        let border_color = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let text_tertiary = ctx.tokens().color_text_tertiary();
        let bg_elevated = ctx.tokens().color_bg_elevated();
        let border_radius_sm = ctx.tokens().border_radius_sm();
        let radius = Some(uix_graphics::Radius::uniform(border_radius_sm));

        // 输入框
        let val = self.value.get();
        let is_default = val == DateValue::default();
        ctx.fill_rect(frame, Color::white(), radius);
        ctx.stroke_rect(frame, if self.focused { primary } else { border_color },
            if self.focused { 2.0 } else { 1.0 }, radius);

        if is_default {
            ctx.draw_text(&self.placeholder,
                Point::new(frame.x + 12.0, frame.y + (frame.h - 14.0) * 0.5),
                text_tertiary, 14.0);
        } else {
            let formatted = val.format();
            ctx.draw_text(&formatted,
                Point::new(frame.x + 12.0, frame.y + (frame.h - 14.0) * 0.5),
                text_color, 14.0);
        }

        // 日历图标
        ctx.draw_text("📅", Point::new(frame.x + frame.w - 24.0, frame.y + (frame.h - 14.0) * 0.5),
            text_secondary, 12.0);

        // 弹出日历面板
        if self.open.get() {
            let cell_w = (frame.w - 16.0) / 7.0;
            let cell_h = 30.0;
            let popup_h = 32.0 + 7.0 * cell_h + 8.0;
            let popup = Rect::new(frame.x, frame.y + frame.h + 2.0, frame.w, popup_h);

            ctx.fill_rect(popup, bg_elevated, radius);
            ctx.stroke_rect(popup, border_color, 1.0, radius);

            // 月份标题 + 切换
            let vy = self.view_year.get();
            let vm = self.view_month.get();
            let title = format!("{}年{:02}月", vy, vm);
            ctx.draw_text(&title,
                Point::new(popup.x + popup.w * 0.5 - 28.0, popup.y + (32.0 - 14.0) * 0.5),
                text_color, 14.0);

            ctx.draw_text("◀", Point::new(popup.x + popup.w * 0.5 - 50.0, popup.y + (32.0 - 14.0) * 0.5),
                text_secondary, 12.0);
            ctx.draw_text("▶", Point::new(popup.x + popup.w * 0.5 + 30.0, popup.y + (32.0 - 14.0) * 0.5),
                text_secondary, 12.0);

            // 星期头
            let weekdays = ["一", "二", "三", "四", "五", "六", "日"];
            for (i, &w) in weekdays.iter().enumerate() {
                let x = popup.x + 8.0 + i as f32 * cell_w;
                ctx.draw_text(w, Point::new(x + cell_w * 0.3, popup.y + 34.0), text_tertiary, 10.0);
            }

            // 日期网格
            let fwd = first_weekday(vy, vm);
            let dim = days_in_month(vy, vm);
            let sel = self.value.get();
            for day in 1..=dim {
                let idx = fwd + day - 1;
                let row = idx / 7;
                let col = idx % 7;
                let x = popup.x + 8.0 + col as f32 * cell_w;
                let y = popup.y + 32.0 + 4.0 + row as f32 * cell_h + 16.0;

                let is_selected = sel.day == day && sel.month == vm && sel.year == vy;
                if is_selected {
                    ctx.fill_rect(Rect::new(x - 2.0, y - cell_h * 0.5 + 2.0, cell_w, cell_h),
                        primary_bg, None);
                }

                ctx.draw_text(&day.to_string(),
                    Point::new(x + cell_w * 0.3, y - 7.0),
                    if is_selected { primary } else { text_color }, 12.0);
            }
        }
    }
}

impl DatePicker {
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
        }
    }

    pub fn value(mut self, v: DateValue) -> Self { self.value.set(v); self }
    pub fn selected(&self) -> DateValue { self.value.get() }
    pub fn set_value(&mut self, v: DateValue) { self.value.set(v); }
}

fn next_month(y: i32, m: usize) -> (i32, usize) {
    if m >= 12 { (y + 1, 1) } else { (y, m + 1) }
}
fn prev_month(y: i32, m: usize) -> (i32, usize) {
    if m <= 1 { (y - 1, 12) } else { (y, m - 1) }
}
