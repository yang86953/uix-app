//! InputNumber 数字输入框 — 基于 Input 增加数字校验和步进按钮。
//!
//! 支持 min/max/step、键盘上下箭头、+/- 按钮。

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::ui::SnapshotFields;
use crate::ui::{ComponentId, EventResult, KeyCode, SemanticEvent, SystemEvent, WidgetTree};
use std::cell::Cell;

component! {
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
        text_buffer: String,
        pending_change: Cell<Option<f64>>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if self.disabled { return EventResult::NotHandled; }
        match event {
            SystemEvent::PointerDown { pos: _, .. } => {
                self.focused = true;
                self.text_buffer = self.value.to_string();
                EventResult::Handled
            }
            SystemEvent::PointerEnter => { self.hovered = true; EventResult::Handled }
            SystemEvent::PointerLeave => { self.hovered = false; EventResult::Handled }
            SystemEvent::FocusOut => { self.focused = false; EventResult::Handled }
            SystemEvent::KeyDown { key, .. } => {
                match key {
                    KeyCode::Up => {
                        let next = (self.value + self.step).min(self.max);
                        if (next - self.value).abs() > f64::EPSILON {
                            self.value = next;
                            self.text_buffer = self.value.to_string();
                            self.pending_change.set(Some(self.value));
                        }
                        EventResult::Handled
                    }
                    KeyCode::Down => {
                        let next = (self.value - self.step).max(self.min);
                        if (self.value - next).abs() > f64::EPSILON {
                            self.value = next;
                            self.text_buffer = self.value.to_string();
                            self.pending_change.set(Some(self.value));
                        }
                        EventResult::Handled
                    }
                    KeyCode::Enter => {
                        let old = self.value;
                        self.commit_buffer();
                        if (self.value - old).abs() > f64::EPSILON {
                            self.pending_change.set(Some(self.value));
                        }
                        EventResult::Handled
                    }
                    KeyCode::Backspace => {
                        self.text_buffer.pop();
                        EventResult::Handled
                    }
                    _ => EventResult::NotHandled,
                }
            }
            SystemEvent::TextInput { text } => {
                if text.chars().any(|c| c.is_control()) {
                    return EventResult::NotHandled;
                }
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

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .take()
            .map(|value| SemanticEvent::change(id, value.to_string()))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let input_frame = Rect::new(frame.x, frame.y, frame.w - 32.0, frame.h);
        let primary = ctx.tokens().color_primary();
        let primary_hover = ctx.tokens().color_primary_hover();
        let border_color = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let text_tertiary = ctx.tokens().color_text_tertiary();
        let text_secondary = ctx.tokens().color_text_secondary();
        let bg_elevated = ctx.tokens().color_bg_elevated();
        let border_radius_sm = ctx.tokens().border_radius_sm();
        let radius = Some(Radius::uniform(border_radius_sm));

        let border_c = if self.focused { primary } else if self.hovered { primary_hover } else { border_color };
        let border_w = if self.focused { 2.0 } else { 1.0 };

        ctx.fill_rect(input_frame, Color::white(), radius);
        ctx.stroke_rect(input_frame, border_c, border_w, radius);

        let display = if self.focused && !self.text_buffer.is_empty() {
            &self.text_buffer
        } else if self.value != 0.0 {
            ""
        } else { &self.placeholder };

        let show = if !self.focused && self.value != 0.0 {
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
            pending_change: Cell::new(None),
        }
    }

    pub fn value(mut self, v: f64) -> Self {
        self.value = v.clamp(self.min, self.max);
        self
    }
    pub fn min(mut self, v: f64) -> Self {
        self.min = v;
        self.value = self.value.max(v);
        self
    }
    pub fn max(mut self, v: f64) -> Self {
        self.max = v;
        self.value = self.value.min(v);
        self
    }
    pub fn step(mut self, v: f64) -> Self {
        self.step = v;
        self
    }
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    pub fn get_value(&self) -> f64 {
        self.value
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(80.0, 32.0)
    }

    fn commit_buffer(&mut self) {
        if let Ok(v) = self.text_buffer.parse::<f64>() {
            self.value = v.clamp(self.min, self.max);
        }
        self.text_buffer = self.value.to_string();
    }
}

impl InputNumber {
    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::InputNumber {
            value: self.value,
            min: self.min,
            max: self.max,
            step: self.step,
            placeholder: self.placeholder.clone(),
            disabled: self.disabled,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.min = next.min;
        self.max = next.max;
        self.step = next.step;
        self.placeholder = next.placeholder;
        self.disabled = next.disabled;

        let next_value = next.value.clamp(self.min, self.max);
        if (self.value - next_value).abs() > f64::EPSILON {
            self.value = next_value;
            self.text_buffer = self.value.to_string();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::traits::WidgetLayout;

    #[test]
    fn measure_clamps_input_number_size() {
        let measured = InputNumber::new("0").measure(Constraints::loose(Size::new(60.0, 24.0)));

        assert_eq!(measured, Size::new(60.0, 24.0));
    }
}
