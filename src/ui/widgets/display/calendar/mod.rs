//! Calendar widget — 日历组件，Ant Design 风格。
//!
//! 月视图展示日期，支持选中日期、月份切换。

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintPass;
use crate::draw::{Color, Radius};
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widgets::input::date_picker::{Date, days_in_month, first_weekday};
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, State, SystemEvent, WidgetId, WidgetTree,
};
use crate::ui::{SnapshotFields, ThemeTokens};
use crate::widget;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

mod geometry;
// 日历标题本地化与文字适配由无状态格式化子模块负责。
mod formatting;
// 受控日期构造器由窄绑定适配子模块负责。
mod binding;

use self::geometry::CalendarGeometry;
// 绘制路径只消费格式化子模块的两个窄函数。
use self::formatting::{fitted_font_size, localized_month_title};

const MIN_YEAR: i32 = 1;
const MAX_YEAR: i32 = 9999;
const MAX_VISIBLE_EVENT_MARKERS: usize = 3;

// 保存由 UIX 声明的日历固有尺寸、标题/星期栏与网格布局。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CalendarGeometryVisual {
    default_cell_size: f32,
    min_cell_size: f32,
    header_height: f32,
    title_height: f32,
    weekday_height: f32,
    grid_columns: usize,
    grid_rows: usize,
    max_scale: f32,
    center_ratio: f32,
    navigation_width_ratio: f32,
}

// 保存由 UIX 声明的排版、边框、焦点与事件标记视觉。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CalendarChromeVisual {
    control_stroke_min: f32,
    title_side_gap: f32,
    title_font_size: f32,
    title_height_ratio: f32,
    previous_icon: &'static str,
    next_icon: &'static str,
    navigation_icon_size: f32,
    weekday_font_size: f32,
    weekday_width_ratio: f32,
    weekday_height_ratio: f32,
    day_radius: f32,
    focus_inset_min: f32,
    focus_stroke_width: f32,
    day_font_size: f32,
    day_width_ratio: f32,
    day_height_ratio: f32,
    grid_stroke_width: f32,
    custom_cell_stroke_width: f32,
    event_marker_limit: usize,
    event_marker_gap: f32,
    event_marker_bottom: f32,
    event_marker_radius: f32,
    event_overflow_inset: f32,
    event_overflow_font_size: f32,
}

// Calendar 外框使用的主题圆角角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CalendarRadiusRole {
    Small,
}

impl CalendarRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

// 保存由 UIX 声明的日历主题语义角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CalendarPaletteVisual {
    primary: ColorValue,
    text: ColorValue,
    text_secondary: ColorValue,
    text_quaternary: ColorValue,
    border: ColorValue,
    background: ColorValue,
    white: ColorValue,
    error: ColorValue,
    control_radius: CalendarRadiusRole,
}

// 全部 Calendar 实例共享的完整静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CalendarVisual {
    geometry: CalendarGeometryVisual,
    chrome: CalendarChromeVisual,
    palette: CalendarPaletteVisual,
}

// 同目录 UIX 生成几何、装饰、色板与根视觉记录及稳定借用。
crate::uix_items!("src/ui/widgets/display/calendar/calendar.uix");

// 保存 Calendar 每帧只解析一次的主题颜色与圆角。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ResolvedCalendarVisual {
    primary: Color,
    text: Color,
    text_secondary: Color,
    text_quaternary: Color,
    border: Color,
    background: Color,
    white: Color,
    error: Color,
    control_radius: f32,
}

impl CalendarVisual {
    fn resolve(self, tokens: &dyn ThemeTokens) -> ResolvedCalendarVisual {
        ResolvedCalendarVisual {
            primary: self.palette.primary.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            text_secondary: self.palette.text_secondary.resolve(tokens),
            text_quaternary: self.palette.text_quaternary.resolve(tokens),
            border: self.palette.border.resolve(tokens),
            background: self.palette.background.resolve(tokens),
            white: self.palette.white.resolve(tokens),
            error: self.palette.error.resolve(tokens),
            control_radius: self.palette.control_radius.resolve(tokens),
        }
    }
}

