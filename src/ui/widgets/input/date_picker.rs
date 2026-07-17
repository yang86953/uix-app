//! DatePicker 日期选择器 — 弹出日历选择日期。
//!
//! 基于 Calendar 的日期逻辑，增加弹出面板和选中回显。

use std::cell::Cell;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Color;
use crate::native::traits::input::ControlSize;
use crate::ui::state::State;
use crate::ui::widgets::input::date_calendar::{
    calendar_popup_rect, draw_calendar_panel, hit_calendar_date, hit_month_navigation,
    CalendarPanelState, MonthNavigation,
};
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent,
    WidgetTree,
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

    /// 返回星期；周一为一周起点。
    pub fn weekday(self) -> Weekday {
        Weekday::from_monday_index((first_weekday(self.year, self.month) + self.day - 1) % 7)
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

/// 公历星期，顺序从周一到周日。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Weekday {
    Monday,
    Tuesday,
    Wednesday,
    Thursday,
    Friday,
    Saturday,
    Sunday,
}

impl Weekday {
    pub const fn is_weekend(self) -> bool {
        matches!(self, Self::Saturday | Self::Sunday)
    }

    pub(crate) const fn monday_index(self) -> usize {
        match self {
            Self::Monday => 0,
            Self::Tuesday => 1,
            Self::Wednesday => 2,
            Self::Thursday => 3,
            Self::Friday => 4,
            Self::Saturday => 5,
            Self::Sunday => 6,
        }
    }

    const fn from_monday_index(index: usize) -> Self {
        match index % 7 {
            0 => Self::Monday,
            1 => Self::Tuesday,
            2 => Self::Wednesday,
            3 => Self::Thursday,
            4 => Self::Friday,
            5 => Self::Saturday,
            _ => Self::Sunday,
        }
    }
}

/// DatePicker 的选择粒度。
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PickerMode {
    #[default]
    Date,
    Week,
    Month,
    Quarter,
}

impl PickerMode {
    fn normalize(self, date: Date) -> Date {
        match self {
            Self::Date => date,
            Self::Week => add_days(date, -(date.weekday().monday_index() as i64)),
            Self::Month => Date::new(date.year, date.month, 1),
            Self::Quarter => Date::new(date.year, ((date.month - 1) / 3) * 3 + 1, 1),
        }
    }
}

pub(crate) type DisabledDate = Arc<dyn Fn(Date) -> bool + Send + Sync>;

