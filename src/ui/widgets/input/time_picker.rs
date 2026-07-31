//! TimePicker 时间选择器 — 选择时:分。
//!
//! 弹出面板含小时/分钟滚动选择，支持 hover 高亮、键盘导航。

use std::cell::Cell;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::native::windowing::input::ControlSize;
use crate::ui::component::paint_context::PaintContext;
use crate::ui::reactive::state::State;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent,
    WidgetTree,
};

const POPUP_GAP: f32 = 2.0;
const POPUP_HEIGHT: f32 = 200.0;
const POPUP_MIN_WIDTH: f32 = 120.0;
const ITEM_HEIGHT: f32 = 32.0;
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
    pub hour: u32,
    pub minute: u32,
}

impl Time {
    pub fn new(hour: u32, minute: u32) -> Self {
        Self {
            hour: hour.min(23),
            minute: minute.min(59),
        }
    }
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

component! {
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
                    let popup = time_popup_rect(frame);
                    if popup.contains(*pos) {
                        let column = if pos.x < popup.x + popup.w * 0.5 {
                            TimeColumn::Hour
                        } else {
                            TimeColumn::Minute
                        };
                        self.active_column.set(column);
                        if self.scroll_column(column, delta.y * WHEEL_STEP) {
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
                            self.ensure_highlight_visible(TimeColumn::Hour);
                            EventResult::Handled
                        }
                        KeyCode::Right => {
                            self.active_column.set(TimeColumn::Minute);
                            self.ensure_highlight_visible(TimeColumn::Minute);
                            EventResult::Handled
                        }
                        KeyCode::Up => {
                            self.move_highlight(-1);
                            EventResult::Handled
                        }
                        KeyCode::Down => {
                            self.move_highlight(1);
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
        self.capture_bound_value_dependency();
        self.last_frame
            .set(Some(Rect::new(0.0, 0.0, frame.w, frame.h)));
        let primary = ctx.tokens().color_primary();
        let border_color = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let text_tertiary = ctx.tokens().color_text_tertiary();
        let bg_elevated = ctx.tokens().color_bg_elevated();
        let primary_bg = ctx.tokens().color_primary_bg();
        let border_radius_sm = ctx.tokens().border_radius_sm();
        let radius = Some(crate::draw::Radius::uniform(border_radius_sm));

        let val = self.value.get();
        let nominal_height = crate::ui::component::config::control_height(self.picker_size);
        let scale = if nominal_height > 0.0 {
            (frame.h / nominal_height).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let font_size = 14.0 * scale;
        let horizontal_padding = 12.0 * scale;
        let icon_gap = 4.0 * scale;
        let icon_slot_width = 24.0 * scale;
        let icon_right_inset = 4.0 * scale;

        ctx.push_clip(frame);
        ctx.fill_rect(frame, ctx.tokens().color_bg_container(), radius);
        ctx.stroke_rect(frame, if self.focused { primary } else { border_color },
            if self.focused { 2.0 } else { 1.0 }, radius);

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
                        text_tertiary,
                        font_size,
                    );
                } else {
                    let formatted = val.format();
                    ctx.draw_text(
                        &formatted,
                        Point::new(text_left, input_text_y),
                        text_color,
                        font_size,
                    );
                }
                ctx.pop_clip();
            }

            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                "clock",
                icon_frame,
                text_secondary,
                font_size,
            );
        }
        ctx.pop_clip();

        if self.open.get() {
            let popup = time_popup_rect(frame);
            ctx.push_clip(popup);
            ctx.fill_rect(popup, bg_elevated, radius);
            ctx.stroke_rect(popup, border_color, 1.0, radius);

            let col_w = popup.w * 0.5;
            let hover_h = self.hover_hour.get();
            let hover_m = self.hover_minute.get();

            let hour_scroll = self.scroll_hour.get();
            let min_scroll = self.scroll_min.get();

            for i in 0..HOUR_COUNT {
                let y = popup.y + i as f32 * ITEM_HEIGHT - hour_scroll;
                if y + ITEM_HEIGHT <= popup.y || y >= popup.y + popup.h { continue; }
                let is_hover = i == hover_h;
                if is_hover {
                    ctx.fill_rect(Rect::new(popup.x, y, col_w, ITEM_HEIGHT), primary_bg, None);
                }
                let item_rect = Rect::new(popup.x, y, col_w, ITEM_HEIGHT);
                let text_y = ctx.visual_center_y(item_rect, 14.0);
                let label = format!("{:02}", i);
                let text_width = ctx.measure_text(&label, 14.0).w;
                ctx.draw_text(
                    &label,
                    Point::new(item_rect.x + (item_rect.w - text_width) * 0.5, text_y),
                    if is_hover { primary } else { text_color },
                    14.0,
                );
            }

            for i in 0..MINUTE_COUNT {
                let y = popup.y + i as f32 * ITEM_HEIGHT - min_scroll;
                if y + ITEM_HEIGHT <= popup.y || y >= popup.y + popup.h { continue; }
                let is_hover = i == hover_m;
                if is_hover {
                    ctx.fill_rect(
                        Rect::new(popup.x + col_w, y, col_w, ITEM_HEIGHT),
                        primary_bg,
                        None,
                    );
                }
                let item_rect = Rect::new(popup.x + col_w, y, col_w, ITEM_HEIGHT);
                let text_y = ctx.visual_center_y(item_rect, 14.0);
                let label = format!("{:02}", i);
                let text_width = ctx.measure_text(&label, 14.0).w;
                ctx.draw_text(
                    &label,
                    Point::new(item_rect.x + (item_rect.w - text_width) * 0.5, text_y),
                    if is_hover { primary } else { text_color },
                    14.0,
                );
            }

            ctx.fill_rect(
                Rect::new(popup.x + col_w - 0.5, popup.y, 1.0, popup.h),
                border_color,
                None,
            );
            ctx.pop_clip();
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        picker_bounds(frame)
    }

    overlay_entry => (&self, id: ComponentId, frame: Rect) -> Option<crate::ui::OverlayEntry> {
        self.open.get().then(|| {
            crate::ui::OverlayEntry::new(id, crate::ui::OverlayKind::Popover)
                .bounds(picker_bounds(frame))
                .z_index(900)
        })
    }
}

