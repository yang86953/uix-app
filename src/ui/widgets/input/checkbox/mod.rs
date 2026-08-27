//! Checkbox — checkbox with label, checked/unchecked state.

use crate::core::{Constraints, Rect, Size};
use crate::draw::Color;
use crate::draw::resources::font::text_backend::estimate_text_metrics;
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

// 保存 UIX 声明的尺寸映射、勾选图标、边框与焦点几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CheckboxVisual {
    small_scale: f32,
    medium_scale: f32,
    large_scale: f32,
    small_font_size: f32,
    medium_font_size: f32,
    large_font_size: f32,
    base_box_size: f32,
    label_gap: f32,
    center_ratio: f32,
    box_radius: f32,
    checked_icon: &'static str,
    checked_icon_size: f32,
    border_width: f32,
    focus_outset: f32,
    focus_radius: f32,
    focus_stroke_width: f32,
    primary: ColorValue,
    primary_hover: ColorValue,
    primary_border: ColorValue,
    border: ColorValue,
    border_secondary: ColorValue,
    text: ColorValue,
    text_disabled: ColorValue,
    checked_icon_color: ColorValue,
}

// 同目录 UIX 生成唯一复选框视觉值及静态借用。
crate::uix_items!("src/ui/widgets/input/checkbox/checkbox.uix");

// 保存每帧一次解析后的主题颜色，绘制分支只选择紧凑值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResolvedCheckboxVisual {
    primary: Color,
    primary_hover: Color,
    primary_border: Color,
    border: Color,
    border_secondary: Color,
    text: Color,
    text_disabled: Color,
    checked_icon_color: Color,
}

impl CheckboxVisual {
    fn resolve(self, tokens: &dyn crate::ui::ThemeTokens) -> ResolvedCheckboxVisual {
        ResolvedCheckboxVisual {
            primary: self.primary.resolve(tokens),
            primary_hover: self.primary_hover.resolve(tokens),
            primary_border: self.primary_border.resolve(tokens),
            border: self.border.resolve(tokens),
            border_secondary: self.border_secondary.resolve(tokens),
            text: self.text.resolve(tokens),
            text_disabled: self.text_disabled.resolve(tokens),
            checked_icon_color: self.checked_icon_color.resolve(tokens),
        }
    }
}

// 向 UIX 提供零分配的主题角色。
const fn checkbox_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
const fn checkbox_primary_hover() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryHover)
}
const fn checkbox_primary_border() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryBorder)
}
const fn checkbox_border() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Border)
}
const fn checkbox_border_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BorderSecondary)
}
const fn checkbox_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
const fn checkbox_text_disabled() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextQuaternary)
}
const fn checkbox_checked_icon_color() -> ColorValue {
    ColorValue::Palette(PaletteColor::White)
}

widget! {
    /// 支持受控勾选状态、标签和禁用语义的复选框组件。
    pub struct Checkbox {
        checked: bool,
        checked_binding: Option<State<bool>>,
        disabled: bool,
        label: String,
        hovered: bool,
        focused: bool,
        checkbox_size: ControlSize,
        // 指针按下武装标记（PointerUp 匹配才切换）。
        pressed: bool,
        // 键盘激活手势的武装键（KeyUp 匹配才切换）。
        pressed_key: Option<KeyCode>,
        pending_change: Cell<Option<bool>>,
        #[snapshot(skip)]
        /// UIX 声明的尺寸、图标、描边与主题角色。
        pub(crate) visual: &'static CheckboxVisual,
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
        let text_c = if self.disabled { visual.text_disabled } else { visual.text };

        let box_size = self.box_size();
        let scale = self.visual_scale();
        let gap = self.label_gap();
        let font_size = self.font_size();
        let box_x = frame.x;
        let box_y = frame.y + (frame.h - box_size) * self.visual.center_ratio;
        let box_r = Rect::new(box_x, box_y, box_size, box_size);
        let corner = Some(crate::draw::Radius::uniform(self.visual.box_radius * scale));

        if self.checked {
            let bg = if self.disabled { visual.primary_border } else if self.hovered { visual.primary_hover } else { visual.primary };
            ctx.fill_rect(box_r, bg, corner);
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                self.visual.checked_icon,
                box_r,
                visual.checked_icon_color,
                self.visual.checked_icon_size * scale,
            );
        } else {
            let border = if self.disabled { visual.border_secondary } else if self.hovered { visual.primary_hover } else { visual.border };
            ctx.stroke_rect(box_r, border, self.visual.border_width, corner);
        }

        if self.focused && tree.keyboard_focus_visible() {
            let outset = self.visual.focus_outset * scale;
            ctx.stroke_rect(
                Rect::new(
                    box_x - outset,
                    box_y - outset,
                    box_size + outset * 2.0,
                    box_size + outset * 2.0,
                ),
                visual.primary,
                self.visual.focus_stroke_width,
                Some(crate::draw::Radius::uniform(self.visual.focus_radius * scale)),
            );
        }

        let label_rect = Rect::new(box_x + box_size + gap, frame.y, (frame.w - box_size - gap).max(0.0), frame.h);
        ctx.text_center(&self.label, label_rect, text_c, font_size);
    }
}

