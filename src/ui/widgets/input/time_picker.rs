//! TimePicker 时间选择器 — 选择时:分。
//!
//! 弹出面板含小时/分钟滚动选择，支持 hover 高亮、键盘导航。

use std::cell::Cell;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Color;
use crate::native::traits::input::ControlSize;
use crate::ui::state::State;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent,
    WidgetTree,
};

const POPUP_GAP: f32 = 2.0;
const POPUP_HEIGHT: f32 = 200.0;

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
                    self.open.set(true);
                    let val = self.value.get();
                    self.hover_hour.set(val.hour as usize);
                    self.hover_minute.set(val.minute as usize);
                    return EventResult::Handled;
                }

                if let Some(frame) = self.last_frame.get() {
                    let popup_y = frame.y + frame.h + POPUP_GAP;
                    let rel_x = pos.x - frame.x;
                    let rel_y = pos.y - popup_y;

                    if (0.0..POPUP_HEIGHT).contains(&rel_y) {
                        let col_w = frame.w * 0.5;
                        if rel_x < col_w {
                            let item_h = 32.0;
                            let idx = ((rel_y + self.scroll_hour.get()) / item_h) as usize;
                            if idx < 24 {
                                self.hover_hour.set(idx);
                                let val = self.value.get();
                                let new_val = Time::new(idx as u32, val.minute);
                                self.commit_value(new_val);
                                self.open.set(false);
                                return EventResult::Handled;
                            }
                        } else {
                            let item_h = 32.0;
                            let idx = ((rel_y + self.scroll_min.get()) / item_h) as usize;
                            if idx < 12 {
                                let minute = idx * 5;
                                self.hover_minute.set(idx);
                                let val = self.value.get();
                                let new_val = Time::new(val.hour, minute as u32);
                                self.commit_value(new_val);
                                self.open.set(false);
                                return EventResult::Handled;
                            }
                        }
                    }
                }
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if self.open.get() {
                    if let Some(frame) = self.last_frame.get() {
                        let popup_y = frame.y + frame.h + POPUP_GAP;
                        let rel_x = pos.x - frame.x;
                        let rel_y = pos.y - popup_y;

                        if (0.0..POPUP_HEIGHT).contains(&rel_y) {
                            let col_w = frame.w * 0.5;
                            let item_h = 32.0;
                            if rel_x < col_w {
                                let idx = ((rel_y + self.scroll_hour.get()) / item_h) as usize;
                                if idx < 24 {
                                    self.hover_hour.set(idx);
                                }
                            } else {
                                let idx = ((rel_y + self.scroll_min.get()) / item_h) as usize;
                                if idx < 12 {
                                    self.hover_minute.set(idx);
                                }
                            }
                            return EventResult::Handled;
                        }
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerLeave => EventResult::NotHandled,
            SystemEvent::FocusOut => { self.focused = false; self.open.set(false); EventResult::Handled }
            SystemEvent::KeyDown { key, .. } => {
                if self.open.get() {
                    match key {
                        KeyCode::Escape => { self.open.set(false); }
                        KeyCode::Up => {
                            let hrs = self.scroll_hour.get();
                            self.scroll_hour.set((hrs - 32.0).max(0.0));
                        }
                        KeyCode::Down => {
                            let hrs = self.scroll_hour.get();
                            self.scroll_hour.set((hrs + 32.0).min((24 * 32) as f32 - 200.0).max(0.0));
                        }
                        _ => {}
                    }
                } else {
                    if *key == KeyCode::Space || *key == KeyCode::Enter {
                        self.open.set(true);
                        let val = self.value.get();
                        self.hover_hour.set(val.hour as usize);
                        self.hover_minute.set(val.minute as usize);
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
        ctx.fill_rect(frame, Color::white(), radius);
        ctx.stroke_rect(frame, if self.focused { primary } else { border_color },
            if self.focused { 2.0 } else { 1.0 }, radius);

        let input_text_y = ctx.visual_center_y(frame, 14.0);
        if !self.value_configured.get() {
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
            "clock",
            Rect::new(frame.x + frame.w - 28.0, frame.y, 24.0, frame.h),
            text_secondary,
            14.0,
        );

        if self.open.get() {
            let popup = Rect::new(
                frame.x,
                frame.y + frame.h + POPUP_GAP,
                frame.w,
                POPUP_HEIGHT,
            );
            ctx.fill_rect(popup, bg_elevated, radius);
            ctx.stroke_rect(popup, border_color, 1.0, radius);

            let col_w = popup.w * 0.5;
            let item_h = 32.0;
            let hover_h = self.hover_hour.get();
            let hover_m = self.hover_minute.get();

            let hour_scroll = self.scroll_hour.get();
            let min_scroll = self.scroll_min.get();

            for i in 0..24 {
                let y = popup.y + i as f32 * item_h - hour_scroll;
                if y + item_h <= popup.y || y >= popup.y + popup.h { continue; }
                let is_hover = i == hover_h;
                if is_hover {
                    ctx.fill_rect(Rect::new(popup.x, y, col_w, item_h), primary_bg, None);
                }
                let item_rect = Rect::new(popup.x, y, col_w, item_h);
                let text_y = ctx.visual_center_y(item_rect, 14.0);
                ctx.draw_text(&format!("{:02}", i),
                    Point::new(popup.x + 16.0, text_y),
                    if is_hover { primary } else { text_color }, 14.0);
            }

            for i in 0..12 {
                let y = popup.y + i as f32 * item_h - min_scroll;
                if y + item_h <= popup.y || y >= popup.y + popup.h { continue; }
                let minute = i * 5;
                let is_hover = i == hover_m;
                if is_hover {
                    ctx.fill_rect(Rect::new(popup.x + col_w, y, col_w, item_h), primary_bg, None);
                }
                let item_rect = Rect::new(popup.x + col_w, y, col_w, item_h);
                let text_y = ctx.visual_center_y(item_rect, 14.0);
                ctx.draw_text(&format!("{:02}", minute),
                    Point::new(popup.x + col_w + 16.0, text_y),
                    if is_hover { primary } else { text_color }, 14.0);
            }
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        picker_bounds(frame)
    }
}

fn picker_bounds(frame: Rect) -> Rect {
    frame.union(&Rect::new(
        frame.x,
        frame.y + frame.h + POPUP_GAP,
        frame.w,
        POPUP_HEIGHT,
    ))
}

impl TimePicker {
    pub fn new() -> Self {
        let config = crate::ui::config::use_config();
        Self {
            value: Cell::new(Time::default()),
            value_configured: Cell::new(false),
            value_binding: None,
            placeholder: crate::ui::locale::use_locale().placeholder.to_owned(),
            open: Cell::new(false),
            focused: false,
            hover_hour: Cell::new(0),
            hover_minute: Cell::new(0),
            scroll_hour: Cell::new(0.0),
            scroll_min: Cell::new(0.0),
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
        Size::new(120.0, crate::ui::config::control_height(self.picker_size))
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::TimePicker {
            placeholder: self.placeholder.clone(),
            value: self
                .value_configured
                .get()
                .then(|| self.value.get().format()),
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let controlled_value = next.value_binding.as_ref().map(|_| next.value.get());
        self.value_binding = next.value_binding;
        if let Some(value) = controlled_value {
            self.value.set(value);
            self.value_configured.set(true);
        }
        self.placeholder = next.placeholder;
        self.picker_size = next.picker_size;
    }

    fn sync_bound_value(&self) {
        if let Some(state) = self.value_binding.as_ref() {
            self.value.set(state.get());
            self.value_configured.set(true);
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
}

impl Default for TimePicker {
    fn default() -> Self {
        Self::new()
    }
}
