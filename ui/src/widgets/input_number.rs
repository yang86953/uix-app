//! InputNumber 数字输入框 — 基于 Input 增加数字校验和步进按钮。
//!
//! 支持 min/max/step、键盘上下箭头、+/- 按钮。

use uix_core::{Point, Rect, Size};
use crate::define_widget;
use uix_graphics::{Color, GraphicsEngine, Radius};
use crate::render_context::RenderContext;
use crate::widget::{EventResult, KeyCode, WidgetEvent, WidgetTree};

define_widget! {
    /// InputNumber — 数字输入框。
    pub struct InputNumber {
        value: f64,
        min: f64,
        max: f64,
        step: f64,
        placeholder: String,
        focused: bool,
        hovered: bool,
        disabled: bool,
        /// 文本缓存（输入过程中暂存）
        text_buffer: String,
    }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        Size::new(80.0, 32.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if self.disabled { return EventResult::NotHandled; }
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                self.focused = true;
                self.text_buffer = self.value.to_string();
                EventResult::Handled
            }
            WidgetEvent::HoverEnter => { self.hovered = true; EventResult::Handled }
            WidgetEvent::HoverLeave => { self.hovered = false; EventResult::Handled }
            WidgetEvent::FocusOut => { self.focused = false; EventResult::Handled }
            WidgetEvent::KeyDown { key, .. } => {
                match key {
                    KeyCode::Up => {
                        self.value = (self.value + self.step).min(self.max);
                        self.text_buffer = self.value.to_string();
                        EventResult::Handled
                    }
                    KeyCode::Down => {
                        self.value = (self.value - self.step).max(self.min);
                        self.text_buffer = self.value.to_string();
                        EventResult::Handled
                    }
                    KeyCode::Enter => {
                        self.commit_buffer();
                        EventResult::Handled
                    }
                    KeyCode::Backspace => {
                        self.text_buffer.pop();
                        EventResult::Handled
                    }
                    _ => EventResult::NotHandled,
                }
            }
            WidgetEvent::KeyPress { text } => {
                if text.chars().any(|c| c.is_control()) {
                    return EventResult::NotHandled;
                }
                // 只允许数字、负号、小数点
                for ch in text.chars() {
                    if ch.is_ascii_digit() || ch == '-' || ch == '.' {
                        self.text_buffer.push(ch);
                    }
                }
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let input_frame = Rect::new(frame.x, frame.y, frame.w - 32.0, frame.h);
        let btn_w = 16.0;
        let primary = ctx.tokens().color_primary();
        let border_color = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let text_tertiary = ctx.tokens().color_text_tertiary();
        let text_secondary = ctx.tokens().color_text_secondary();
        let bg_elevated = ctx.tokens().color_bg_elevated();
        let border_radius_sm = ctx.tokens().border_radius_sm();
        let radius = Some(Radius::uniform(border_radius_sm));

        // 输入框
        ctx.fill_rect(input_frame, Color::white(), radius);
        ctx.stroke_rect(input_frame, if self.focused { primary } else { border_color },
            if self.focused { 2.0 } else { 1.0 }, radius);

        let display = if self.focused && !self.text_buffer.is_empty() {
            &self.text_buffer
        } else if self.value != 0.0 {
            // 显示值
            ""
        } else { &self.placeholder };

        let show = if !self.focused && self.value != 0.0 {
            // 格式化显示
            let s = if self.value == self.value.trunc() {
                format!("{}", self.value as i64)
            } else {
                format!("{:.2}", self.value)
            };
            s
        } else {
            display.to_string()
        };

        let draw_y = ctx.visual_center_y(input_frame, 14.0);
        ctx.draw_text(if self.focused { &self.text_buffer } else { &show },
            Point::new(input_frame.x + 12.0, draw_y),
            if self.focused || self.value != 0.0 { text_color } else { text_tertiary }, 14.0);

        // 步进按钮
        let btn_area = Rect::new(frame.x + frame.w - 32.0, frame.y, 32.0, frame.h);
        ctx.fill_rect(btn_area, bg_elevated, None);

        let up_rect = Rect::new(btn_area.x, btn_area.y, btn_area.w, btn_area.h * 0.5);
        let dn_rect = Rect::new(btn_area.x, btn_area.y + btn_area.h * 0.5, btn_area.w, btn_area.h * 0.5);
        let up_y = ctx.visual_center_y(up_rect, 10.0);
        let dn_y = ctx.visual_center_y(dn_rect, 10.0);
        ctx.draw_text("▲", Point::new(btn_area.x + 8.0, up_y), text_secondary, 10.0);
        ctx.draw_text("▼", Point::new(btn_area.x + 8.0, dn_y), text_secondary, 10.0);
    }
}

impl InputNumber {
    pub fn new(placeholder: impl Into<String>) -> Self {
        Self {
            value: 0.0,
            min: f64::MIN,
            max: f64::MAX,
            step: 1.0,
            placeholder: placeholder.into(),
            focused: false,
            hovered: false,
            disabled: false,
            text_buffer: String::new(),
        }
    }

    pub fn value(mut self, v: f64) -> Self { self.value = v.clamp(self.min, self.max); self }
    pub fn min(mut self, v: f64) -> Self { self.min = v; self.value = self.value.max(v); self }
    pub fn max(mut self, v: f64) -> Self { self.max = v; self.value = self.value.min(v); self }
    pub fn step(mut self, v: f64) -> Self { self.step = v; self }
    pub fn get_value(&self) -> f64 { self.value }

    fn commit_buffer(&mut self) {
        if let Ok(v) = self.text_buffer.parse::<f64>() {
            self.value = v.clamp(self.min, self.max);
        }
        self.text_buffer = self.value.to_string();
    }
}
