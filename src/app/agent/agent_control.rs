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
use crate::ui::accessibility::semantic_snapshot::SemanticTarget;
use crate::ui::semantic_action::{SemanticAction, SemanticActionError};
use crate::ui::WidgetTree;
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
                let _ = tree.dispatch_event(&SystemEvent::KeyDown {
                    key,
                    mods: modifiers,
                });
                let _ = tree.dispatch_event(&SystemEvent::KeyUp {
                    key,
                    mods: modifiers,
                });
            }
            AgentWindowAction::ClickAt { position } => {
                let _ = tree.dispatch_event(&SystemEvent::PointerDown {
                    pos: position,
                    button: MouseButton::Left,
                    mods: KeyMod::NONE,
                });
                let _ = tree.dispatch_event(&SystemEvent::PointerUp {
                    pos: position,
                    button: MouseButton::Left,
                    mods: KeyMod::NONE,
                });
            }
            AgentWindowAction::PointerMove { position } => {
                let _ = tree.dispatch_event(&SystemEvent::PointerMove {
                    pos: position,
                    mods: KeyMod::NONE,
                });
            }
            AgentWindowAction::PointerDown { position } => {
                let _ = tree.dispatch_event(&SystemEvent::PointerDown {
                    pos: position,
                    button: MouseButton::Left,
                    mods: KeyMod::NONE,
                });
            }
            AgentWindowAction::PointerUp { position } => {
                let _ = tree.dispatch_event(&SystemEvent::PointerUp {
                    pos: position,
                    button: MouseButton::Left,
                    mods: KeyMod::NONE,
                });
            }
        }
        Ok(())
    }
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

