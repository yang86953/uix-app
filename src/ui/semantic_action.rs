//! Shared semantic actions for headless automation and the Agent Bridge.
//!
//! The executor deliberately stays below transport/protocol code: callers
//! resolve a window and node, then enqueue one of these actions onto that
//! window's normal UI path.

// Agent control remains opt-in. Keep the shared executor compiled without
// warning noise when neither its runtime feature nor `test-harness` is active.
#![cfg_attr(not(any(test, feature = "test-harness")), allow(dead_code))]

use std::fmt;

use crate::core::{ComponentId, Point};
use crate::platform::windowing::{KeyCode, KeyMod, MouseButton};
use crate::ui::component::widget::{EventResult, WidgetTree};
use crate::ui::component_snapshot::AccessibilityRole;
use crate::ui::event::{ClickEvent, SemanticEvent, SemanticKind, SystemEvent};
use crate::ui::widgets::Input;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticActionKind {
    Invoke,
    Focus,
    SetValue,
    InsertText,
    Select,
    Toggle,
    Increment,
    Decrement,
    Adjust,
    Scroll,
}

#[allow(dead_code)]
impl SemanticActionKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Invoke => "invoke",
            Self::Focus => "focus",
            Self::SetValue => "set_value",
            Self::InsertText => "insert_text",
            Self::Select => "select",
            Self::Toggle => "toggle",
            Self::Increment => "increment",
            Self::Decrement => "decrement",
            Self::Adjust => "adjust",
            Self::Scroll => "scroll",
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, PartialEq)]
pub enum SemanticAction {
    Invoke,
    Focus,
    SetValue(String),
    InsertText(String),
    Select(String),
    Toggle,
    Increment,
    Decrement,
    /// 连续值调整能力声明（E-05）：目标支持在 `min..=max` 范围内调整；
    /// 方向性步进经 `Increment` / `Decrement` 动作执行。
    Adjust {
        min: f64,
        max: f64,
    },
    Scroll {
        delta: Point,
    },
}

impl SemanticAction {
    pub const fn kind(&self) -> SemanticActionKind {
        match self {
            Self::Invoke => SemanticActionKind::Invoke,
            Self::Focus => SemanticActionKind::Focus,
            Self::SetValue(_) => SemanticActionKind::SetValue,
            Self::InsertText(_) => SemanticActionKind::InsertText,
            Self::Select(_) => SemanticActionKind::Select,
            Self::Toggle => SemanticActionKind::Toggle,
            Self::Increment => SemanticActionKind::Increment,
            Self::Decrement => SemanticActionKind::Decrement,
            Self::Adjust { .. } => SemanticActionKind::Adjust,
            Self::Scroll { .. } => SemanticActionKind::Scroll,
        }
    }
}

