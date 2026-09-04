//! Shared semantic actions for headless automation and the Agent Bridge.
//!
//! The executor deliberately stays below transport/protocol code: callers
//! resolve a window and node, then enqueue one of these actions onto that
//! window's normal UI path.

// Agent control remains opt-in. Keep the shared executor compiled without
// warning noise when neither its runtime feature nor `test-harness` is active.
#![cfg_attr(not(any(test, feature = "test-harness")), allow(dead_code))]

use std::fmt;

use crate::core::{Point, WidgetId};
use crate::platform::windowing::{KeyCode, KeyMod, MouseButton};
use crate::ui::event::{ClickEvent, SemanticEvent, SemanticKind, SystemEvent};
use crate::ui::widget_runtime::widget::{EventResult, WidgetTree};
use crate::ui::widget_snapshot::{AccessibilityRole, SnapshotFields};
use crate::ui::widgets::Input;
use crate::ui::widgets::window_chrome::WindowInteractionRegion;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// 不携带动作载荷的稳定语义动作种类。
pub enum SemanticActionKind {
    /// 激活按钮或其他可调用目标。
    Invoke,
    /// 将键盘焦点移动到目标。
    Focus,
    /// 用完整文本替换目标当前值。
    SetValue,
    /// 在目标当前编辑位置插入文本。
    InsertText,
    /// 用前缀与后缀包裹目标当前选区；无选区时在光标处插入成对标记。
    WrapSelection,
    /// 按稳定选项值选择单选目标。
    Select,
    /// 切换复选框或开关状态。
    Toggle,
    /// 按目标自身步长增加连续值。
    Increment,
    /// 按目标自身步长减少连续值。
    Decrement,
    /// 声明目标支持在给定范围内调整连续值。
    Adjust,
    /// 按二维增量滚动目标视口。
    Scroll,
}

