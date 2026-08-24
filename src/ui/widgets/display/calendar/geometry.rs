//! 日历几何布局计算。

use crate::core::{Point, Rect};
use crate::ui::widgets::input::date_picker::{days_in_month, first_weekday};

use super::CalendarGeometryVisual;
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CalendarGeometry {
    pub(crate) control: Rect,
    pub(crate) title_row: Rect,
    pub(crate) weekday_row: Rect,
    pub(crate) grid: Rect,
    pub(crate) cell_size: f32,
    pub(crate) navigation_width: f32,
    pub(crate) scale: f32,
    grid_columns: usize,
}

impl CalendarGeometry {
    pub(crate) fn new(
        frame: Rect,
        preferred_cell_size: f32,
        visual: CalendarGeometryVisual,
    ) -> Option<Self> {
        let frame = Rect::new(frame.x, frame.y, frame.w.max(0.0), frame.h.max(0.0));
        let intrinsic_width = preferred_cell_size * visual.grid_columns as f32;
        let intrinsic_height = preferred_cell_size * visual.grid_rows as f32 + visual.header_height;
        if frame.w <= 0.0 || frame.h <= 0.0 || intrinsic_width <= 0.0 || intrinsic_height <= 0.0 {
            return None;
        }

        let scale = (frame.w / intrinsic_width)
            .min(frame.h / intrinsic_height)
            .min(visual.max_scale);
        if !scale.is_finite() || scale <= 0.0 {
            return None;
        }

        let control_width = intrinsic_width * scale;
        let control_height = intrinsic_height * scale;
        let control = Rect::new(
            frame.x + (frame.w - control_width) * visual.center_ratio,
            frame.y + (frame.h - control_height) * visual.center_ratio,
            control_width,
            control_height,
        );
        let title_height = visual.title_height * scale;
        let weekday_height = visual.weekday_height * scale;
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
            preferred_cell_size * visual.grid_rows as f32 * scale,
        );

        Some(Self {
            control,
            title_row,
            weekday_row,
            grid,
            cell_size: preferred_cell_size * scale,
            navigation_width: (visual.title_height * scale)
                .min(control.w * visual.navigation_width_ratio),
            scale,
            grid_columns: visual.grid_columns.max(1),
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
        let row = slot / self.grid_columns;
        let column = slot % self.grid_columns;
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
        let slot = row * self.grid_columns + column;
        let first = first_weekday(year, month);
        let day = slot.checked_sub(first)? + 1;
        (day <= days_in_month(year, month)).then_some(day)
    }
}
