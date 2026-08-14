//! Calendar widget — 日历组件，Ant Design 风格。
//!
//! 月视图展示日期，支持选中日期、月份切换。

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintPass;
use crate::draw::{Color, Radius};
use crate::ui::SnapshotFields;
use crate::ui::component::locale::Locale;
use crate::ui::component::paint_context::PaintContext;
use crate::ui::widgets::input::date_picker::{Date, days_in_month, first_weekday};
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent, WidgetTree,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

mod geometry;

use self::geometry::CalendarGeometry;

// 日历头部总高（40.0，标题行 24 + 星期栏 16）；同名常量在 selectable_list/collapse/
// date_calendar 各为 48/36/32，组件独立设计。
const HEADER_HEIGHT: f32 = 40.0;
const TITLE_HEIGHT: f32 = 24.0;
// 星期栏高度：头部 40 − 标题行 24 推导得 16；date_calendar 面板独立用字面量 24.0。
const WEEKDAY_HEIGHT: f32 = HEADER_HEIGHT - TITLE_HEIGHT;
const DEFAULT_CELL_SIZE: f32 = 40.0;
const MIN_CELL_SIZE: f32 = 20.0;
const MIN_YEAR: i32 = 1;
const MAX_YEAR: i32 = 9999;

/// 日期格渲染上下文。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarCellInfo {
    pub is_today: bool,
    pub is_selected: bool,
    pub is_current_month: bool,
}

/// 日期格中的事件标记。
#[derive(Debug, Clone, PartialEq)]
pub struct CalendarEvent {
    pub date: Date,
    pub title: String,
    pub color: Color,
}

