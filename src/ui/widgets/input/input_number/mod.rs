//! InputNumber 数字输入框 — 基于 Input 增加数字校验和步进按钮。
//!
//! 支持 min/max/step、键盘上下箭头、+/- 按钮。

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::{Color, Radius};
use crate::platform::windowing::{ControlSize, KeyMod};
use crate::ui::SnapshotFields;
use crate::ui::reactive::state::State;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{
    EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent, View, ViewNode, WidgetId,
    WidgetTree,
};
use crate::widget;
use std::cell::Cell;

/// 可绑定到 `InputNumber` 的数值类型。
pub trait InputNumberValue: Clone + PartialEq + Send + Sync + 'static {
    /// 将绑定值投影为组件内部使用的 `f64`。
    fn to_f64(&self) -> f64;

    /// 将组件提交的 `f64` 转换回绑定类型。
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

// f64 在十进制量化时最多保留十五位有效精度。
const MAX_PRECISION: u8 = 15;

// InputNumber 使用的主题圆角角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputNumberRadiusRole {
    Small,
}

impl InputNumberRadiusRole {
    fn resolve(self, tokens: &dyn crate::ui::ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

// 保存 UIX 声明的固有宽度、文字、光标、步进区和边框几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct InputNumberVisual {
    intrinsic_input_width: f32,
    center_ratio: f32,
    text_left_padding: f32,
    text_horizontal_inset: f32,
    font_size: f32,
    cursor_vertical_inset: f32,
    cursor_width: f32,
    normal_border_width: f32,
    focused_border_width: f32,
    separator_inset: f32,
    separator_width: f32,
    up_icon: &'static str,
    down_icon: &'static str,
    step_icon_size: f32,
    radius: InputNumberRadiusRole,
    primary: ColorValue,
    primary_hover: ColorValue,
    border: ColorValue,
    text: ColorValue,
    placeholder: ColorValue,
    text_disabled: ColorValue,
    step_text: ColorValue,
    background_disabled: ColorValue,
    background: ColorValue,
}

// 同目录 UIX 生成唯一数字输入视觉值及静态借用。
crate::uix_items!("src/ui/widgets/input/input_number/input_number.uix");

// 保存每帧一次解析后的颜色与圆角。
#[derive(Debug, Clone, Copy, PartialEq)]
struct ResolvedInputNumberVisual {
    primary: Color,
    primary_hover: Color,
    border: Color,
    text: Color,
    placeholder: Color,
    text_disabled: Color,
    step_text: Color,
    background_disabled: Color,
    background: Color,
    radius: f32,
}

impl InputNumberVisual {
    fn resolve(self, tokens: &dyn crate::ui::ThemeTokens) -> ResolvedInputNumberVisual {
        ResolvedInputNumberVisual {
            primary: self.primary.resolve(tokens),
            primary_hover: self.primary_hover.resolve(tokens),
            border: self.border.resolve(tokens),
            text: self.text.resolve(tokens),
            placeholder: self.placeholder.resolve(tokens),
            text_disabled: self.text_disabled.resolve(tokens),
            step_text: self.step_text.resolve(tokens),
            background_disabled: self.background_disabled.resolve(tokens),
            background: self.background.resolve(tokens),
            radius: self.radius.resolve(tokens),
        }
    }
}

// 向 UIX 提供圆角和零分配主题角色。
const fn input_number_small_radius() -> InputNumberRadiusRole {
    InputNumberRadiusRole::Small
}
const fn input_number_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
const fn input_number_primary_hover() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryHover)
}
const fn input_number_border() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Border)
}
const fn input_number_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
const fn input_number_placeholder() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextTertiary)
}
const fn input_number_text_disabled() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextQuaternary)
}
const fn input_number_step_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
const fn input_number_background_disabled() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}
const fn input_number_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}

