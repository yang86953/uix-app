//! TimePicker 时间选择器 — 选择时:分。
//!
//! 弹出面板含小时/分钟滚动选择，支持 hover 高亮、键盘导航。

use std::cell::Cell;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::core::{Constraints, Point, Rect, Size};
use crate::platform::windowing::ControlSize;
use crate::ui::reactive::state::State;
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent, WidgetId,
    WidgetTree,
};
use crate::widget;

// 声明时间面板的私有几何解析模块。
mod geometry;
// 声明时间选择器的私有弹层缓存方法模块。
mod methods;
// 声明 UIX 静态视觉契约与主题解析模块。
mod presentation;

// 引入时间面板绝对与本地坐标转换。
use geometry::{
    // 将本地时间面板转换为窗口绝对矩形。
    absolute_time_popup_rect,
    // 将触发器与时间面板占用区裁剪到当前表面。
    time_surface_rect,
};
use presentation::*;

const HOUR_COUNT: usize = 24;
const MINUTE_COUNT: usize = 60;
const WHEEL_STEP: f32 = 40.0;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum TimeColumn {
    #[default]
    Hour,
    Minute,
}

/// 时间结构
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Time {
    /// 小时分量；通过 [`Time::new`] 构造时会夹紧到 `0..=23`。
    pub hour: u32,
    /// 分钟分量；通过 [`Time::new`] 构造时会夹紧到 `0..=59`。
    pub minute: u32,
}

impl Time {
    /// 创建时间值，并将小时与分钟分别夹紧到有效范围。
    pub fn new(hour: u32, minute: u32) -> Self {
        Self {
            hour: hour.min(23),
            minute: minute.min(59),
        }
    }
    /// 按二十四小时制 `HH:MM` 格式返回时间文本。
    pub fn format(&self) -> String {
        format!("{:02}:{:02}", self.hour, self.minute)
    }

    /// 返回当前 UTC 时间，精确到分钟。
    pub fn now() -> Self {
        let elapsed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO);
        let seconds = elapsed.as_secs() % 86_400;
        Self::new((seconds / 3_600) as u32, ((seconds % 3_600) / 60) as u32)
    }
}