impl CalendarEvent {
    pub fn new(date: Date, title: impl Into<String>, color: Color) -> Self {
        Self {
            date,
            title: title.into(),
            color,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CalendarCellEntry {
    date: Date,
    info: CalendarCellInfo,
    factory_generation: u64,
}

component! {
    /// Internal viewport that confines a custom date-cell View to one grid slot.
    struct CalendarCellHost {
        _date: Date,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::zero())
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(ComponentId, Rect)>
    {
        children.iter().map(|child| (child.id, frame)).collect()
    }

    children_clip => (&self, frame: Rect) -> Option<Rect> { Some(frame) }

    render => (&self, _frame: Rect, _ctx: &mut PaintContext, _tree: &WidgetTree) {}
}

// Calendar — 日历组件。
component! {
    pub struct Calendar {
        year: Cell<i32>,
        month: Cell<usize>,
        selected_date: Cell<Option<Date>>,
        focused_day: Cell<usize>,
        cell_size: f32,
        year_jump: bool,
        events: Vec<CalendarEvent>,
        disabled_predicate: Option<Rc<dyn Fn(Date) -> bool>>,
        custom_cell: bool,
        #[snapshot(skip)]
        custom_cell_factory:
            Option<Rc<dyn Fn(Date, CalendarCellInfo) -> crate::ui::view::ViewNode>>,
        custom_cell_factory_generation: Cell<u64>,
        #[snapshot(skip)]
        materialized_cells: RefCell<Vec<CalendarCellEntry>>,
        focused: bool,
        pending_change: Cell<Option<Date>>,
        layout_requested: Cell<bool>,
        last_geometry: Cell<Option<CalendarGeometry>>,
    }

    tab_index => (&self) -> i32 { 1 }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(crate::ui::ComponentId, Rect)>
    {
        let geometry = CalendarGeometry::new(
            Rect::new(0.0, 0.0, frame.w.max(0.0), frame.h.max(0.0)),
            self.cell_size,
        );
        let entries = self.materialized_cells.borrow();
        children
            .iter()
            .enumerate()
            .filter_map(|(index, child)| {
                let entry = entries.get(index)?;
                let child_frame = geometry
                    .and_then(|geometry| {
                        geometry
                            .cell_rect(
                                self.year.get(),
                                self.month.get(),
                                entry.date.day,
                            )
                            .map(|rect| Rect::new(frame.x + rect.x, frame.y + rect.y, rect.w, rect.h))
                    })
                    .unwrap_or_default();
                Some((child.id, child_frame))
            })
            .collect()
    }

    children_clip => (&self, frame: Rect) -> Option<Rect> {
        Some(Rect::new(frame.x, frame.y, frame.w.max(0.0), frame.h.max(0.0)))
    }

    take_layout_request => (&mut self) -> bool {
        self.layout_requested.replace(false)
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let Some(geometry) = self.interaction_geometry() else {
                    return EventResult::NotHandled;
                };
                if !geometry.control.contains(*pos) {
                    return EventResult::NotHandled;
                }
                if geometry.title_row.contains(*pos) {
                    if geometry.previous_navigation().contains(*pos) {
                        self.shift_months(-self.navigation_month_delta());
                        return EventResult::Handled;
                    }
                    if geometry.next_navigation().contains(*pos) {
                        self.shift_months(self.navigation_month_delta());
                        return EventResult::Handled;
                    }
                    return EventResult::NotHandled;
                }
                if let Some(day) = geometry.day_at(*pos, self.year.get(), self.month.get()) {
                    let date = Date::new(self.year.get(), self.month.get(), day);
                    if self.is_disabled(date) {
                        return EventResult::NotHandled;
                    }
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
        let frame = Rect::new(frame.x, frame.y, frame.w.max(0.0), frame.h.max(0.0));
        let local_frame = Rect::new(0.0, 0.0, frame.w, frame.h);
        let Some(local_geometry) = CalendarGeometry::new(local_frame, self.cell_size) else {
            if ctx.paint_pass() == PaintPass::Content {
                self.last_geometry.set(None);
            }
            return;
        };
        if ctx.paint_pass() == PaintPass::AfterChildren {
            if !self.custom_cell {
                return;
            }
            let geometry = local_geometry.offset(frame.x, frame.y);
            let text_sec = ctx.tokens().color_text_secondary();
            ctx.push_clip(frame);
            for day in 1..=days_in_month(self.year.get(), self.month.get()) {
                let Some(cell_rect) =
                    geometry.cell_rect(self.year.get(), self.month.get(), day)
                else {
                    continue;
                };
                self.paint_event_markers(
                    Date::new(self.year.get(), self.month.get(), day),
                    cell_rect,
                    text_sec,
                    geometry.scale,
                    ctx,
                );
            }
            ctx.pop_clip();
            return;
        }
        if ctx.paint_pass() != PaintPass::Content {
            return;
        }
        self.last_geometry.set(Some(local_geometry));
        let geometry = local_geometry.offset(frame.x, frame.y);
        let primary = ctx.tokens().color_primary();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let border = ctx.tokens().color_border_secondary();
        let bg = ctx.tokens().color_bg_container();
        let selected = self.selected_date.get();
        let focused_day = self.focused_day.get();
        let cur_year = self.year.get();
        let cur_month = self.month.get();
        let radius = ctx.tokens().border_radius_sm() * geometry.scale;

        ctx.push_clip(frame);
        ctx.fill_rect(
            geometry.control,
            bg,
            Some(Radius::uniform(radius)),
        );
        ctx.stroke_rect(
            geometry.control,
            border,
            geometry.scale.max(0.5),
            Some(Radius::uniform(radius)),
        );

        let loc = crate::ui::component::locale::use_locale();
        let title = localized_month_title(&loc, cur_year, cur_month);
        let title_width = (geometry.title_row.w - geometry.navigation_width * 2.0
            - 8.0 * geometry.scale)
            .max(0.0);
        let title_font = fitted_font_size(
            ctx,
            &title,
            15.0 * geometry.scale,
            title_width,
            geometry.title_row.h * 0.8,
        );
        if title_font > 0.0 {
            let measured = ctx.measure_text(&title, title_font);
            let title_y = ctx.visual_center_y(geometry.title_row, title_font);
            ctx.draw_text(
                &title,
                Point::new(
                    geometry.title_row.x + (geometry.title_row.w - measured.w) * 0.5,
                    title_y,
                ),
                text,
                title_font,
            );
        }
        crate::ui::widgets::icon::Icon::paint_in_frame(
            ctx,
            "chevron-left",
            geometry.previous_navigation(),
            primary,
            12.0 * geometry.scale,
        );
        crate::ui::widgets::icon::Icon::paint_in_frame(
            ctx,
            "chevron-right",
            geometry.next_navigation(),
            primary,
            12.0 * geometry.scale,
        );

        let weekdays = loc.weekdays_short;
        let weekday_font = weekdays.iter().fold(11.0 * geometry.scale, |font, weekday| {
            font.min(fitted_font_size(
                ctx,
                weekday,
                11.0 * geometry.scale,
                geometry.cell_size * 0.82,
                geometry.weekday_row.h * 0.8,
            ))
        });
        let weekday_y = ctx.visual_center_y(geometry.weekday_row, weekday_font);
        for (index, weekday) in weekdays.iter().enumerate() {
            let measured = ctx.measure_text(weekday, weekday_font);
            ctx.draw_text(
                weekday,
                Point::new(
                    geometry.weekday_row.x
                        + index as f32 * geometry.cell_size
                        + (geometry.cell_size - measured.w) * 0.5,
                    weekday_y,
                ),
                text_sec,
                weekday_font,
            );
        }

        let first = first_weekday(cur_year, cur_month);
        for day in 1..=days_in_month(cur_year, cur_month) {
            let Some(cell_rect) = geometry.cell_rect(cur_year, cur_month, day) else {
                continue;
            };
            let column = (first + day - 1) % 7;
            let date = Date::new(cur_year, cur_month, day);
            let is_selected = selected == Some(date);
            let is_disabled = self.is_disabled(date);
            let is_focused = self.focused && focused_day == day;
            let is_weekend = column >= 5;
            let text_color = if is_disabled {
                ctx.tokens().color_text_quaternary()
            } else if is_selected {
                // 选中日文字：白色 token。
                ctx.tokens().color_white()
            } else if is_weekend {
                ctx.tokens().color_error()
            } else {
                text
            };
            if is_selected {
                ctx.fill_rect(
                    cell_rect,
                    primary,
                    Some(Radius::uniform(4.0 * geometry.scale)),
                );
            }
            if is_focused {
                let inset = geometry.scale.max(0.5);
                let focus_color = if is_selected {
                    // 选中日焦点描边：白色 token。
                    ctx.tokens().color_white()
                } else {
                    primary
                };
                ctx.stroke_rect(
                    Rect::new(
                        cell_rect.x + inset,
                        cell_rect.y + inset,
                        (cell_rect.w - inset * 2.0).max(0.0),
                        (cell_rect.h - inset * 2.0).max(0.0),
                    ),
                    focus_color,
                    1.5 * geometry.scale,
                    Some(Radius::uniform(4.0 * geometry.scale)),
                );
            }
            let day_text = day.to_string();
            let day_font = fitted_font_size(
                ctx,
                &day_text,
                13.0 * geometry.scale,
                cell_rect.w * 0.8,
                cell_rect.h * 0.8,
            );
            if !self.custom_cell && day_font > 0.0 {
                let measured = ctx.measure_text(&day_text, day_font);
                let text_y = ctx.visual_center_y(cell_rect, day_font);
                ctx.draw_text(
                    &day_text,
                    Point::new(cell_rect.x + (cell_rect.w - measured.w) * 0.5, text_y),
                    text_color,
                    day_font,
                );
            }
            ctx.stroke_rect(cell_rect, border, 0.5 * geometry.scale, None);
            let event_count = self.events.iter().filter(|event| event.date == date).count();
            if self.custom_cell && !is_disabled {
                ctx.stroke_rect(cell_rect, primary, 0.75 * geometry.scale, None);
            }
            if !self.custom_cell && event_count > 0 {
                self.paint_event_markers(
                    date,
                    cell_rect,
                    text_sec,
                    geometry.scale,
                    ctx,
                );
            }
        }
        ctx.pop_clip();
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
            events: Vec::new(),
            disabled_predicate: None,
            custom_cell: false,
            custom_cell_factory: None,
            custom_cell_factory_generation: Cell::new(0),
            materialized_cells: RefCell::new(Vec::new()),
            focused: false,
            pending_change: Cell::new(None),
            layout_requested: Cell::new(false),
            last_geometry: Cell::new(None),
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

    /// 覆盖日期格的公开扩展入口；默认绘制继续使用框架日历样式。
    pub fn date_cell<F, V>(mut self, factory: F) -> Self
    where
        F: Fn(Date, CalendarCellInfo) -> V + 'static,
        V: crate::ui::view::View,
    {
        self.custom_cell = true;
        self.custom_cell_factory = Some(Rc::new(move |date, info| {
            crate::ui::view::View::build(factory(date, info))
        }));
        self
    }

    pub fn events(mut self, events: Vec<CalendarEvent>) -> Self {
        self.events = events;
        self
    }

    pub fn disabled_date<F>(self, predicate: F) -> Self
    where
        F: Fn(Date) -> bool + 'static,
    {
        let mut calendar = self;
        calendar.disabled_predicate = Some(Rc::new(predicate));
        calendar
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
        let next_cell_size = Self::normalize_cell_size(next.cell_size);
        if self.cell_size != next_cell_size {
            self.last_geometry.set(None);
        }
        self.cell_size = next_cell_size;
        self.year_jump = next.year_jump;
        self.events = next.events;
        self.disabled_predicate = next.disabled_predicate;
        self.custom_cell = next.custom_cell;
        self.custom_cell_factory = next.custom_cell_factory;
        self.custom_cell_factory_generation.set(
            self.custom_cell_factory_generation
                .get()
                .wrapping_add(u64::from(self.custom_cell)),
        );
        if !self.custom_cell {
            self.materialized_cells.borrow_mut().clear();
        }
    }

    fn desired_cell_entries(&self) -> Vec<CalendarCellEntry> {
        if self.custom_cell_factory.is_none() {
            return Vec::new();
        }
        let year = self.year.get();
        let month = self.month.get();
        let selected = self.selected_date.get();
        let today = Date::today();
        let factory_generation = self.custom_cell_factory_generation.get();
        (1..=days_in_month(year, month))
            .map(|day| {
                let date = Date::new(year, month, day);
                CalendarCellEntry {
                    date,
                    info: CalendarCellInfo {
                        is_today: date == today,
                        is_selected: selected == Some(date),
                        is_current_month: true,
                    },
                    factory_generation,
                }
            })
            .collect()
    }

    // 按当前 Calendar owner 的动态捕获能力构建待协调的日期格宿主节点。
    fn cell_views(
        // 借用仅由宿主树签发且固定 owner 的动态捕获上下文。
        &self,
        // 接收区分日期格状态所有权的窄捕获能力。
        capture_context: &crate::ui::adapter::DynamicViewCaptureContext,
        // 接收本轮已经确定的日期业务条目。
        entries: &[CalendarCellEntry],
    ) -> Vec<crate::ui::view::ViewNode> {
        let Some(factory) = self.custom_cell_factory.as_ref() else {
            return Vec::new();
        };
        entries
            .iter()
            .map(|entry| {
                // 日期文本同时作为 keyed reconcile 与组件私有状态捕获的稳定业务身份。
                let stable_key = entry.date.format();
                // 在已注册的 Calendar owner 下捕获日期格工厂，隔离不同日期的私有状态。
                let cell_view = capture_context.capture(
                    // 固定槽位避免日期格与同一 Calendar 的其他延迟工厂共享命名空间。
                    "calendar-cell",
                    // 复用日期稳定键让同日刷新恢复既有组件状态。
                    stable_key.clone(),
                    // 只在完整动态捕获边界内调用应用提供的日期格工厂。
                    || factory(entry.date, entry.info),
                );
                // 每个日期宿主继续使用同一稳定键参与父级 keyed reconcile。
                crate::ui::view::ViewNode::new(
                    // 宿主仅负责把已捕获的日期格限制在当前网格单元。
                    CalendarCellHost { _date: entry.date },
                    // 把捕获结果作为宿主唯一子节点，交由现有生命周期协调路径接管。
                    vec![cell_view],
                )
                // 让节点结构身份与捕获命名空间的日期身份一致。
                .key(format!("calendar-cell:{stable_key}"))
            })
            .collect()
    }

    pub(crate) fn cell_views_for_refresh(
        &self,
        // 接收本次 Calendar owner 对应的动态状态捕获能力。
        capture_context: &crate::ui::adapter::DynamicViewCaptureContext,
        current_child_count: usize,
    ) -> Option<(Vec<crate::ui::view::ViewNode>, Vec<CalendarCellEntry>)> {
        let entries = self.desired_cell_entries();
        if current_child_count == entries.len() && *self.materialized_cells.borrow() == entries {
            return None;
        }
        // 仅在确认需要发布新日期格后调用延迟工厂，避免无变更刷新重建状态。
        Some((self.cell_views(capture_context, &entries), entries))
    }

    pub(crate) fn mark_cells_materialized(&self, entries: Vec<CalendarCellEntry>) {
        self.materialized_cells.replace(entries);
    }

    pub(crate) fn owns_custom_cell_children(&self) -> bool {
        self.custom_cell || !self.materialized_cells.borrow().is_empty()
    }

    fn paint_event_markers(
        &self,
        date: Date,
        cell_rect: Rect,
        overflow_text: Color,
        scale: f32,
        ctx: &mut PaintContext,
    ) {
        let event_count = self
            .events
            .iter()
            .filter(|event| event.date == date)
            .count();
        if event_count == 0 {
            return;
        }
        let visible_events = event_count.min(3);
        let dot_gap = 4.0 * scale;
        let total_width = visible_events as f32 * dot_gap;
        for (index, event) in self
            .events
            .iter()
            .filter(|event| event.date == date)
            .take(visible_events)
            .enumerate()
        {
            ctx.fill_circle(
                cell_rect.x + cell_rect.w * 0.5 + (index as f32 + 0.5) * dot_gap
                    - total_width * 0.5,
                cell_rect.y + cell_rect.h - 4.0 * scale,
                1.5 * scale,
                event.color,
            );
        }
        if event_count > visible_events {
            ctx.draw_text(
                &format!("+{}", event_count - visible_events),
                Point::new(cell_rect.x + 2.0 * scale, cell_rect.y + 2.0 * scale),
                overflow_text,
                8.0 * scale,
            );
        }
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
        if self.year_jump { 12 } else { 1 }
    }

    fn interaction_geometry(&self) -> Option<CalendarGeometry> {
        self.last_geometry.get().or_else(|| {
            let intrinsic = self.intrinsic_size();
            CalendarGeometry::new(
                Rect::new(0.0, 0.0, intrinsic.w, intrinsic.h),
                self.cell_size,
            )
        })
    }

    fn shift_months(&self, delta: i32) {
        let year = self.year.get().clamp(MIN_YEAR, MAX_YEAR);
        let month = self.month.get().clamp(1, 12);
        let current = i64::from(year - MIN_YEAR) * 12 + month as i64 - 1;
        let maximum = i64::from(MAX_YEAR - MIN_YEAR + 1) * 12 - 1;
        let shifted = (current + i64::from(delta)).clamp(0, maximum);
        let next_year = MIN_YEAR + (shifted / 12) as i32;
        let next_month = (shifted % 12) as usize + 1;
        let displayed_changed = (year, month) != (next_year, next_month);
        self.year.set(next_year);
        self.month.set(next_month);
        self.clamp_focused_day();
        if displayed_changed && self.custom_cell {
            self.layout_requested.set(true);
        }
    }

    fn shift_focused_day(&self, delta: i32) {
        let displayed_before = (self.year.get(), self.month.get());
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
        if displayed_before != (year, month) && self.custom_cell {
            self.layout_requested.set(true);
        }
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
        if self.is_disabled(date) {
            return;
        }
        if self.selected_date.get() != Some(date) {
            self.selected_date.set(Some(date));
            self.pending_change.set(Some(date));
            if self.custom_cell {
                self.layout_requested.set(true);
            }
        }
    }

    fn is_disabled(&self, date: Date) -> bool {
        self.disabled_predicate
            .as_ref()
            .is_some_and(|predicate| predicate(date))
    }

    // 测试目标保留日历控制区域观测入口，供交互几何测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn control_rect_for_test(&self) -> Option<Rect> {
        self.interaction_geometry().map(|geometry| geometry.control)
    }

    // 测试目标保留日期中心点观测入口，供交互几何测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn day_center_for_test(&self, day: usize) -> Option<Point> {
        self.interaction_geometry()?
            .cell_rect(self.year.get(), self.month.get(), day)
            .map(|rect| Point::new(rect.x + rect.w * 0.5, rect.y + rect.h * 0.5))
    }
}

fn localized_month_title(locale: &Locale, year: i32, month: usize) -> String {
    let month_index = month.saturating_sub(1).min(11);
    let numeric_month = (month_index + 1).to_string();
    let year = year.to_string();
    let affixed_numeric = locale.year_format != "{0}" || locale.month_format != "{0}";
    let (first, second) = if affixed_numeric {
        (year.as_str(), numeric_month.as_str())
    } else {
        (locale.months_long[month_index], year.as_str())
    };
    replace_two_placeholders(locale.month_year_format, first, second).unwrap_or_else(|| {
        if affixed_numeric {
            format!("{year}-{:02}", month_index + 1)
        } else {
            format!("{} {year}", locale.months_long[month_index])
        }
    })
}

fn replace_two_placeholders(pattern: &str, first: &str, second: &str) -> Option<String> {
    let with_first = pattern.replacen("{}", first, 1);
    if with_first == pattern {
        return None;
    }
    let with_second = with_first.replacen("{}", second, 1);
    (with_second != with_first).then_some(with_second)
}

fn fitted_font_size(
    ctx: &mut PaintContext,
    text: &str,
    base_size: f32,
    max_width: f32,
    max_height: f32,
) -> f32 {
    if !base_size.is_finite()
        || base_size <= 0.0
        || !max_width.is_finite()
        || max_width <= 0.0
        || !max_height.is_finite()
        || max_height <= 0.0
    {
        return 0.0;
    }
    let measured = ctx.measure_text(text, base_size);
    let width_scale = if measured.w > 0.0 {
        max_width / measured.w
    } else {
        1.0
    };
    let height_scale = if measured.h > 0.0 {
        max_height / measured.h
    } else {
        1.0
    };
    base_size * width_scale.min(height_scale).clamp(0.0, 1.0)
}

impl Default for Calendar {
    fn default() -> Self {
        Self::new()
    }
}
