//! Agent 命令执行 — 语义动作应用到 WidgetTree。
//!
//! 本模块是 `agent` Module 对 System 私有边界执行契约
//! （`queues::window_agent_state::AgentCommandExecutor`）的实现：校验代际 /
//! 修订后把语义动作应用到窗口 UI turn 的 WidgetTree。命令队列与状态机
//! （`AgentCommandQueue` / `WindowAgentState`）归 System 私有边界所有，由
//! 组合根 `session_runtime` 组装期注入本实现。

use crate::app::agent::agent_policy::{AgentPolicy, PolicyDecision};
use crate::app::queues::agent_command_queue::AgentCommandError;
#[cfg(any(test, feature = "agent-control"))]
use crate::app::queues::agent_command_queue::AgentWindowAction;
#[cfg(any(test, feature = "agent-control"))]
use crate::app::queues::window_agent_state::AgentWindowOps;
use crate::app::queues::window_agent_state::{AgentCommandExecutor, AgentWindowOpsError};
use crate::app::window_semantics::{WindowSemanticSnapshot, WindowSemanticState};
use crate::core::WidgetId;
use crate::ui::WidgetTree;
use crate::ui::accessibility::semantic_snapshot::SemanticTarget;
use crate::ui::semantic_action::{SemanticAction, SemanticActionError};
#[cfg(any(test, feature = "agent-control"))]
use crate::ui::{KeyMod, MouseButton, SystemEvent};

/// 无状态命令执行器：由组合根创建，注入每窗口 `WindowAgentState`。
/// 持有动作策略（授权第二层），在 UI turn 入口检查后放行动作。
pub(crate) struct AgentCommandExecutorImpl {
    policy: std::sync::Arc<AgentPolicy>,
    isolated: bool,
    confirmation_approved: bool,
}

impl AgentCommandExecutorImpl {
    pub(crate) fn new(policy: std::sync::Arc<AgentPolicy>) -> Self {
        Self {
            policy,
            isolated: false,
            confirmation_approved: false,
        }
    }
    #[cfg(feature = "agent-control")]
    pub(crate) fn isolated(policy: std::sync::Arc<AgentPolicy>) -> Self {
        Self {
            policy,
            isolated: true,
            confirmation_approved: false,
        }
    }
}

impl AgentCommandExecutor for AgentCommandExecutorImpl {
    fn perform(
        &self,
        tree: &mut WidgetTree,
        semantic_state: &WindowSemanticState,
        _presentable: bool,
        generation: u64,
        expected_revision: Option<u64>,
        target: &SemanticTarget,
        action: &SemanticAction,
    ) -> Result<(), AgentCommandError> {
        let snapshot = validate_command(semantic_state, generation, expected_revision)?;
        let node_id = resolve_target(snapshot, target)?;
        if self.isolated
            && matches!(action, SemanticAction::Invoke)
            && tree.get(node_id).is_some_and(|node| {
                node.widget_snapshot(node_id).fields.invoke_window_action().is_some()
            })
        {
            return Err(AgentCommandError::UnsupportedAction {
                target: target.label(),
                action: action.kind(),
            });
        }
        // 动作策略检查：受保护目标 / 禁止动作 / 只读模式在进入 UI 语义路径前拒绝；
        // 需要确认的目标登记确认流程（confirm_id 由状态机分配）。
        let automation_id = snapshot
            .nodes
            .iter()
            .find(|node| node.id == node_id)
            .and_then(|node| node.automation_id.as_deref());
        match self.policy.check_semantic(automation_id, action.kind()) {
            PolicyDecision::Forbidden => {
                return Err(AgentCommandError::Forbidden {
                    target: target.label(),
                });
            }
            PolicyDecision::RequiresConfirmation if !self.confirmation_approved => {
                return Err(AgentCommandError::RequiresConfirmation {
                    target: target.label(),
                    action: action.kind(),
                    confirm_id: 0,
                });
            }
            PolicyDecision::Allow | PolicyDecision::RequiresConfirmation => {}
        }
        // Agent 已经过连接、调用者与动作策略门禁；专用入口允许后台窗口接收
        // 合成键盘与文本事件，同时不放宽普通应用输入的前台焦点约束。
        let outcome = tree
            .perform_agent_semantic_action(node_id, action)
            .map_err(|error| map_action_error(target.label(), error));
        reject_desktop_actions(self.isolated, tree)?;
        outcome
    }

    fn perform_confirmed(
        &self,
        tree: &mut WidgetTree,
        semantic_state: &WindowSemanticState,
        presentable: bool,
        generation: u64,
        expected_revision: Option<u64>,
        target: &SemanticTarget,
        action: &SemanticAction,
    ) -> Result<(), AgentCommandError> {
        // 批准仅存在于这一动作调用，不能修改全局策略或免除其他目标确认。
        Self {
            policy: self.policy.clone(),
            isolated: self.isolated,
            confirmation_approved: true,
        }
        .perform(
            tree,
            semantic_state,
            presentable,
            generation,
            expected_revision,
            target,
            action,
        )
    }

