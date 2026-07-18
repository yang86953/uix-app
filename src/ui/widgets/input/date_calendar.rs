//! 日期输入组件共用的月视图面板。

use crate::core::{Point, Rect};
use crate::draw::painting::PaintContext;
use crate::ui::widgets::input::date_picker::{days_in_month, first_weekday, Date, DisabledDate};

pub(crate) const CALENDAR_PANEL_HEIGHT: f32 = 250.0;
const CALENDAR_PANEL_MIN_WIDTH: f32 = 160.0;
const HEADER_HEIGHT: f32 = 32.0;
const WEEKDAY_HEIGHT: f32 = 24.0;
const CELL_HEIGHT: f32 = 30.0;
const HORIZONTAL_INSET: f32 = 8.0;
const NAVIGATION_WIDTH: f32 = 32.0;
const GRID_TOP: f32 = HEADER_HEIGHT + WEEKDAY_HEIGHT;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MonthNavigation {
    Previous,
    Next,
}

pub(crate) struct CalendarPanelState<'a> {
    pub year: i32,
    pub month: usize,
    pub active: Option<Date>,
    pub range: Option<(Date, Date)>,
    pub hover: Option<Date>,
    pub disabled_date: Option<&'a DisabledDate>,
}

impl CalendarPanelState<'_> {
    fn is_disabled(&self, date: Date) -> bool {
        self.disabled_date.is_some_and(|predicate| predicate(date))
    }

    fn is_in_range(&self, date: Date) -> bool {
        self.range
            .is_some_and(|(start, end)| start <= date && date <= end)
    }
}

pub(crate) fn calendar_popup_rect(frame: Rect) -> Rect {
    Rect::new(
        frame.x,
        frame.y + frame.h + 2.0,
        frame.w.max(CALENDAR_PANEL_MIN_WIDTH),
        CALENDAR_PANEL_HEIGHT,
    )
}

pub(crate) fn hit_month_navigation(frame: Rect, position: Point) -> Option<MonthNavigation> {
    let popup = calendar_popup_rect(frame);
    let relative = Point::new(position.x - popup.x, position.y - popup.y);
    if !(0.0..popup.w).contains(&relative.x) || !(0.0..HEADER_HEIGHT).contains(&relative.y) {
        return None;
    }
    if relative.x < NAVIGATION_WIDTH {
        Some(MonthNavigation::Previous)
    } else if relative.x >= popup.w - NAVIGATION_WIDTH {
        Some(MonthNavigation::Next)
    } else {
        None
    }
}

pub(crate) fn hit_calendar_date(
    frame: Rect,
    position: Point,
    year: i32,
    month: usize,
) -> Option<Date> {
    let popup = calendar_popup_rect(frame);
    let relative = Point::new(position.x - popup.x, position.y - popup.y);
    if relative.y < GRID_TOP
        || relative.y >= GRID_TOP + CELL_HEIGHT * 6.0
        || relative.x < HORIZONTAL_INSET
        || relative.x >= popup.w - HORIZONTAL_INSET
    {
        return None;
    }

    let cell_width = (popup.w - 2.0 * HORIZONTAL_INSET) / 7.0;
    if cell_width <= 0.0 {
        return None;
    }
    let row = ((relative.y - GRID_TOP) / CELL_HEIGHT) as usize;
    let column = ((relative.x - HORIZONTAL_INSET) / cell_width) as usize;
    if row >= 6 || column >= 7 {
        return None;
    }

    let slot = row * 7 + column;
    let first = first_weekday(year, month);
    if slot < first {
        return None;
    }
    let day = slot - first + 1;
    (day <= days_in_month(year, month)).then(|| Date::new(year, month, day))
}

