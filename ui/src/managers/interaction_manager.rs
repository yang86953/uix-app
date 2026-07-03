use crate::widget::{EventResult, WidgetEvent};
use uix_platform::Point;

/// Callback types for user interactions.
pub type ClickCallback = Box<dyn FnMut(&Point)>;
pub type ChangedCallback = Box<dyn FnMut()>;
pub type SubmitCallback = Box<dyn FnMut()>;

/// Manages mouse/touch interaction callbacks.
///
/// 事件坐标应为相对于 widget 左上角的偏移量。
/// `handle_event` 的 `widget_size` 参数用于验证 `MouseUp` 位置
/// 是否仍在 widget 范围内，避免在 widget 外释放误触 click。
#[derive(Default)]
pub struct InteractionManager {
    on_click: Option<ClickCallback>,
    on_changed: Option<ChangedCallback>,
    on_submit: Option<SubmitCallback>,
    hovered: bool,
    pressed: bool,
    /// MouseDown 时的位置，用于 MouseUp 边界验证。
    press_pos: Option<Point>,
}

impl InteractionManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_on_click<F: FnMut(&Point) + 'static>(&mut self, f: F) {
        self.on_click = Some(Box::new(f));
    }

    pub fn set_on_changed<F: FnMut() + 'static>(&mut self, f: F) {
        self.on_changed = Some(Box::new(f));
    }

    pub fn set_on_submit<F: FnMut() + 'static>(&mut self, f: F) {
        self.on_submit = Some(Box::new(f));
    }

    pub fn hovered(&self) -> bool {
        self.hovered
    }
    pub fn pressed(&self) -> bool {
        self.pressed
    }

    /// 处理事件。`widget_size` 为 widget 的 (宽度, 高度)，
    /// 用于验证 MouseUp 是否在 widget 范围内触发 click。
    pub fn handle_event(&mut self, event: &WidgetEvent, widget_size: (f32, f32)) -> EventResult {
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                self.pressed = true;
                self.press_pos = Some(*pos);
                EventResult::Handled
            }
            WidgetEvent::MouseUp { pos, .. } => {
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
                if within_bounds && started_inside {
                    if let Some(ref mut cb) = self.on_click {
                        cb(pos);
                    }
                }
                self.press_pos = None;
                EventResult::Handled
            }
            WidgetEvent::HoverEnter => {
                self.hovered = true;
                EventResult::Handled
            }
            WidgetEvent::HoverLeave => {
                self.hovered = false;
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }
}
