//! InputNumber 数字输入框 — 基于 Input 增加数字校验和步进按钮。
//!
//! 支持 min/max/step、键盘上下箭头、+/- 按钮。

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Radius;
use crate::native::windowing::input::{ControlSize, KeyMod};
use crate::ui::component::paint_context::PaintContext;
use crate::ui::reactive::state::State;
use crate::ui::SnapshotFields;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent, WidgetTree,
};
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
type NumberFormatter = Box<dyn Fn(f64) -> String + Send + Sync>;

fn decimal_places(value: f64) -> i32 {
    if !value.is_finite() || value == 0.0 {
        return 0;
    }
    let text = value.abs().to_string();
    let (mantissa, exponent) = text
        .split_once(['e', 'E'])
        .map_or((text.as_str(), 0), |(mantissa, exponent)| {
            (mantissa, exponent.parse::<i32>().unwrap_or(0))
        });
    let fraction = mantissa
        .split_once('.')
        .map_or(0, |(_, fraction)| fraction.len() as i32);
    (fraction - exponent).max(0)
}

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
        keyboard: bool,
        formatter: Option<NumberFormatter>,
        input_size: ControlSize,
        text_buffer: String,
        pending_change: Cell<Option<f64>>,
        cursor_rect: Cell<Rect>,
        step_button_rect: Cell<Rect>,
    }

    tab_index => (&self) -> i32 { 1 }

    accepts_text_input => (&self) -> bool { !self.disabled && self.keyboard }

    text_input_cursor_rect => (&self) -> Rect { self.cursor_rect.get() }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        self.sync_bound_value();
        if self.disabled { return EventResult::NotHandled; }
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let step_rect = self.step_button_rect.get();
                if step_rect.contains(*pos) {
                    let direction = if pos.y < step_rect.y + step_rect.h * 0.5 {
                        1.0
                    } else {
                        -1.0
                    };
                    self.step_by(direction);
                    return EventResult::Handled;
                }
                if self.keyboard {
                    self.begin_editing();
                } else {
                    self.focused = true;
                }
                EventResult::Handled
            }
            SystemEvent::PointerEnter => { self.hovered = true; EventResult::Handled }
            SystemEvent::PointerLeave => { self.hovered = false; EventResult::Handled }
            SystemEvent::FocusIn => {
                if self.keyboard {
                    self.begin_editing();
                } else {
                    self.focused = true;
                }
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                if self.keyboard {
                    self.commit_buffer();
                }
                self.focused = false;
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, mods } => {
                let semantic_step = mods.contains(KeyMod::SYNTHETIC)
                    && matches!(key, KeyCode::Up | KeyCode::Down);
                if !self.keyboard && !semantic_step {
                    return EventResult::NotHandled;
                }
                match key {
                    KeyCode::Up => {
                        self.step_by(1.0);
                        EventResult::Handled
                    }
                    KeyCode::Down => {
                        self.step_by(-1.0);
                        EventResult::Handled
                    }
                    KeyCode::Enter => {
                        self.commit_buffer();
                        EventResult::Handled
                    }
                    KeyCode::Backspace => {
                        if self.text_buffer.pop().is_some() {
                            EventResult::Handled
                        } else {
                            EventResult::NotHandled
                        }
                    }
                    _ => EventResult::NotHandled,
                }
            }
            SystemEvent::TextInput { .. } | SystemEvent::Paste { .. } if !self.keyboard =>
                EventResult::NotHandled,
            SystemEvent::TextInput { text } | SystemEvent::Paste { text } =>
                self.append_numeric_text(text),
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
        let control_height = crate::ui::component::config::control_height(self.input_size).min(frame.h);
        let control_frame = Rect::new(frame.x, frame.y, frame.w, control_height);
        let step_width = control_height.min(control_frame.w);
        let input_frame = Rect::new(
            control_frame.x,
            control_frame.y,
            (control_frame.w - step_width).max(0.0),
            control_frame.h,
        );
        let btn_area = Rect::new(
            control_frame.x + control_frame.w - step_width,
            control_frame.y,
            step_width,
            control_frame.h,
        );
        self.step_button_rect.set(Rect::new(
            btn_area.x - control_frame.x,
            btn_area.y - control_frame.y,
            btn_area.w,
            btn_area.h,
        ));
        let primary = ctx.tokens().color_primary();
        let primary_hover = ctx.tokens().color_primary_hover();
        let border_color = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let text_tertiary = ctx.tokens().color_text_tertiary();
        let text_quaternary = ctx.tokens().color_text_quaternary();
        let text_secondary = ctx.tokens().color_text_secondary();
        let fill_tertiary = ctx.tokens().color_fill_tertiary();
        let border_radius_sm = ctx.tokens().border_radius_sm();
        let radius = Some(Radius::uniform(border_radius_sm));

        let border_c = if self.disabled {
            border_color
        } else if self.focused {
            primary
        } else if self.hovered {
            primary_hover
        } else {
            border_color
        };
        let border_w = if self.focused { 2.0 } else { 1.0 };
        let control_bg = if self.disabled {
            fill_tertiary
        } else {
            ctx.tokens().color_bg_container()
        };

        ctx.fill_rect(control_frame, control_bg, radius);

        let show = if self.value_configured {
            self.display_value_text()
        } else {
            self.placeholder.clone()
        };
        let editing = self.focused && self.keyboard;
        let display = if editing {
            &self.text_buffer
        } else {
            &show
        };
        let showing_placeholder = !editing && !self.value_configured;
        let display_color = if self.disabled {
            text_quaternary
        } else if showing_placeholder {
            text_tertiary
        } else {
            text_color
        };
        let text_area = Rect::new(
            input_frame.x + 12.0,
            input_frame.y,
            (input_frame.w - 20.0).max(0.0),
            input_frame.h,
        );
        let display_width = if display.is_empty() {
            0.0
        } else {
            ctx.measure_text(display, 14.0).w
        };
        let draw_x = if !showing_placeholder && display_width > text_area.w {
            text_area.x + text_area.w - display_width
        } else {
            text_area.x
        };
        let draw_y = ctx.visual_center_y(text_area, 14.0);
        if text_area.w > 0.0 {
            ctx.push_clip(text_area);
            ctx.draw_text(
                display,
                Point::new(draw_x, draw_y),
                display_color,
                14.0,
            );
            let cursor_x = (draw_x + if editing { display_width } else { 0.0 })
                .clamp(text_area.x, text_area.x + text_area.w);
            let cursor_rect = Rect::new(
                cursor_x,
                input_frame.y + 4.0,
                1.0,
                (input_frame.h - 8.0).max(0.0),
            );
            self.cursor_rect
                .set(if editing { cursor_rect } else { Rect::zero() });
            if editing {
                ctx.fill_rect(cursor_rect, primary, None);
            }
            ctx.pop_clip();
        } else {
            self.cursor_rect.set(Rect::zero());
        }

        let up_rect = Rect::new(btn_area.x, btn_area.y, btn_area.w, btn_area.h * 0.5);
        let dn_rect = Rect::new(btn_area.x, btn_area.y + btn_area.h * 0.5, btn_area.w, btn_area.h * 0.5);
        let step_color = if self.disabled {
            text_quaternary
        } else {
            text_secondary
        };
        if btn_area.w > 0.0 && btn_area.h > 0.0 {
            ctx.draw_line(
                btn_area.x,
                btn_area.y + 1.0,
                btn_area.x,
                btn_area.y + btn_area.h - 1.0,
                border_color,
                1.0,
            );
            ctx.draw_line(
                btn_area.x,
                btn_area.y + btn_area.h * 0.5,
                btn_area.x + btn_area.w - 1.0,
                btn_area.y + btn_area.h * 0.5,
                border_color,
                1.0,
            );
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                "chevron-up",
                up_rect,
                step_color,
                10.0,
            );
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                "chevron-down",
                dn_rect,
                step_color,
                10.0,
            );
        }
        ctx.stroke_rect(control_frame, border_c, border_w, radius);
    }
}

