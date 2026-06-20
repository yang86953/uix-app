//! TimePicker 时间选择器 — 选择时:分。
//!
//! 弹出面板含小时/分钟滚动选择。

use std::cell::Cell;

use crate::base::{Point, Rect, Size};
use crate::define_widget;
use crate::graphics::{Color, GraphicsEngine};
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, KeyCode, WidgetEvent, WidgetTree};

/// 时间结构
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TimeValue {
    pub hour: u32,
    pub minute: u32,
}

impl TimeValue {
    pub fn new(hour: u32, minute: u32) -> Self {
        Self { hour: hour.min(23), minute: minute.min(59) }
    }
    pub fn format(&self) -> String {
        format!("{:02}:{:02}", self.hour, self.minute)
    }
}

define_widget! {
    /// TimePicker — 时间选择器。
    pub struct TimePicker {
        /// 当前选中时间
        value: TimeValue,
        /// 占位文本
        placeholder: String,
        /// 是否展开弹出
        open: bool,
        /// 焦点
        focused: bool,
        /// 悬停小时索引
        hover_hour: usize,
        /// 悬停分钟索引
        hover_minute: usize,
        /// 当前 frame
        last_frame: Cell<Option<Rect>>,
    }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        Size::new(120.0, 32.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                self.focused = true;
                if !self.open {
                    self.open = true;
                    self.hover_hour = self.value.hour as usize;
                    self.hover_minute = self.value.minute as usize;
                    return EventResult::Handled;
                }
                // 点击弹出层中的选项
                if let Some(frame) = self.last_frame.get() {
                    let popup_y = frame.y + frame.h + 2.0;
                    let rel_y = pos.y - popup_y;
                    if rel_y >= 0.0 && rel_y < 200.0 {
                        let col = if pos.x - frame.x < frame.w * 0.5 { 0 } else { 1 };
                        let item_idx = (rel_y / 32.0) as usize;
                        if col == 0 && item_idx < 24 {
                            self.hover_hour = item_idx;
                            self.value = TimeValue::new(item_idx as u32, self.value.minute);
                        } else if col == 1 && item_idx < 12 {
                            let minute = item_idx as u32 * 5;
                            self.hover_minute = item_idx;
                            self.value = TimeValue::new(self.value.hour, minute);
                        }
                        // 点击确认
                        self.open = false;
                        return EventResult::Handled;
                    }
                }
                EventResult::Handled
            }
            WidgetEvent::FocusOut => { self.focused = false; self.open = false; EventResult::Handled }
            WidgetEvent::KeyDown { key, .. } => {
                if *key == KeyCode::Escape { self.open = false; EventResult::Handled }
                else { EventResult::NotHandled }
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
        let border_radius_sm = ctx.tokens().border_radius_sm();
        let radius = Some(crate::graphics::Radius::uniform(border_radius_sm));

        // 输入框
        let is_default = self.value == TimeValue::default();
        ctx.fill_rect(frame, Color::white(), radius);
        ctx.stroke_rect(frame, if self.focused { primary } else { border_color },
            if self.focused { 2.0 } else { 1.0 }, radius);

        let display = if is_default { &self.placeholder } else { "" };
        let display_val = if is_default { "" } else { "HH:MM" };
        let show = if is_default { display } else { &self.value.format() };
        ctx.draw_text(show,
            Point::new(frame.x + 12.0, frame.y + (frame.h - 14.0) * 0.5),
            if is_default { text_tertiary } else { text_color }, 14.0);

        // 时钟图标
        ctx.draw_text("🕐", Point::new(frame.x + frame.w - 24.0, frame.y + (frame.h - 14.0) * 0.5),
            text_secondary, 12.0);

        // 弹出层
        if self.open {
            let popup = Rect::new(frame.x, frame.y + frame.h + 2.0, frame.w, 200.0);
            ctx.fill_rect(popup, bg_elevated, radius);
            ctx.stroke_rect(popup, border_color, 1.0, radius);

            let col_w = popup.w * 0.5;
            let item_h = 32.0;

            // 小时列
            for i in 0..24 {
                let y = popup.y + i as f32 * item_h;
                if y + item_h > popup.y + popup.h { break; }
                let is_hover = i == self.hover_hour;
                if is_hover {
                    ctx.fill_rect(Rect::new(popup.x, y, col_w, item_h),
                        ctx.tokens().color_primary_bg(), None);
                }
                ctx.draw_text(&format!("{:02}", i),
                    Point::new(popup.x + 16.0, y + (item_h - 14.0) * 0.5),
                    if is_hover { primary } else { text_color }, 14.0);
            }

            // 分钟列（每 5 分钟一跳）
            for i in 0..12 {
                let y = popup.y + i as f32 * item_h;
                if y + item_h > popup.y + popup.h { break; }
                let minute = i * 5;
                let is_hover = i == self.hover_minute;
                if is_hover {
                    ctx.fill_rect(Rect::new(popup.x + col_w, y, col_w, item_h),
                        ctx.tokens().color_primary_bg(), None);
                }
                ctx.draw_text(&format!("{:02}", minute),
                    Point::new(popup.x + col_w + 16.0, y + (item_h - 14.0) * 0.5),
                    if is_hover { primary } else { text_color }, 14.0);
            }
        }
    }
}

impl TimePicker {
    pub fn new(placeholder: impl Into<String>) -> Self {
        Self {
            value: TimeValue::new(0, 0),
            placeholder: placeholder.into(),
            open: false,
            focused: false,
            hover_hour: 0,
            hover_minute: 0,
            last_frame: Cell::new(None),
        }
    }

    pub fn value(mut self, v: TimeValue) -> Self { self.value = v; self }
    pub fn selected(&self) -> TimeValue { self.value }
    pub fn set_value(&mut self, v: TimeValue) { self.value = v; }
}
