//! DatePicker 日期选择器 — 弹出日历选择日期。
//!
//! 基于 Calendar 的日期逻辑，增加弹出面板和选中回显。

use std::cell::Cell;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::core::{Constraints, Point, Rect, Size};
use crate::platform::windowing::ControlSize;
use crate::ui::reactive::state::State;
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widgets::binding::write_if_changed;
use crate::ui::widgets::input::date_calendar::{
    CalendarPanelState, MonthNavigation, draw_calendar_panel_in_rect_with_visual,
    hit_calendar_date_in_rect_with_visual, hit_month_navigation_in_rect_with_visual,
};
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent, WidgetId,
    WidgetTree,
};
use crate::widget;

// 声明 DatePicker 的表面约束几何模块。
mod geometry;
// 声明 DatePicker 的弹层缓存方法模块。
mod methods;
// 声明 DatePicker 的 UIX 静态视觉与主题解析模块。
pub(crate) mod presentation;

use presentation::*;

// 引入日期面板绝对坐标转换与表面裁剪函数。
use geometry::{absolute_date_picker_popup_rect, date_picker_surface_rect};

/// 归一到合法年月日的公历日期。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Date {
    /// 公历年份。
    pub year: i32,
    /// 公历月份；通过构造器创建时范围为一至十二。
    pub month: usize,
    /// 月内日期；通过构造器创建时夹取到该月有效范围。
    pub day: usize,
}

impl Date {
    /// 创建日期，并将月份和月内日期夹取到有效公历范围。
    pub fn new(year: i32, month: usize, day: usize) -> Self {
        let month = month.clamp(1, 12);
        let day = day.clamp(1, days_in_month(year, month));
        Self { year, month, day }
    }
    /// 返回以连字符分隔并补零的年月日文本。
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
        let (year, month, day) = days_to_date(days);
        Self::new(year, month as usize, day as usize)
    }
}

/// 公历星期，顺序从周一到周日。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Weekday {
    /// 星期一。
    Monday,
    /// 星期二。
    Tuesday,
    /// 星期三。
    Wednesday,
    /// 星期四。
    Thursday,
    /// 星期五。
    Friday,
    /// 星期六。
    Saturday,
    /// 星期日。
    Sunday,
}

