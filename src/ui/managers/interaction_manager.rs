use crate::core::Point;
use crate::ui::{EventResult, SystemEvent};

/// Tracks mouse/touch interaction state.
///
/// 事件坐标应为相对于 widget 左上角的偏移量。
/// `handle_event` 的 `widget_size` 参数用于验证 `PointerUp` 位置
/// 是否仍在 widget 范围内，避免在 widget 外释放误触 click。
#[derive(Default)]
pub struct InteractionManager {
    hovered: bool,
    pressed: bool,
    hovered_widget: Option<usize>,
    pressed_widget: Option<usize>,
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

    pub fn hovered_widget(&self) -> Option<usize> {
        self.hovered_widget
    }

    pub fn set_hovered_widget(&mut self, id: Option<usize>) {
        self.hovered_widget = id;
        self.hovered = id.is_some();
    }

    pub fn pressed_widget(&self) -> Option<usize> {
        self.pressed_widget
    }

    pub fn set_pressed_widget(&mut self, id: Option<usize>) {
        self.pressed_widget = id;
        self.pressed = id.is_some();
        if id.is_none() {
            self.press_pos = None;
        }
    }

    pub fn unregister_widget(&mut self, widget_id: usize) {
        if self.hovered_widget == Some(widget_id) {
            self.set_hovered_widget(None);
        }
        if self.pressed_widget == Some(widget_id) {
            self.set_pressed_widget(None);
        }
    }

    pub fn clear_tree_interaction(&mut self) {
        self.set_hovered_widget(None);
        self.set_pressed_widget(None);
    }

    /// 处理事件。`widget_size` 为 widget 的 (宽度, 高度)，
    /// 用于验证 PointerUp 是否仍在 widget 范围内。
    pub fn handle_event(&mut self, event: &SystemEvent, widget_size: (f32, f32)) -> EventResult {
        match event {
            SystemEvent::PointerDown { pos, .. } => {
                self.pressed = true;
                self.press_pos = Some(*pos);
                EventResult::Handled
            }
            SystemEvent::PointerUp { pos, .. } => {
                self.pressed = false;
                // 仅在按下和松开都在 widget 范围内才触发 click
                let within_bounds = pos.x >= 0.0
                    && pos.y >= 0.0
                    && pos.x <= widget_size.0
                    && pos.y <= widget_size.1;
                let started_inside = self
                    .press_pos
                    .map(|p| {
                        p.x >= 0.0 && p.y >= 0.0 && p.x <= widget_size.0 && p.y <= widget_size.1
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
