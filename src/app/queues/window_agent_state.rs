//! 每窗口 Agent 命令状态机 — app System 私有边界（SMC-06 P1 修复上移）。
//!
//! `WindowAgentState` 原为 `agent` Module 私有实现类型，被 `event_loop` /
//! `window` 两兄弟 Module 直接引用，违反「Module 私有实现类型不得成为其他
//! Modules 共同依赖」。修复：状态机与执行契约上移本边界，`agent` Module 只
//! 提供执行实现（`AgentCommandExecutor`），由组合根 `session_runtime` 组装
//! 期注入。

use std::sync::Arc;
use std::sync::mpsc::SyncSender;
use std::time::Instant;

#[cfg(any(test, feature = "agent-control"))]
use crate::app::queues::agent_command_queue::AgentWindowAction;
use crate::app::queues::agent_command_queue::MAX_AGENT_CONFIRM_TTL;
use crate::app::queues::agent_command_queue::{
    AgentCommandError, AgentCommandQueue, AgentCommandRequest, AgentCommandResponse,
    AgentCommandResult, AgentConfirmationRequest, MAX_AGENT_SETTLE_PASSES, send_result,
};
use crate::app::window_semantics::WindowSemanticState;
use crate::ui::WidgetTree;
use crate::ui::accessibility::semantic_snapshot::SemanticTarget;
use crate::ui::semantic_action::SemanticAction;

/// 窗口级 Agent 动作的操作契约：由 window 系统实现，经 UI turn 传入。
///
/// 窗口动作（缩放 / 移动 / 最大化等）必须通过平台窗口执行；执行器不持有
/// 平台窗口引用，由驱动层在每帧 UI turn 内提供借用。契约只在窗口 UI
/// 线程内同步调用，不要求 Send / Sync。
pub(crate) trait AgentWindowOps {
    fn resize(&mut self, width: i32, height: i32) -> Result<(), AgentWindowOpsError>;
    fn move_to(&mut self, x: i32, y: i32) -> Result<(), AgentWindowOpsError>;
    fn maximize(&mut self) -> Result<(), AgentWindowOpsError>;
    fn minimize(&mut self) -> Result<(), AgentWindowOpsError>;
    fn restore(&mut self) -> Result<(), AgentWindowOpsError>;
}

/// 窗口管理动作的平台失败载荷。
#[derive(Debug, Clone)]
pub(crate) struct AgentWindowOpsError {
    pub(crate) operation: &'static str,
    pub(crate) message: String,
}

/// Agent 命令执行契约：由 `agent` Module 实现，组合根组装期注入。
///
/// 执行器是无状态命令处理器：校验代际 / 修订并应用语义动作到 WidgetTree。
pub(crate) trait AgentCommandExecutor: Send + Sync {
    /// 在窗口 UI turn 内执行一次语义动作（命令执行路径）。
    fn perform(
        &self,
        tree: &mut WidgetTree,
        semantic_state: &WindowSemanticState,
        presentable: bool,
        generation: u64,
        expected_revision: Option<u64>,
        target: &SemanticTarget,
        action: &SemanticAction,
    ) -> Result<(), AgentCommandError>;

    /// 在窗口 UI turn 内执行一次窗口级自动化动作（测试 / agent-control 路径）。
    #[cfg(any(test, feature = "agent-control"))]
    fn perform_window(
        &self,
        tree: &mut WidgetTree,
        semantic_state: &WindowSemanticState,
        presentable: bool,
        generation: u64,
        expected_revision: Option<u64>,
        window: &mut dyn AgentWindowOps,
        action: AgentWindowAction,
    ) -> Result<(), AgentCommandError>;
}

struct PendingAgentResponse {
    response: std::sync::mpsc::SyncSender<AgentCommandResult>,
}

/// 待用户确认的 Agent 动作（确认流程状态）。
///
/// `response` 在 AI 发出 `Confirm` 请求后填充；`ResolveConfirmation`
/// 在 UI turn 内交回用户决定并完成该响应。超过 `MAX_AGENT_CONFIRM_TTL`
/// 未 resolve 的确认在下次命令处理时惰性清理为 `confirmation_not_found`。
struct PendingConfirmation {
    confirm_id: u64,
    generation: u64,
    /// 保留原始乐观并发条件，用户确认后仍必须针对同一语义修订执行。
    expected_revision: Option<u64>,
    target: crate::ui::accessibility::semantic_snapshot::SemanticTarget,
    action: crate::ui::semantic_action::SemanticAction,
    created_at: Instant,
    response: Option<std::sync::mpsc::SyncSender<AgentCommandResult>>,
}