impl Clone for InputNumber {
    /// 配置克隆：复制公开配置（值/值域/步进/占位/尺寸/禁用/键盘）。
    ///
    /// 不复制运行时状态（焦点/悬停/缓冲区/光标）与闭包绑定（formatter /
    /// value_binding）；值绑定在构建 View 时经 `.value(&State)` 重建（E-01）。
    fn clone(&self) -> Self {
        Self {
            value: self.value,
            min: self.min,
            max: self.max,
            step: self.step,
            // 保留声明式精度配置。
            precision: self.precision,
            value_configured: self.value_configured,
            value_binding: None,
            placeholder: self.placeholder.clone(),
            focused: false,
            hovered: false,
            disabled: self.disabled,
            keyboard: self.keyboard,
            formatter: None,
            input_size: self.input_size,
            text_buffer: String::new(),
            pending_change: Cell::new(None),
            cursor_rect: Cell::new(Rect::default()),
            step_button_rect: Cell::new(Rect::default()),
            visual: self.visual,
        }
    }
}

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

widget! {
    /// InputNumber — 数字输入框。
    pub struct InputNumber {
        value: f64,
        min: f64,
        max: f64,
        step: f64,
        // 保存声明式小数位精度；None 表示沿用数值本身。
        precision: Option<u8>,
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
        #[snapshot(skip)]
        /// UIX 声明的文字、光标、步进区、边框与主题角色。
        pub(crate) visual: &'static InputNumberVisual,
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
                let direction = if pos.y < step_rect.y + step_rect.h * self.visual.center_ratio {
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

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .take()
            .map(|value| SemanticEvent::change(id, value.to_string()))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.capture_bound_value_dependency();
        let control_height = crate::ui::widget_runtime::config::control_height(self.input_size).min(frame.h);
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
        let visual = self.visual.resolve(ctx.tokens());
        let radius = Some(Radius::uniform(visual.radius));

        let border_c = if self.disabled {
            visual.border
        } else if self.focused {
            visual.primary
        } else if self.hovered {
            visual.primary_hover
        } else {
            visual.border
        };
        let border_w = if self.focused {
            self.visual.focused_border_width
        } else {
            self.visual.normal_border_width
        };
        let control_bg = if self.disabled {
            visual.background_disabled
        } else {
            visual.background
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
            visual.text_disabled
        } else if showing_placeholder {
            visual.placeholder
        } else {
            visual.text
        };
        let text_area = Rect::new(
            input_frame.x + self.visual.text_left_padding,
            input_frame.y,
            (input_frame.w - self.visual.text_horizontal_inset).max(0.0),
            input_frame.h,
        );
        let display_width = if display.is_empty() {
            0.0
        } else {
            ctx.measure_text(display, self.visual.font_size).w
        };
        let draw_x = if !showing_placeholder && display_width > text_area.w {
            text_area.x + text_area.w - display_width
        } else {
            text_area.x
        };
        let draw_y = ctx.visual_center_y(text_area, self.visual.font_size);
        if text_area.w > 0.0 {
            ctx.push_clip(text_area);
            ctx.draw_text(
                display,
                Point::new(draw_x, draw_y),
                display_color,
                self.visual.font_size,
            );
            let cursor_x = (draw_x + if editing { display_width } else { 0.0 })
                .clamp(text_area.x, text_area.x + text_area.w);
            let cursor_rect = Rect::new(
                cursor_x,
                input_frame.y + self.visual.cursor_vertical_inset,
                self.visual.cursor_width,
                (input_frame.h - self.visual.cursor_vertical_inset * 2.0).max(0.0),
            );
            self.cursor_rect
                .set(if editing { cursor_rect } else { Rect::zero() });
            if editing {
                ctx.fill_rect(cursor_rect, visual.primary, None);
            }
            ctx.pop_clip();
        } else {
            self.cursor_rect.set(Rect::zero());
        }

        let up_rect = Rect::new(
            btn_area.x,
            btn_area.y,
            btn_area.w,
            btn_area.h * self.visual.center_ratio,
        );
        let dn_rect = Rect::new(
            btn_area.x,
            btn_area.y + btn_area.h * self.visual.center_ratio,
            btn_area.w,
            btn_area.h * self.visual.center_ratio,
        );
        let step_color = if self.disabled {
            visual.text_disabled
        } else {
            visual.step_text
        };
        if btn_area.w > 0.0 && btn_area.h > 0.0 {
            ctx.draw_line(
                btn_area.x,
                btn_area.y + self.visual.separator_inset,
                btn_area.x,
                btn_area.y + btn_area.h - self.visual.separator_inset,
                visual.border,
                self.visual.separator_width,
            );
            ctx.draw_line(
                btn_area.x,
                btn_area.y + btn_area.h * self.visual.center_ratio,
                btn_area.x + btn_area.w - self.visual.separator_inset,
                btn_area.y + btn_area.h * self.visual.center_ratio,
                visual.border,
                self.visual.separator_width,
            );
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                self.visual.up_icon,
                up_rect,
                step_color,
                self.visual.step_icon_size,
            );
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                self.visual.down_icon,
                dn_rect,
                step_color,
                self.visual.step_icon_size,
            );
        }
        ctx.stroke_rect(control_frame, border_c, border_w, radius);
    }
}