widget! {
    /// TimePicker — 时间选择器。
    pub struct TimePicker {
        value: Cell<Time>,
        value_configured: Cell<bool>,
        value_binding: Option<State<Time>>,
        placeholder: String,
        open: Cell<bool>,
        focused: bool,
        hover_hour: Cell<usize>,
        hover_minute: Cell<usize>,
        /// 小时滚动偏移（行号）
        scroll_hour: Cell<f32>,
        scroll_min: Cell<f32>,
        active_column: Cell<TimeColumn>,
        picker_size: ControlSize,
        last_frame: Cell<Option<Rect>>,
        pending_change: Cell<Option<Time>>,
        // 缓存相对触发器原点的最终时间面板矩形。
        popup_rect: Cell<Rect>,
        // 缓存最近登记或绘制使用的逻辑表面。
        surface_rect: Cell<Option<Rect>>,
        // 缓存最近登记使用的绝对触发器锚点。
        popup_anchor_frame: Cell<Option<Rect>>,
        // 累计当前呈现周期内的绝对时间面板脏区。
        popup_damage_rect: Cell<Rect>,
        // 同目录 UIX 生成的唯一静态视觉表。
        #[snapshot(skip)]
        visual: &'static TimePickerVisual,
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
                    if let Some((column, index)) = self.item_at(frame, *pos) {
                        self.active_column.set(column);
                        let value = self.value.get();
                        let next = match column {
                            TimeColumn::Hour => {
                                self.hover_hour.set(index);
                                Time::new(index as u32, value.minute)
                            }
                            TimeColumn::Minute => {
                                self.hover_minute.set(index);
                                Time::new(value.hour, index as u32)
                            }
                        };
                        self.commit_value(next);
                        self.close_popup();
                        return EventResult::Handled;
                    }
                }
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if self.open.get() {
                    if let Some(frame) = self.last_frame.get() {
                        if let Some((column, index)) = self.item_at(frame, *pos) {
                            self.active_column.set(column);
                            let changed = match column {
                                TimeColumn::Hour => self.hover_hour.replace(index) != index,
                                TimeColumn::Minute => self.hover_minute.replace(index) != index,
                            };
                            return if changed {
                                EventResult::Handled
                            } else {
                                EventResult::NotHandled
                            };
                        }
                        if self.reset_highlight_to_value() {
                            return EventResult::Handled;
                        }
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerLeave => {
                if self.open.get() && self.reset_highlight_to_value() {
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
            SystemEvent::Wheel { pos, delta } => {
                if !self.open.get() || !delta.y.is_finite() {
                    return EventResult::NotHandled;
                }
                if let Some(frame) = self.last_frame.get() {
                    // 读取同帧登记、绘制和命中共享的实际时间面板。
                    let popup = self.interaction_popup_rect(frame);
                    if popup.contains(*pos) {
                        let column =
                            if pos.x < popup.x + popup.w * self.visual.layout.column_ratio {
                            TimeColumn::Hour
                        } else {
                            TimeColumn::Minute
                        };
                        self.active_column.set(column);
                        // 使用实际视口高度计算滚动上限。
                        if self.scroll_column(column, delta.y * WHEEL_STEP, popup.h) {
                            return EventResult::Handled;
                        }
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::KeyDown { key, .. } => {
                if self.open.get() {
                    match key {
                        KeyCode::Escape => {
                            self.close_popup();
                            EventResult::Handled
                        }
                        KeyCode::Left => {
                            self.active_column.set(TimeColumn::Hour);
                            // 使用实际时间面板高度显露小时高亮。
                            self.ensure_highlight_visible(
                                // 指定小时列。
                                TimeColumn::Hour,
                                // 读取同帧实际视口高度。
                                self.interaction_viewport_height(),
                            );
                            EventResult::Handled
                        }
                        KeyCode::Right => {
                            self.active_column.set(TimeColumn::Minute);
                            // 使用实际时间面板高度显露分钟高亮。
                            self.ensure_highlight_visible(
                                // 指定分钟列。
                                TimeColumn::Minute,
                                // 读取同帧实际视口高度。
                                self.interaction_viewport_height(),
                            );
                            EventResult::Handled
                        }
                        KeyCode::Up => {
                            // 在实际视口内向上移动当前列高亮。
                            self.move_highlight(-1, self.interaction_viewport_height());
                            EventResult::Handled
                        }
                        KeyCode::Down => {
                            // 在实际视口内向下移动当前列高亮。
                            self.move_highlight(1, self.interaction_viewport_height());
                            EventResult::Handled
                        }
                        KeyCode::Enter => {
                            self.commit_value(Time::new(
                                self.hover_hour.get() as u32,
                                self.hover_minute.get() as u32,
                            ));
                            self.close_popup();
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
            // 解析并缓存当前实际时间面板。
            let popup = self.remember_popup_rect(frame, surface);
            // 将相对时间面板转换为窗口绝对坐标。
            let popup = absolute_time_popup_rect(frame, popup);
            // 触发器与当前时间面板命中框共同收敛到表面内。
            time_surface_rect(frame, popup, surface)
        } else {
            frame
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.capture_bound_value_dependency();
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        // 触发器与双列面板共享同一次主题解析。
        let visual = self.visual.resolve(ctx.tokens());
        let radius = Some(crate::draw::Radius::uniform(visual.radius));

        let val = self.value.get();
        let nominal_height = self.visual.layout.control_height(self.picker_size);
        let scale = if nominal_height > 0.0 {
            (frame.h / nominal_height).clamp(0.0, 1.0)
        } else {
            0.0
        };
        // UIX 字号与触发器几何按当前控件高度缩放。
        let font_size = visual.trigger_font_size * scale;
        let icon_size = visual.icon_size * scale;
        let horizontal_padding = self.visual.layout.horizontal_padding * scale;
        let icon_gap = self.visual.layout.icon_gap * scale;
        let icon_slot_width = self.visual.layout.icon_slot_width * scale;
        let icon_right_inset = self.visual.layout.icon_right_inset * scale;

        ctx.push_clip(frame);
        ctx.fill_rect(frame, visual.background, radius);
        ctx.stroke_rect(
            frame,
            if self.focused {
                visual.primary
            } else {
                visual.border
            },
            if self.focused {
                self.visual.chrome.focus_border_width
            } else {
                self.visual.chrome.border_width
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
                if !self.value_configured.get() {
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
                self.visual.icons.clock,
                icon_frame,
                visual.text_secondary,
                icon_size,
            );
        }
        ctx.pop_clip();

        if self.open.get() {
            // 从绘制上下文读取当前逻辑表面尺寸。
            let surface_size = ctx.logical_surface_size();
            // 将窗口原点与逻辑尺寸组合为当前表面矩形。
            let surface = Rect::new(0.0, 0.0, surface_size.w, surface_size.h);
            // 在绘制前解析并缓存同帧最终时间面板几何。
            let popup = self.remember_popup_rect(frame, surface);
            // 将相对触发器缓存转换为窗口绝对时间面板矩形。
            let popup = absolute_time_popup_rect(frame, popup);
            // 将时间面板绘制限制在当前逻辑表面。
            ctx.push_clip(surface);
            // 将两列内容限制在最终实际视口内。
            ctx.push_clip(popup);
            ctx.fill_rect(popup, visual.popup_background, radius);
            ctx.stroke_rect(
                popup,
                visual.border,
                self.visual.chrome.border_width,
                radius,
            );

            let col_w = popup.w * self.visual.layout.column_ratio;
            let hover_h = self.hover_hour.get();
            let hover_m = self.hover_minute.get();

            let hour_scroll = self.scroll_hour.get();
            let min_scroll = self.scroll_min.get();

            for i in 0..HOUR_COUNT {
                let y = popup.y + i as f32 * self.visual.layout.item_height - hour_scroll;
                if y + self.visual.layout.item_height <= popup.y || y >= popup.y + popup.h {
                    continue;
                }
                let is_hover = i == hover_h;
                if is_hover {
                    ctx.fill_rect(
                        Rect::new(popup.x, y, col_w, self.visual.layout.item_height),
                        visual.primary_background,
                        None,
                    );
                }
                let item_rect = Rect::new(popup.x, y, col_w, self.visual.layout.item_height);
                let text_y = ctx.visual_center_y(item_rect, visual.item_font_size);
                let label = format!("{:02}", i);
                let text_width = ctx.measure_text(&label, visual.item_font_size).w;
                ctx.draw_text(
                    &label,
                    Point::new(item_rect.x + (item_rect.w - text_width) * 0.5, text_y),
                    if is_hover { visual.primary } else { visual.text },
                    visual.item_font_size,
                );
            }

            for i in 0..MINUTE_COUNT {
                let y = popup.y + i as f32 * self.visual.layout.item_height - min_scroll;
                if y + self.visual.layout.item_height <= popup.y || y >= popup.y + popup.h {
                    continue;
                }
                let is_hover = i == hover_m;
                if is_hover {
                    ctx.fill_rect(
                        Rect::new(
                            popup.x + col_w,
                            y,
                            col_w,
                            self.visual.layout.item_height,
                        ),
                        visual.primary_background,
                        None,
                    );
                }
                let item_rect = Rect::new(
                    popup.x + col_w,
                    y,
                    col_w,
                    self.visual.layout.item_height,
                );
                let text_y = ctx.visual_center_y(item_rect, visual.item_font_size);
                let label = format!("{:02}", i);
                let text_width = ctx.measure_text(&label, visual.item_font_size).w;
                ctx.draw_text(
                    &label,
                    Point::new(item_rect.x + (item_rect.w - text_width) * 0.5, text_y),
                    if is_hover { visual.primary } else { visual.text },
                    visual.item_font_size,
                );
            }

            ctx.fill_rect(
                Rect::new(
                    popup.x + col_w - self.visual.chrome.divider_width * 0.5,
                    popup.y,
                    self.visual.chrome.divider_width,
                    popup.h,
                ),
                visual.border,
                None,
            );
            ctx.pop_clip();
            // 结束当前逻辑表面裁剪。
            ctx.pop_clip();
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        // 读取最近登记或绘制记录的当前逻辑表面。
        let surface = self.surface_or_fallback(frame);
        // 合并当前与本次呈现周期历史时间面板脏区。
        let popup = self.damage_popup_rect(frame, surface);
        // 将输入框和时间面板脏区限制在当前表面内。
        time_surface_rect(frame, popup, surface)
    }

    overlay_entry => (&self, id: WidgetId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        self.open.get().then(|| {
            // 读取最近记录的表面或首次有限回退。
            let surface = self.surface_or_fallback(frame);
            // 解析并缓存当前实际时间面板。
            let popup = self.remember_popup_rect(frame, surface);
            // 将相对时间面板转换为窗口绝对坐标。
            let bounds = absolute_time_popup_rect(frame, popup);
            // 创建只覆盖实际时间面板的浮层登记。
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Popover)
                // 登记最终时间面板而不并入触发器。
                .bounds(bounds)
                // 使用 UIX 声明的时间面板层级。
                .z_index(self.visual.chrome.overlay_z)
        })
    }

    // 使用组件树提供的同帧表面创建时间面板登记。
    overlay_entry_for_surface => (&self, id: WidgetId, frame: Rect, surface: Rect) -> Option<crate::ui::OverlayEntry> {
        // 仅在打开状态刷新表面与实际时间面板缓存。
        if self.open.get() {
            // 缓存组件树提供的真实表面。
            self.remember_popup_rect(frame, surface);
        }
        // 复用统一的时间面板登记逻辑。
        self.overlay_entry(id, frame)
    }
}

impl TimePicker {
    /// 创建使用当前区域占位文本、默认尺寸且未展开的时间选择器。
    pub fn new() -> Self {
        let config = crate::ui::widget_runtime::config::use_config();
        Self {
            value: Cell::new(Time::default()),
            value_configured: Cell::new(false),
            value_binding: None,
            placeholder: crate::ui::widget_runtime::locale::use_locale()
                .placeholder
                .to_owned(),
            open: Cell::new(false),
            focused: false,
            hover_hour: Cell::new(0),
            hover_minute: Cell::new(0),
            scroll_hour: Cell::new(0.0),
            scroll_min: Cell::new(0.0),
            active_column: Cell::new(TimeColumn::Hour),
            picker_size: config.size,
            last_frame: Cell::new(None),
            pending_change: Cell::new(None),
            // 初始尚无最终时间面板。
            popup_rect: Cell::new(Rect::zero()),
            // 初始尚未记录逻辑表面。
            surface_rect: Cell::new(None),
            // 初始尚未记录绝对触发器锚点。
            popup_anchor_frame: Cell::new(None),
            // 初始呈现周期没有历史时间面板脏区。
            popup_damage_rect: Cell::new(Rect::zero()),
            // 全部实例共享 UIX 编译生成的视觉表。
            visual: TIME_PICKER_VISUAL_REF,
        }
    }

    /// 将时间绑定到外部 `State<Time>`。
    pub fn value(mut self, state: &State<Time>) -> Self {
        self.value_binding = Some(state.clone());
        self.value.set(state.get());
        self.value_configured.set(true);
        self
    }

    /// 设置非受控时间选择器的初始值。
    pub fn default_value(mut self, value: Time) -> Self {
        self.value_binding = None;
        self.value.set(value);
        self.value_configured.set(true);
        self
    }

    /// 设置尚未配置时间值时显示的占位文本。
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// 设置触发器采用的控件尺寸规格。
    pub fn size(mut self, size: ControlSize) -> Self {
        self.picker_size = size;
        self
    }

    /// 返回组件当前缓存值；controlled 用法应以绑定的 `State` 为真值来源。
    pub fn current_value(&self) -> Time {
        self.value.get()
    }

    /// 返回时间选择面板当前是否处于逻辑展开状态。
    pub fn is_open(&self) -> bool {
        self.open.get()
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(
            self.visual.layout.natural_width,
            self.visual.layout.control_height(self.picker_size),
        )
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::TimePicker {
            placeholder: self.placeholder.clone(),
            value: self
                .value_configured
                .get()
                .then(|| self.value.get().format()),
            open: self.open.get(),
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        if !std::ptr::eq(self.visual, next.visual) {
            self.visual = next.visual;
            self.reset_popup_presentation();
        }
        let controlled_value = next.value_binding.as_ref().map(|_| next.value.get());
        self.value_binding = next.value_binding;
        if let Some(value) = controlled_value {
            self.value.set(value);
            self.value_configured.set(true);
            if self.open.get() {
                // 使用当前实际视口同步受控值的高亮和滚动。
                self.sync_highlight_to_value(true, self.interaction_viewport_height());
            }
        }
        self.placeholder = next.placeholder;
        self.picker_size = next.picker_size;
    }

    fn sync_bound_value(&self) {
        if let Some(state) = self.value_binding.as_ref() {
            let value = state.get();
            let changed = self.value.replace(value) != value;
            self.value_configured.set(true);
            if changed && self.open.get() {
                // 使用当前实际视口同步外部值的高亮和滚动。
                self.sync_highlight_to_value(true, self.interaction_viewport_height());
            }
        }
    }

    fn capture_bound_value_dependency(&self) {
        if let Some(state) = self.value_binding.as_ref() {
            let _ = state.get();
        }
    }

    fn commit_value(&self, value: Time) {
        if self.value_configured.get() && self.value.get() == value {
            return;
        }
        self.value.set(value);
        self.value_configured.set(true);
        if let Some(state) = self.value_binding.as_ref() {
            if state.get() != value {
                state.set(value);
            }
        }
        self.pending_change.set(Some(value));
    }

    fn open_popup(&self) {
        // 仅在关闭到开启的边沿开始新的面板呈现周期。
        if !self.open.get() {
            // 清除上一次呈现留下的表面与脏区缓存。
            self.reset_popup_presentation();
        }
        self.sync_bound_value();
        self.active_column.set(TimeColumn::Hour);
        // 首次正式登记前按自然视口同步高亮和滚动。
        self.sync_highlight_to_value(true, self.visual.layout.popup_height);
        self.open.set(true);
    }

    fn close_popup(&self) {
        self.open.set(false);
    }

    fn item_at(&self, frame: Rect, pos: Point) -> Option<(TimeColumn, usize)> {
        // 读取同帧登记、绘制和滚动共享的实际时间面板。
        let popup = self.interaction_popup_rect(frame);
        if !popup.contains(pos) {
            return None;
        }
        let column = if pos.x < popup.x + popup.w * self.visual.layout.column_ratio {
            TimeColumn::Hour
        } else {
            TimeColumn::Minute
        };
        let scroll = self.scroll_offset(column);
        let index = ((pos.y - popup.y + scroll) / self.visual.layout.item_height).floor();
        if !index.is_finite() || index < 0.0 {
            return None;
        }
        let index = index as usize;
        (index < Self::row_count(column)).then_some((column, index))
    }

    fn row_count(column: TimeColumn) -> usize {
        match column {
            TimeColumn::Hour => HOUR_COUNT,
            TimeColumn::Minute => MINUTE_COUNT,
        }
    }

    // 按指定列与实际视口返回最大滚动偏移。
    fn max_scroll(&self, column: TimeColumn, viewport_height: f32) -> f32 {
        // 归一化实际视口高度以避免非有限滚动范围。
        let viewport_height = finite_viewport_height(viewport_height);
        // 使用固定行高内容与实际视口计算最大滚动。
        (Self::row_count(column) as f32 * self.visual.layout.item_height - viewport_height).max(0.0)
    }

    // 按实际视口返回目标选项的居中滚动偏移。
    fn centered_scroll(&self, column: TimeColumn, index: usize, viewport_height: f32) -> f32 {
        // 归一化实际视口高度。
        let viewport_height = finite_viewport_height(viewport_height);
        // 计算让目标行尽量居中的滚动位置。
        let centered = index as f32 * self.visual.layout.item_height
            - (viewport_height - self.visual.layout.item_height) * 0.5;
        // 将居中位置限制在实际视口的合法范围内。
        centered.clamp(0.0, self.max_scroll(column, viewport_height))
    }

    fn scroll_offset(&self, column: TimeColumn) -> f32 {
        match column {
            TimeColumn::Hour => self.scroll_hour.get(),
            TimeColumn::Minute => self.scroll_min.get(),
        }
    }

    fn set_scroll_offset(&self, column: TimeColumn, offset: f32) {
        match column {
            TimeColumn::Hour => self.scroll_hour.set(offset),
            TimeColumn::Minute => self.scroll_min.set(offset),
        }
    }

    // 在实际视口范围内滚动指定时间列。
    fn scroll_column(&self, column: TimeColumn, delta: f32, viewport_height: f32) -> bool {
        if !delta.is_finite() {
            return false;
        }
        let current = self.scroll_offset(column);
        // 使用实际视口高度限制下一滚动位置。
        let next = (current + delta).clamp(0.0, self.max_scroll(column, viewport_height));
        if (next - current).abs() <= f32::EPSILON {
            false
        } else {
            self.set_scroll_offset(column, next);
            true
        }
    }

    // 保证指定列的当前高亮与实际视口相交。
    fn ensure_highlight_visible(&self, column: TimeColumn, viewport_height: f32) {
        // 归一化实际视口高度。
        let viewport_height = finite_viewport_height(viewport_height);
        let index = match column {
            TimeColumn::Hour => self.hover_hour.get(),
            TimeColumn::Minute => self.hover_minute.get(),
        };
        let current = self.scroll_offset(column);
        let row_top = index as f32 * self.visual.layout.item_height;
        let row_bottom = row_top + self.visual.layout.item_height;
        let next = if row_top < current {
            row_top
        } else if row_bottom > current + viewport_height {
            row_bottom - viewport_height
        } else {
            current
        };
        // 将显露位置限制在实际视口的合法滚动范围内。
        self.set_scroll_offset(
            // 指定需要调整的时间列。
            column,
            // 使用实际视口计算最大滚动。
            next.clamp(0.0, self.max_scroll(column, viewport_height)),
        );
    }

    // 在指定实际视口内移动当前列高亮。
    fn move_highlight(&self, delta: i32, viewport_height: f32) {
        let column = self.active_column.get();
        let count = Self::row_count(column) as i32;
        let current = match column {
            TimeColumn::Hour => self.hover_hour.get(),
            TimeColumn::Minute => self.hover_minute.get(),
        } as i32;
        let next = (current + delta).clamp(0, count - 1) as usize;
        match column {
            TimeColumn::Hour => self.hover_hour.set(next),
            TimeColumn::Minute => self.hover_minute.set(next),
        }
        // 使用实际视口保证移动后的高亮可见。
        self.ensure_highlight_visible(column, viewport_height);
    }

    fn reset_highlight_to_value(&self) -> bool {
        let value = self.value.get();
        let hour_changed = self.hover_hour.replace(value.hour as usize) != value.hour as usize;
        let minute_changed =
            self.hover_minute.replace(value.minute as usize) != value.minute as usize;
        hour_changed || minute_changed
    }

    // 将高亮与滚动同步到当前值和实际视口。
    fn sync_highlight_to_value(&self, center: bool, viewport_height: f32) {
        let value = self.value.get();
        self.hover_hour.set(value.hour as usize);
        self.hover_minute.set(value.minute as usize);
        if center {
            // 按实际视口居中小时选项。
            self.scroll_hour
                // 保存小时列居中滚动。
                .set(self.centered_scroll(
                    // 指定小时列。
                    TimeColumn::Hour,
                    // 使用当前小时索引。
                    value.hour as usize,
                    // 使用实际视口高度。
                    viewport_height,
                ));
            // 按实际视口居中分钟选项。
            self.scroll_min.set(self.centered_scroll(
                // 指定分钟列。
                TimeColumn::Minute,
                // 使用当前分钟索引。
                value.minute as usize,
                // 使用实际视口高度。
                viewport_height,
            ));
        }
    }
}

// 将任意视口高度收敛为有限非负值。
fn finite_viewport_height(value: f32) -> f32 {
    // 只保留有限输入。
    if value.is_finite() {
        // 负高度收敛为零。
        value.max(0.0)
    // 处理非有限输入。
    } else {
        // 非有限高度回退为零。
        0.0
    }
}

// 把 TimePicker 的 Rust 时间状态内核与 UIX 静态视觉组合为单一组件节点。
fn build_time_picker_view(mut kernel: TimePicker, visual: &'static TimePickerVisual) -> ViewNode {
    kernel.visual = visual;
    kernel.reset_popup_presentation();
    ViewNode::leaf(kernel)
}

impl View for TimePicker {
    fn build(self) -> ViewNode {
        build_time_picker_view(self, TIME_PICKER_VISUAL_REF)
    }
}

impl Default for TimePicker {
    fn default() -> Self {
        Self::new()
    }
}

// 在测试构建中加载时间面板表面几何契约。