impl fmt::Debug for SemanticAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invoke => f.write_str("Invoke"),
            Self::Focus => f.write_str("Focus"),
            Self::SetValue(_) => f.write_str("SetValue(<redacted>)"),
            Self::InsertText(_) => f.write_str("InsertText(<redacted>)"),
            Self::Select(_) => f.write_str("Select(<redacted>)"),
            Self::Toggle => f.write_str("Toggle"),
            Self::Increment => f.write_str("Increment"),
            Self::Decrement => f.write_str("Decrement"),
            Self::Adjust { min, max } => f
                .debug_struct("Adjust")
                .field("min", min)
                .field("max", max)
                .finish(),
            Self::Scroll { delta } => f.debug_struct("Scroll").field("delta", delta).finish(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SemanticActionError {
    NodeNotFound(ComponentId),
    UnsupportedAction {
        target: ComponentId,
        action: SemanticActionKind,
    },
    NotVisible(ComponentId),
    Disabled(ComponentId),
    SelectionDisabled {
        target: ComponentId,
        index: usize,
    },
    InvalidValue {
        target: ComponentId,
        action: SemanticActionKind,
    },
    Blocked {
        target: ComponentId,
        blocker: ComponentId,
    },
    NotHandled {
        target: ComponentId,
        action: SemanticActionKind,
    },
}

impl WidgetTree {
    pub(crate) fn supported_semantic_actions(&self, id: ComponentId) -> Vec<SemanticActionKind> {
        let Some(node) = self.get(id) else {
            return Vec::new();
        };
        let snapshot = node.component_snapshot(id);
        let accessibility = snapshot.accessibility();
        let selection = snapshot.selection();
        let role = accessibility.role;
        if role == AccessibilityRole::None {
            return Vec::new();
        }
        let has_click_handler = node
            .handler_signatures()
            .iter()
            .any(|signature| signature.kind == SemanticKind::Click);
        let accepts_text = node
            .as_text_input()
            .is_some_and(|client| client.accepts_text_input());
        let is_input = node.component().as_any().is::<Input>();
        let can_focus = node.is_focusable()
            || (node.accepts_events()
                && matches!(
                    role,
                    AccessibilityRole::Button
                        | AccessibilityRole::Checkbox
                        | AccessibilityRole::Combobox
                        | AccessibilityRole::RadioGroup
                        | AccessibilityRole::Slider
                        | AccessibilityRole::SpinButton
                        | AccessibilityRole::Switch
                        | AccessibilityRole::TextBox
                ));

        let mut actions = Vec::new();
        // 组件声明（E-05）：`component!` 的 `semantic_actions` 槽位在 role 推断
        // 之外补充自定义能力（如连续值 `Adjust`）。
        for declared in node.component().declared_semantic_actions() {
            let kind = declared.kind();
            if !actions.contains(&kind) {
                actions.push(kind);
            }
        }
        if role == AccessibilityRole::Button || has_click_handler {
            actions.push(SemanticActionKind::Invoke);
        }
        if can_focus {
            actions.push(SemanticActionKind::Focus);
        }
        if is_input {
            actions.push(SemanticActionKind::SetValue);
        }
        if accepts_text {
            actions.push(SemanticActionKind::InsertText);
        }
        if selection.as_ref().is_some_and(|selection| {
            !selection.multiple
                && !selection.options.is_empty()
                && selection.selected_indices.len() == 1
        }) {
            actions.push(SemanticActionKind::Select);
        }
        if matches!(
            role,
            AccessibilityRole::Checkbox | AccessibilityRole::Switch
        ) {
            actions.push(SemanticActionKind::Toggle);
        }
        if matches!(
            role,
            AccessibilityRole::Slider | AccessibilityRole::SpinButton
        ) {
            actions.push(SemanticActionKind::Increment);
            actions.push(SemanticActionKind::Decrement);
            actions.push(SemanticActionKind::Adjust);
        }
        if node.viewport_scroll_offset().is_some() {
            actions.push(SemanticActionKind::Scroll);
        }
        actions
    }

    pub(crate) fn blocking_modal_for(&self, id: ComponentId) -> Option<ComponentId> {
        self.overlay_stack
            .top()
            .filter(|entry| entry.is_modal())
            .map(|entry| entry.owner())
            .filter(|&owner| id != owner && !self.is_descendant_of(id, owner))
    }

    pub(crate) fn perform_semantic_action(
        &mut self,
        id: ComponentId,
        action: &SemanticAction,
    ) -> Result<(), SemanticActionError> {
        // 停止树必须在读取节点快照或用户 handler 前拒绝语义动作。
        if !self.accepts_external_work() {
            // 复用既有未处理错误以维持调用方的动作失败契约。
            return Err(SemanticActionError::NotHandled {
                // 保留调用方原始目标身份供上层映射错误。
                target: id,
                // 仅读取调用方动作种类，不访问停止树中的节点元数据。
                action: action.kind(),
            });
        }
        let action_kind = action.kind();
        let Some(node) = self.get(id) else {
            return Err(SemanticActionError::NodeNotFound(id));
        };
        let accessibility = node.component_snapshot(id).accessibility();
        let role = accessibility.role;
        let actions = self.supported_semantic_actions(id);
        if !actions.contains(&action_kind) {
            return Err(SemanticActionError::UnsupportedAction {
                target: id,
                action: action_kind,
            });
        }
        let Some(visible_bounds) = self.visible_rect_for(id) else {
            return Err(SemanticActionError::NotVisible(id));
        };
        if accessibility.state.disabled {
            return Err(SemanticActionError::Disabled(id));
        }
        if let Some(blocker) = self.blocking_modal_for(id) {
            return Err(SemanticActionError::Blocked {
                target: id,
                blocker,
            });
        }

        let handled = match action {
            SemanticAction::Invoke => {
                let keyboard = if role == AccessibilityRole::Button {
                    self.focus_and_press(id, KeyCode::Enter)
                } else {
                    EventResult::NotHandled
                };
                if keyboard == EventResult::Handled {
                    keyboard
                } else {
                    let click = ClickEvent {
                        button: MouseButton::Left,
                        pos: center(visible_bounds),
                        modifiers: KeyMod::NONE,
                    };
                    self.dispatch_semantic(SemanticEvent::click(id, click))
                }
            }
            SemanticAction::Focus => {
                self.set_keyboard_focus_visible(true);
                self.set_focus(Some(id));
                if self.managers().focus.focused_component() == Some(id) {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SemanticAction::SetValue(value) => self.set_input_value(id, value),
            SemanticAction::InsertText(text) => {
                self.set_focus(Some(id));
                if text.is_empty() {
                    EventResult::Handled
                } else {
                    self.dispatch_event(&SystemEvent::TextInput { text: text.clone() })
                }
            }
            SemanticAction::Toggle => self.focus_and_press(id, KeyCode::Space),
            SemanticAction::Increment => self.focus_and_press(id, KeyCode::Up),
            SemanticAction::Decrement => self.focus_and_press(id, KeyCode::Down),
            // 连续值调整能力（E-05）：执行时聚焦目标并返回 Handled，方向性
            // 步进由 `Increment` / `Decrement` 动作驱动；min/max 已进入语义快照。
            SemanticAction::Adjust { .. } => {
                self.set_focus(Some(id));
                if self.managers().focus.focused_component() == Some(id) {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SemanticAction::Scroll { delta } => {
                if delta.x == 0.0 && delta.y == 0.0 {
                    EventResult::Handled
                } else {
                    // 语义滚动必须命中已解析的目标；按坐标重新命中会被目标内部的
                    // Table / ScrollView 截获，导致自动化滚动了错误的视口。
                    self.dispatch_to(
                        id,
                        &SystemEvent::Wheel {
                            pos: center(visible_bounds),
                            delta: *delta,
                        },
                    )
                }
            }
            SemanticAction::Select(value) => self.select_option(id, role, value)?,
        };

        if handled == EventResult::Handled {
            Ok(())
        } else {
            Err(SemanticActionError::NotHandled {
                target: id,
                action: action_kind,
            })
        }
    }

    fn focus_and_press(&mut self, id: ComponentId, key: KeyCode) -> EventResult {
        self.set_focus(Some(id));
        self.press_key(key)
    }

    fn press_key(&mut self, key: KeyCode) -> EventResult {
        let down = self.dispatch_event(&SystemEvent::KeyDown {
            key,
            mods: KeyMod::SYNTHETIC,
        });
        let up = self.dispatch_event(&SystemEvent::KeyUp {
            key,
            mods: KeyMod::SYNTHETIC,
        });
        if down == EventResult::Handled || up == EventResult::Handled {
            EventResult::Handled
        } else {
            EventResult::NotHandled
        }
    }

    fn select_option(
        &mut self,
        id: ComponentId,
        role: AccessibilityRole,
        value: &str,
    ) -> Result<EventResult, SemanticActionError> {
        let selection = self
            .get(id)
            .map(|node| node.component_snapshot(id))
            .and_then(|snapshot| snapshot.selection())
            .ok_or(SemanticActionError::NotHandled {
                target: id,
                action: SemanticActionKind::Select,
            })?;
        let index = if !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()) {
            value.parse::<usize>().ok()
        } else {
            None
        }
        .filter(|index| *index < selection.options.len())
        .ok_or(SemanticActionError::InvalidValue {
            target: id,
            action: SemanticActionKind::Select,
        })?;
        if selection.disabled_indices.contains(&index) {
            return Err(SemanticActionError::SelectionDisabled { target: id, index });
        }
        let Some(mut current) = selection.selected_indices.first().copied() else {
            return Ok(EventResult::NotHandled);
        };
        if current == index {
            return Ok(EventResult::Handled);
        }

        self.set_focus(Some(id));
        if self.managers().focus.focused_component() != Some(id) {
            return Ok(EventResult::NotHandled);
        }

        if role == AccessibilityRole::Combobox {
            if !selection.expanded && self.press_key(KeyCode::Down) != EventResult::Handled {
                return Ok(EventResult::NotHandled);
            }
            let key = if index > current {
                KeyCode::Down
            } else {
                KeyCode::Up
            };
            for _ in 0..current.abs_diff(index) {
                if self.press_key(key) != EventResult::Handled {
                    let _ = self.press_key(KeyCode::Escape);
                    return Ok(EventResult::NotHandled);
                }
            }
            let _ = self.press_key(KeyCode::Escape);
        } else {
            while current != index {
                let next = if index > current {
                    ((current + 1)..=index)
                        .find(|candidate| !selection.disabled_indices.contains(candidate))
                } else {
                    (index..current)
                        .rev()
                        .find(|candidate| !selection.disabled_indices.contains(candidate))
                };
                let Some(next) = next else {
                    return Ok(EventResult::NotHandled);
                };
                let key = if next > current {
                    KeyCode::Down
                } else {
                    KeyCode::Up
                };
                if self.press_key(key) != EventResult::Handled {
                    return Ok(EventResult::NotHandled);
                }
                current = next;
            }
        }

        let selected = self
            .get(id)
            .map(|node| node.component_snapshot(id))
            .and_then(|snapshot| snapshot.selection())
            .is_some_and(|selection| selection.selected_indices.as_slice() == [index]);
        Ok(if selected {
            EventResult::Handled
        } else {
            EventResult::NotHandled
        })
    }

    fn set_input_value(&mut self, id: ComponentId, value: &str) -> EventResult {
        let Some(current_value) = self.get(id).and_then(|node| {
            node.component()
                .as_any()
                .downcast_ref::<Input>()
                .map(|input| input.current_value().to_owned())
        }) else {
            return EventResult::NotHandled;
        };

        self.set_focus(Some(id));
        let _ = self.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::A,
            mods: KeyMod::CTRL,
        });
        let _ = self.dispatch_event(&SystemEvent::KeyUp {
            key: KeyCode::A,
            mods: KeyMod::CTRL,
        });
        if value.is_empty() {
            if current_value.is_empty() {
                EventResult::Handled
            } else {
                let down = self.dispatch_event(&SystemEvent::KeyDown {
                    key: KeyCode::Backspace,
                    mods: KeyMod::NONE,
                });
                let _ = self.dispatch_event(&SystemEvent::KeyUp {
                    key: KeyCode::Backspace,
                    mods: KeyMod::NONE,
                });
                down
            }
        } else {
            self.dispatch_event(&SystemEvent::TextInput {
                text: value.to_owned(),
            })
        }
    }
}

fn center(bounds: crate::core::Rect) -> Point {
    Point::new(bounds.x + bounds.w * 0.5, bounds.y + bounds.h * 0.5)
}
