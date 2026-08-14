//! 日历几何布局计算。

use crate::core::{Point, Rect};
use crate::ui::widgets::input::date_picker::{days_in_month, first_weekday};

use super::{HEADER_HEIGHT, TITLE_HEIGHT, WEEKDAY_HEIGHT};
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CalendarGeometry {
    pub(crate) control: Rect,
    pub(crate) title_row: Rect,
    pub(crate) weekday_row: Rect,
    pub(crate) grid: Rect,
    pub(crate) cell_size: f32,
    pub(crate) navigation_width: f32,
    pub(crate) scale: f32,
}

impl CalendarGeometry {
    pub(crate) fn new(frame: Rect, preferred_cell_size: f32) -> Option<Self> {
        let frame = Rect::new(frame.x, frame.y, frame.w.max(0.0), frame.h.max(0.0));
        let intrinsic_width = preferred_cell_size * 7.0;
        let intrinsic_height = preferred_cell_size * 6.0 + HEADER_HEIGHT;
        if frame.w <= 0.0 || frame.h <= 0.0 || intrinsic_width <= 0.0 || intrinsic_height <= 0.0 {
            return None;
        }

        let scale = (frame.w / intrinsic_width)
            .min(frame.h / intrinsic_height)
            .min(1.0);
        if !scale.is_finite() || scale <= 0.0 {
            return None;
        }

        let control_width = intrinsic_width * scale;
        let control_height = intrinsic_height * scale;
        let control = Rect::new(
            frame.x + (frame.w - control_width) * 0.5,
            frame.y + (frame.h - control_height) * 0.5,
            control_width,
            control_height,
        );
        let title_height = TITLE_HEIGHT * scale;
        let weekday_height = WEEKDAY_HEIGHT * scale;
        let title_row = Rect::new(control.x, control.y, control.w, title_height);
        let weekday_row = Rect::new(
            control.x,
            title_row.y + title_row.h,
            control.w,
            weekday_height,
        );
        let grid = Rect::new(
            control.x,
            weekday_row.y + weekday_row.h,
            control.w,
            preferred_cell_size * 6.0 * scale,
        );

        Some(Self {
            control,
            title_row,
            weekday_row,
            grid,
            cell_size: preferred_cell_size * scale,
            navigation_width: (TITLE_HEIGHT * scale).min(control.w / 3.0),
            scale,
        })
    }

    pub(crate) fn offset(self, dx: f32, dy: f32) -> Self {
        let translate = |rect: Rect| Rect::new(rect.x + dx, rect.y + dy, rect.w, rect.h);
        Self {
            control: translate(self.control),
            title_row: translate(self.title_row),
            weekday_row: translate(self.weekday_row),
            grid: translate(self.grid),
            ..self
        }
    }

    pub(crate) fn previous_navigation(self) -> Rect {
        Rect::new(
            self.title_row.x,
            self.title_row.y,
            self.navigation_width,
            self.title_row.h,
        )
    }

    pub(crate) fn next_navigation(self) -> Rect {
        Rect::new(
            self.title_row.x + self.title_row.w - self.navigation_width,
            self.title_row.y,
            self.navigation_width,
            self.title_row.h,
        )
    }

    pub(crate) fn cell_rect(self, year: i32, month: usize, day: usize) -> Option<Rect> {
        if day == 0 || day > days_in_month(year, month) {
            return None;
        }
        let slot = first_weekday(year, month) + day - 1;
        let row = slot / 7;
        let column = slot % 7;
        Some(Rect::new(
            self.grid.x + column as f32 * self.cell_size,
            self.grid.y + row as f32 * self.cell_size,
            self.cell_size,
            self.cell_size,
        ))
    }

    pub(crate) fn day_at(self, position: Point, year: i32, month: usize) -> Option<usize> {
        if position.x < self.grid.x
            || position.x >= self.grid.x + self.grid.w
            || position.y < self.grid.y
            || position.y >= self.grid.y + self.grid.h
        {
            return None;
        }
        let column = ((position.x - self.grid.x) / self.cell_size) as usize;
        let row = ((position.y - self.grid.y) / self.cell_size) as usize;
        let slot = row * 7 + column;
        let first = first_weekday(year, month);
        let day = slot.checked_sub(first)? + 1;
        (day <= days_in_month(year, month)).then_some(day)
    }
}
