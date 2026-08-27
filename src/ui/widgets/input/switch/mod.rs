//! Switch — on/off toggle switch.

use crate::core::{Constraints, Rect, Size};
use crate::draw::Color;
use crate::platform::windowing::ControlSize;
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

// 保存 UIX 声明的控件尺寸映射、轨道/滑块几何与焦点描边。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SwitchVisual {
    small_track_height: f32,
    medium_track_height: f32,
    large_track_height: f32,
    track_width_factor: f32,
    track_width_subtract: f32,
    center_ratio: f32,
    thumb_inset: f32,
    focus_outset: f32,
    focus_stroke_width: f32,
    primary: ColorValue,
    primary_hover: ColorValue,
    primary_border: ColorValue,
    fill_secondary: ColorValue,
    fill_tertiary: ColorValue,
    thumb_disabled: ColorValue,
    thumb: ColorValue,
}

// 同目录 UIX 生成唯一开关视觉值及静态借用。
crate::uix_items!("src/ui/widgets/input/switch/switch.uix");

// 保存每帧一次解析后的主题颜色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResolvedSwitchVisual {
    primary: Color,
    primary_hover: Color,
    primary_border: Color,
    fill_secondary: Color,
    fill_tertiary: Color,
    thumb_disabled: Color,
    thumb: Color,
}

impl SwitchVisual {
    fn resolve(self, tokens: &dyn crate::ui::ThemeTokens) -> ResolvedSwitchVisual {
        ResolvedSwitchVisual {
            primary: self.primary.resolve(tokens),
            primary_hover: self.primary_hover.resolve(tokens),
            primary_border: self.primary_border.resolve(tokens),
            fill_secondary: self.fill_secondary.resolve(tokens),
            fill_tertiary: self.fill_tertiary.resolve(tokens),
            thumb_disabled: self.thumb_disabled.resolve(tokens),
            thumb: self.thumb.resolve(tokens),
        }
    }

    fn track_height(self, size: ControlSize) -> f32 {
        match size {
            ControlSize::Small => self.small_track_height,
            ControlSize::Medium => self.medium_track_height,
            ControlSize::Large => self.large_track_height,
        }
    }
}

// 向 UIX 提供零分配主题角色。
const fn switch_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
const fn switch_primary_hover() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryHover)
}
const fn switch_primary_border() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryBorder)
}
const fn switch_fill_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillSecondary)
}
const fn switch_fill_tertiary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}
const fn switch_thumb_disabled() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}
const fn switch_thumb() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgElevated)
}