impl Default for Checkbox {
    fn default() -> Self {
        Self::new("")
    }
}

impl Checkbox {
    /// 创建带指定标签且初始未勾选的复选框。
    pub fn new(label: impl Into<String>) -> Self {
        let config = crate::ui::widget_runtime::config::use_config();
        Self {
            checked: false,
            checked_binding: None,
            disabled: false,
            label: label.into(),
            hovered: false,
            focused: false,
            checkbox_size: config.size,
            // 无指针按下与键盘激活手势。
            pressed: false,
            pressed_key: None,
            pending_change: Cell::new(None),
            visual: CHECKBOX_VISUAL_REF,
        }
    }
    /// 将勾选值绑定到外部 `State<bool>`；用户切换与外部更新保持双向同步。
    pub fn checked(mut self, state: &State<bool>) -> Self {
        self.checked_binding = Some(state.clone());
        self.checked = state.get();
        self
    }
    /// 设置非受控组件的初始勾选值。
    pub fn default_checked(mut self, value: bool) -> Self {
        self.checked_binding = None;
        self.checked = value;
        self
    }
    /// 设置复选框是否禁用交互。
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    /// 设置复选框采用的控件尺寸。
    pub fn size(mut self, size: ControlSize) -> Self {
        self.checkbox_size = size;
        self
    }
    /// 返回复选框当前是否处于勾选状态。
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
        let text_w =
            estimate_text_metrics(&self.label, f32::INFINITY, self.font_size()).max_line_width;
        Size::new(
            self.box_size() + self.label_gap() + text_w,
            crate::ui::widget_runtime::config::control_height(self.checkbox_size),
        )
    }

    fn label_gap(&self) -> f32 {
        if self.label.is_empty() {
            0.0
        } else {
            self.visual.label_gap * self.visual_scale()
        }
    }

    fn visual_scale(&self) -> f32 {
        match self.checkbox_size {
            ControlSize::Small => self.visual.small_scale,
            ControlSize::Medium => self.visual.medium_scale,
            ControlSize::Large => self.visual.large_scale,
        }
    }

    fn box_size(&self) -> f32 {
        self.visual.base_box_size * self.visual_scale()
    }

    fn font_size(&self) -> f32 {
        match self.checkbox_size {
            ControlSize::Small => self.visual.small_font_size,
            ControlSize::Medium => self.visual.medium_font_size,
            ControlSize::Large => self.visual.large_font_size,
        }
    }
}

impl Checkbox {
    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Checkbox {
            checked: self.checked,
            disabled: self.disabled,
            label: self.label.clone(),
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let controlled_checked = next.checked_binding.as_ref().map(|_| next.checked);
        self.checked_binding = next.checked_binding;
        if let Some(checked) = controlled_checked {
            self.checked = checked;
        }
        self.disabled = next.disabled;
        self.label = next.label;
        self.checkbox_size = next.checkbox_size;
        self.visual = next.visual;
    }
}

// 把 Checkbox Rust 状态内核与 UIX 静态视觉组合为单一组件节点。
fn build_checkbox_view(mut kernel: Checkbox, visual: &'static CheckboxVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Checkbox {
    fn build(self) -> ViewNode {
        build_checkbox_uix_root(self)
    }
}

// 为 UIX 根提供稳定的 Rust 内核绑定名称。
fn build_checkbox_uix_root(kernel: Checkbox) -> ViewNode {
    crate::uix!("src/ui/widgets/input/checkbox/checkbox.uix")
}
