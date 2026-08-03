//! 每窗口 Agent 命令状态机 — app System 私有边界（SMC-06 P1 修复上移）。
//!
//! `WindowAgentState` 原为 `agent` Module 私有实现类型，被 `event_loop` /
//! `window` 两兄弟 Module 直接引用，违反「Module 私有实现类型不得成为其他
//! Modules 共同依赖」。修复：状态机与执行契约上移本边界，`agent` Module 只
//! 提供执行实现（`AgentCommandExecutor`），由组合根 `session_runtime` 组装
//! 期注入。

use std::sync::Arc;

use crate::app::queues::agent_command_queue::{
    send_result, AgentCommandError, AgentCommandQueue, AgentCommandRequest,
    AgentCommandResponse, AgentCommandResult, MAX_AGENT_SETTLE_PASSES,
};
#[cfg(any(test, feature = "agent-control"))]
use crate::app::queues::agent_command_queue::AgentWindowAction;
use crate::app::window_semantics::WindowSemanticState;
use crate::ui::accessibility::semantic_snapshot::SemanticTarget;
use crate::ui::semantic_action::SemanticAction;
use crate::ui::WidgetTree;

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
        action: AgentWindowAction,
    ) -> Result<(), AgentCommandError>;
}

struct PendingAgentResponse {
    response: std::sync::mpsc::SyncSender<AgentCommandResult>,
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
        }
    }

    /// 组装期注入执行器（组合根调用；替换队列会先关闭旧队列）。
    pub(crate) fn set_executor(&mut self, executor: Arc<dyn AgentCommandExecutor>) {
        self.executor = Some(executor);
    }

    pub(crate) fn replace_queue(&mut self, queue: AgentCommandQueue) {
        self.close();
        self.queue = queue;
    }

    pub(crate) fn queue(&self) -> AgentCommandQueue {
        self.queue.clone()
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
        action: AgentWindowAction,
    ) -> Result<(), AgentCommandError> {
        match self.executor.as_deref() {
            Some(executor) => executor.perform_window(
                tree,
                semantic_state,
                presentable,
                generation,
                expected_revision,
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
    }

    pub(crate) fn close(&mut self) {
        self.queue.close();
        if !self.in_flight.is_empty() {
            self.finish_all(Err(AgentCommandError::AppClosed));
        }
    }

    fn finish_all(&mut self, result: AgentCommandResult) {
        for pending in self.in_flight.drain(..) {
            send_result(pending.response, result.clone());
        }
        self.settle_passes = 0;
    }
}


