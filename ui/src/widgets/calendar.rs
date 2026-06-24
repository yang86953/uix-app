//! Calendar widget — 日历组件，Ant Design 风格。
//!
//! 月视图展示日期，支持选中日期、月份切换。

use uix_core::{Point, Rect, Size};
use crate::define_widget;
use uix_graphics::{Color, Radius};
use crate::render_context::RenderContext;
use crate::widget::{EventResult, WidgetEvent, WidgetTree};
use std::cell::Cell;

// 简化日期结构
#[derive(Debug, Clone, Copy, PartialEq)]
struct SimpleDate { year: i32, month: usize, day: usize }

fn days_in_month(year: i32, month: usize) -> usize {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if (year % 4 == 0 && year % 100 != 0) || year % 400 == 0 { 29 } else { 28 },
        _ => 30,
    }
}

fn first_weekday(year: i32, month: usize) -> usize {
    // Zeller 公式简化 (0=Sun, 1=Mon, ..., 6=Sat)
    let m = if month <= 2 { month + 12 } else { month };
    let y = if month <= 2 { (year - 1) as usize } else { year as usize };
    let c = y / 100;
    let y_mod = y % 100;
    let w = (1usize + (13 * (m + 1)) / 5 + y_mod + y_mod / 4 + c / 4).wrapping_sub(2 * c) % 7;
    (w + 6) % 7 // 转成周一为0
}

/// Calendar — 日历组件。
define_widget! {
    pub struct Calendar {
        year: Cell<i32>,
        month: Cell<usize>,
        selected_day: Cell<Option<usize>>,
        cell_size: f32,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        Size::new(self.cell_size * 7.0, self.cell_size * 7.0 + 40.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if let WidgetEvent::MouseDown { pos, .. } = event {
            let header_h = 40.0;
            if pos.y < header_h {
                // 月份切换
                if pos.x < 40.0 {
                    let m = self.month.get();
                    if m == 1 { self.month.set(12); self.year.set(self.year.get() - 1); }
                    else { self.month.set(m - 1); }
                    return EventResult::Handled;
                }
                if pos.x > self.cell_size * 7.0 - 40.0 {
                    let m = self.month.get();
                    if m == 12 { self.month.set(1); self.year.set(self.year.get() + 1); }
                    else { self.month.set(m + 1); }
                    return EventResult::Handled;
                }
            } else {
                // 日期选择
                let col = (pos.x / self.cell_size) as usize;
                let row = ((pos.y - header_h) / self.cell_size) as usize;
                let day_num = row * 7 + col + 1;
                let wd = first_weekday(self.year.get(), self.month.get());
                let day = day_num as i32 - wd as i32;
                let dim = days_in_month(self.year.get(), self.month.get());
                if day >= 1 && day <= dim as i32 {
                    self.selected_day.set(Some(day as usize));
                    return EventResult::Handled;
                }
            }
        }
        EventResult::NotHandled
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let primary = ctx.tokens().color_primary();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_quaternary();
        let border = ctx.tokens().color_border_secondary();
        let fill = ctx.tokens().color_fill_tertiary();
        let bg = ctx.tokens().color_bg_container();
        let cs = self.cell_size;
        let y = frame.y;
        let x = frame.x;
        let sel = self.selected_day.get();
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
        let weekdays = ["一","二","三","四","五","六","日"];
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
            let is_sel = sel == Some(d);
            let is_weekend = col >= 5;
            let tc = if is_sel { Color::white() } else if is_weekend { ctx.tokens().color_error() } else { text };
            if is_sel {
                ctx.fill_rect(cell_rect, primary, Some(Radius::uniform(4.0)));
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
            year: Cell::new(2026), month: Cell::new(6),
            selected_day: Cell::new(None), cell_size: 40.0,
        }
    }
    pub fn cell_size(mut self, s: f32) -> Self { self.cell_size = s; self }
    pub fn selected_day(&self) -> Option<usize> { self.selected_day.get() }
}

impl Default for Calendar { fn default() -> Self { Self::new() } }