/// 每窗口 Agent 命令状态机：有界队列 + in-flight 追踪 + settle 轮次上限。
///
/// 无执行器注入时（独立 event-loop 路径，无 agent 会话）Perform 命令返回
/// `Internal`——该路径不存在任何提交者，正常不会触达。
pub(crate) struct WindowAgentState {
    queue: AgentCommandQueue,
    in_flight: Vec<PendingAgentResponse>,
    settle_passes: usize,
    executor: Option<Arc<dyn AgentCommandExecutor>>,
    /// 待用户确认的动作（确认流程状态机）。
    confirm_pending: Vec<PendingConfirmation>,
    confirm_next_id: u64,
    /// 本状态机所属窗口（组装期注入，确认载荷用）。
    window_id: Option<crate::core::WindowId>,
    /// 应用注入的确认 UI 回调：在 UI turn 内收到待确认请求并展示确认界面。
    confirm_ui: Option<Arc<dyn Fn(AgentConfirmationRequest) + Send + Sync>>,
}

impl Default for WindowAgentState {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowAgentState {
    pub(crate) fn new() -> Self {
        Self {
            queue: AgentCommandQueue::new(),
            in_flight: Vec::new(),
            settle_passes: 0,
            executor: None,
            confirm_pending: Vec::new(),
            confirm_next_id: 1,
            window_id: None,
            confirm_ui: None,
        }
    }

    /// 组装期注入确认 UI 回调（组合根提供，应用配置）。
    pub(crate) fn set_confirm_ui(
        &mut self,
        window_id: crate::core::WindowId,
        handler: Option<Arc<dyn Fn(AgentConfirmationRequest) + Send + Sync>>,
    ) {
        self.window_id = Some(window_id);
        self.confirm_ui = handler.map(|handler| {
            let wrapped: Arc<dyn Fn(AgentConfirmationRequest) + Send + Sync> =
                Arc::new(move |request: AgentConfirmationRequest| {
                    handler(AgentConfirmationRequest {
                        window_id,
                        ..request
                    })
                });
            wrapped
        });
    }

    /// 组装期注入执行器（组合根调用；替换队列会先关闭旧队列）。
    pub(crate) fn set_executor(&mut self, executor: Arc<dyn AgentCommandExecutor>) {
        self.executor = Some(executor);
    }

    pub(crate) fn replace_queue(&mut self, queue: AgentCommandQueue) {
        self.close();
        self.queue = queue;
    }

    pub(crate) fn has_work(&self) -> bool {
        !self.in_flight.is_empty() || !self.queue.is_empty()
    }

    pub(crate) fn has_in_flight(&self) -> bool {
        !self.in_flight.is_empty()
    }

    pub(crate) fn drain_ready(
        &mut self,
        tree: &mut WidgetTree,
        semantic_state: &mut WindowSemanticState,
        presentable: bool,
        #[cfg_attr(not(any(test, feature = "agent-control")), allow(unused_variables))]
        window: &mut dyn AgentWindowOps,
    ) -> bool {
        if !self.in_flight.is_empty() {
            return false;
        }

        let mut did_work = false;
        let mut batched_action = false;
        let drain_budget = self.queue.len();
        for _ in 0..drain_budget {
            let Some(envelope) = self.queue.pop_front() else {
                break;
            };
            if batched_action && matches!(envelope.request, AgentCommandRequest::Snapshot) {
                self.queue.push_front(envelope);
                break;
            }
            if !envelope.try_start() {
                continue;
            }
            did_work = true;
            semantic_state.enable(tree);
            semantic_state.refresh(tree);
            match envelope.request {
                AgentCommandRequest::Snapshot => {
                    let result = semantic_state
                        .snapshot()
                        .cloned()
                        .map(AgentCommandResponse::Snapshot)
                        .ok_or(AgentCommandError::Internal);
                    send_result(envelope.response, result);
                }
                AgentCommandRequest::Perform {
                    generation,
                    expected_revision,
                    target,
                    action,
                } => match self.perform_command(
                    tree,
                    semantic_state,
                    presentable,
                    generation,
                    expected_revision,
                    &target,
                    &action,
                ) {
                    Ok(()) => {
                        batched_action = true;
                        self.in_flight.push(PendingAgentResponse {
                            response: envelope.response,
                        });
                    }
                    // 命中确认策略：登记待确认动作，错误携带一次性 confirm_id。
                    Err(AgentCommandError::RequiresConfirmation {
                        target: _,
                        action: _,
                        confirm_id: _,
                    }) => {
                        let confirm_id = self.next_confirm_id();
                        let target_label = target.label();
                        let action_kind = action.kind();
                        self.confirm_pending.push(PendingConfirmation {
                            confirm_id,
                            generation,
                            expected_revision,
                            target,
                            action,
                            created_at: Instant::now(),
                            response: None,
                        });
                        send_result(
                            envelope.response,
                            Err(AgentCommandError::RequiresConfirmation {
                                target: target_label,
                                action: action_kind,
                                confirm_id,
                            }),
                        );
                    }
                    Err(error) => send_result(envelope.response, Err(error)),
                },
                #[cfg(any(test, feature = "agent-control"))]
                AgentCommandRequest::PerformWindow {
                    generation,
                    expected_revision,
                    action,
                } => match self.perform_window_command(
                    tree,
                    semantic_state,
                    presentable,
                    generation,
                    expected_revision,
                    window,
                    action,
                ) {
                    Ok(()) => {
                        batched_action = true;
                        self.in_flight.push(PendingAgentResponse {
                            response: envelope.response,
                        });
                    }
                    Err(error) => send_result(envelope.response, Err(error)),
                },
                AgentCommandRequest::Confirm { confirm_id } => {
                    self.handle_confirm_request(envelope.response, confirm_id);
                }
                AgentCommandRequest::ResolveConfirmation { confirm_id, allow } => {
                    self.handle_resolve_confirmation(
                        tree,
                        semantic_state,
                        presentable,
                        envelope.response,
                        confirm_id,
                        allow,
                    );
                }
            }
        }

        if !self.in_flight.is_empty() {
            self.settle_passes = 0;
        }
        did_work
    }