impl InputNumber {
    /// 创建使用无限制值域、步长一和标准配置尺寸的数字输入框。
    pub fn new() -> Self {
        let config = crate::ui::widget_runtime::config::use_config();
        let visual = INPUT_NUMBER_VISUAL_REF;
        let control_height = crate::ui::widget_runtime::config::control_height(config.size);
        Self {
            value: 0.0,
            min: f64::MIN,
            max: f64::MAX,
            step: 1.0,
            // 默认不强制小数位精度。
            precision: None,
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
            step_button_rect: Cell::new(Rect::new(
                visual.intrinsic_input_width,
                0.0,
                control_height,
                control_height,
            )),
            visual,
        }
    }

    /// 将数值绑定到外部 `State`。
    pub fn value<T: InputNumberValue>(mut self, state: &State<T>) -> Self {
        let binding = InputNumberValueBinding::new(state);
        // 受控初值同时遵守已声明的精度与范围。
        self.value = self.clamp_value(self.quantize_value((binding.read)()));
        self.value_configured = true;
        self.text_buffer = self.raw_value_text();
        self.value_binding = Some(binding);
        self
    }

    /// 设置非受控数字输入框的初始值。
    pub fn default_value<T: InputNumberValue>(mut self, value: T) -> Self {
        self.value_binding = None;
        // 非受控初值同时遵守已声明的精度与范围。
        self.value = self.clamp_value(self.quantize_value(value.to_f64()));
        self.value_configured = true;
        self.text_buffer = self.raw_value_text();
        self
    }

    /// 设置尚未配置数值时显示的占位文本。
    pub fn placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    /// 设置有限下界，并在必要时同步抬高上界和当前值。
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

    /// 设置有限上界，并在必要时同步降低下界和当前值。
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

    /// 设置正有限步长；非法值回退为一。
    pub fn step(mut self, v: f64) -> Self {
        self.step = if v.is_finite() && v > 0.0 { v } else { 1.0 };
        self
    }

    /// 设置数值量化与显示使用的小数位精度。
    pub fn precision(mut self, precision: u8) -> Self {
        // f64 超过十五位小数无法稳定兑现，因此统一收敛到有效上限。
        self.precision = Some(precision.min(MAX_PRECISION));
        // 已配置值立即按新精度归一化，避免构建顺序影响显示。
        self.clamp_current_value();
        // 返回可继续配置的组件。
        self
    }

    /// 设置是否禁止所有用户输入交互。
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

    /// 设置数字输入框使用的标准控件尺寸。
    pub fn size(mut self, size: ControlSize) -> Self {
        self.input_size = size;
        self
    }

    /// 返回组件当前缓存值；controlled 用法应以绑定的 `State` 为真值来源。
    pub fn current_value(&self) -> f64 {
        self.value
    }

