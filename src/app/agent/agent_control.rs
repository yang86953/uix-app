//! Agent 命令执行 — 语义动作应用到 WidgetTree。
//!
//! 本模块是 `agent` Module 对 System 私有边界执行契约
//! （`queues::window_agent_state::AgentCommandExecutor`）的实现：校验代际 /
//! 修订后把语义动作应用到窗口 UI turn 的 WidgetTree。命令队列与状态机
//! （`AgentCommandQueue` / `WindowAgentState`）归 System 私有边界所有，由
//! 组合根 `session_runtime` 组装期注入本实现。

use crate::app::queues::agent_command_queue::AgentCommandError;
#[cfg(any(test, feature = "agent-control"))]
use crate::app::queues::agent_command_queue::AgentWindowAction;
use crate::app::queues::window_agent_state::AgentCommandExecutor;
use crate::app::window_semantics::{WindowSemanticSnapshot, WindowSemanticState};
use crate::core::ComponentId;
use crate::ui::WidgetTree;
use crate::ui::accessibility::semantic_snapshot::SemanticTarget;
use crate::ui::semantic_action::{SemanticAction, SemanticActionError};
#[cfg(any(test, feature = "agent-control"))]
use crate::ui::{KeyMod, MouseButton, SystemEvent};

/// 无状态命令执行器：由组合根创建，注入每窗口 `WindowAgentState`。
pub(crate) struct AgentCommandExecutorImpl;

impl AgentCommandExecutor for AgentCommandExecutorImpl {
    fn perform(
        &self,
        tree: &mut WidgetTree,
        semantic_state: &WindowSemanticState,
        presentable: bool,
        generation: u64,
        expected_revision: Option<u64>,
        target: &SemanticTarget,
        action: &SemanticAction,
    ) -> Result<(), AgentCommandError> {
        let node_id = resolve_target(
            validate_command(semantic_state, generation, expected_revision)?,
            target,
        )?;
        if !presentable {
            return Err(AgentCommandError::NotPresentable);
        }
        tree.perform_semantic_action(node_id, action)
            .map_err(|error| map_action_error(target.label(), error))
    }

    #[cfg(any(test, feature = "agent-control"))]
    fn perform_window(
        &self,
        tree: &mut WidgetTree,
        semantic_state: &WindowSemanticState,
        presentable: bool,
        generation: u64,
        expected_revision: Option<u64>,
        action: AgentWindowAction,
    ) -> Result<(), AgentCommandError> {
        validate_command(semantic_state, generation, expected_revision)?;
        if !presentable {
            return Err(AgentCommandError::NotPresentable);
        }

        match action {
            AgentWindowAction::PressKey { key, modifiers } => {
                // KeyDown 未被任何组件消费（如无焦点组件）时命令必须失败，
                // 静默忽略会让 agent 误以为按键已生效（假成功）。
                dispatch_window_event(
                    tree,
                    &SystemEvent::KeyDown {
                        key,
                        mods: modifiers,
                    },
                )?;
                // KeyUp 与 KeyDown 同样要求被消费，保证按键序列完整生效。
                dispatch_window_event(
                    tree,
                    &SystemEvent::KeyUp {
                        key,
                        mods: modifiers,
                    },
                )?;
            }
            AgentWindowAction::ClickAt { position } => {
                // 点击落在空白处（无组件消费）对 agent 而言动作未生效，报告失败。
                dispatch_window_event(
                    tree,
                    &SystemEvent::PointerDown {
                        pos: position,
                        button: MouseButton::Left,
                        mods: KeyMod::NONE,
                    },
                )?;
                dispatch_window_event(
                    tree,
                    &SystemEvent::PointerUp {
                        pos: position,
                        button: MouseButton::Left,
                        mods: KeyMod::NONE,
                    },
                )?;
            }
            AgentWindowAction::PointerMove { position } => {
                // 指针移动无组件接收同样视为动作未生效，避免假成功。
                dispatch_window_event(
                    tree,
                    &SystemEvent::PointerMove {
                        pos: position,
                        mods: KeyMod::NONE,
                    },
                )?;
            }
            AgentWindowAction::PointerDown { position } => {
                dispatch_window_event(
                    tree,
                    &SystemEvent::PointerDown {
                        pos: position,
                        button: MouseButton::Left,
                        mods: KeyMod::NONE,
                    },
                )?;
            }
            AgentWindowAction::PointerUp { position } => {
                dispatch_window_event(
                    tree,
                    &SystemEvent::PointerUp {
                        pos: position,
                        button: MouseButton::Left,
                        mods: KeyMod::NONE,
                    },
                )?;
            }
        }
        Ok(())
    }
}