impl Weekday {
    /// 返回当前星期是否为星期六或星期日。
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
    /// 选择并写回具体公历日期。
    Date,
    /// 选择日期所在周，并写回该周星期一。
    Week,
    /// 选择月份，并写回该月第一天。
    Month,
    /// 选择季度，并写回该季度第一天。
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

/// 将自 Unix 纪元以来的天数转换为公历日期（年、月、日）。
///
/// 原为 `core::diagnostic::timestamp::days_to_date`；旧日志/诊断轮子删除后
/// 归属 UI 日期组件（与 `first_weekday` / `days_in_month` 同族）。
pub(crate) fn days_to_date(days: u64) -> (i32, u32, u32) {
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era as i64 * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    (y as i32, mo as u32, d as u32)
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

widget! {
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
        // 缓存相对触发器原点的最终日期面板矩形。
        popup_rect: Cell<Rect>,
        // 缓存最近登记或绘制使用的逻辑表面。
        surface_rect: Cell<Option<Rect>>,
        // 缓存最终日期面板对应的绝对触发器矩形。
        popup_anchor_frame: Cell<Option<Rect>>,
        // 累计当前呈现周期内旧新日期面板的绝对脏区。
        popup_damage_rect: Cell<Rect>,
        // 同目录 UIX 生成的唯一静态视觉表。
        #[snapshot(skip)]
        visual: &'static DatePickerVisual,
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
                    self.open_popup();
                    return EventResult::Handled;
                }

                if let Some(frame) = self.last_frame.get() {
                    // 读取同帧登记、绘制和命中共享的实际面板矩形。
                    let popup = self.interaction_popup_rect(frame);
                    // 在缩放后的实际标题栏中命中月份导航。
                    if let Some(navigation) = hit_month_navigation_in_rect_with_visual(
                        popup,
                        *pos,
                        self.visual.calendar,
                    ) {
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
                        self.hover_date.set(None);
                        return EventResult::Handled;
                    }

                    // 在缩放后的实际日期网格中命中日期。
                    if let Some(hit_date) = hit_calendar_date_in_rect_with_visual(
                        popup,
                        *pos,
                        self.view_year.get(),
                        self.view_month.get(),
                        self.visual.calendar,
                    ) {
                        if !self.is_date_disabled(hit_date) {
                            self.commit_value(self.mode.normalize(hit_date));
                            self.close_popup();
                        }
                    }
                }
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if self.open.get() {
                    if let Some(frame) = self.last_frame.get() {
                        // 读取同帧登记、绘制和命中共享的实际面板矩形。
                        let popup = self.interaction_popup_rect(frame);
                        // 在缩放后的实际日期网格中解析悬停日期。
                        let hover = hit_calendar_date_in_rect_with_visual(
                            popup,
                            *pos,
                            self.view_year.get(),
                            self.view_month.get(),
                            self.visual.calendar,
                        )
                        .filter(|date| !self.is_date_disabled(*date));
                        if self.hover_date.replace(hover) != hover {
                            return EventResult::Handled;
                        }
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerLeave => {
                if self.hover_date.replace(None).is_some() {
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
                self.close_popup();
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } => {
                if self.open.get() {
                    match key {
                        KeyCode::Escape => {
                            self.close_popup();
                            EventResult::Handled
                        }
                        KeyCode::Left => {
                            let (y, m) = prev_month(self.view_year.get(), self.view_month.get());
                            self.view_year.set(y); self.view_month.set(m);
                            self.hover_date.set(None);
                            EventResult::Handled
                        }
                        KeyCode::Right => {
                            let (y, m) = next_month(self.view_year.get(), self.view_month.get());
                            self.view_year.set(y); self.view_month.set(m);
                            self.hover_date.set(None);
                            EventResult::Handled
                        }
                        // 上下方向键以周为单位移动键盘聚焦日期；Left/Right 保持切月，
                        // 不再承担选日职责（与 TimePicker 的 Up/Down 移动高亮语义对齐）。
                        KeyCode::Up | KeyCode::Down => {
                            // 无焦点日期时以当前选中值（或今天）为锚点。
                            let anchor = self
                                .hover_date
                                .get()
                                .unwrap_or_else(|| self.selected_or_today());
                            // 在日历网格中按周移动：上移 7 天、下移 7 天。
                            let moved = add_days(anchor, if *key == KeyCode::Up { -7 } else { 7 });
                            // 禁用日期不承接键盘焦点（与指针悬停过滤一致）。
                            if !self.is_date_disabled(moved) {
                                self.hover_date.set(Some(moved));
                                // 跨月移动时同步视图月份，保持网格内可见焦点。
                                self.view_year.set(moved.year);
                                self.view_month.set(moved.month);
                            }
                            EventResult::Handled
                        }
                        // 回车确认：优先提交键盘聚焦日期，无聚焦时确认当前选中值
                        // （与 TimePicker 初始高亮即当前值的 Enter 语义一致）。
                        KeyCode::Enter => {
                            let target = self
                                .hover_date
                                .get()
                                .unwrap_or_else(|| self.selected_or_today());
                            // 按模式归一（周/月/季度对齐）。
                            let target = self.mode.normalize(target);
                            if !self.is_date_disabled(target) {
                                self.commit_value(target);
                                self.close_popup();
                            }
                            EventResult::Handled
                        }
                        _ => EventResult::NotHandled,
                    }
                } else if *key == KeyCode::Space || *key == KeyCode::Enter {
                    self.open_popup();
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
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

    hit_test_frame => (&self, frame: Rect) -> Rect {
        if self.open.get() {
            // 读取最近登记或绘制记录的当前逻辑表面。
            let surface = self.surface_or_fallback(frame);
            // 解析并缓存当前实际日期面板。
            let popup = self.remember_popup_rect(frame, surface);
            // 将相对面板转换为窗口绝对坐标。
            let popup = absolute_date_picker_popup_rect(frame, popup);
            // 触发器与当前面板命中框共同收敛到表面内。
            date_picker_surface_rect(frame, popup, surface)
        } else {
            frame
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.sync_bound_value();
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        // 触发框与月历面板共享同帧一次 UIX 主题解析。
        let visual = self.visual.resolve(ctx.tokens());
        let trigger = self.visual.trigger;
        let radius = Some(crate::draw::Radius::uniform(visual.trigger_radius));

        let val = self.value.get();
        let is_default = val == Date::default();
        let nominal_height = crate::ui::widget_runtime::config::control_height(self.picker_size);
        let scale = if nominal_height > 0.0 {
            (frame.h / nominal_height).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let font_size = trigger.font_size * scale;
        let horizontal_padding = trigger.horizontal_padding * scale;
        let icon_gap = trigger.icon_gap * scale;
        let icon_slot_width = trigger.icon_slot_width * scale;
        let icon_right_inset = trigger.icon_right_inset * scale;

        ctx.push_clip(frame);
        ctx.fill_rect(frame, visual.trigger_background, radius);
        ctx.stroke_rect(
            frame,
            if self.focused { visual.primary } else { visual.border },
            if self.focused {
                trigger.focus_border_width
            } else {
                trigger.border_width
            },
            radius,
        );

        if font_size > 0.0 && frame.w > 0.0 {
            let icon_frame = Rect::new(
                frame.x + frame.w - icon_right_inset - icon_slot_width,
                frame.y,
                icon_slot_width,
                frame.h,
            );
            let text_left = frame.x + horizontal_padding;
            let text_right = (icon_frame.x - icon_gap).max(text_left);
            let text_area = Rect::new(text_left, frame.y, text_right - text_left, frame.h);
            let input_text_y = ctx.visual_center_y(frame, font_size);
            if text_area.w > 0.0 {
                ctx.push_clip(text_area);
                if is_default {
                    ctx.draw_text(
                        &self.placeholder,
                        Point::new(text_left, input_text_y),
                        visual.text_tertiary,
                        font_size,
                    );
                } else {
                    let formatted = val.format();
                    ctx.draw_text(
                        &formatted,
                        Point::new(text_left, input_text_y),
                        visual.text,
                        font_size,
                    );
                }
                ctx.pop_clip();
            }

            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                self.visual.chrome.trigger_icon,
                icon_frame,
                visual.text_secondary,
                font_size,
            );
        }
        ctx.pop_clip();

        if self.open.get() {
            // 从绘制上下文读取当前逻辑表面尺寸。
            let surface_size = ctx.logical_surface_size();
            // 将窗口原点与逻辑尺寸组合为当前表面矩形。
            let surface = Rect::new(0.0, 0.0, surface_size.w, surface_size.h);
            // 在绘制前解析并缓存同帧最终日期面板几何。
            let popup = self.remember_popup_rect(frame, surface);
            // 将相对触发器缓存转换为窗口绝对面板矩形。
            let popup = absolute_date_picker_popup_rect(frame, popup);
            // 将弹层绘制限制在当前逻辑表面。
            ctx.push_clip(surface);
            // 使用最终面板矩形驱动缩放月历绘制。
            draw_calendar_panel_in_rect_with_visual(
                popup,
                ctx,
                CalendarPanelState {
                    year: self.view_year.get(),
                    month: self.view_month.get(),
                    active: (!is_default).then_some(val),
                    range: None,
                    hover: self.hover_date.get(),
                    disabled_date: self.disabled_date.as_ref(),
                },
                self.visual.calendar,
                self.visual.calendar_icons,
                visual.calendar(),
            );
            // 恢复日期面板外层的逻辑表面裁剪。
            ctx.pop_clip();
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        // 读取最近登记或绘制记录的当前逻辑表面。
        let surface = self.surface_or_fallback(frame);
        // 合并当前与本次呈现周期历史日期面板脏区。
        let popup = self.damage_popup_rect(frame, surface);
        // 将输入框和日期面板脏区限制在当前表面内。
        date_picker_surface_rect(frame, popup, surface)
    }

    overlay_entry => (&self, id: WidgetId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        self.open.get().then(|| {
            // 读取最近记录的表面或首次有限回退。
            let surface = self.surface_or_fallback(frame);
            // 解析并缓存当前实际日期面板。
            let popup = self.remember_popup_rect(frame, surface);
            // 将相对面板转换为窗口绝对坐标。
            let bounds = absolute_date_picker_popup_rect(frame, popup);
            // 创建只覆盖实际日期面板的浮层登记。
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Popover)
                // 登记边界与绘制、命中共用同一矩形。
                .bounds(bounds)
                .z_index(self.visual.chrome.overlay_z)
        })
    }

    // 使用组件树提供的同帧表面创建日期面板登记。
    overlay_entry_for_surface => (&self, id: WidgetId, frame: Rect, surface: Rect) -> Option<crate::ui::OverlayEntry> {
        // 在旧登记入口执行前刷新表面与实际面板缓存。
        self.remember_popup_rect(frame, surface);
        // 复用统一的日期面板登记逻辑。
        self.overlay_entry(id, frame)
    }
}

impl DatePicker {
    fn intrinsic_size(&self) -> Size {
        Size::new(
            self.visual.layout.intrinsic_width,
            crate::ui::widget_runtime::config::control_height(self.picker_size),
        )
    }

    /// 创建未选值、按日期选择且使用当前 Provider 尺寸的选择器。
    pub fn new() -> Self {
        let today = Date::today();
        let config = crate::ui::widget_runtime::config::use_config();
        Self {
            value: Cell::new(Date::default()),
            value_binding: None,
            view_year: Cell::new(today.year),
            view_month: Cell::new(today.month),
            placeholder: crate::ui::widget_runtime::locale::use_locale()
                .placeholder
                .to_owned(),
            mode: PickerMode::Date,
            disabled_date: None,
            open: Cell::new(false),
            focused: false,
            hover_date: Cell::new(None),
            picker_size: config.size,
            last_frame: Cell::new(None),
            pending_change: Cell::new(None),
            // 初始化为空的本地日期面板缓存。
            popup_rect: Cell::new(Rect::zero()),
            // 初始化为尚未取得真实逻辑表面。
            surface_rect: Cell::new(None),
            // 初始化为尚未记录绝对触发器锚点。
            popup_anchor_frame: Cell::new(None),
            // 初始化为空的日期面板历史脏区。
            popup_damage_rect: Cell::new(Rect::zero()),
            visual: DATE_PICKER_VISUAL_REF,
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

    /// 设置未选择日期时显示的占位文本。
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// 设置日期输入框使用的标准控件尺寸。
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
    /// 返回日期面板当前是否已打开。
    pub fn is_open(&self) -> bool {
        self.open.get()
    }
    /// 返回当前日期、周、月或季度选择粒度。
    pub fn picker_mode(&self) -> PickerMode {
        self.mode
    }
    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        let value = self.current_value();
        SnapshotFields::DatePicker {
            placeholder: self.placeholder.clone(),
            value: (value != Date::default()).then(|| value.format()),
            open: self.open.get(),
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
        if !std::ptr::eq(self.visual, next.visual) {
            self.visual = next.visual;
            self.reset_popup_presentation();
        }
    }

    fn selected_or_today(&self) -> Date {
        let value = self.value.get();
        if value == Date::default() {
            Date::today()
        } else {
            value
        }
    }

    fn open_popup(&self) {
        // 仅在关闭到开启的边沿开始新的面板呈现周期。
        if !self.open.get() {
            // 清除上一次呈现留下的表面与脏区缓存。
            self.reset_popup_presentation();
        }
        let value = self.selected_or_today();
        self.view_year.set(value.year);
        self.view_month.set(value.month);
        self.hover_date.set(None);
        self.open.set(true);
    }

    fn close_popup(&self) {
        self.open.set(false);
        self.hover_date.set(None);
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
        write_if_changed(self.value_binding.as_ref(), value);
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

// 把 DatePicker Rust 状态内核与 UIX 静态视觉组合为单一组件节点。
fn build_date_picker_view(mut kernel: DatePicker, visual: &'static DatePickerVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

// 为 UIX 根提供稳定的 Rust 内核绑定名称。
fn build_date_picker_uix_root(kernel: DatePicker) -> ViewNode {
    crate::uix!("src/ui/widgets/input/date_picker/date_picker.uix")
}

impl View for DatePicker {
    fn build(self) -> ViewNode {
        build_date_picker_uix_root(self)
    }
}

pub(crate) fn next_month(y: i32, m: usize) -> (i32, usize) {
    if m >= 12 { (y + 1, 1) } else { (y, m + 1) }
}
pub(crate) fn prev_month(y: i32, m: usize) -> (i32, usize) {
    if m <= 1 { (y - 1, 12) } else { (y, m - 1) }
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

// 将 DatePicker 表面约束契约放在独立测试文件中。