    fn intrinsic_size(&self) -> Size {
        let height = crate::ui::widget_runtime::config::control_height(self.input_size);
        Size::new(self.visual.intrinsic_input_width + height, height)
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
        // 用户提交值先按精度量化再限制到声明范围。
        let value = self.clamp_value(self.quantize_value(value));
        // 外部绑定写回可能转换类型，因此返回值再次精度化并限制范围。
        let value = self.clamp_value(self.quantize_value(self.write_bound_value(value)));
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
        // 显式精度优先，否则保持既有按当前值与步长推导的行为。
        let precision = self.precision.map(i32::from).unwrap_or_else(|| {
            // 推导精度同样受 f64 有效位数上限约束。
            decimal_places(self.value)
                .max(decimal_places(self.step))
                .min(i32::from(MAX_PRECISION))
        });
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

    // 按显式精度量化有限值，未配置精度时保持原值。
    fn quantize_value(&self, value: f64) -> f64 {
        // 没有显式精度时不改变业务数值。
        let Some(precision) = self.precision else {
            // 返回原始输入。
            return value;
        };
        // 构造稳定的十进制缩放因子。
        let factor = 10.0_f64.powi(i32::from(precision));
        // 缩放结果用于四舍五入到目标小数位。
        let scaled = value * factor;
        // 非有限输入交给后续范围归一化处理。
        if !scaled.is_finite() {
            // 保留原始值避免溢出扩散。
            return value;
        }
        // 执行十进制量化并恢复原单位。
        scaled.round() / factor
    }

    fn clamp_current_value(&mut self) {
        if !self.value_configured {
            return;
        }
        // 精度量化后再次应用范围，确保舍入不会越过边界。
        self.value = self.clamp_value(self.quantize_value(self.value));
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
        // 自定义 formatter 具有最高显示优先级。
        if let Some(formatter) = self.formatter.as_ref() {
            // 返回调用方自定义的显示文本。
            return formatter(self.value);
        }
        // 显式精度生成固定小数位文本。
        if let Some(precision) = self.precision {
            // Rust 动态精度格式化接收 usize。
            return format!("{:.*}", usize::from(precision), self.value);
        }
        // 未配置格式化时沿用最短数值文本。
        self.raw_value_text()
    }

    fn sync_bound_value(&mut self) {
        let Some(value) = self.value_binding.as_ref().map(|binding| (binding.read)()) else {
            return;
        };
        // 外部受控值按当前精度与范围归一化后进入运行时缓存。
        let value = self.clamp_value(self.quantize_value(value));
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
            // 暴露声明式精度供配置差异比较。
            precision: self.precision,
            placeholder: self.placeholder.clone(),
            disabled: self.disabled,
            keyboard: self.keyboard,
            // 自定义格式化或固定精度都会改变可访问显示文本。
            formatted: self.formatter.is_some() || self.precision.is_some(),
            display_value: self.value_configured.then(|| self.display_value_text()),
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let controlled_value = next.value_binding.as_ref().map(|_| next.value);
        self.min = next.min;
        self.max = next.max;
        self.step = next.step;
        // 同步声明式精度配置。
        self.precision = next.precision;
        self.placeholder = next.placeholder;
        self.disabled = next.disabled;
        self.keyboard = next.keyboard;
        self.formatter = next.formatter;
        self.input_size = next.input_size;
        self.value_binding = next.value_binding;
        if self.disabled {
            self.focused = false;
        }

        // 未受控组件也必须在 precision 变更后重新量化现有值。
        let next_value = controlled_value
            // 没有外部值时归一化当前运行时值。
            .unwrap_or_else(|| self.clamp_value(self.quantize_value(self.value)));
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
        self.visual = next.visual;
    }
}

// 把 InputNumber Rust 数值内核与 UIX 静态视觉组合为单一组件节点。
fn build_input_number_view(
    mut kernel: InputNumber,
    visual: &'static InputNumberVisual,
) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for InputNumber {
    fn build(self) -> ViewNode {
        build_input_number_uix_root(self)
    }
}

// 为 UIX 根提供稳定的 Rust 内核绑定名称。
fn build_input_number_uix_root(kernel: InputNumber) -> ViewNode {
    crate::uix!("src/ui/widgets/input/input_number/input_number.uix")
}

// 验证 InputNumber 精度配置、状态回写与显示快照。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../tests/unit/ui/widgets/input/input_number__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