fn picker_bounds(frame: Rect) -> Rect {
    frame.union(&time_popup_rect(frame))
}

fn time_popup_rect(frame: Rect) -> Rect {
    Rect::new(
        frame.x,
        frame.y + frame.h + POPUP_GAP,
        frame.w.max(POPUP_MIN_WIDTH),
        POPUP_HEIGHT,
    )
}

impl TimePicker {
    pub fn new() -> Self {
        let config = crate::ui::component::config::use_config();
        Self {
            value: Cell::new(Time::default()),
            value_configured: Cell::new(false),
            value_binding: None,
            placeholder: crate::ui::component::locale::use_locale()
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

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.picker_size = size;
        self
    }

    /// 返回组件当前缓存值；controlled 用法应以绑定的 `State` 为真值来源。
    pub fn current_value(&self) -> Time {
        self.value.get()
    }

    pub fn is_open(&self) -> bool {
        self.open.get()
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(
            120.0,
            crate::ui::component::config::control_height(self.picker_size),
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
        let controlled_value = next.value_binding.as_ref().map(|_| next.value.get());
        self.value_binding = next.value_binding;
        if let Some(value) = controlled_value {
            self.value.set(value);
            self.value_configured.set(true);
            if self.open.get() {
                self.sync_highlight_to_value(true);
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
                self.sync_highlight_to_value(true);
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
        self.sync_bound_value();
        self.active_column.set(TimeColumn::Hour);
        self.sync_highlight_to_value(true);
        self.open.set(true);
    }

    fn close_popup(&self) {
        self.open.set(false);
    }

    fn item_at(&self, frame: Rect, pos: Point) -> Option<(TimeColumn, usize)> {
        let popup = time_popup_rect(frame);
        if !popup.contains(pos) {
            return None;
        }
        let column = if pos.x < popup.x + popup.w * 0.5 {
            TimeColumn::Hour
        } else {
            TimeColumn::Minute
        };
        let scroll = self.scroll_offset(column);
        let index = ((pos.y - popup.y + scroll) / ITEM_HEIGHT).floor();
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

    fn max_scroll(column: TimeColumn) -> f32 {
        (Self::row_count(column) as f32 * ITEM_HEIGHT - POPUP_HEIGHT).max(0.0)
    }

    fn centered_scroll(column: TimeColumn, index: usize) -> f32 {
        let centered = index as f32 * ITEM_HEIGHT - (POPUP_HEIGHT - ITEM_HEIGHT) * 0.5;
        centered.clamp(0.0, Self::max_scroll(column))
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

    fn scroll_column(&self, column: TimeColumn, delta: f32) -> bool {
        if !delta.is_finite() {
            return false;
        }
        let current = self.scroll_offset(column);
        let next = (current + delta).clamp(0.0, Self::max_scroll(column));
        if (next - current).abs() <= f32::EPSILON {
            false
        } else {
            self.set_scroll_offset(column, next);
            true
        }
    }

    fn ensure_highlight_visible(&self, column: TimeColumn) {
        let index = match column {
            TimeColumn::Hour => self.hover_hour.get(),
            TimeColumn::Minute => self.hover_minute.get(),
        };
        let current = self.scroll_offset(column);
        let row_top = index as f32 * ITEM_HEIGHT;
        let row_bottom = row_top + ITEM_HEIGHT;
        let next = if row_top < current {
            row_top
        } else if row_bottom > current + POPUP_HEIGHT {
            row_bottom - POPUP_HEIGHT
        } else {
            current
        };
        self.set_scroll_offset(column, next.clamp(0.0, Self::max_scroll(column)));
    }

    fn move_highlight(&self, delta: i32) {
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
        self.ensure_highlight_visible(column);
    }

    fn reset_highlight_to_value(&self) -> bool {
        let value = self.value.get();
        let hour_changed = self.hover_hour.replace(value.hour as usize) != value.hour as usize;
        let minute_changed =
            self.hover_minute.replace(value.minute as usize) != value.minute as usize;
        hour_changed || minute_changed
    }

    fn sync_highlight_to_value(&self, center: bool) {
        let value = self.value.get();
        self.hover_hour.set(value.hour as usize);
        self.hover_minute.set(value.minute as usize);
        if center {
            self.scroll_hour
                .set(Self::centered_scroll(TimeColumn::Hour, value.hour as usize));
            self.scroll_min.set(Self::centered_scroll(
                TimeColumn::Minute,
                value.minute as usize,
            ));
        }
    }
}

impl Default for TimePicker {
    fn default() -> Self {
        Self::new()
    }
}
