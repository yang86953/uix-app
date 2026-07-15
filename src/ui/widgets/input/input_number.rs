//! InputNumber 数字输入框 — 基于 Input 增加数字校验和步进按钮。
//!
//! 支持 min/max/step、键盘上下箭头、+/- 按钮。

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::ui::state::State;
use crate::ui::SnapshotFields;
use crate::ui::{ComponentId, EventResult, KeyCode, SemanticEvent, SystemEvent, WidgetTree};
use std::cell::Cell;

/// 可绑定到 `InputNumber` 的数值类型。
pub trait InputNumberValue: Clone + PartialEq + Send + Sync + 'static {
    fn to_f64(&self) -> f64;
    fn from_f64(value: f64) -> Self;
}

impl InputNumberValue for f64 {
    fn to_f64(&self) -> f64 {
        *self
    }

    fn from_f64(value: f64) -> Self {
        value
    }
}

impl InputNumberValue for f32 {
    fn to_f64(&self) -> f64 {
        f64::from(*self)
    }

    fn from_f64(value: f64) -> Self {
        value.clamp(f64::from(f32::MIN), f64::from(f32::MAX)) as f32
    }
}

macro_rules! impl_input_number_integer {
    ($($type:ty),+ $(,)?) => {
        $(
            impl InputNumberValue for $type {
                fn to_f64(&self) -> f64 {
                    *self as f64
                }

                fn from_f64(value: f64) -> Self {
                    value.round() as $type
                }
            }
        )+
    };
}

impl_input_number_integer!(i8, i16, i32, i64, isize, u8, u16, u32, u64, usize);

type ReadNumber = Box<dyn Fn() -> f64 + Send + Sync>;
type WriteNumber = Box<dyn Fn(f64) -> f64 + Send + Sync>;

struct InputNumberValueBinding {
    read: ReadNumber,
    write: WriteNumber,
    capture: Box<dyn Fn() + Send + Sync>,
}

impl InputNumberValueBinding {
    fn new<T: InputNumberValue>(state: &State<T>) -> Self {
        let read_state = state.clone();
        let write_state = state.clone();
        let capture_state = state.clone();
        Self {
            read: Box::new(move || read_state.get().to_f64()),
            write: Box::new(move |value| {
                let value = T::from_f64(value);
                if write_state.get() != value {
                    write_state.set(value.clone());
                }
                value.to_f64()
            }),
            capture: Box::new(move || {
                let _ = capture_state.get();
            }),
        }
    }
}

component! {
    /// InputNumber — 数字输入框。
    pub struct InputNumber {
        value: f64,
        min: f64,
        max: f64,
        step: f64,
        value_configured: bool,
        value_binding: Option<InputNumberValueBinding>,
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
        self.sync_bound_value();
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
                        self.set_value(self.value + self.step);
                        EventResult::Handled
                    }
                    KeyCode::Down => {
                        self.set_value(self.value - self.step);
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
        self.capture_bound_value_dependency();
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

        let show = if self.value_configured {
            self.format_value()
        } else {
            self.placeholder.clone()
        };
        let display = if self.focused {
            &self.text_buffer
        } else {
            &show
        };

        let draw_y = ctx.visual_center_y(input_frame, 14.0);
        ctx.draw_text(display,
            Point::new(input_frame.x + 12.0, draw_y),
            if self.focused || self.value_configured { text_color } else { text_tertiary }, 14.0);

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
    pub fn new() -> Self {
        let config = crate::ui::config::use_config();
        Self {
            value: 0.0,
            min: f64::MIN,
            max: f64::MAX,
            step: 1.0,
            value_configured: false,
            value_binding: None,
            placeholder: String::new(),
            focused: false,
            hovered: false,
            disabled: config.disabled,
            text_buffer: String::new(),
            pending_change: Cell::new(None),
        }
    }

    /// 将数值绑定到外部 `State`。
    pub fn value<T: InputNumberValue>(mut self, state: &State<T>) -> Self {
        let binding = InputNumberValueBinding::new(state);
        self.value = self.clamp_value((binding.read)());
        self.value_configured = true;
        self.text_buffer = self.format_value();
        self.value_binding = Some(binding);
        self
    }

    /// 设置非受控数字输入框的初始值。
    pub fn default_value<T: InputNumberValue>(mut self, value: T) -> Self {
        self.value_binding = None;
        self.value = self.clamp_value(value.to_f64());
        self.value_configured = true;
        self.text_buffer = self.format_value();
        self
    }

    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn min(mut self, v: f64) -> Self {
        if v.is_finite() {
            self.min = v;
            if self.min > self.max {
                self.max = self.min;
            }
            self.clamp_current_value();
        }
        self
    }

    pub fn max(mut self, v: f64) -> Self {
        if v.is_finite() {
            self.max = v;
            if self.max < self.min {
                self.min = self.max;
            }
            self.clamp_current_value();
        }
        self
    }

    pub fn step(mut self, v: f64) -> Self {
        self.step = if v.is_finite() && v > 0.0 { v } else { 1.0 };
        self
    }

    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }

    /// 返回组件当前缓存值；controlled 用法应以绑定的 `State` 为真值来源。
    pub fn current_value(&self) -> f64 {
        self.value
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(80.0, 32.0)
    }

    fn commit_buffer(&mut self) {
        if let Ok(v) = self.text_buffer.parse::<f64>() {
            self.set_value(v);
        }
        self.text_buffer = if self.value_configured {
            self.format_value()
        } else {
            String::new()
        };
    }

    fn set_value(&mut self, value: f64) {
        let value = self.clamp_value(value);
        let value = self.clamp_value(self.write_bound_value(value));
        let changed = !self.value_configured || value != self.value;
        self.value = value;
        self.value_configured = true;
        self.text_buffer = self.format_value();
        if changed {
            self.pending_change.set(Some(value));
        }
    }

    fn clamp_value(&self, value: f64) -> f64 {
        let value = if value.is_nan() { 0.0 } else { value };
        value.clamp(self.min, self.max)
    }

    fn clamp_current_value(&mut self) {
        if !self.value_configured {
            return;
        }
        self.value = self.clamp_value(self.value);
        self.text_buffer = self.format_value();
    }

    fn format_value(&self) -> String {
        if self.value == self.value.trunc() {
            format!("{:.0}", self.value)
        } else {
            format!("{:.2}", self.value)
        }
    }

    fn sync_bound_value(&mut self) {
        let Some(value) = self.value_binding.as_ref().map(|binding| (binding.read)()) else {
            return;
        };
        let value = self.clamp_value(value);
        if !self.value_configured || self.value != value {
            self.value = value;
            self.value_configured = true;
            self.text_buffer = self.format_value();
        }
    }

    fn capture_bound_value_dependency(&self) {
        if let Some(binding) = self.value_binding.as_ref() {
            (binding.capture)();
        }
    }

    fn write_bound_value(&self, value: f64) -> f64 {
        self.value_binding
            .as_ref()
            .map_or(value, |binding| (binding.write)(value))
    }
}

impl Default for InputNumber {
    fn default() -> Self {
        Self::new()
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
        let controlled_value = next.value_binding.as_ref().map(|_| next.value);
        self.min = next.min;
        self.max = next.max;
        self.step = next.step;
        self.placeholder = next.placeholder;
        self.disabled = next.disabled;
        self.value_binding = next.value_binding;

        let next_value = controlled_value.unwrap_or_else(|| self.clamp_value(self.value));
        if controlled_value.is_some() {
            self.value_configured = true;
        }
        if self.value != next_value {
            self.value = next_value;
            if self.value_configured {
                self.text_buffer = self.format_value();
            }
        }
    }
}
