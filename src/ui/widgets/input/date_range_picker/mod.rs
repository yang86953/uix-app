//! DateRangePicker 日期范围选择器。

use std::cell::Cell;
use std::sync::Arc;

use crate::core::{Constraints, Point, Rect, Size};
use crate::platform::windowing::ControlSize;
use crate::ui::reactive::state::State;
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widgets::input::date_calendar::{
    CalendarPanelState, MonthNavigation, draw_calendar_panel_in_rect_with_visual,
    hit_calendar_date_in_rect_with_visual, hit_month_navigation_in_rect_with_visual,
};
use crate::ui::widgets::input::date_picker::{
    Date, DisabledDate, add_days, days_in_month, next_month, prev_month,
};
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent, WidgetId,
    WidgetTree,
};
use crate::widget;

// 声明 DateRangePicker 的表面约束几何模块。
mod geometry;
// 声明 DateRangePicker 的组合面板缓存方法模块。
mod methods;
// 声明 DateRangePicker 的 UIX 静态视觉与主题解析模块。
mod presentation;

use presentation::*;

// 引入组合面板绝对坐标转换、分区与表面裁剪函数。
use geometry::{
    DateRangePopupParts, absolute_date_range_popup_rect, date_range_popup_parts,
    date_range_surface_rect,
};

/// DateRangePicker 的具名范围预设。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresetDate {
    start: Date,
    end: Date,
}

impl PresetDate {
    /// 创建按时间先后排序起止日期的范围预设。
    pub fn new(start: Date, end: Date) -> Self {
        let (start, end) = ordered_range(start, end);
        Self { start, end }
    }

    /// 创建起止日期都为今天的范围预设。
    pub fn today() -> Self {
        let today = Date::today();
        Self::new(today, today)
    }

    /// 创建包含今天的本周周一至周日范围预设。
    pub fn this_week() -> Self {
        let today = Date::today();
        let start = add_days(today, -(today.weekday().monday_index() as i64));
        Self::new(start, add_days(start, 6))
    }

    /// 创建包含今天的自然月完整范围预设。
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

    /// 返回范围中较早的起始日期。
    pub const fn start(self) -> Date {
        self.start
    }

    /// 返回范围中较晚的结束日期。
    pub const fn end(self) -> Date {
        self.end
    }
}

