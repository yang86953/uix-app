use crate::core::Point;
use crate::ui::{ComponentId, EventResult, MouseButton, SystemEvent};

/// Tracks mouse/touch interaction state.
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
    pub fn new() -> Self {
        Self::default()
    }

    pub fn hovered(&self) -> bool {
        self.hovered
    }
    pub fn pressed(&self) -> bool {
        self.pressed
    }

    pub fn hovered_component(&self) -> Option<ComponentId> {
        self.hovered_component
    }

    pub fn set_hovered_component(&mut self, id: Option<ComponentId>) {
        self.hovered_component = id;
        self.hovered = id.is_some();
    }

    pub fn pressed_component(&self) -> Option<ComponentId> {
        self.pressed_component
    }

    pub fn pressed_button(&self) -> Option<MouseButton> {
        self.pressed_button
    }

    pub fn set_pressed_component(&mut self, id: Option<ComponentId>) {
        self.pressed_component = id;
        self.pressed_button = None;
        self.pressed = id.is_some();
        if id.is_none() {
            self.press_pos = None;
        }
    }

    pub fn begin_pressed_pointer(&mut self, id: Option<ComponentId>, button: MouseButton) -> bool {
        if self.pressed_component.is_some() && self.pressed_button != Some(button) {
            return false;
        }
        self.set_pressed_component(id);
        self.pressed_button = id.map(|_| button);
        true
    }

    pub fn release_pressed_pointer(&mut self, button: MouseButton) -> Option<ComponentId> {
        if self.pressed_button != Some(button) {
            return None;
        }
        let pressed = self.pressed_component;
        self.set_pressed_component(None);
        pressed
    }

    pub fn unregister_component(&mut self, component_id: ComponentId) {
        if self.hovered_component == Some(component_id) {
            self.set_hovered_component(None);
        }
        if self.pressed_component == Some(component_id) {
            self.set_pressed_component(None);
        }
    }

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