/// 派发窗口级输入事件；事件未被任何组件消费时映射为命令失败。
///
/// 窗口动作没有语义目标，`dispatch_event` 返回 `NotHandled` 表示动作未生效；
/// 此前静默忽略会让 agent 得到假成功。事件未处理（无接收方）映射为
/// `NotInteractable`（携带事件描述）；树已停止等系统级不可用同样表现为
/// `NotHandled`，一并按失败处理，避免与可交互目标区分过细。
#[cfg(any(test, feature = "agent-control"))]
fn dispatch_window_event(
    tree: &mut WidgetTree,
    event: &SystemEvent,
) -> Result<(), AgentCommandError> {
    if tree.dispatch_event(event) == crate::ui::event::EventResult::NotHandled {
        // 事件没有可交互的接收方：向调用方报告动作未生效。
        return Err(AgentCommandError::NotInteractable(format!("{event:?}")));
    }
    Ok(())
}

fn validate_command(
    semantic_state: &WindowSemanticState,
    generation: u64,
    expected_revision: Option<u64>,
) -> Result<&WindowSemanticSnapshot, AgentCommandError> {
    let snapshot = semantic_state
        .snapshot()
        .ok_or(AgentCommandError::Internal)?;
    if generation != snapshot.generation {
        return Err(AgentCommandError::StaleWindow {
            expected: generation,
            actual: snapshot.generation,
        });
    }
    if let Some(expected) = expected_revision {
        if expected != snapshot.revision {
            return Err(AgentCommandError::StaleRevision {
                expected,
                actual: snapshot.revision,
            });
        }
    }
    Ok(snapshot)
}

fn resolve_target(
    snapshot: &WindowSemanticSnapshot,
    target: &SemanticTarget,
) -> Result<ComponentId, AgentCommandError> {
    match target {
        SemanticTarget::NodeId(id) => snapshot
            .nodes
            .iter()
            .find(|node| node.id == *id)
            .map(|node| node.id)
            .ok_or_else(|| AgentCommandError::NodeNotFound(id.to_string())),
        SemanticTarget::AutomationId(automation_id) => {
            let mut matches = snapshot
                .nodes
                .iter()
                .filter(|node| node.automation_id.as_deref() == Some(automation_id));
            let Some(first) = matches.next() else {
                return Err(AgentCommandError::NodeNotFound(automation_id.clone()));
            };
            let count = 1 + matches.count();
            if count > 1 {
                Err(AgentCommandError::AmbiguousTarget {
                    automation_id: automation_id.clone(),
                    count,
                })
            } else {
                Ok(first.id)
            }
        }
    }
}

fn map_action_error(target: String, error: SemanticActionError) -> AgentCommandError {
    match error {
        SemanticActionError::NodeNotFound(_) => AgentCommandError::NodeNotFound(target),
        SemanticActionError::UnsupportedAction { action, .. } => {
            AgentCommandError::UnsupportedAction { target, action }
        }
        SemanticActionError::NotVisible(_) | SemanticActionError::Disabled(_) => {
            AgentCommandError::NotInteractable(target)
        }
        SemanticActionError::SelectionDisabled { .. } => AgentCommandError::NotInteractable(target),
        SemanticActionError::InvalidValue { action, .. } => {
            AgentCommandError::InvalidValue { target, action }
        }
        SemanticActionError::Blocked { blocker, .. } => {
            AgentCommandError::Blocked { target, blocker }
        }
        SemanticActionError::NotHandled { .. } => AgentCommandError::Internal,
    }
}