#[allow(dead_code)]
impl SemanticActionKind {
    /// 返回供协议与诊断使用的规范蛇形命名标识。
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Invoke => "invoke",
            Self::Focus => "focus",
            Self::SetValue => "set_value",
            Self::InsertText => "insert_text",
            Self::WrapSelection => "wrap_selection",
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
/// 要通过目标窗口正常 UI 路径执行的语义动作及其载荷。
pub enum SemanticAction {
    /// 激活按钮或其他可调用目标。
    Invoke,
    /// 将键盘焦点移动到目标。
    Focus,
    /// 用所给文本完整替换目标当前值。
    SetValue(String),
    /// 在目标当前编辑位置插入所给文本。
    InsertText(String),
    /// 用所给前缀与后缀包裹目标当前选区；无选区时在光标处插入成对标记。
    WrapSelection {
        /// 选区前粘贴的前缀标记。
        prefix: String,
        /// 选区后粘贴的后缀标记。
        suffix: String,
    },
    /// 按所给稳定选项值选择单选目标。
    Select(String),
    /// 切换复选框或开关状态。
    Toggle,
    /// 按目标自身步长增加连续值。
    Increment,
    /// 按目标自身步长减少连续值。
    Decrement,
    /// 连续值调整能力声明（E-05）：目标支持在 `min..=max` 范围内调整；
    /// 方向性步进经 `Increment` / `Decrement` 动作执行。
    Adjust {
        /// 目标允许调整到的最小值。
        min: f64,
        /// 目标允许调整到的最大值。
        max: f64,
    },
    /// 按二维坐标增量滚动目标视口。
    Scroll {
        /// 要应用到目标视口的水平与垂直滚动增量。
        delta: Point,
    },
}

impl SemanticAction {
    /// 返回当前动作不含载荷的稳定种类。
    pub const fn kind(&self) -> SemanticActionKind {
        match self {
            Self::Invoke => SemanticActionKind::Invoke,
            Self::Focus => SemanticActionKind::Focus,
            Self::SetValue(_) => SemanticActionKind::SetValue,
            Self::InsertText(_) => SemanticActionKind::InsertText,
            Self::WrapSelection { .. } => SemanticActionKind::WrapSelection,
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
            Self::WrapSelection { .. } => f.write_str("WrapSelection(<redacted>)"),
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
    NodeNotFound(WidgetId),
    UnsupportedAction {
        target: WidgetId,
        action: SemanticActionKind,
    },
    NotVisible(WidgetId),
    Disabled(WidgetId),
    SelectionDisabled {
        target: WidgetId,
        index: usize,
    },
    InvalidValue {
        target: WidgetId,
        action: SemanticActionKind,
    },
    Blocked {
        target: WidgetId,
        blocker: WidgetId,
    },
    NotHandled {
        target: WidgetId,
        action: SemanticActionKind,
    },
}

impl WidgetTree {
    pub(crate) fn supported_semantic_actions(&self, id: WidgetId) -> Vec<SemanticActionKind> {
        let Some(node) = self.get(id) else {
            return Vec::new();
        };
        let snapshot = node.widget_snapshot(id);
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
        let is_input = node.widget().as_any().is::<Input>();
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
        // 组件声明（E-05）：`widget!` 的 `semantic_actions` 槽位在 role 推断
        // 之外补充自定义能力（如连续值 `Adjust`）。
        for declared in node.widget().declared_semantic_actions() {
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
            actions.push(SemanticActionKind::WrapSelection);
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

    pub(crate) fn blocking_modal_for(&self, id: WidgetId) -> Option<WidgetId> {
        self.overlay_stack
            .top()
            .filter(|entry| entry.is_modal())
            .map(|entry| entry.owner())
            .filter(|&owner| id != owner && !self.is_descendant_of(id, owner))
    }

    pub(crate) fn perform_semantic_action(
        &mut self,
        id: WidgetId,
        action: &SemanticAction,
    ) -> Result<(), SemanticActionError> {
        self.perform_semantic_action_with_focus_policy(id, action, false)
    }

    /// 经 Agent 所有权与动作策略校验后执行语义动作，允许目标窗口位于后台。
    pub(crate) fn perform_agent_semantic_action(
        &mut self,
        id: WidgetId,
        action: &SemanticAction,
    ) -> Result<(), SemanticActionError> {
        self.perform_semantic_action_with_focus_policy(id, action, true)
    }

    fn perform_semantic_action_with_focus_policy(
        &mut self,
        id: WidgetId,
        action: &SemanticAction,
        allow_unfocused_input: bool,
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
        let widget_snapshot = node.widget_snapshot(id);
        let accessibility = widget_snapshot.accessibility();
        // 原生窗口控件的 Invoke 是类型化窗口动作，不应绕经键盘事件再猜测
        // 默认行为。先复制公开枚举，释放节点借用后由 WidgetTree 唯一队列提交。
        let window_control = match widget_snapshot.fields {
            SnapshotFields::WindowControl { control, .. } => Some(control),
            _ => None,
        };
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
                if let Some(control) = window_control {
                    self.pending_window_actions
                        .push(WindowInteractionRegion::window_action(control));
                    EventResult::Handled
                } else {
                    let keyboard = if role == AccessibilityRole::Button {
                        self.focus_and_press(id, KeyCode::Enter, allow_unfocused_input)
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
            }
            SemanticAction::Focus => {
                self.set_keyboard_focus_visible(true);
                self.set_focus(Some(id));
                if self.managers().focus.focused_widget() == Some(id) {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SemanticAction::SetValue(value) => {
                self.set_input_value(id, value, allow_unfocused_input)
            }
            SemanticAction::InsertText(text) => {
                self.set_focus(Some(id));
                if text.is_empty() {
                    EventResult::Handled
                } else {
                    self.dispatch_synthetic_input(
                        &SystemEvent::TextInput { text: text.clone() },
                        allow_unfocused_input,
                    )
                }
            }
            SemanticAction::WrapSelection { prefix, suffix } => {
                self.wrap_input_selection(id, &prefix.clone(), &suffix.clone(), allow_unfocused_input)
            }
            SemanticAction::Toggle => {
                self.focus_and_press(id, KeyCode::Space, allow_unfocused_input)
            }
            SemanticAction::Increment => {
                self.focus_and_press(id, KeyCode::Up, allow_unfocused_input)
            }
            SemanticAction::Decrement => {
                self.focus_and_press(id, KeyCode::Down, allow_unfocused_input)
            }
            // 连续值调整能力（E-05）：执行时聚焦目标并返回 Handled，方向性
            // 步进由 `Increment` / `Decrement` 动作驱动；min/max 已进入语义快照。
            SemanticAction::Adjust { .. } => {
                self.set_focus(Some(id));
                if self.managers().focus.focused_widget() == Some(id) {
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
                    let result = self.dispatch_to(
                        id,
                        &SystemEvent::Wheel {
                            pos: center(visible_bounds),
                            delta: *delta,
                        },
                    );
                    if result == EventResult::NotHandled {
                        // 目标已声明滚动能力（viewport_scroll_offset 门禁）；滚到
                        // 边界或滚动范围未就绪时被夹取成无位移，是动作的合法结果
                        // 而非内部故障——曾按 NotHandled 上报被误映射为
                        // internal command failure。实际位移由调用方经快照核对；
                        // 保留告警以区分边界夹取与滚动链路真正断裂。
                        tracing::warn!(
                            target: "uix::semantic",
                            node = ?id,
                            "semantic scroll produced no movement"
                        );
                    }
                    EventResult::Handled
                }
            }
            SemanticAction::Select(value) => {
                self.select_option(id, role, value, allow_unfocused_input)?
            }
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

    fn focus_and_press(
        &mut self,
        id: WidgetId,
        key: KeyCode,
        allow_unfocused_input: bool,
    ) -> EventResult {
        self.set_focus(Some(id));
        self.press_key(key, allow_unfocused_input)
    }

    fn press_key(&mut self, key: KeyCode, allow_unfocused_input: bool) -> EventResult {
        let down = self.dispatch_synthetic_input(
            &SystemEvent::KeyDown {
                key,
                mods: KeyMod::SYNTHETIC,
            },
            allow_unfocused_input,
        );
        let up = self.dispatch_synthetic_input(
            &SystemEvent::KeyUp {
                key,
                mods: KeyMod::SYNTHETIC,
            },
            allow_unfocused_input,
        );
        if down == EventResult::Handled || up == EventResult::Handled {
            EventResult::Handled
        } else {
            EventResult::NotHandled
        }
    }

    fn dispatch_synthetic_input(
        &mut self,
        event: &SystemEvent,
        allow_unfocused_input: bool,
    ) -> EventResult {
        if allow_unfocused_input {
            self.dispatch_agent_event(event)
        } else {
            self.dispatch_event(event)
        }
    }

    fn select_option(
        &mut self,
        id: WidgetId,
        role: AccessibilityRole,
        value: &str,
        allow_unfocused_input: bool,
    ) -> Result<EventResult, SemanticActionError> {
        let selection = self
            .get(id)
            .map(|node| node.widget_snapshot(id))
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
        if self.managers().focus.focused_widget() != Some(id) {
            return Ok(EventResult::NotHandled);
        }

        if role == AccessibilityRole::Combobox {
            if !selection.expanded
                && self.press_key(KeyCode::Down, allow_unfocused_input) != EventResult::Handled
            {
                return Ok(EventResult::NotHandled);
            }
            let key = if index > current {
                KeyCode::Down
            } else {
                KeyCode::Up
            };
            for _ in 0..current.abs_diff(index) {
                if self.press_key(key, allow_unfocused_input) != EventResult::Handled {
                    let _ = self.press_key(KeyCode::Escape, allow_unfocused_input);
                    return Ok(EventResult::NotHandled);
                }
            }
            let _ = self.press_key(KeyCode::Escape, allow_unfocused_input);
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
                if self.press_key(key, allow_unfocused_input) != EventResult::Handled {
                    return Ok(EventResult::NotHandled);
                }
                current = next;
            }
        }

        let selected = self
            .get(id)
            .map(|node| node.widget_snapshot(id))
            .and_then(|snapshot| snapshot.selection())
            .is_some_and(|selection| selection.selected_indices.as_slice() == [index]);
        Ok(if selected {
            EventResult::Handled
        } else {
            EventResult::NotHandled
        })
    }

    /// 用前缀与后缀包裹目标输入框当前选区；无选区时在光标处插入成对标记。
    /// 光标与选区均为字符索引，替换前统一换算到字节边界；组合编辑作为
    /// 一次完整值替换落盘，保持与既有语义动作相同的焦点语义。
    fn wrap_input_selection(
        &mut self,
        id: WidgetId,
        prefix: &str,
        suffix: &str,
        allow_unfocused_input: bool,
    ) -> EventResult {
        // 先在不可变借用内取出值、光标与选区，释放借用后再执行替换写入。
        let edit = self.get(id).and_then(|node| {
            let input = node.widget().as_any().downcast_ref::<Input>()?;
            let value = input.current_value().to_owned();
            let selection = input.selection.get();
            let caret = input.cursor_char;
            Some((value, selection, caret))
        });
        let Some((value, selection, caret)) = edit else {
            return EventResult::NotHandled;
        };
        // 字符索引到字节偏移的换算；越过末尾视为越界。
        let char_to_byte = |index: usize| {
            if index > value.chars().count() {
                return None;
            }
            Some(
                value
                    .char_indices()
                    .nth(index)
                    .map(|(byte, _)| byte)
                    .unwrap_or(value.len()),
            )
        };
        let replaced = match selection {
            // 有非空选区：包裹选中文本（选区按字符索引归一顺序）。
            Some((start, end)) if start != end => {
                let (start, end) = (start.min(end), end.max(start));
                let (Some(start_byte), Some(end_byte)) =
                    (char_to_byte(start), char_to_byte(end))
                else {
                    return EventResult::NotHandled;
                };
                let mut out =
                    String::with_capacity(value.len() + prefix.len() + suffix.len());
                out.push_str(&value[..start_byte]);
                out.push_str(prefix);
                out.push_str(&value[start_byte..end_byte]);
                out.push_str(suffix);
                out.push_str(&value[end_byte..]);
                out
            }
            // 无选区：在光标处插入成对标记。
            _ => {
                let Some(caret_byte) = char_to_byte(caret) else {
                    return EventResult::NotHandled;
                };
                let mut out =
                    String::with_capacity(value.len() + prefix.len() + suffix.len());
                out.push_str(&value[..caret_byte]);
                out.push_str(prefix);
                out.push_str(suffix);
                out.push_str(&value[caret_byte..]);
                out
            }
        };
        self.set_input_value(id, &replaced, allow_unfocused_input)
    }

    fn set_input_value(
        &mut self,
        id: WidgetId,
        value: &str,
        allow_unfocused_input: bool,
    ) -> EventResult {
        let Some(current_value) = self.get(id).and_then(|node| {
            node.widget()
                .as_any()
                .downcast_ref::<Input>()
                .map(|input| input.current_value().to_owned())
        }) else {
            return EventResult::NotHandled;
        };

        self.set_focus(Some(id));
        let _ = self.dispatch_synthetic_input(
            &SystemEvent::KeyDown {
                key: KeyCode::A,
                mods: KeyMod::CTRL,
            },
            allow_unfocused_input,
        );
        let _ = self.dispatch_synthetic_input(
            &SystemEvent::KeyUp {
                key: KeyCode::A,
                mods: KeyMod::CTRL,
            },
            allow_unfocused_input,
        );
        if value.is_empty() {
            if current_value.is_empty() {
                EventResult::Handled
            } else {
                let down = self.dispatch_synthetic_input(
                    &SystemEvent::KeyDown {
                        key: KeyCode::Backspace,
                        mods: KeyMod::NONE,
                    },
                    allow_unfocused_input,
                );
                let _ = self.dispatch_synthetic_input(
                    &SystemEvent::KeyUp {
                        key: KeyCode::Backspace,
                        mods: KeyMod::NONE,
                    },
                    allow_unfocused_input,
                );
                down
            }
        } else {
            self.dispatch_synthetic_input(
                &SystemEvent::TextInput {
                    text: value.to_owned(),
                },
                allow_unfocused_input,
            )
        }
    }
}

fn center(bounds: crate::core::Rect) -> Point {
    Point::new(bounds.x + bounds.w * 0.5, bounds.y + bounds.h * 0.5)
}
