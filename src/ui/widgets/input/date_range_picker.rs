//! DateRangePicker 日期范围选择器。

use std::cell::Cell;
use std::sync::Arc;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Color;
use crate::native::traits::input::ControlSize;
use crate::ui::state::State;
use crate::ui::widgets::input::date_calendar::{
    calendar_popup_rect, draw_calendar_panel, hit_calendar_date, hit_month_navigation,
    CalendarPanelState, MonthNavigation, CALENDAR_PANEL_HEIGHT,
};
use crate::ui::widgets::input::date_picker::{
    add_days, days_in_month, next_month, prev_month, Date, DisabledDate,
};
use crate::ui::{
    ComponentId, EventResult, KeyCode, SemanticEvent, SnapshotFields, SystemEvent, WidgetTree,
};

const PRESET_GAP: f32 = 2.0;
const PRESET_ROW_HEIGHT: f32 = 24.0;
const PRESET_VERTICAL_INSET: f32 = 4.0;

/// DateRangePicker 的具名范围预设。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresetDate {
    start: Date,
    end: Date,
}

impl PresetDate {
    pub fn new(start: Date, end: Date) -> Self {
        let (start, end) = ordered_range(start, end);
        Self { start, end }
    }

    pub fn today() -> Self {
        let today = Date::today();
        Self::new(today, today)
    }

    pub fn this_week() -> Self {
        let today = Date::today();
        let start = add_days(today, -(today.weekday().monday_index() as i64));
        Self::new(start, add_days(start, 6))
    }

    pub fn this_month() -> Self {
        let today = Date::today();
        Self::new(
            Date::new(today.year, today.month, 1),
            Date::new(
                today.year,
                today.month,
                days_in_month(today.year, today.month),
            ),
        )
    }

    /// 返回含今天在内的最近 `days` 天；`0` 按一天处理。
    pub fn last_days(days: u32) -> Self {
        let today = Date::today();
        let days = i64::from(days.max(1));
        Self::new(add_days(today, 1 - days), today)
    }

    pub const fn start(self) -> Date {
        self.start
    }

    pub const fn end(self) -> Date {
        self.end
    }
}