widget! {
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
        // 缓存相对触发器原点的最终组合面板矩形。
        popup_rect: Cell<Rect>,
        // 缓存最近登记或绘制使用的逻辑表面。
        surface_rect: Cell<Option<Rect>>,
        // 缓存最终组合面板对应的绝对触发器矩形。
        popup_anchor_frame: Cell<Option<Rect>>,
        // 累计当前呈现周期内旧新组合面板的绝对脏区。
        popup_damage_rect: Cell<Rect>,
        // 同目录 UIX 生成的唯一静态视觉表。
        #[snapshot(skip)]
        visual: &'static DateRangePickerVisual,
    }

    tab_index => (&self) -> i32 { 1 }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(
            self.visual.trigger.intrinsic_width,
            crate::ui::widget_runtime::config::control_height(self.picker_size),
        ))
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        self.sync_bound_values();
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                self.focused = true;
                if !self.open.get() {
                    self.open_from_current_value();
                    return EventResult::Handled;
                }

                if let Some(frame) = self.last_frame.get() {
                    // 读取同帧登记、绘制和命中共享的实际组合面板。
                    let popup = self.interaction_popup_rect(frame);
                    // 从最终组合面板派生月历与预设分区。
                    let parts = date_range_popup_parts(popup, self.presets.len(), self.visual);
                    // 在缩放后的实际预设页脚中命中预设。
                    if let Some(index) = self.hit_preset(parts, *pos) {
                        if let Some((_, preset)) = self.presets.get(index) {
                            if !self.range_has_disabled_endpoint(preset.start, preset.end) {
                                self.commit_range(preset.start, preset.end);
                                self.close_popup();
                            }
                        }
                        return EventResult::Handled;
                    }
                    // 在缩放后的实际月历标题栏中命中月份导航。
                    if let Some(navigation) = hit_month_navigation_in_rect_with_visual(
                        parts.calendar,
                        *pos,
                        self.visual.calendar,
                    )
                    {
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
                        self.hover_date.set(None);
                        return EventResult::Handled;
                    }
                    // 在缩放后的实际日期网格中命中日期。
                    if let Some(date) = hit_calendar_date_in_rect_with_visual(
                        parts.calendar,
                        *pos,
                        self.view_year.get(),
                        self.view_month.get(),
                        self.visual.calendar,
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
                        // 读取同帧登记、绘制和命中共享的实际组合面板。
                        let popup = self.interaction_popup_rect(frame);
                        // 从最终组合面板派生实际月历分区。
                        let parts = date_range_popup_parts(popup, self.presets.len(), self.visual);
                        // 在缩放后的实际日期网格中解析悬停日期。
                        let hit = hit_calendar_date_in_rect_with_visual(
                            parts.calendar,
                            *pos,
                            self.view_year.get(),
                            self.view_month.get(),
                            self.visual.calendar,
                        )
                        .filter(|date| !self.is_date_disabled(*date));
                        if self.hover_date.replace(hit) != hit {
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
                            let (year, month) =
                                prev_month(self.view_year.get(), self.view_month.get());
                            self.view_year.set(year);
                            self.view_month.set(month);
                            self.hover_date.set(None);
                            EventResult::Handled
                        }
                        KeyCode::Right => {
                            let (year, month) =
                                next_month(self.view_year.get(), self.view_month.get());
                            self.view_year.set(year);
                            self.view_month.set(month);
                            self.hover_date.set(None);
                            EventResult::Handled
                        }
                        // 上下方向键以周为单位移动键盘聚焦日期；Left/Right 保持切月。
                        KeyCode::Up | KeyCode::Down => {
                            // 无聚焦日期时依次回退到待选起点、当前范围起点或今天。
                            let anchor = self
                                .hover_date
                                .get()
                                .or_else(|| self.pending_start.get())
                                .or_else(|| self.current_range().map(|(start, _)| start))
                                .unwrap_or_else(Date::today);
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
                        // 回车确认：无聚焦日期时以当前范围起点为锚，复用指针点击的
                        // 两段式选择语义（先定起点、再定终点）。
                        KeyCode::Enter => {
                            let target = self
                                .hover_date
                                .get()
                                .or_else(|| self.pending_start.get())
                                .or_else(|| self.current_range().map(|(start, _)| start))
                                .unwrap_or_else(Date::today);
                            if !self.is_date_disabled(target) {
                                if let Some(start) = self.pending_start.get() {
                                    self.commit_range(start, target);
                                    self.close_popup();
                                } else {
                                    self.pending_start.set(Some(target));
                                }
                            }
                            EventResult::Handled
                        }
                        _ => EventResult::NotHandled,
                    }
                } else if *key == KeyCode::Space || *key == KeyCode::Enter {
                    self.open_from_current_value();
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change.take().map(|(start, end)| {
            SemanticEvent::change(id, format!("{} / {}", start.format(), end.format()))
        })
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        if self.open.get() {
            // 读取最近登记或绘制记录的当前逻辑表面。
            let surface = self.surface_or_fallback(frame);
            // 解析并缓存当前实际组合面板。
            let popup = self.remember_popup_rect(frame, surface);
            // 将相对组合面板转换为窗口绝对坐标。
            let popup = absolute_date_range_popup_rect(frame, popup);
            // 触发器与当前组合面板命中框共同收敛到表面内。
            date_range_surface_rect(frame, popup, surface)
        } else {
            frame
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.sync_bound_values();
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        // 触发框、月历与预设页脚共享同帧一次 UIX 主题解析。
        let visual = self.visual.resolve(ctx.tokens());
        let trigger = self.visual.trigger;
        let radius = Some(crate::draw::Radius::uniform(visual.trigger_radius));

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
            let text_y = ctx.visual_center_y(frame, font_size);
            if text_area.w > 0.0 {
                ctx.push_clip(text_area);
                if let Some((start, end)) = self.current_range() {
                    let start = start.format();
                    let end = end.format();
                    let start_width = ctx.measure_text(&start, font_size).w;
                    let range_icon_gap = trigger.range_icon_gap * scale;
                    let range_icon_width = trigger.range_icon_width * scale;
                    ctx.draw_text(
                        &start,
                        Point::new(text_left, text_y),
                        visual.text,
                        font_size,
                    );
                    let range_icon_frame = Rect::new(
                        text_left + start_width + range_icon_gap,
                        text_area.y,
                        range_icon_width,
                        text_area.h,
                    );
                    crate::ui::widgets::icon::Icon::paint_in_frame(
                        ctx,
                        self.visual.chrome.range_icon,
                        range_icon_frame,
                        visual.text_secondary,
                        font_size,
                    );
                    ctx.draw_text(
                        &end,
                        Point::new(
                            range_icon_frame.x + range_icon_frame.w + range_icon_gap,
                            text_y,
                        ),
                        visual.text,
                        font_size,
                    );
                } else {
                    ctx.draw_text(
                        &self.placeholder,
                        Point::new(text_left, text_y),
                        visual.text_tertiary,
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
            // 在绘制前解析并缓存同帧最终组合面板几何。
            let popup = self.remember_popup_rect(frame, surface);
            // 将相对触发器缓存转换为窗口绝对组合面板矩形。
            let popup = absolute_date_range_popup_rect(frame, popup);
            // 从最终组合面板派生月历与预设分区。
            let parts = date_range_popup_parts(popup, self.presets.len(), self.visual);
            let pending = self.pending_start.get();
            let preview_range = pending
                .zip(self.hover_date.get())
                .map(|(start, end)| ordered_range(start, end));
            // 将组合弹层绘制限制在当前逻辑表面。
            ctx.push_clip(surface);
            // 使用最终月历分区驱动缩放月历绘制。
            draw_calendar_panel_in_rect_with_visual(
                parts.calendar,
                ctx,
                CalendarPanelState {
                    year: self.view_year.get(),
                    month: self.view_month.get(),
                    active: pending,
                    range: preview_range.or_else(|| self.current_range()),
                    hover: self.hover_date.get(),
                    disabled_date: self.disabled_date.as_ref(),
                },
                self.visual.calendar,
                self.visual.calendar_icons,
                visual.calendar(),
            );
            // 使用同一组合分区绘制缩放后的预设页脚。
            self.draw_presets(parts, ctx, visual);
            // 恢复组合弹层外层的逻辑表面裁剪。
            ctx.pop_clip();
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        // 读取最近登记或绘制记录的当前逻辑表面。
        let surface = self.surface_or_fallback(frame);
        // 合并当前与本次呈现周期历史组合面板脏区。
        let popup = self.damage_popup_rect(frame, surface);
        // 将输入框和组合面板脏区限制在当前表面内。
        date_range_surface_rect(frame, popup, surface)
    }

    overlay_entry => (&self, id: WidgetId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        self.open.get().then(|| {
            // 读取最近记录的表面或首次有限回退。
            let surface = self.surface_or_fallback(frame);
            // 解析并缓存当前实际组合面板。
            let popup = self.remember_popup_rect(frame, surface);
            // 将相对组合面板转换为窗口绝对坐标。
            let bounds = absolute_date_range_popup_rect(frame, popup);
            // 创建只覆盖实际组合面板的浮层登记。
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Popover)
                // 登记边界与绘制、命中共用同一矩形。
                .bounds(bounds)
                .z_index(self.visual.chrome.overlay_z)
        })
    }

    // 使用组件树提供的同帧表面创建日期范围面板登记。
    overlay_entry_for_surface => (&self, id: WidgetId, frame: Rect, surface: Rect) -> Option<crate::ui::OverlayEntry> {
        // 在旧登记入口执行前刷新表面与实际组合面板缓存。
        self.remember_popup_rect(frame, surface);
        // 复用统一的组合面板登记逻辑。
        self.overlay_entry(id, frame)
    }
}

impl DateRangePicker {
    /// 创建未选择范围、默认关闭并使用当前区域设置占位文本的选择器。
    pub fn new() -> Self {
        let today = Date::today();
        let config = crate::ui::widget_runtime::config::use_config();
        Self {
            start_value: Cell::new(Date::default()),
            end_value: Cell::new(Date::default()),
            start_binding: None,
            end_binding: None,
            view_year: Cell::new(today.year),
            view_month: Cell::new(today.month),
            placeholder: crate::ui::widget_runtime::locale::use_locale()
                .placeholder
                .to_owned(),
            presets: Vec::new(),
            disabled_date: None,
            open: Cell::new(false),
            focused: false,
            pending_start: Cell::new(None),
            hover_date: Cell::new(None),
            picker_size: config.size,
            last_frame: Cell::new(None),
            pending_change: Cell::new(None),
            // 初始化为空的本地组合面板缓存。
            popup_rect: Cell::new(Rect::zero()),
            // 初始化为尚未取得真实逻辑表面。
            surface_rect: Cell::new(None),
            // 初始化为尚未记录绝对触发器锚点。
            popup_anchor_frame: Cell::new(None),
            // 初始化为空的组合面板历史脏区。
            popup_damage_rect: Cell::new(Rect::zero()),
            visual: DATE_RANGE_PICKER_VISUAL_REF,
        }
    }

    /// 将起始日期双向绑定到外部响应式状态。
    pub fn start(mut self, state: &State<Date>) -> Self {
        self.start_binding = Some(state.clone());
        self.start_value.set(state.get());
        self
    }

    /// 将结束日期双向绑定到外部响应式状态。
    pub fn end(mut self, state: &State<Date>) -> Self {
        self.end_binding = Some(state.clone());
        self.end_value.set(state.get());
        self
    }

    /// 设置按时间排序的非受控默认范围，并移除起止状态绑定。
    pub fn default_range(mut self, start: Date, end: Date) -> Self {
        let (start, end) = ordered_range(start, end);
        self.start_binding = None;
        self.end_binding = None;
        self.start_value.set(start);
        self.end_value.set(end);
        self
    }

    /// 设置尚未选择完整范围时显示的占位文本。
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// 设置日期范围输入框的控件尺寸档位。
    pub fn size(mut self, size: ControlSize) -> Self {
        self.picker_size = size;
        self
    }

    /// 替换面板中可直接提交的具名日期范围预设。
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

    /// 设置返回 `true` 时禁止选择对应日期的线程安全谓词。
    pub fn disabled_date(
        mut self,
        predicate: impl Fn(Date) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.disabled_date = Some(Arc::new(predicate));
        self
    }

    /// 返回按时间排序的完整当前范围；任一端未设置时返回 `None`。
    pub fn current_range(&self) -> Option<(Date, Date)> {
        let start = self.start_value.get();
        let end = self.end_value.get();
        (start != Date::default() && end != Date::default()).then(|| ordered_range(start, end))
    }

    /// 返回日期范围面板当前是否打开。
    pub fn is_open(&self) -> bool {
        self.open.get()
    }

    /// 返回是否已选择临时起点并正在等待结束日期。
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
            open: self.open.get(),
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
        if !std::ptr::eq(self.visual, next.visual) {
            self.visual = next.visual;
            self.reset_popup_presentation();
        }
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
        // 仅在关闭到开启的边沿开始新的组合面板呈现周期。
        if !self.open.get() {
            // 清除上一次呈现留下的表面与脏区缓存。
            self.reset_popup_presentation();
        }
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

    fn hit_preset(&self, parts: DateRangePopupParts, position: Point) -> Option<usize> {
        // 无预设时不参与页脚命中。
        if self.presets.is_empty() {
            // 返回未命中。
            return None;
        }
        // 读取最终组合分区中的预设页脚。
        let footer = parts.footer?;
        // 无有效缩放行高时不参与命中。
        if parts.preset_row_height <= 0.0 {
            // 返回未命中。
            return None;
        }
        // 检查页脚横向范围与缩放后首行起点。
        if position.x < footer.x
            // 检查页脚右边界。
            || position.x >= footer.x + footer.w
            // 检查缩放后的顶部内边距。
            || position.y < footer.y + parts.preset_vertical_inset
            // 检查页脚底边。
            || position.y >= footer.y + footer.h
        {
            // 页脚外返回未命中。
            return None;
        }
        // 按最终页脚起点、内边距和行高解析预设索引。
        let index = ((position.y - footer.y - parts.preset_vertical_inset)
            // 使用缩放后的真实预设行高。
            / parts.preset_row_height) as usize;
        // 只返回当前预设集合内的索引。
        (index < self.presets.len()).then_some(index)
    }

    fn draw_presets(
        &self,
        parts: DateRangePopupParts,
        ctx: &mut PaintContext,
        visual: ResolvedDateRangePickerVisual,
    ) {
        // 无预设时不绘制页脚。
        if self.presets.is_empty() {
            // 提前结束空页脚绘制。
            return;
        }
        // 读取最终组合分区中的预设页脚。
        let Some(footer) = parts.footer else {
            // 缩为空时不生成页脚绘制命令。
            return;
        };
        // 空页脚或零字号不生成绘制命令。
        if footer.w <= 0.0 || footer.h <= 0.0 || parts.preset_font_size <= 0.0 {
            // 提前结束不可见页脚绘制。
            return;
        }
        let radius = Some(crate::draw::Radius::uniform(visual.popup_radius));
        ctx.push_clip(footer);
        ctx.fill_rect(footer, visual.popup_background, radius);
        ctx.stroke_rect(
            footer,
            visual.border,
            self.visual.popup.preset_border_width,
            radius,
        );
        for (index, (label, _)) in self.presets.iter().enumerate() {
            // 按最终页脚指标构造当前预设行。
            let row = Rect::new(
                footer.x + parts.preset_horizontal_inset,
                footer.y + parts.preset_vertical_inset + index as f32 * parts.preset_row_height,
                (footer.w - parts.preset_horizontal_inset * 2.0).max(0.0),
                parts.preset_row_height,
            );
            // 计算缩放后预设文字基线。
            let text_y = ctx.visual_center_y(row, parts.preset_font_size);
            ctx.push_clip(row);
            // 在当前实际预设行内绘制文字。
            ctx.draw_text(
                label,
                Point::new(row.x, text_y),
                visual.primary,
                parts.preset_font_size,
            );
            ctx.pop_clip();
        }
        ctx.pop_clip();
    }
}

impl Default for DateRangePicker {
    fn default() -> Self {
        Self::new()
    }
}

// 把 DateRangePicker Rust 状态内核与 UIX 静态视觉组合为单一组件节点。
fn build_date_range_picker_view(
    mut kernel: DateRangePicker,
    visual: &'static DateRangePickerVisual,
) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

// 为 UIX 根提供稳定的 Rust 内核绑定名称。
fn build_date_range_picker_uix_root(kernel: DateRangePicker) -> ViewNode {
    crate::uix!("src/ui/widgets/input/date_range_picker/date_range_picker.uix")
}

impl View for DateRangePicker {
    fn build(self) -> ViewNode {
        build_date_range_picker_uix_root(self)
    }
}

fn ordered_range(start: Date, end: Date) -> (Date, Date) {
    if start <= end {
        (start, end)
    } else {
        (end, start)
    }
}

// 将 DateRangePicker 表面约束契约放在独立测试文件中。
