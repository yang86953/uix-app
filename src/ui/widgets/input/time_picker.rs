//! TimePicker 时间选择器 — 选择时:分。
//!
//! 弹出面板含小时/分钟滚动选择，支持 hover 高亮、键盘导航。

use std::cell::Cell;

use crate::define_widget;
use crate::draw::painting::RenderContext;
use crate::ui::{EventResult, KeyCode, WidgetEvent, WidgetTree};
use crate::draw::{Color, traits::GraphicsEngine};
use crate::native::{Point, Rect, Size};

/// 时间结构
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TimeValue {
    pub hour: u32,
    pub minute: u32,
}

impl TimeValue {
    pub fn new(hour: u32, minute: u32) -> Self {
        Self {
            hour: hour.min(23),
            minute: minute.min(59),
        }
    }
    pub fn format(&self) -> String {
        format!("{:02}:{:02}", self.hour, self.minute)
    }
}

define_widget! {
    /// TimePicker — 时间选择器。
    pub struct TimePicker {
        value: Cell<TimeValue>,
        placeholder: String,
        open: Cell<bool>,
        focused: bool,
        hover_hour: Cell<usize>,
        hover_minute: Cell<usize>,
        /// 小时滚动偏移（行号）
        scroll_hour: Cell<f32>,
        scroll_min: Cell<f32>,
        last_frame: Cell<Option<Rect>>,
        on_change: Option<Box<dyn FnMut(TimeValue) + 'static>>,
    }


    tab_index => (&self) -> i32 { 1 }
    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        Size::new(120.0, 32.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                self.focused = true;
                if !self.open.get() {
                    self.open.set(true);
                    let val = self.value.get();
                    self.hover_hour.set(val.hour as usize);
                    self.hover_minute.set(val.minute as usize);
                    return EventResult::Handled;
                }

                if let Some(frame) = self.last_frame.get() {
                    let popup_y = frame.y + frame.h + 2.0;
                    let rel_x = pos.x - frame.x;
                    let rel_y = pos.y - popup_y;

                    if (0.0..200.0).contains(&rel_y) {
                        let col_w = frame.w * 0.5;
                        if rel_x < col_w {
                            let item_h = 32.0;
                            let idx = ((rel_y + self.scroll_hour.get()) / item_h) as usize;
                            if idx < 24 {
                                self.hover_hour.set(idx);
                                let val = self.value.get();
                                let new_val = TimeValue::new(idx as u32, val.minute);
                                self.value.set(new_val);
                                self.open.set(false);
                                if let Some(ref mut cb) = self.on_change { cb(new_val); }
                                return EventResult::Handled;
                            }
                        } else {
                            let item_h = 32.0;
                            let idx = ((rel_y + self.scroll_min.get()) / item_h) as usize;
                            if idx < 12 {
                                let minute = idx * 5;
                                self.hover_minute.set(idx);
                                let val = self.value.get();
                                let new_val = TimeValue::new(val.hour, minute as u32);
                                self.value.set(new_val);
                                self.open.set(false);
                                if let Some(ref mut cb) = self.on_change { cb(new_val); }
                                return EventResult::Handled;
                            }
                        }
                    }
                }
                EventResult::Handled
            }
            WidgetEvent::MouseMove { pos, .. } => {
                if self.open.get() {
                    if let Some(frame) = self.last_frame.get() {
                        let popup_y = frame.y + frame.h + 2.0;
                        let rel_x = pos.x - frame.x;
                        let rel_y = pos.y - popup_y;

                        if (0.0..200.0).contains(&rel_y) {
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
            WidgetEvent::HoverLeave => EventResult::NotHandled,
            WidgetEvent::FocusOut => { self.focused = false; self.open.set(false); EventResult::Handled }
            WidgetEvent::KeyDown { key, .. } => {
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

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        self.last_frame.set(Some(frame));
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
        let is_default = val == TimeValue::default();
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

        let icon_y = ctx.visual_center_y(frame, 12.0);
        ctx.draw_text("🕐", Point::new(frame.x + frame.w - 24.0, icon_y),
            text_secondary, 12.0);

        if self.open.get() {
            let popup = Rect::new(frame.x, frame.y + frame.h + 2.0, frame.w, 200.0);
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
}

impl TimePicker {
    pub fn new(placeholder: impl Into<String>) -> Self {
        Self {
            value: Cell::new(TimeValue::new(0, 0)),
            placeholder: placeholder.into(),
            open: Cell::new(false),
            focused: false,
            hover_hour: Cell::new(0),
            hover_minute: Cell::new(0),
            scroll_hour: Cell::new(0.0),
            scroll_min: Cell::new(0.0),
            last_frame: Cell::new(None),
            on_change: None,
        }
    }

    pub fn value(self, v: TimeValue) -> Self {
        self.value.set(v);
        self
    }
    pub fn selected(&self) -> TimeValue {
        self.value.get()
    }
    pub fn set_value(&mut self, v: TimeValue) {
        self.value.set(v);
    }
    pub fn on_change<F: FnMut(TimeValue) + 'static>(mut self, f: F) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }
}