component! {
    /// 通过两次日期命中或具名预设提交日期范围。
    pub struct DateRangePicker {
        start_value: Cell<Date>,
        end_value: Cell<Date>,
        start_binding: Option<State<Date>>,
        end_binding: Option<State<Date>>,
        view_year: Cell<i32>,
        view_month: Cell<usize>,
        placeholder: String,
        presets: Vec<(String, PresetDate)>,
        disabled_date: Option<DisabledDate>,
        open: Cell<bool>,
        focused: bool,
        pending_start: Cell<Option<Date>>,
        hover_date: Cell<Option<Date>>,
        picker_size: ControlSize,
        last_frame: Cell<Option<Rect>>,
        pending_change: Cell<Option<(Date, Date)>>,
    }

    tab_index => (&self) -> i32 { 1 }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(
            280.0,
            crate::ui::config::control_height(self.picker_size),
        ))
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        self.sync_bound_values();
        match event {
            SystemEvent::PointerDown { pos, .. } => {
                self.focused = true;
                if !self.open.get() {
                    self.open_from_current_value();
                    return EventResult::Handled;
                }

                if let Some(frame) = self.last_frame.get() {
                    if let Some(index) = self.hit_preset(frame, *pos) {
                        if let Some((_, preset)) = self.presets.get(index) {
                            if !self.range_has_disabled_endpoint(preset.start, preset.end) {
                                self.commit_range(preset.start, preset.end);
                                self.close_popup();
                            }
                        }
                        return EventResult::Handled;
                    }
                    if let Some(navigation) = hit_month_navigation(frame, *pos) {
                        let (year, month) = match navigation {
                            MonthNavigation::Previous => {
                                prev_month(self.view_year.get(), self.view_month.get())
                            }
                            MonthNavigation::Next => {
                                next_month(self.view_year.get(), self.view_month.get())
                            }
                        };
                        self.view_year.set(year);
                        self.view_month.set(month);
                        return EventResult::Handled;
                    }
                    if let Some(date) = hit_calendar_date(
                        frame,
                        *pos,
                        self.view_year.get(),
                        self.view_month.get(),
                    ) {
                        if !self.is_date_disabled(date) {
                            if let Some(start) = self.pending_start.get() {
                                self.commit_range(start, date);
                                self.close_popup();
                            } else {
                                self.pending_start.set(Some(date));
                                self.hover_date.set(Some(date));
                            }
                        }
                    }
                }
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if self.open.get() {
                    if let Some(frame) = self.last_frame.get() {
                        let hit = hit_calendar_date(
                            frame,
                            *pos,
                            self.view_year.get(),
                            self.view_month.get(),
                        )
                        .filter(|date| !self.is_date_disabled(*date));
                        self.hover_date.set(hit);
                        if hit.is_some() {
                            return EventResult::Handled;
                        }
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerLeave => {
                self.hover_date.set(None);
                EventResult::NotHandled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.close_popup();
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } => {
                if self.open.get() {
                    match key {
                        KeyCode::Escape => self.close_popup(),
                        KeyCode::Left => {
                            let (year, month) =
                                prev_month(self.view_year.get(), self.view_month.get());
                            self.view_year.set(year);
                            self.view_month.set(month);
                        }
                        KeyCode::Right => {
                            let (year, month) =
                                next_month(self.view_year.get(), self.view_month.get());
                            self.view_year.set(year);
                            self.view_month.set(month);
                        }
                        _ => {}
                    }
                } else if *key == KeyCode::Space || *key == KeyCode::Enter {
                    self.open_from_current_value();
                }
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change.take().map(|(start, end)| {
            SemanticEvent::change(id, format!("{} / {}", start.format(), end.format()))
        })
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        if self.open.get() {
            self.popup_bounds(frame)
        } else {
            frame
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.sync_bound_values();
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        let primary = ctx.tokens().color_primary();
        let border_color = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let text_tertiary = ctx.tokens().color_text_tertiary();
        let radius = Some(crate::draw::Radius::uniform(ctx.tokens().border_radius_sm()));

        ctx.fill_rect(frame, Color::white(), radius);
        ctx.stroke_rect(
            frame,
            if self.focused { primary } else { border_color },
            if self.focused { 2.0 } else { 1.0 },
            radius,
        );
        let text_y = ctx.visual_center_y(frame, 14.0);
        if let Some((start, end)) = self.current_range() {
            ctx.draw_text(
                &format!("{}  →  {}", start.format(), end.format()),
                Point::new(frame.x + 12.0, text_y),
                text_color,
                14.0,
            );
        } else {
            ctx.draw_text(
                &self.placeholder,
                Point::new(frame.x + 12.0, text_y),
                text_tertiary,
                14.0,
            );
        }
        let icon_y = ctx.visual_center_y(frame, 12.0);
        ctx.draw_text(
            "📅",
            Point::new(frame.x + frame.w - 24.0, icon_y),
            text_secondary,
            12.0,
        );

        if self.open.get() {
            let pending = self.pending_start.get();
            let preview_range = pending
                .zip(self.hover_date.get())
                .map(|(start, end)| ordered_range(start, end));
            draw_calendar_panel(
                frame,
                ctx,
                CalendarPanelState {
                    year: self.view_year.get(),
                    month: self.view_month.get(),
                    active: pending,
                    range: preview_range.or_else(|| self.current_range()),
                    hover: self.hover_date.get(),
                    disabled_date: self.disabled_date.as_ref(),
                },
            );
            self.draw_presets(frame, ctx);
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        self.popup_bounds(frame)
    }
}

impl DateRangePicker {
    pub fn new() -> Self {
        let today = Date::today();
        let config = crate::ui::config::use_config();
        Self {
            start_value: Cell::new(Date::default()),
            end_value: Cell::new(Date::default()),
            start_binding: None,
            end_binding: None,
            view_year: Cell::new(today.year),
            view_month: Cell::new(today.month),
            placeholder: crate::ui::locale::use_locale().placeholder.to_owned(),
            presets: Vec::new(),
            disabled_date: None,
            open: Cell::new(false),
            focused: false,
            pending_start: Cell::new(None),
            hover_date: Cell::new(None),
            picker_size: config.size,
            last_frame: Cell::new(None),
            pending_change: Cell::new(None),
        }
    }

    pub fn start(mut self, state: &State<Date>) -> Self {
        self.start_binding = Some(state.clone());
        self.start_value.set(state.get());
        self
    }

    pub fn end(mut self, state: &State<Date>) -> Self {
        self.end_binding = Some(state.clone());
        self.end_value.set(state.get());
        self
    }

    pub fn default_range(mut self, start: Date, end: Date) -> Self {
        let (start, end) = ordered_range(start, end);
        self.start_binding = None;
        self.end_binding = None;
        self.start_value.set(start);
        self.end_value.set(end);
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.picker_size = size;
        self
    }

    pub fn presets<I, L>(mut self, presets: I) -> Self
    where
        I: IntoIterator<Item = (L, PresetDate)>,
        L: Into<String>,
    {
        self.presets = presets
            .into_iter()
            .map(|(label, preset)| (label.into(), preset))
            .collect();
        self
    }

    pub fn disabled_date(
        mut self,
        predicate: impl Fn(Date) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.disabled_date = Some(Arc::new(predicate));
        self
    }

    pub fn current_range(&self) -> Option<(Date, Date)> {
        let start = self.start_value.get();
        let end = self.end_value.get();
        (start != Date::default() && end != Date::default()).then(|| ordered_range(start, end))
    }

    pub fn is_open(&self) -> bool {
        self.open.get()
    }

    pub fn is_selecting_end(&self) -> bool {
        self.pending_start.get().is_some()
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        let (start, end) = self
            .current_range()
            .map(|(start, end)| (Some(start.format()), Some(end.format())))
            .unwrap_or((None, None));
        SnapshotFields::DateRangePicker {
            placeholder: self.placeholder.clone(),
            start,
            end,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let controlled_start = next.start_binding.as_ref().map(|_| next.start_value.get());
        let controlled_end = next.end_binding.as_ref().map(|_| next.end_value.get());
        self.start_binding = next.start_binding;
        self.end_binding = next.end_binding;
        if let Some(value) = controlled_start {
            self.start_value.set(value);
        }
        if let Some(value) = controlled_end {
            self.end_value.set(value);
        }
        self.placeholder = next.placeholder;
        self.presets = next.presets;
        self.disabled_date = next.disabled_date;
        self.picker_size = next.picker_size;
    }

    fn sync_bound_values(&self) {
        if let Some(state) = self.start_binding.as_ref() {
            self.start_value.set(state.get());
        }
        if let Some(state) = self.end_binding.as_ref() {
            self.end_value.set(state.get());
        }
    }

    fn open_from_current_value(&self) {
        let anchor = self
            .current_range()
            .map(|(start, _)| start)
            .unwrap_or_else(Date::today);
        self.view_year.set(anchor.year);
        self.view_month.set(anchor.month);
        self.pending_start.set(None);
        self.hover_date.set(None);
        self.open.set(true);
    }

    fn close_popup(&self) {
        self.open.set(false);
        self.pending_start.set(None);
        self.hover_date.set(None);
    }

    fn commit_range(&self, start: Date, end: Date) {
        let (start, end) = ordered_range(start, end);
        let changed = self.start_value.get() != start || self.end_value.get() != end;
        self.start_value.set(start);
        self.end_value.set(end);
        if let Some(state) = self.start_binding.as_ref() {
            if state.get() != start {
                state.set(start);
            }
        }
        if let Some(state) = self.end_binding.as_ref() {
            if state.get() != end {
                state.set(end);
            }
        }
        if changed {
            self.pending_change.set(Some((start, end)));
        }
    }

    fn is_date_disabled(&self, date: Date) -> bool {
        self.disabled_date
            .as_ref()
            .is_some_and(|predicate| predicate(date))
    }

    fn range_has_disabled_endpoint(&self, start: Date, end: Date) -> bool {
        self.is_date_disabled(start) || self.is_date_disabled(end)
    }

    fn hit_preset(&self, frame: Rect, position: Point) -> Option<usize> {
        if self.presets.is_empty() {
            return None;
        }
        let popup = calendar_popup_rect(frame);
        let footer_y = popup.y + CALENDAR_PANEL_HEIGHT + PRESET_GAP;
        if position.x < popup.x
            || position.x >= popup.x + popup.w
            || position.y < footer_y + PRESET_VERTICAL_INSET
        {
            return None;
        }
        let index = ((position.y - footer_y - PRESET_VERTICAL_INSET) / PRESET_ROW_HEIGHT) as usize;
        (index < self.presets.len()).then_some(index)
    }

    fn popup_bounds(&self, frame: Rect) -> Rect {
        let popup = calendar_popup_rect(frame);
        let footer_height = if self.presets.is_empty() {
            0.0
        } else {
            PRESET_GAP + PRESET_VERTICAL_INSET * 2.0 + PRESET_ROW_HEIGHT * self.presets.len() as f32
        };
        frame.union(&Rect::new(
            popup.x,
            popup.y,
            popup.w,
            CALENDAR_PANEL_HEIGHT + footer_height,
        ))
    }

    fn draw_presets(&self, frame: Rect, ctx: &mut PaintContext<'_>) {
        if self.presets.is_empty() {
            return;
        }
        let popup = calendar_popup_rect(frame);
        let footer = Rect::new(
            popup.x,
            popup.y + CALENDAR_PANEL_HEIGHT + PRESET_GAP,
            popup.w,
            PRESET_VERTICAL_INSET * 2.0 + PRESET_ROW_HEIGHT * self.presets.len() as f32,
        );
        let radius = Some(crate::draw::Radius::uniform(
            ctx.tokens().border_radius_sm(),
        ));
        ctx.fill_rect(footer, ctx.tokens().color_bg_elevated(), radius);
        ctx.stroke_rect(footer, ctx.tokens().color_border(), 1.0, radius);
        let primary = ctx.tokens().color_primary();
        for (index, (label, _)) in self.presets.iter().enumerate() {
            let row = Rect::new(
                footer.x + 8.0,
                footer.y + PRESET_VERTICAL_INSET + index as f32 * PRESET_ROW_HEIGHT,
                footer.w - 16.0,
                PRESET_ROW_HEIGHT,
            );
            let text_y = ctx.visual_center_y(row, 12.0);
            ctx.draw_text(label, Point::new(row.x, text_y), primary, 12.0);
        }
    }
}

impl Default for DateRangePicker {
    fn default() -> Self {
        Self::new()
    }
}

fn ordered_range(start: Date, end: Date) -> (Date, Date) {
    if start <= end {
        (start, end)
    } else {
        (end, start)
    }
}