    fn perform_command(
        &self,
        tree: &mut WidgetTree,
        semantic_state: &WindowSemanticState,
        presentable: bool,
        generation: u64,
        expected_revision: Option<u64>,
        target: &SemanticTarget,
        action: &SemanticAction,
    ) -> Result<(), AgentCommandError> {
        match self.executor.as_deref() {
            Some(executor) => executor.perform(
                tree,
                semantic_state,
                presentable,
                generation,
                expected_revision,
                target,
                action,
            ),
            None => Err(AgentCommandError::Internal),
        }
    }

    #[cfg(any(test, feature = "agent-control"))]
    fn perform_window_command(
        &self,
        tree: &mut WidgetTree,
        semantic_state: &WindowSemanticState,
        presentable: bool,
        generation: u64,
        expected_revision: Option<u64>,
        window: &mut dyn AgentWindowOps,
        action: AgentWindowAction,
    ) -> Result<(), AgentCommandError> {
        match self.executor.as_deref() {
            Some(executor) => executor.perform_window(
                tree,
                semantic_state,
                presentable,
                generation,
                expected_revision,
                window,
                action,
            ),
            None => Err(AgentCommandError::Internal),
        }
    }

    pub(crate) fn finish_or_defer(
        &mut self,
        semantic_state: &WindowSemanticState,
        synchronous_work_pending: bool,
    ) -> bool {
        if self.in_flight.is_empty() {
            return false;
        }
        if synchronous_work_pending {
            self.settle_passes = self.settle_passes.saturating_add(1);
            if self.settle_passes < MAX_AGENT_SETTLE_PASSES {
                return false;
            }
            self.finish_all(Err(AgentCommandError::DidNotSettle {
                passes: self.settle_passes,
            }));
            return true;
        }

        let result = semantic_state.snapshot().map_or_else(
            || Err(AgentCommandError::Internal),
            |snapshot| {
                Ok(AgentCommandResponse::Performed {
                    window_id: snapshot.window_id,
                    generation: snapshot.generation,
                    revision: snapshot.revision,
                    presented_revision: snapshot.presented_revision,
                    settled: true,
                })
            },
        );
        self.finish_all(result);
        true
    }

    pub(crate) fn fail_not_presentable(&mut self) {
        if !self.in_flight.is_empty() {
            self.finish_all(Err(AgentCommandError::NotPresentable));
        }
        self.fail_confirmations();
    }

    pub(crate) fn close(&mut self) {
        self.queue.close();
        if !self.in_flight.is_empty() {
            self.finish_all(Err(AgentCommandError::AppClosed));
        }
        self.fail_confirmations();
    }