    #[cfg(any(test, feature = "agent-control"))]
    fn perform_window(
        &self,
        tree: &mut WidgetTree,
        semantic_state: &WindowSemanticState,
        _presentable: bool,
        generation: u64,
        expected_revision: Option<u64>,
        window: &mut dyn AgentWindowOps,
        action: AgentWindowAction,
    ) -> Result<(), AgentCommandError> {
        validate_command(semantic_state, generation, expected_revision)?;
        // 窗口动作都是写操作：只读策略拒绝全部窗口动作。
        if self.policy.check_window() == PolicyDecision::Forbidden {
            return Err(AgentCommandError::Forbidden {
                target: format!("window {action:?}"),
            });
        }
        let outcome = (|| {
            match action {
                AgentWindowAction::PressKey { key, modifiers } => {
                    // 始终补齐按下与抬起；输入框通常只消费 KeyDown。
                    let down = tree.dispatch_agent_event(&SystemEvent::KeyDown {
                        key,
                        mods: modifiers,
                    });
                    let up = tree.dispatch_agent_event(&SystemEvent::KeyUp {
                        key,
                        mods: modifiers,
                    });
                    // 已消费或观察的完整序列成功；两段都无人处理仍明确拒绝。
                    if down == crate::ui::EventResult::NotHandled
                        && up == crate::ui::EventResult::NotHandled
                    {
                        return Err(AgentCommandError::NotInteractable(format!(
                            "press_key {key:?}"
                        )));
                    }
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
                    // 指针移动的动作效果是悬停事实更新（enter/leave 与悬停目标切换），
                    // 由树在分发过程中同步完成；是否仍有组件继续消费 move 不改变该事实，
                    // 因此不以 `Handled` 判定成败，避免悬停生效却被报告为失败。
                    let _ = tree.dispatch_agent_event(&SystemEvent::PointerMove {
                        pos: position,
                        mods: KeyMod::NONE,
                    });
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
                // 窗口管理动作：经平台窗口操作契约执行，失败映射为命令失败。
                AgentWindowAction::Resize { width, height } => {
                    window.resize(width, height).map_err(map_window_ops_error)?
                }
                AgentWindowAction::Move { x, y } => {
                    window.move_to(x, y).map_err(map_window_ops_error)?
                }
                AgentWindowAction::Maximize => window.maximize().map_err(map_window_ops_error)?,
                AgentWindowAction::Minimize => window.minimize().map_err(map_window_ops_error)?,
                AgentWindowAction::Restore => window.restore().map_err(map_window_ops_error)?,
                // 激活请求经平台 raise 契约提交（Wayland 走 xdg-activation）；
                // 成功只表示请求已建立；Agent 定向输入本身不依赖该前台焦点。
                AgentWindowAction::Activate => window.raise().map_err(map_window_ops_error)?,
                // 关闭只提交平台请求；实际关闭由后续窗口生命周期事实证明。
                AgentWindowAction::Close => window.request_close().map_err(map_window_ops_error)?,
            }
            Ok(())
        })();
        reject_desktop_actions(self.isolated, tree)?;
        outcome
    }

    #[cfg(any(test, feature = "agent-control"))]
    fn screenshot(
        &self,
        tree: &mut WidgetTree,
        presentable: bool,
    ) -> Result<(), AgentCommandError> {
        // 截屏是读取类能力：只读与目标动作策略不适用于它，与语义快照同级。
        if !presentable {
            return Err(AgentCommandError::NotPresentable);
        }
        // 强制整树重绘：settle 内的下一次真实 present 才会完成回读票据；
        // 空闲窗口没有失效就没有呈现帧，截屏会退化为纯等待。
        if let Some(root) = tree.root_id() {
            tree.invalidate_paint_subtree(root);
        }
        Ok(())
    }
}

// 组件回调也可能排入原生动作；离屏执行器必须清除并显式拒绝，不能伪报桌面成功。
fn reject_desktop_actions(isolated: bool, tree: &mut WidgetTree) -> Result<(), AgentCommandError> {
    if isolated && !tree.take_window_actions().is_empty() {
        return Err(AgentCommandError::WindowOperationFailed {
            operation: "desktop_interaction",
            message: "desktop operations are unavailable; callback-local effects may already have occurred".into(),
        });
    }
    Ok(())
}

fn map_window_ops_error(error: AgentWindowOpsError) -> AgentCommandError {
    AgentCommandError::WindowOperationFailed {
        operation: error.operation,
        message: error.message,
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
    if tree.dispatch_agent_event(event) == crate::ui::event::EventResult::NotHandled {
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
) -> Result<WidgetId, AgentCommandError> {
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