pub(crate) fn draw_calendar_panel(
    frame: Rect,
    ctx: &mut PaintContext<'_>,
    state: CalendarPanelState<'_>,
) {
    let primary = ctx.tokens().color_primary();
    let primary_bg = ctx.tokens().color_primary_bg();
    let border_color = ctx.tokens().color_border();
    let text_color = ctx.tokens().color_text();
    let text_secondary = ctx.tokens().color_text_secondary();
    let text_tertiary = ctx.tokens().color_text_tertiary();
    let bg_elevated = ctx.tokens().color_bg_elevated();
    let fill_tertiary = ctx.tokens().color_fill_tertiary();
    let radius = Some(crate::draw::Radius::uniform(
        ctx.tokens().border_radius_sm(),
    ));
    let popup = calendar_popup_rect(frame);

    ctx.push_clip(popup);
    ctx.fill_rect(popup, bg_elevated, radius);
    ctx.stroke_rect(popup, border_color, 1.0, radius);

    let title = format!("{}年{:02}月", state.year, state.month);
    let header_rect = Rect::new(popup.x, popup.y, popup.w, HEADER_HEIGHT);
    let title_y = ctx.visual_center_y(header_rect, 14.0);
    let title_width = ctx.measure_text(&title, 14.0).w;
    ctx.draw_text(
        &title,
        Point::new(popup.x + (popup.w - title_width) * 0.5, title_y),
        text_color,
        14.0,
    );
    crate::ui::widgets::icon::paint_icon_in_frame(
        ctx,
        "chevron-left",
        Rect::new(popup.x, popup.y, NAVIGATION_WIDTH, HEADER_HEIGHT),
        text_secondary,
        12.0,
    );
    crate::ui::widgets::icon::paint_icon_in_frame(
        ctx,
        "chevron-right",
        Rect::new(
            popup.x + popup.w - NAVIGATION_WIDTH,
            popup.y,
            NAVIGATION_WIDTH,
            HEADER_HEIGHT,
        ),
        text_secondary,
        12.0,
    );

    let cell_width = (popup.w - 2.0 * HORIZONTAL_INSET) / 7.0;
    let weekday_row = Rect::new(
        popup.x + HORIZONTAL_INSET,
        popup.y + HEADER_HEIGHT,
        popup.w - HORIZONTAL_INSET * 2.0,
        WEEKDAY_HEIGHT,
    );
    let weekday_text_y = ctx.visual_center_y(weekday_row, 10.0);
    for (index, weekday) in crate::ui::locale::use_locale()
        .weekdays_short
        .iter()
        .enumerate()
    {
        let x = popup.x + HORIZONTAL_INSET + index as f32 * cell_width;
        let text_width = ctx.measure_text(weekday, 10.0).w;
        ctx.draw_text(
            weekday,
            Point::new(x + (cell_width - text_width) * 0.5, weekday_text_y),
            text_tertiary,
            10.0,
        );
    }

    let first = first_weekday(state.year, state.month);
    for day in 1..=days_in_month(state.year, state.month) {
        let date = Date::new(state.year, state.month, day);
        let slot = first + day - 1;
        let row = slot / 7;
        let column = slot % 7;
        let x = popup.x + HORIZONTAL_INSET + column as f32 * cell_width;
        let cell_rect = Rect::new(
            x,
            popup.y + GRID_TOP + row as f32 * CELL_HEIGHT,
            cell_width,
            CELL_HEIGHT,
        );
        let is_active = state.active == Some(date);
        let is_in_range = state.is_in_range(date);
        let is_hovered = state.hover == Some(date);
        let is_disabled = state.is_disabled(date);

        if is_active || is_in_range {
            ctx.fill_rect(cell_rect, primary_bg, None);
        } else if is_hovered {
            ctx.fill_rect(cell_rect, fill_tertiary, None);
        }

        let color = if is_disabled {
            text_tertiary
        } else if is_active {
            primary
        } else {
            text_color
        };
        let day_text = day.to_string();
        let text_width = ctx.measure_text(&day_text, 12.0).w;
        let text_y = ctx.visual_center_y(cell_rect, 12.0);
        ctx.draw_text(
            &day_text,
            Point::new(x + (cell_width - text_width) * 0.5, text_y),
            color,
            12.0,
        );
    }
    ctx.pop_clip();
}