widget! {
    /// 支持受控布尔状态、标签和键盘切换的开关组件。
    pub struct Switch {
        // 展示名称，供无障碍播报与语义快照使用。
        label: String,
        checked: bool,
        checked_binding: Option<State<bool>>,
        disabled: bool,
        control_size: ControlSize,
        size: f32,
        hovered: bool,
        focused: bool,
        // 指针按下武装标记（PointerUp 匹配才切换）。
        pressed: bool,
        // 键盘激活手势的武装键（KeyUp 匹配才切换）。
        pressed_key: Option<KeyCode>,
        pending_change: Cell<Option<bool>>,
        #[snapshot(skip)]
        /// UIX 声明的尺寸映射、轨道/滑块几何与主题角色。
        pub(crate) visual: &'static SwitchVisual,
    }


    tab_index => (&self) -> i32 { 1 }
    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        // 与 Segmented 一致：disabled 拦截前先清理 FocusOut，避免禁用后残留焦点态。
        if matches!(event, SystemEvent::FocusOut) {
            self.focused = false;
            self.pressed = false;
            self.pressed_key = None;
            return EventResult::Handled;
        }
        if self.disabled { return EventResult::NotHandled; }
        self.sync_bound_checked();
        match event {
            // 激活时序与 Button 统一：PointerDown 只武装，PointerUp 完成切换，
            // 按下后移出再松开不会误触发。
            SystemEvent::PointerDown {
                button: MouseButton::Left,
                ..
            } => {
                self.pressed = true;
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::PointerUp {
                button: MouseButton::Left,
                ..
            } => {
                // 只有同一目标内的完整按下/释放序列才切换。
                if self.pressed {
                    self.toggle_checked();
                }
                self.pressed = false;
                EventResult::Handled
            }
            SystemEvent::PointerEnter => { self.hovered = true; EventResult::Handled }
            // 指针离开取消未完成的按下手势。
            SystemEvent::PointerLeave => {
                self.hovered = false;
                self.pressed = false;
                EventResult::Handled
            }
            SystemEvent::FocusIn => { self.focused = true; EventResult::Handled }
            // 键盘激活：KeyDown 只武装，KeyUp 完成切换（与 Button 激活模式一致）。
            SystemEvent::KeyDown { key, .. } => {
                if *key == KeyCode::Space || *key == KeyCode::Enter {
                    self.pressed_key = Some(*key);
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::KeyUp { key, .. } => {
                // 只有配对的释放键才完成键盘切换；非配对释放不消耗武装。
                if self.pressed_key == Some(*key) {
                    self.pressed_key = None;
                    self.toggle_checked();
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            _ => EventResult::NotHandled
        }
    }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .take()
            .map(|checked| SemanticEvent::change(id, checked.to_string()))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        self.capture_bound_checked_dependency();
        let visual = self.visual.resolve(ctx.tokens());

        let h = self.size;
        let w = self.track_width();
        let track_x = frame.x;
        let track_y = frame.y + (frame.h - h) * self.visual.center_ratio;
        let track_r = h * self.visual.center_ratio;
        let thumb_r = track_r - self.visual.thumb_inset;
        let thumb_x = if self.checked {
            track_x + w - self.visual.thumb_inset - thumb_r * 2.0
        } else {
            track_x + self.visual.thumb_inset
        };

        let track_c = if self.checked {
            if self.disabled { visual.primary_border } else if self.hovered { visual.primary_hover } else { visual.primary }
        } else {
            if self.disabled { visual.fill_tertiary } else { visual.fill_secondary }
        };
        let thumb_c = if self.disabled { visual.thumb_disabled } else { visual.thumb };

        let radius = Some(crate::draw::Radius::uniform(track_r));
        ctx.fill_rect(Rect::new(track_x, track_y, w, h), track_c, radius);
        let thumb = Rect::new(
            thumb_x,
            track_y + self.visual.thumb_inset,
            thumb_r * 2.0,
            thumb_r * 2.0,
        );
        ctx.fill_rect(thumb, thumb_c, Some(crate::draw::Radius::uniform(thumb_r)));

        if self.focused && tree.keyboard_focus_visible() {
            let outset = self.visual.focus_outset;
            ctx.stroke_rect(
                Rect::new(
                    track_x - outset,
                    track_y - outset,
                    w + outset * 2.0,
                    h + outset * 2.0,
                ),
                visual.primary,
                self.visual.focus_stroke_width,
                radius,
            );
        }
    }
}

impl Default for Switch {
    fn default() -> Self {
        Self::new()
    }
}

impl Switch {
    /// 创建初始关闭且可交互的开关。
    pub fn new() -> Self {
        let config = crate::ui::widget_runtime::config::use_config();
        let visual = SWITCH_VISUAL_REF;
        Self {
            // 无展示名称的开关保持匿名播报。
            label: String::new(),
            checked: false,
            checked_binding: None,
            disabled: false,
            control_size: config.size,
            size: visual.track_height(config.size),
            hovered: false,
            focused: false,
            // 无指针按下与键盘激活手势。
            pressed: false,
            pressed_key: None,
            pending_change: Cell::new(None),
            visual,
        }
    }
    /// 设置开关的展示名称（供无障碍播报）。
    pub fn label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }
    /// 将开关值绑定到外部 `State<bool>`；用户切换与外部更新保持双向同步。
    pub fn checked(mut self, state: &State<bool>) -> Self {
        self.checked_binding = Some(state.clone());
        self.checked = state.get();
        self
    }
    /// 设置非受控组件的初始开启值。
    pub fn default_checked(mut self, value: bool) -> Self {
        self.checked_binding = None;
        self.checked = value;
        self
    }
    /// 设置开关是否禁用交互。
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    /// 设置开关采用的控件尺寸。
    pub fn size(mut self, size: ControlSize) -> Self {
        self.control_size = size;
        self.size = self.visual.track_height(size);
        self
    }
    /// 返回开关当前是否处于开启状态。
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    fn sync_bound_checked(&mut self) {
        if let Some(state) = self.checked_binding.as_ref() {
            self.checked = state.get();
        }
    }

    fn capture_bound_checked_dependency(&self) {
        if let Some(state) = self.checked_binding.as_ref() {
            let _ = state.get();
        }
    }

    fn toggle_checked(&mut self) {
        self.checked = !self.checked;
        if let Some(state) = self.checked_binding.as_ref() {
            if state.get() != self.checked {
                state.set(self.checked);
            }
        }
        self.pending_change.set(Some(self.checked));
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(
            self.track_width(),
            crate::ui::widget_runtime::config::control_height(self.control_size),
        )
    }

    fn track_width(&self) -> f32 {
        self.size * self.visual.track_width_factor - self.visual.track_width_subtract
    }
}

impl Switch {
    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Switch {
            // 快照携带展示名称供无障碍转换使用。
            label: self.label.clone(),
            checked: self.checked,
            disabled: self.disabled,
            size: self.size,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let controlled_checked = next.checked_binding.as_ref().map(|_| next.checked);
        self.label = next.label;
        self.checked_binding = next.checked_binding;
        if let Some(checked) = controlled_checked {
            self.checked = checked;
        }
        self.disabled = next.disabled;
        self.control_size = next.control_size;
        self.size = next.size;
        self.visual = next.visual;
    }
}

// 把 Switch Rust 状态内核与 UIX 静态视觉组合为单一组件节点。
fn build_switch_view(mut kernel: Switch, visual: &'static SwitchVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Switch {
    fn build(self) -> ViewNode {
        build_switch_uix_root(self)
    }
}

// 为 UIX 根提供稳定的 Rust 内核绑定名称。
fn build_switch_uix_root(kernel: Switch) -> ViewNode {
    crate::uix!("src/ui/widgets/input/switch/switch.uix")
}