impl InputNumber {
    pub fn new() -> Self {
        let config = crate::ui::component::config::use_config();
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
            keyboard: true,
            formatter: None,
            input_size: config.size,
            text_buffer: String::new(),
            pending_change: Cell::new(None),
            cursor_rect: Cell::new(Rect::zero()),
            step_button_rect: Cell::new(Rect::new(80.0, 0.0, 32.0, 32.0)),
        }
    }

    /// 将数值绑定到外部 `State`。
    pub fn value<T: InputNumberValue>(mut self, state: &State<T>) -> Self {
        let binding = InputNumberValueBinding::new(state);
        self.value = self.clamp_value((binding.read)());
        self.value_configured = true;
        self.text_buffer = self.raw_value_text();
        self.value_binding = Some(binding);
        self
    }

    /// 设置非受控数字输入框的初始值。
    pub fn default_value<T: InputNumberValue>(mut self, value: T) -> Self {
        self.value_binding = None;
        self.value = self.clamp_value(value.to_f64());
        self.value_configured = true;
        self.text_buffer = self.raw_value_text();
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

    /// 控制实体键盘、文本输入与粘贴；关闭后仅保留指针步进按钮和语义步进。
    pub fn keyboard(mut self, enabled: bool) -> Self {
        self.keyboard = enabled;
        if !enabled {
            self.text_buffer = if self.value_configured {
                self.raw_value_text()
            } else {
                String::new()
            };
            self.cursor_rect.set(Rect::zero());
        }
        self
    }

    /// 自定义非编辑态的显示文本；编辑、State 与 Change 始终使用原始数值。
    pub fn formatter(mut self, formatter: impl Fn(f64) -> String + Send + Sync + 'static) -> Self {
        self.formatter = Some(Box::new(formatter));
        self
    }

    pub fn size(mut self, size: ControlSize) -> Self {
        self.input_size = size;
        self
    }

    /// 返回组件当前缓存值；controlled 用法应以绑定的 `State` 为真值来源。
    pub fn current_value(&self) -> f64 {
        self.value
    }

    fn intrinsic_size(&self) -> Size {
        let height = crate::ui::component::config::control_height(self.input_size);
        Size::new(80.0 + height, height)
    }

    fn begin_editing(&mut self) {
        if self.focused {
            return;
        }
        self.focused = true;
        self.text_buffer = if self.value_configured {
            self.raw_value_text()
        } else {
            String::new()
        };
    }

    fn append_numeric_text(&mut self, text: &str) -> EventResult {
        if text.is_empty() || text.chars().any(char::is_control) {
            return EventResult::NotHandled;
        }
        let previous_len = self.text_buffer.len();
        self.text_buffer.extend(
            text.chars()
                .filter(|character| character.is_ascii_digit() || matches!(character, '-' | '.')),
        );
        if self.text_buffer.len() > previous_len {
            EventResult::Handled
        } else {
            EventResult::NotHandled
        }
    }

    fn commit_buffer(&mut self) {
        if let Ok(v) = self.text_buffer.parse::<f64>() {
            self.set_value(v);
        }
        self.text_buffer = if self.value_configured {
            self.raw_value_text()
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
        self.text_buffer = self.raw_value_text();
        if changed {
            self.pending_change.set(Some(value));
        }
    }

    fn step_by(&mut self, direction: f64) {
        if self.focused && self.keyboard {
            self.commit_buffer();
        }
        let raw = self.value + self.step * direction;
        let precision = decimal_places(self.value)
            .max(decimal_places(self.step))
            .min(15);
        let factor = 10.0f64.powi(precision);
        let scaled = raw * factor;
        let stepped = if factor.is_finite() && scaled.is_finite() {
            scaled.round() / factor
        } else {
            raw
        };
        self.set_value(stepped);
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
        self.text_buffer = self.raw_value_text();
    }

    fn raw_value_text(&self) -> String {
        if self.value == 0.0 {
            "0".to_string()
        } else {
            self.value.to_string()
        }
    }

    fn display_value_text(&self) -> String {
        self.formatter
            .as_ref()
            .map_or_else(|| self.raw_value_text(), |formatter| formatter(self.value))
    }

    fn sync_bound_value(&mut self) {
        let Some(value) = self.value_binding.as_ref().map(|binding| (binding.read)()) else {
            return;
        };
        let value = self.clamp_value(value);
        if !self.value_configured || self.value != value {
            self.value = value;
            self.value_configured = true;
            self.text_buffer = self.raw_value_text();
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
            keyboard: self.keyboard,
            formatted: self.formatter.is_some(),
            display_value: self.value_configured.then(|| self.display_value_text()),
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let controlled_value = next.value_binding.as_ref().map(|_| next.value);
        self.min = next.min;
        self.max = next.max;
        self.step = next.step;
        self.placeholder = next.placeholder;
        self.disabled = next.disabled;
        self.keyboard = next.keyboard;
        self.formatter = next.formatter;
        self.input_size = next.input_size;
        self.value_binding = next.value_binding;
        if self.disabled {
            self.focused = false;
        }

        let next_value = controlled_value.unwrap_or_else(|| self.clamp_value(self.value));
        if controlled_value.is_some() {
            self.value_configured = true;
        }
        if self.value != next_value {
            self.value = next_value;
            if self.value_configured {
                self.text_buffer = self.raw_value_text();
            }
        }
        if !self.keyboard {
            self.text_buffer = if self.value_configured {
                self.raw_value_text()
            } else {
                String::new()
            };
            self.cursor_rect.set(Rect::zero());
        }
    }
}
