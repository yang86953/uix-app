use crate::core::Point;
use crate::ui::{ComponentId, EventResult, MouseButton, SystemEvent};

/// 跟踪鼠标和触摸交互状态。
///
/// 事件坐标应为相对于 component 左上角的偏移量。
/// `handle_event` 的 `component_size` 参数用于验证 `PointerUp` 位置
/// 是否仍在 component 范围内，避免在 component 外释放误触 click。
#[derive(Default)]
pub struct InteractionManager {
    hovered: bool,
    pressed: bool,
    hovered_component: Option<ComponentId>,
    pressed_component: Option<ComponentId>,
    pressed_button: Option<MouseButton>,
    /// PointerDown 时的位置，用于 PointerUp 边界验证。
    press_pos: Option<Point>,
}

impl InteractionManager {
    /// 创建没有悬停或按压目标的交互管理器。
    pub fn new() -> Self {
        Self::default()
    }

    /// 返回当前是否处于悬停状态。
    pub fn hovered(&self) -> bool {
        self.hovered
    }
    /// 返回当前是否处于按压状态。
    pub fn pressed(&self) -> bool {
        self.pressed
    }

    /// 返回当前悬停组件标识。
    pub fn hovered_component(&self) -> Option<ComponentId> {
        self.hovered_component
    }

    /// 设置悬停组件，并同步悬停状态。
    pub fn set_hovered_component(&mut self, id: Option<ComponentId>) {
        self.hovered_component = id;
        self.hovered = id.is_some();
    }

    /// 返回当前按压组件标识。
    pub fn pressed_component(&self) -> Option<ComponentId> {
        self.pressed_component
    }

    /// 返回启动当前组件按压的指针按钮。
    pub fn pressed_button(&self) -> Option<MouseButton> {
        self.pressed_button
    }

    /// 设置按压组件，清除按钮，并在取消按压时清除起始位置。
    pub fn set_pressed_component(&mut self, id: Option<ComponentId>) {
        self.pressed_component = id;
        self.pressed_button = None;
        self.pressed = id.is_some();
        if id.is_none() {
            self.press_pos = None;
        }
    }

    /// 开始组件指针按压；已有不同按钮的按压时返回 `false`。
    pub fn begin_pressed_pointer(&mut self, id: Option<ComponentId>, button: MouseButton) -> bool {
        if self.pressed_component.is_some() && self.pressed_button != Some(button) {
            return false;
        }
        self.set_pressed_component(id);
        self.pressed_button = id.map(|_| button);
        true
    }

    /// 释放匹配按钮的组件按压，并返回此前的按压组件标识。
    pub fn release_pressed_pointer(&mut self, button: MouseButton) -> Option<ComponentId> {
        if self.pressed_button != Some(button) {
            return None;
        }
        let pressed = self.pressed_component;
        self.set_pressed_component(None);
        pressed
    }

    /// 注销组件，并清除该组件持有的悬停或按压状态。
    pub fn unregister_component(&mut self, component_id: ComponentId) {
        if self.hovered_component == Some(component_id) {
            self.set_hovered_component(None);
        }
        if self.pressed_component == Some(component_id) {
            self.set_pressed_component(None);
        }
    }

    /// 清除整个组件树的悬停和按压状态。
    pub fn clear_tree_interaction(&mut self) {
        self.set_hovered_component(None);
        self.set_pressed_component(None);
    }

    /// 处理事件。`component_size` 为 component 的 (宽度, 高度)，
    /// 用于验证 PointerUp 是否仍在 component 范围内。
    pub fn handle_event(&mut self, event: &SystemEvent, component_size: (f32, f32)) -> EventResult {
        match event {
            SystemEvent::PointerDown { pos, .. } => {
                self.pressed = true;
                self.press_pos = Some(*pos);
                EventResult::Handled
            }
            SystemEvent::PointerUp { pos, .. } => {
                self.pressed = false;
                // 仅在按下和松开都在 component 范围内才触发 click
                let within_bounds = pos.x >= 0.0
                    && pos.y >= 0.0
                    && pos.x <= component_size.0
                    && pos.y <= component_size.1;
                let started_inside = self
                    .press_pos
                    .map(|p| {
                        p.x >= 0.0
                            && p.y >= 0.0
                            && p.x <= component_size.0
                            && p.y <= component_size.1
                    })
                    .unwrap_or(false);
                self.press_pos = None;
                if within_bounds && started_inside {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerEnter => {
                self.hovered = true;
                EventResult::Handled
            }
            SystemEvent::PointerLeave => {
                self.hovered = false;
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }
}