pub(crate) fn days_in_month(year: i32, month: usize) -> usize {
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

pub(crate) fn first_weekday(year: i32, month: usize) -> usize {
    let m = if month <= 2 { month + 12 } else { month } as i64;
    let y = if month <= 2 {
        year as i64 - 1
    } else {
        year as i64
    };
    let century = y.div_euclid(100);
    let year_in_century = y.rem_euclid(100);
    let zeller = (1 + (13 * (m + 1)) / 5 + year_in_century + year_in_century / 4 + century / 4
        - 2 * century)
        .rem_euclid(7);
    ((zeller + 5) % 7) as usize
}

component! {
    /// DatePicker — 日期选择器。
    pub struct DatePicker {
        value: Cell<Date>,
        value_binding: Option<State<Date>>,
        view_year: Cell<i32>,
        view_month: Cell<usize>,
        placeholder: String,
        mode: PickerMode,
        disabled_date: Option<DisabledDate>,
        open: Cell<bool>,
        focused: bool,
        hover_date: Cell<Option<Date>>,
        picker_size: ControlSize,
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
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                self.focused = true;
                if !self.open.get() {
                    self.open.set(true);
                    let val = self.selected_or_today();
                    self.view_year.set(val.year);
                    self.view_month.set(val.month);
                    return EventResult::Handled;
                }

                if let Some(frame) = self.last_frame.get() {
                    if let Some(navigation) = hit_month_navigation(frame, *pos) {
                        match navigation {
                            MonthNavigation::Next => {
                            let (y, m) = next_month(self.view_year.get(), self.view_month.get());
                            self.view_year.set(y); self.view_month.set(m);
                            }
                            MonthNavigation::Previous => {
                            let (y, m) = prev_month(self.view_year.get(), self.view_month.get());
                            self.view_year.set(y); self.view_month.set(m);
                            }
                        }
                        return EventResult::Handled;
                    }

                    if let Some(hit_date) = hit_calendar_date(
                        frame,
                        *pos,
                        self.view_year.get(),
                        self.view_month.get(),
                    ) {
                        if !self.is_date_disabled(hit_date) {
                            self.commit_value(self.mode.normalize(hit_date));
                            self.open.set(false);
                        }
                    }
                }
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if self.open.get() {
                    if let Some(frame) = self.last_frame.get() {
                        if let Some(hit_date) = hit_calendar_date(
                            frame,
                            *pos,
                            self.view_year.get(),
                            self.view_month.get(),
                        ) {
                            if !self.is_date_disabled(hit_date) {
                                self.hover_date.set(Some(hit_date));
                                return EventResult::Handled;
                            }
                        }
                        self.hover_date.set(None);
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerLeave => { self.hover_date.set(None); EventResult::NotHandled }
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

    hit_test_frame => (&self, frame: Rect) -> Rect {
        if self.open.get() {
            picker_bounds(frame)
        } else {
            frame
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.sync_bound_value();
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        let primary = ctx.tokens().color_primary();
        let border_color = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let text_tertiary = ctx.tokens().color_text_tertiary();
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

        crate::ui::widgets::icon::paint_icon_in_frame(
            ctx,
            "calendar",
            Rect::new(frame.x + frame.w - 28.0, frame.y, 24.0, frame.h),
            text_secondary,
            14.0,
        );

        if self.open.get() {
            draw_calendar_panel(
                frame,
                ctx,
                CalendarPanelState {
                    year: self.view_year.get(),
                    month: self.view_month.get(),
                    active: (!is_default).then_some(val),
                    range: None,
                    hover: self.hover_date.get(),
                    disabled_date: self.disabled_date.as_ref(),
                },
            );
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        picker_bounds(frame)
    }
}

fn picker_bounds(frame: Rect) -> Rect {
    frame.union(&calendar_popup_rect(frame))
}

impl DatePicker {
    fn intrinsic_size(&self) -> Size {
        Size::new(160.0, crate::ui::config::control_height(self.picker_size))
    }

    pub fn new() -> Self {
        let today = Date::today();
        let config = crate::ui::config::use_config();
        Self {
            value: Cell::new(Date::default()),
            value_binding: None,
            view_year: Cell::new(today.year),
            view_month: Cell::new(today.month),
            placeholder: crate::ui::locale::use_locale().placeholder.to_owned(),
            mode: PickerMode::Date,
            disabled_date: None,
            open: Cell::new(false),
            focused: false,
            hover_date: Cell::new(None),
            picker_size: config.size,
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

    pub fn size(mut self, size: ControlSize) -> Self {
        self.picker_size = size;
        self
    }

    /// 设置选择粒度；Week / Month / Quarter 分别写回周期起点。
    pub fn mode(mut self, mode: PickerMode) -> Self {
        self.mode = mode;
        self
    }

    /// 禁止选择满足谓词的日期。
    pub fn disabled_date(
        mut self,
        predicate: impl Fn(Date) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.disabled_date = Some(Arc::new(predicate));
        self
    }

    /// 返回组件当前缓存值；controlled 用法应以绑定的 `State` 为真值来源。
    pub fn current_value(&self) -> Date {
        self.value.get()
    }
    pub fn is_open(&self) -> bool {
        self.open.get()
    }
    pub fn picker_mode(&self) -> PickerMode {
        self.mode
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
        self.mode = next.mode;
        self.disabled_date = next.disabled_date;
        self.picker_size = next.picker_size;
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

    fn is_date_disabled(&self, date: Date) -> bool {
        self.disabled_date
            .as_ref()
            .is_some_and(|predicate| predicate(date))
    }
}

impl Default for DatePicker {
    fn default() -> Self {
        Self::new()
    }
}

pub(crate) fn next_month(y: i32, m: usize) -> (i32, usize) {
    if m >= 12 {
        (y + 1, 1)
    } else {
        (y, m + 1)
    }
}
pub(crate) fn prev_month(y: i32, m: usize) -> (i32, usize) {
    if m <= 1 {
        (y - 1, 12)
    } else {
        (y, m - 1)
    }
}

pub(crate) fn add_days(date: Date, days: i64) -> Date {
    let minimum = civil_day_number(Date::new(i32::MIN, 1, 1));
    let maximum = civil_day_number(Date::new(i32::MAX, 12, 31));
    let target = civil_day_number(date)
        .saturating_add(days)
        .clamp(minimum, maximum);
    date_from_civil_day_number(target)
}

fn civil_day_number(date: Date) -> i64 {
    let mut year = i64::from(date.year);
    if date.month <= 2 {
        year -= 1;
    }
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let shifted_month = date.month as i64 + if date.month > 2 { -3 } else { 9 };
    let day_of_year = (153 * shifted_month + 2) / 5 + date.day as i64 - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era
}

fn date_from_civil_day_number(day_number: i64) -> Date {
    let era = day_number.div_euclid(146_097);
    let day_of_era = day_number - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = shifted_month + if shifted_month < 10 { 3 } else { -9 };
    if month <= 2 {
        year += 1;
    }
    Date::new(year as i32, month as usize, day as usize)
}