// 限制 UIX 声明的可见事件标记数量，保持固定栈数组边界。
const fn calendar_event_marker_limit(limit: usize) -> usize {
    if limit > MAX_VISIBLE_EVENT_MARKERS {
        MAX_VISIBLE_EVENT_MARKERS
    } else {
        limit
    }
}

// 向 UIX 提供主题角色。
const fn calendar_small_radius() -> CalendarRadiusRole {
    CalendarRadiusRole::Small
}
const fn calendar_primary_color() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
const fn calendar_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
const fn calendar_secondary_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
const fn calendar_quaternary_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextQuaternary)
}
const fn calendar_border_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BorderSecondary)
}
const fn calendar_background_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}
const fn calendar_white_color() -> ColorValue {
    ColorValue::Palette(PaletteColor::White)
}
const fn calendar_error_color() -> ColorValue {
    ColorValue::Palette(PaletteColor::Error)
}

// 日号静态文本避免每帧为最多三十一个日期分配 String。
const CALENDAR_DAY_LABELS: [&str; 31] = [
    "1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15", "16", "17",
    "18", "19", "20", "21", "22", "23", "24", "25", "26", "27", "28", "29", "30", "31",
];

// 保存某日事件总数与前三个稳定索引，不产生堆分配。
#[derive(Debug, Clone, Copy, Default)]
struct CalendarDayEventBucket {
    count: usize,
    visible_count: usize,
    visible_indices: [usize; MAX_VISIBLE_EVENT_MARKERS],
}

/// 日期格渲染上下文。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarCellInfo {
    /// 当前日期格是否对应系统今天。
    pub is_today: bool,
    /// 当前日期格是否对应日历选中日期。
    pub is_selected: bool,
    /// 当前日期格是否属于正在展示的月份。
    pub is_current_month: bool,
}

/// 日期格中的事件标记。
#[derive(Debug, Clone, PartialEq)]
pub struct CalendarEvent {
    /// 事件标记所属的日期。
    pub date: Date,
    /// 事件在日期格中显示的标题。
    pub title: String,
    /// 事件标记使用的颜色。
    pub color: Color,
}

impl CalendarEvent {
    /// 创建指定日期、标题与颜色的日历事件标记。
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

widget! {
    /// Internal viewport that confines a custom date-cell View to one grid slot.
    struct CalendarCellHost {
        _date: Date,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::zero())
    }

