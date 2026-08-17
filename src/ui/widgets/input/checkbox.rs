//! Checkbox — checkbox with label, checked/unchecked state.

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::resources::font::text_backend::estimate_text_metrics;
use crate::platform::windowing::ControlSize;
use crate::ui::SnapshotFields;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::reactive::state::State;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent, WidgetTree,
};
use std::cell::Cell;

component! {
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

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .take()
            .map(|checked| SemanticEvent::change(id, checked.to_string()))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        self.capture_bound_checked_dependency();
        let primary = ctx.tokens().color_primary();
        let primary_hover = ctx.tokens().color_primary_hover();
        let primary_border = ctx.tokens().color_primary_border();
        let border_c = ctx.tokens().color_border();
        let border_sec = ctx.tokens().color_border_secondary();
        let text_c = if self.disabled { ctx.tokens().color_text_quaternary() } else { ctx.tokens().color_text() };
        // 勾选图标反白色：白色 token。
        let white = ctx.tokens().color_white();

        let box_size = self.box_size();
        let scale = self.visual_scale();
        let gap = self.label_gap();
        let font_size = self.font_size();
        let box_x = frame.x;
        let box_y = frame.y + (frame.h - box_size) * 0.5;
        let box_r = Rect::new(box_x, box_y, box_size, box_size);
        let corner = Some(crate::draw::Radius::uniform(3.0 * scale));

        if self.checked {
            let bg = if self.disabled { primary_border } else if self.hovered { primary_hover } else { primary };
            ctx.fill_rect(box_r, bg, corner);
            crate::ui::widgets::icon::Icon::paint_in_frame(
                ctx,
                "check",
                box_r,
                white,
                12.0 * scale,
            );
        } else {
            let border = if self.disabled { border_sec } else if self.hovered { primary_hover } else { border_c };
            ctx.stroke_rect(box_r, border, 1.5, corner);
        }

        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(Rect::new(box_x - scale, box_y - scale, box_size + 2.0 * scale, box_size + 2.0 * scale), primary, 1.5, Some(crate::draw::Radius::uniform(4.0 * scale)));
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
            6.0 * self.visual_scale()
        }
    }

    fn visual_scale(&self) -> f32 {
        match self.checkbox_size {
            ControlSize::Small => 0.875,
            ControlSize::Medium => 1.0,
            ControlSize::Large => 1.125,
        }
    }

    fn box_size(&self) -> f32 {
        16.0 * self.visual_scale()
    }

    fn font_size(&self) -> f32 {
        match self.checkbox_size {
            ControlSize::Small => 12.0,
            ControlSize::Medium => 13.0,
            ControlSize::Large => 14.0,
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
    }
}

// 仅在测试构建中编译勾选框激活时序契约。
#[cfg(test)]
mod tests {
    // 复用被测模块中的勾选框组件与事件类型。
    use super::*;
    // 引入事件行为 trait 与指针坐标类型。
    use crate::core::Point;
    use crate::ui::{KeyMod, widget_runtime::traits::EventHandler};

    // 键盘激活：KeyDown 只武装，配对的 KeyUp 才完成切换（与 Button 一致）。
    #[test]
    fn keyboard_activation_completes_on_matching_key_up() {
        // 构造未勾选勾选框。
        let mut checkbox = Checkbox::new("同意");
        // KeyDown 按下激活键。
        let _ = EventHandler::on_event(
            &mut checkbox,
            &SystemEvent::KeyDown {
                key: KeyCode::Enter,
                mods: KeyMod::NONE,
            },
        );
        // 按下阶段不得立即切换。
        assert!(!checkbox.checked);
        // 配对的 Enter 释放完成切换。
        let _ = EventHandler::on_event(
            &mut checkbox,
            &SystemEvent::KeyUp {
                key: KeyCode::Enter,
                mods: KeyMod::NONE,
            },
        );
        assert!(checkbox.checked);
    }

    // 指针激活：PointerDown 只武装，PointerUp 才完成切换。
    #[test]
    fn pointer_activation_completes_on_pointer_up() {
        // 构造未勾选勾选框。
        let mut checkbox = Checkbox::new("同意");
        // PointerDown 按下。
        let _ = EventHandler::on_event(
            &mut checkbox,
            &SystemEvent::PointerDown {
                pos: Point::new(8.0, 8.0),
                button: MouseButton::Left,
                mods: KeyMod::NONE,
            },
        );
        // 按下阶段不得立即切换。
        assert!(!checkbox.checked);
        // PointerUp 完成切换。
        let _ = EventHandler::on_event(
            &mut checkbox,
            &SystemEvent::PointerUp {
                pos: Point::new(8.0, 8.0),
                button: MouseButton::Left,
                mods: KeyMod::NONE,
            },
        );
        assert!(checkbox.checked);
    }
}