    /// AI 确认执行：挂起响应并调用应用注入的确认 UI（UI turn 内）。
    fn handle_confirm_request(
        &mut self,
        response: SyncSender<AgentCommandResult>,
        confirm_id: u64,
    ) {
        // 惰性清理：过期的待确认动作直接失效。
        self.expire_confirmations();
        let Some(index) = self
            .confirm_pending
            .iter()
            .position(|pending| pending.confirm_id == confirm_id)
        else {
            send_result(
                response,
                Err(AgentCommandError::ConfirmationNotFound { confirm_id }),
            );
            return;
        };
        // 重复确认请求拒绝：一次确认流程只接受一次 Confirm。
        if self.confirm_pending[index].response.is_some() {
            send_result(
                response,
                Err(AgentCommandError::ConfirmationNotFound { confirm_id }),
            );
            return;
        }
        // 未注入确认 UI：立即失败并移除登记，避免 ticket 永久挂起。
        let Some(handler) = self.confirm_ui.clone() else {
            self.confirm_pending.remove(index);
            send_result(
                response,
                Err(AgentCommandError::ConfirmationNotFound { confirm_id }),
            );
            return;
        };
        let request = AgentConfirmationRequest {
            window_id: self.window_id.unwrap_or(crate::core::WindowId::ROOT),
            confirm_id,
            target: self.confirm_pending[index].target.label(),
            action: self.confirm_pending[index]
                .action
                .kind()
                .as_str()
                .to_owned(),
        };
        self.confirm_pending[index].response = Some(response);
        handler(request);
    }

    /// 应用侧交回用户决定：允许则执行登记的动作，拒绝则完成拒绝错误。
    fn handle_resolve_confirmation(
        &mut self,
        tree: &mut WidgetTree,
        semantic_state: &WindowSemanticState,
        presentable: bool,
        response: SyncSender<AgentCommandResult>,
        confirm_id: u64,
        allow: bool,
    ) {
        // 用户决定也必须遵守确认 TTL；不能因确认 UI 已展示就无限延长授权窗口。
        self.expire_confirmations();
        let Some(index) = self
            .confirm_pending
            .iter()
            .position(|pending| pending.confirm_id == confirm_id)
        else {
            send_result(
                response,
                Err(AgentCommandError::ConfirmationNotFound { confirm_id }),
            );
            return;
        };
        let mut pending = self.confirm_pending.remove(index);
        if !allow {
            if let Some(ticket_response) = pending.response.take() {
                send_result(
                    ticket_response,
                    Err(AgentCommandError::ConfirmationRejected { confirm_id }),
                );
            }
            let result = semantic_state.snapshot().map_or_else(
                || Err(AgentCommandError::Internal),
                |snapshot| {
                    Ok(AgentCommandResponse::Performed {
                        window_id: snapshot.window_id,
                        generation: snapshot.generation,
                        revision: snapshot.revision,
                        presented_revision: snapshot.presented_revision,
                        settled: true,
                    })
                },
            );
            send_result(response, result);
            return;
        }
        // 允许：在 UI turn 内执行登记的动作；成功进入 in-flight settle。
        match self.perform_command(
            tree,
            semantic_state,
            presentable,
            pending.generation,
            pending.expected_revision,
            &pending.target,
            &pending.action,
        ) {
            Ok(()) => {
                self.in_flight.push(PendingAgentResponse { response });
                if let Some(ticket_response) = pending.response.take() {
                    self.in_flight.push(PendingAgentResponse {
                        response: ticket_response,
                    });
                }
            }
            Err(error) => {
                if let Some(ticket_response) = pending.response.take() {
                    send_result(ticket_response, Err(error.clone()));
                }
                send_result(response, Err(error));
            }
        }
    }

    fn next_confirm_id(&mut self) -> u64 {
        let id = self.confirm_next_id;
        self.confirm_next_id = self.confirm_next_id.saturating_add(1).max(1);
        id
    }

    /// 惰性清理过期确认：有挂起响应的完成 `confirmation_not_found`。
    fn expire_confirmations(&mut self) {
        let expired = self
            .confirm_pending
            .iter()
            .filter(|pending| pending.created_at.elapsed() > MAX_AGENT_CONFIRM_TTL)
            .map(|pending| pending.confirm_id)
            .collect::<Vec<_>>();
        for confirm_id in expired {
            let Some(index) = self
                .confirm_pending
                .iter()
                .position(|pending| pending.confirm_id == confirm_id)
            else {
                continue;
            };
            let mut pending = self.confirm_pending.remove(index);
            if let Some(ticket_response) = pending.response.take() {
                send_result(
                    ticket_response,
                    Err(AgentCommandError::ConfirmationNotFound { confirm_id }),
                );
            }
        }
    }

    fn fail_confirmations(&mut self) {
        for mut pending in self.confirm_pending.drain(..) {
            if let Some(ticket_response) = pending.response.take() {
                send_result(
                    ticket_response,
                    Err(AgentCommandError::ConfirmationNotFound {
                        confirm_id: pending.confirm_id,
                    }),
                );
            }
        }
    }

    fn finish_all(&mut self, result: AgentCommandResult) {
        for pending in self.in_flight.drain(..) {
            send_result(pending.response, result.clone());
        }
        self.settle_passes = 0;
    }
}

#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../tests/unit/app/queues/window_agent_state__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