    measure_children_into => (
        &self,
        _frame: Rect,
        children: &[WidgetId],
        _tree: &WidgetTree,
        output: &mut Vec<crate::ui::LayoutChild>
    ) {
        output.clear();
        output.reserve(children.len());
        output.extend(
            children
                .iter()
                .copied()
                .map(|id| crate::ui::LayoutChild::new(id, Size::zero())),
        );
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(WidgetId, Rect)>
    {
        let mut output = Vec::with_capacity(children.len());
        self.layout_cell_children_into(frame, children, &mut output);
        output
    }

    layout_children_into => (
        &self,
        frame: Rect,
        children: &[crate::ui::LayoutChild],
        _tree: &WidgetTree,
        _scratch: &mut crate::ui::LayoutEngineScratch,
        output: &mut Vec<(WidgetId, Rect)>
    ) {
        self.layout_cell_children_into(frame, children, output);
    }

    children_clip => (&self, frame: Rect) -> Option<Rect> { Some(frame) }

    render => (&self, _frame: Rect, _ctx: &mut PaintContext, _tree: &WidgetTree) {}
}

impl CalendarCellHost {
    // 将日期格内容位置写入布局树拥有的跨帧数组。
    fn layout_cell_children_into(
        &self,
        frame: Rect,
        children: &[crate::ui::LayoutChild],
        output: &mut Vec<(WidgetId, Rect)>,
    ) {
        output.clear();
        output.reserve(children.len());
        output.extend(children.iter().map(|child| (child.id, frame)));
    }
}

// Calendar — 日历组件。
widget! {
    /// 拥有月份导航、日期选择与键盘焦点状态的月历组件。
    pub struct Calendar {
        year: Cell<i32>,
        month: Cell<usize>,
        selected_date: Cell<Option<Date>>,
        // 可选受控状态是选中日期的唯一外部所有者。
        value_binding: Option<State<Date>>,
        focused_day: Cell<usize>,
        cell_size: f32,
        // 标记日期格尺寸是否由 Rust 调用方显式覆盖。
        #[snapshot(skip)]
        cell_size_authored: bool,
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
        // 全部实例共享 UIX 声明固化后的只读视觉配置。
        #[snapshot(skip)]
        visual: &'static CalendarVisual,
    }

    tab_index => (&self) -> i32 { 1 }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    measure_children_into => (
        &self,
        _frame: Rect,
        children: &[WidgetId],
        _tree: &WidgetTree,
        output: &mut Vec<crate::ui::LayoutChild>
    ) {
        output.clear();
        output.reserve(children.len());
        output.extend(
            children
                .iter()
                .copied()
                .map(|id| crate::ui::LayoutChild::new(id, Size::zero())),
        );
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(crate::ui::WidgetId, Rect)>
    {
        let mut output = Vec::with_capacity(children.len());
        self.layout_calendar_children_into(frame, children, &mut output);
        output
    }

    layout_children_into => (
        &self,
        frame: Rect,
        children: &[crate::ui::LayoutChild],
        _tree: &WidgetTree,
        _scratch: &mut crate::ui::LayoutEngineScratch,
        output: &mut Vec<(crate::ui::WidgetId, Rect)>
    ) {
        self.layout_calendar_children_into(frame, children, output);
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

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .take()
            .map(|date| SemanticEvent::change(id, date.format()))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let frame = Rect::new(frame.x, frame.y, frame.w.max(0.0), frame.h.max(0.0));
        let local_frame = Rect::new(0.0, 0.0, frame.w, frame.h);
        let Some(local_geometry) =
            CalendarGeometry::new(local_frame, self.cell_size, self.visual.geometry)
        else {
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
            let resolved = self.visual.resolve(ctx.tokens());
            let cur_year = self.year.get();
            let cur_month = self.month.get();
            let event_buckets = self.event_buckets(cur_year, cur_month);
            ctx.push_clip(frame);
            for day in 1..=days_in_month(cur_year, cur_month) {
                let Some(cell_rect) = geometry.cell_rect(cur_year, cur_month, day) else {
                    continue;
                };
                self.paint_event_markers(
                    &event_buckets[day - 1],
                    cell_rect,
                    resolved.text_secondary,
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
        let resolved = self.visual.resolve(ctx.tokens());
        let chrome = self.visual.chrome;
        let selected = self.selected_date.get();
        let focused_day = self.focused_day.get();
        let cur_year = self.year.get();
        let cur_month = self.month.get();
        let event_buckets = self.event_buckets(cur_year, cur_month);
        let radius = resolved.control_radius * geometry.scale;

        ctx.push_clip(frame);
        ctx.fill_rect(
            geometry.control,
            resolved.background,
            Some(Radius::uniform(radius)),
        );
        ctx.stroke_rect(
            geometry.control,
            resolved.border,
            geometry.scale.max(chrome.control_stroke_min),
            Some(Radius::uniform(radius)),
        );

        let loc = crate::ui::widget_runtime::locale::use_locale();
        let title = localized_month_title(&loc, cur_year, cur_month);
        let title_width = (geometry.title_row.w - geometry.navigation_width * 2.0
            - chrome.title_side_gap * geometry.scale)
            .max(0.0);
        let title_font = fitted_font_size(
            ctx,
            &title,
            chrome.title_font_size * geometry.scale,
            title_width,
            geometry.title_row.h * chrome.title_height_ratio,
        );
        if title_font > 0.0 {
            let measured = ctx.measure_text(&title, title_font);
            let title_y = ctx.visual_center_y(geometry.title_row, title_font);
            ctx.draw_text(
                &title,
                Point::new(
                    geometry.title_row.x
                        + (geometry.title_row.w - measured.w)
                            * self.visual.geometry.center_ratio,
                    title_y,
                ),
                resolved.text,
                title_font,
            );
        }
        crate::ui::widgets::icon::Icon::paint_in_frame(
            ctx,
            chrome.previous_icon,
            geometry.previous_navigation(),
            resolved.primary,
            chrome.navigation_icon_size * geometry.scale,
        );
        crate::ui::widgets::icon::Icon::paint_in_frame(
            ctx,
            chrome.next_icon,
            geometry.next_navigation(),
            resolved.primary,
            chrome.navigation_icon_size * geometry.scale,
        );

        let weekdays = loc.weekdays_short;
        let weekday_font = weekdays.iter().fold(
            chrome.weekday_font_size * geometry.scale,
            |font, weekday| {
                font.min(fitted_font_size(
                    ctx,
                    weekday,
                    chrome.weekday_font_size * geometry.scale,
                    geometry.cell_size * chrome.weekday_width_ratio,
                    geometry.weekday_row.h * chrome.weekday_height_ratio,
                ))
            },
        );
        let weekday_y = ctx.visual_center_y(geometry.weekday_row, weekday_font);
        for (index, weekday) in weekdays.iter().enumerate() {
            let measured = ctx.measure_text(weekday, weekday_font);
            ctx.draw_text(
                weekday,
                Point::new(
                    geometry.weekday_row.x
                        + index as f32 * geometry.cell_size
                        + (geometry.cell_size - measured.w)
                            * self.visual.geometry.center_ratio,
                    weekday_y,
                ),
                resolved.text_secondary,
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
                resolved.text_quaternary
            } else if is_selected {
                resolved.white
            } else if is_weekend {
                resolved.error
            } else {
                resolved.text
            };
            if is_selected {
                ctx.fill_rect(
                    cell_rect,
                    resolved.primary,
                    Some(Radius::uniform(chrome.day_radius * geometry.scale)),
                );
            }
            if is_focused {
                let inset = geometry.scale.max(chrome.focus_inset_min);
                let focus_color = if is_selected {
                    resolved.white
                } else {
                    resolved.primary
                };
                ctx.stroke_rect(
                    Rect::new(
                        cell_rect.x + inset,
                        cell_rect.y + inset,
                        (cell_rect.w - inset * 2.0).max(0.0),
                        (cell_rect.h - inset * 2.0).max(0.0),
                    ),
                    focus_color,
                    chrome.focus_stroke_width * geometry.scale,
                    Some(Radius::uniform(chrome.day_radius * geometry.scale)),
                );
            }
            let day_text = CALENDAR_DAY_LABELS[day - 1];
            let day_font = fitted_font_size(
                ctx,
                day_text,
                chrome.day_font_size * geometry.scale,
                cell_rect.w * chrome.day_width_ratio,
                cell_rect.h * chrome.day_height_ratio,
            );
            if !self.custom_cell && day_font > 0.0 {
                let measured = ctx.measure_text(day_text, day_font);
                let text_y = ctx.visual_center_y(cell_rect, day_font);
                ctx.draw_text(
                    day_text,
                    Point::new(
                        cell_rect.x
                            + (cell_rect.w - measured.w) * self.visual.geometry.center_ratio,
                        text_y,
                    ),
                    text_color,
                    day_font,
                );
            }
            ctx.stroke_rect(
                cell_rect,
                resolved.border,
                chrome.grid_stroke_width * geometry.scale,
                None,
            );
            if self.custom_cell && !is_disabled {
                ctx.stroke_rect(
                    cell_rect,
                    resolved.primary,
                    chrome.custom_cell_stroke_width * geometry.scale,
                    None,
                );
            }
            if !self.custom_cell && event_buckets[day - 1].count > 0 {
                self.paint_event_markers(
                    &event_buckets[day - 1],
                    cell_rect,
                    resolved.text_secondary,
                    geometry.scale,
                    ctx,
                );
            }
        }
        ctx.pop_clip();
    }
}

impl Calendar {
    // 将当前月份的自定义日期格位置写入布局树拥有的跨帧数组。
    fn layout_calendar_children_into(
        &self,
        frame: Rect,
        children: &[crate::ui::LayoutChild],
        output: &mut Vec<(WidgetId, Rect)>,
    ) {
        let geometry = CalendarGeometry::new(
            Rect::new(0.0, 0.0, frame.w.max(0.0), frame.h.max(0.0)),
            self.cell_size,
            self.visual.geometry,
        );
        let entries = self.materialized_cells.borrow();
        output.clear();
        output.reserve(children.len().min(entries.len()));
        for (index, child) in children.iter().enumerate() {
            let Some(entry) = entries.get(index) else {
                break;
            };
            let child_frame = geometry
                .and_then(|geometry| {
                    geometry
                        .cell_rect(self.year.get(), self.month.get(), entry.date.day)
                        .map(|rect| Rect::new(frame.x + rect.x, frame.y + rect.y, rect.w, rect.h))
                })
                .unwrap_or_default();
            output.push((child.id, child_frame));
        }
    }

    /// 创建展示 2026 年 6 月、尚未选择日期的日历。
    pub fn new() -> Self {
        Self {
            year: Cell::new(2026),
            month: Cell::new(6),
            selected_date: Cell::new(None),
            // 默认保持非受控选择生命周期。
            value_binding: None,
            focused_day: Cell::new(1),
            cell_size: CALENDAR_VISUAL.geometry.default_cell_size,
            cell_size_authored: false,
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
            visual: CALENDAR_VISUAL_REF,
        }
    }
    /// 设置日期格首选边长；无效值会归一化到支持范围。
    pub fn cell_size(mut self, s: f32) -> Self {
        self.cell_size = Self::normalize_cell_size(s, self.visual.geometry);
        self.cell_size_authored = true;
        self
    }
    /// 返回当前选中日期的日号；尚未选择时返回 `None`。
    pub fn selected_day(&self) -> Option<usize> {
        self.selected_date.get().map(|date| date.day)
    }
    /// 返回当前完整选中日期；尚未选择时返回 `None`。
    pub fn selected_date(&self) -> Option<Date> {
        self.selected_date.get()
    }
    /// 返回当前展示的年份与一至十二月月份编号。
    pub fn displayed_month(&self) -> (i32, usize) {
        (self.year.get(), self.month.get())
    }
    /// 设置初始选中日期；后续 reconcile 保留用户运行态选择。
    pub fn default_date(mut self, date: Date) -> Self {
        // 最后调用的默认值显式切回非受控模式。
        self.value_binding = None;
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

    /// 设置按日期绘制的事件标记集合。
    pub fn events(mut self, events: Vec<CalendarEvent>) -> Self {
        self.events = events;
        self
    }

    /// 设置禁止选择日期的判定函数。
    pub fn disabled_date<F>(self, predicate: F) -> Self
    where
        F: Fn(Date) -> bool + 'static,
    {
        let mut calendar = self;
        calendar.disabled_predicate = Some(Rc::new(predicate));
        calendar
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(
            self.cell_size * self.visual.geometry.grid_columns as f32,
            self.cell_size * self.visual.geometry.grid_rows as f32
                + self.visual.geometry.header_height,
        )
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
        let next_cell_size = Self::normalize_cell_size(next.cell_size, next.visual.geometry);
        if self.cell_size != next_cell_size {
            self.last_geometry.set(None);
        }
        self.cell_size = next_cell_size;
        self.cell_size_authored = next.cell_size_authored;
        self.visual = next.visual;
        self.year_jump = next.year_jump;
        self.events = next.events;
        self.disabled_predicate = next.disabled_predicate;
        self.custom_cell = next.custom_cell;
        self.custom_cell_factory = next.custom_cell_factory;
        // 由窄绑定适配器协调受控值与独立显示月份。
        self.sync_value_binding(next.value_binding);
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

    // 单次线性扫描把当前月份事件归入固定日桶，避免每个日期重复遍历事件表。
    fn event_buckets(&self, year: i32, month: usize) -> [CalendarDayEventBucket; 31] {
        let mut buckets = [CalendarDayEventBucket::default(); 31];
        for (event_index, event) in self.events.iter().enumerate() {
            if event.date.year != year || event.date.month != month || event.date.day == 0 {
                continue;
            }
            let Some(bucket) = buckets.get_mut(event.date.day - 1) else {
                continue;
            };
            bucket.count += 1;
            if bucket.visible_count < self.visual.chrome.event_marker_limit {
                bucket.visible_indices[bucket.visible_count] = event_index;
                bucket.visible_count += 1;
            }
        }
        buckets
    }

    fn paint_event_markers(
        &self,
        bucket: &CalendarDayEventBucket,
        cell_rect: Rect,
        overflow_text: Color,
        scale: f32,
        ctx: &mut PaintContext,
    ) {
        if bucket.count == 0 {
            return;
        }
        let chrome = self.visual.chrome;
        let dot_gap = chrome.event_marker_gap * scale;
        let total_width = bucket.visible_count as f32 * dot_gap;
        for (index, event_index) in bucket.visible_indices[..bucket.visible_count]
            .iter()
            .enumerate()
        {
            let event = &self.events[*event_index];
            ctx.fill_circle(
                cell_rect.x
                    + cell_rect.w * self.visual.geometry.center_ratio
                    + (index as f32 + self.visual.geometry.center_ratio) * dot_gap
                    - total_width * self.visual.geometry.center_ratio,
                cell_rect.y + cell_rect.h - chrome.event_marker_bottom * scale,
                chrome.event_marker_radius * scale,
                event.color,
            );
        }
        if bucket.count > bucket.visible_count {
            ctx.draw_text(
                &format!("+{}", bucket.count - bucket.visible_count),
                Point::new(
                    cell_rect.x + chrome.event_overflow_inset * scale,
                    cell_rect.y + chrome.event_overflow_inset * scale,
                ),
                overflow_text,
                chrome.event_overflow_font_size * scale,
            );
        }
    }

    fn normalize_cell_size(value: f32, visual: CalendarGeometryVisual) -> f32 {
        if value.is_finite() {
            value.max(visual.min_cell_size)
        } else {
            visual.default_cell_size
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
                self.visual.geometry,
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
            // 受控模式先提交唯一外部状态，再允许观察者读取 Change。
            if let Some(state) = self.value_binding.as_ref() {
                // 避免向相同日期产生冗余状态版本。
                if state.get() != date {
                    // 回写已归一化且确定的用户选择。
                    state.set(date);
                }
            }
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

    // 测试目标观察 UIX 声明的关键视觉契约，不扩大公开 API。
    #[cfg(test)]
    fn visual_contract_for_test(&self) -> (f32, f32, f32, f32, f32, usize) {
        (
            self.visual.geometry.default_cell_size,
            self.visual.geometry.min_cell_size,
            self.visual.geometry.header_height,
            self.visual.geometry.title_height,
            self.visual.chrome.day_radius,
            self.visual.chrome.event_marker_limit,
        )
    }

    // 测试目标确认实例共享同一份 UIX 视觉表。
    #[cfg(test)]
    fn shares_visual_with_for_test(&self, other: &Self) -> bool {
        std::ptr::eq(self.visual, other.visual)
    }
}

// 把日期状态、动态日期格与 UIX 静态视觉融合为单一根节点。
fn build_calendar_view(mut kernel: Calendar, visual: &'static CalendarVisual) -> ViewNode {
    if !kernel.cell_size_authored {
        kernel.cell_size = visual.geometry.default_cell_size;
    }
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Calendar {
    fn build(self) -> ViewNode {
        // UIX 拥有公开根与静态视觉；Rust 保留日期状态、动态子树、输入与绘制算法。
        let kernel = self;
        crate::uix!("src/ui/widgets/display/calendar/calendar.uix")
    }
}

impl Default for Calendar {
    fn default() -> Self {
        Self::new()
    }
}
