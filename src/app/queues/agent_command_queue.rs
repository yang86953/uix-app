//! Agent 命令契约与有界队列 — app System 私有边界（SMC-06 P1 修复上移）。
//!
//! 本模块拥有跨 Module 共享的 Agent 命令协议契约（请求 / 响应 / 错误 / 票据）
//! 与有界命令队列。`agent` Module 是实现者（消费本契约并注入执行逻辑），
//! `event_loop` / `window` 与组合根 `session_runtime` 消费本契约，任何 Module
//! 不拥有队列实例（由组合根创建并注入）。

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU8, Ordering};
#[cfg(any(test, feature = "agent-control"))]
use std::sync::mpsc::RecvTimeoutError;
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::app::window_semantics::WindowSemanticSnapshot;
#[cfg(any(test, feature = "agent-control"))]
use crate::core::Point;
use crate::core::{WidgetId, WindowId};
use crate::ui::accessibility::semantic_snapshot::SemanticTarget;
use crate::ui::semantic_action::{SemanticAction, SemanticActionKind};
#[cfg(any(test, feature = "agent-control"))]
use crate::ui::{KeyCode, KeyMod};

/// 每窗口 Agent 命令队列默认容量。
pub(crate) const DEFAULT_AGENT_COMMAND_QUEUE_CAPACITY: usize = 64;
/// in-flight 动作允许的最大 settle 轮数，超限判为 DidNotSettle。
pub(crate) const MAX_AGENT_SETTLE_PASSES: usize = 32;

/// 一次待用户确认的 Agent 动作请求（确认 UI 回调载荷）。
///
/// 应用注入的确认 UI 收到本请求后展示确认界面；用户在界面上的决定通过
/// `AppHandle::resolve_agent_confirmation` 交回框架。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentConfirmationRequest {
    /// 目标窗口。
    pub window_id: crate::core::WindowId,
    /// 一次性确认标识：`resolve_agent_confirmation` 用其定位本次确认。
    pub confirm_id: u64,
    /// 目标描述（automation_id 或节点身份）。
    pub target: String,
    /// 请求的动作名称（语义动作蛇形名）。
    pub action: String,
}

/// 窗口级自动化动作（测试 / agent-control 专用命令载荷）。
#[cfg(any(test, feature = "agent-control"))]
#[derive(Debug, Clone, Copy, PartialEq)]
// 默认库测试不启动 agent 消费者，但仍需保留窗口级动作载荷契约。
#[cfg_attr(test, allow(dead_code))]
pub(crate) enum AgentWindowAction {
    PressKey {
        key: KeyCode,
        modifiers: KeyMod,
    },
    ClickAt {
        position: Point,
    },
    PointerMove {
        position: Point,
    },
    PointerDown {
        position: Point,
    },
    PointerUp {
        position: Point,
    },
    Resize {
        width: i32,
        height: i32,
    },
    Move {
        x: i32,
        y: i32,
    },
    Maximize,
    Minimize,
    Restore,
    /// 请求合成器激活并聚焦窗口；成功只表示请求已提交，不保证已获焦点。
    /// 指针动作不要求焦点，键盘/IME/剪贴板保留焦点要求，自动化驱动键盘前先激活。
    Activate,
    /// 只提交平台关闭请求，不声明窗口已经关闭。
    Close,
}

/// 命令请求契约：快照或执行语义动作。
#[derive(Debug, Clone, PartialEq)]
// 默认库测试不启动 agent 消费者，但仍需保留快照/语义动作请求契约。
#[cfg_attr(test, allow(dead_code))]
pub(crate) enum AgentCommandRequest {
    Snapshot,
    Perform {
        generation: u64,
        expected_revision: Option<u64>,
        target: SemanticTarget,
        action: SemanticAction,
    },
    #[cfg(any(test, feature = "agent-control"))]
    PerformWindow {
        generation: u64,
        expected_revision: Option<u64>,
        action: AgentWindowAction,
    },
    /// Agent 截屏：UI turn 内强制整树重绘，由下一次真实 present 完成回读票据。
    /// 截屏是读取类能力，与快照同级：不校验代际、只读策略放行。
    #[cfg(any(test, feature = "agent-control"))]
    Screenshot,
    /// 用户确认流程：AI 确认执行先前命中 `requires_confirmation` 的动作。
    Confirm {
        confirm_id: u64,
    },
    /// 用户确认流程：应用侧交回用户决定（UI turn 内处理）。
    ResolveConfirmation {
        confirm_id: u64,
        allow: bool,
    },
}

/// 确认流程有效期：确认请求在登记后超过该时长仍未 resolve 视为超时。
pub(crate) const MAX_AGENT_CONFIRM_TTL: Duration = Duration::from_secs(60);

/// 命令响应契约。
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum AgentCommandResponse {
    Snapshot(WindowSemanticSnapshot),
    Performed {
        window_id: WindowId,
        generation: u64,
        revision: u64,
        presented_revision: u64,
        settled: bool,
    },
}

/// 面向传输层的错误码（agent-control 传输协议映射用）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// 默认库测试不启动传输层，但仍需保留错误码枚举供协议映射使用。
#[cfg_attr(test, allow(dead_code))]
pub(crate) enum AgentErrorCode {
    #[cfg(feature = "agent-control")]
    Unauthorized,
    #[cfg(feature = "agent-control")]
    UnsupportedSchema,
    InvalidRequest,
    WindowNotFound,
    StaleWindow,
    #[cfg(any(test, feature = "agent-control"))]
    StaleRevision,
    /// 命令已开始执行但等待响应超时，调用方不能假定动作未发生。
    #[cfg(feature = "agent-control")]
    OutcomeUnknown,
    #[cfg(any(test, feature = "agent-control"))]
    NodeNotFound,
    #[cfg(any(test, feature = "agent-control"))]
    AmbiguousTarget,
    #[cfg(any(test, feature = "agent-control"))]
    UnsupportedAction,
    #[cfg(any(test, feature = "agent-control"))]
    Forbidden,
    #[cfg(any(test, feature = "agent-control"))]
    RequiresConfirmation,
    #[cfg(any(test, feature = "agent-control"))]
    ConfirmationRejected,
    #[cfg(any(test, feature = "agent-control"))]
    ConfirmationNotFound,
    #[cfg(any(test, feature = "agent-control"))]
    InvalidValue,
    #[cfg(any(test, feature = "agent-control"))]
    NotInteractable,
    #[cfg(any(test, feature = "agent-control"))]
    Blocked,
    #[cfg(any(test, feature = "agent-control"))]
    DidNotSettle,
    #[cfg(any(test, feature = "agent-control"))]
    NotPresentable,
    /// 窗口管理动作调用平台窗口失败。
    #[cfg(any(test, feature = "agent-control"))]
    WindowOperationFailed,
    /// 截屏 PNG 编码结果超出协议载荷上限。
    #[cfg(any(test, feature = "agent-control"))]
    PayloadTooLarge,
    Timeout,
    AppClosed,
    #[cfg(any(test, feature = "agent-control"))]
    Internal,
}

// 错误码字符串映射只在 agent transport 或专门 GUI 测试中消费。
#[cfg_attr(test, allow(dead_code))]
impl AgentErrorCode {
    #[cfg(any(test, feature = "agent-control"))]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            #[cfg(feature = "agent-control")]
            Self::Unauthorized => "unauthorized",
            #[cfg(feature = "agent-control")]
            Self::UnsupportedSchema => "unsupported_schema",
            Self::InvalidRequest => "invalid_request",
            Self::WindowNotFound => "window_not_found",
            Self::StaleWindow => "stale_window",
            Self::StaleRevision => "stale_revision",
            #[cfg(feature = "agent-control")]
            Self::OutcomeUnknown => "outcome_unknown",
            Self::NodeNotFound => "node_not_found",
            Self::AmbiguousTarget => "ambiguous_target",
            Self::UnsupportedAction => "unsupported_action",
            Self::Forbidden => "forbidden",
            Self::RequiresConfirmation => "requires_confirmation",
            Self::ConfirmationRejected => "confirmation_rejected",
            Self::ConfirmationNotFound => "confirmation_not_found",
            Self::InvalidValue => "invalid_value",
            Self::NotInteractable => "not_interactable",
            Self::Blocked => "blocked",
            Self::DidNotSettle => "did_not_settle",
            Self::NotPresentable => "not_presentable",
            Self::WindowOperationFailed => "window_operation_failed",
            Self::PayloadTooLarge => "payload_too_large",
            Self::Timeout => "timeout",
            Self::AppClosed => "app_closed",
            Self::Internal => "internal",
        }
    }
}

/// 命令执行错误契约。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentCommandError {
    StaleWindow {
        expected: u64,
        actual: u64,
    },
    StaleRevision {
        expected: u64,
        actual: u64,
    },
    NodeNotFound(String),
    AmbiguousTarget {
        automation_id: String,
        count: usize,
    },
    UnsupportedAction {
        target: String,
        action: SemanticActionKind,
    },
    /// 动作策略拒绝：只读模式、受保护目标或禁止的动作类别。
    Forbidden {
        target: String,
    },
    /// 动作需要用户确认（命中 `require_confirm` 策略）。
    ///
    /// 执行器返回时 `confirm_id` 为 0（占位）；`WindowAgentState` 登记
    /// 待确认动作后以真实一次性标识重建错误回给请求方。
    RequiresConfirmation {
        target: String,
        action: SemanticActionKind,
        confirm_id: u64,
    },
    /// 确认流程被用户拒绝。
    ConfirmationRejected {
        confirm_id: u64,
    },
    /// 确认流程超时或确认不存在（已失效）。
    ConfirmationNotFound {
        confirm_id: u64,
    },
    InvalidValue {
        target: String,
        action: SemanticActionKind,
    },
    NotInteractable(String),
    Blocked {
        target: String,
        blocker: WidgetId,
    },
    DidNotSettle {
        passes: usize,
    },
    /// 窗口管理动作调用平台窗口失败。
    WindowOperationFailed {
        operation: &'static str,
        message: String,
    },
    NotPresentable,
    AppClosed,
    Internal,
}

// 命令错误码映射只在 agent transport 或专门 GUI 测试中消费。
#[cfg_attr(test, allow(dead_code))]
impl AgentCommandError {
    #[cfg(any(test, feature = "agent-control"))]
    pub(crate) const fn code(&self) -> AgentErrorCode {
        match self {
            Self::StaleWindow { .. } => AgentErrorCode::StaleWindow,
            Self::StaleRevision { .. } => AgentErrorCode::StaleRevision,
            Self::NodeNotFound(_) => AgentErrorCode::NodeNotFound,
            Self::AmbiguousTarget { .. } => AgentErrorCode::AmbiguousTarget,
            Self::UnsupportedAction { .. } => AgentErrorCode::UnsupportedAction,
            Self::Forbidden { .. } => AgentErrorCode::Forbidden,
            Self::RequiresConfirmation { .. } => AgentErrorCode::RequiresConfirmation,
            Self::ConfirmationRejected { .. } => AgentErrorCode::ConfirmationRejected,
            Self::ConfirmationNotFound { .. } => AgentErrorCode::ConfirmationNotFound,
            Self::InvalidValue { .. } => AgentErrorCode::InvalidValue,
            Self::NotInteractable(_) => AgentErrorCode::NotInteractable,
            Self::Blocked { .. } => AgentErrorCode::Blocked,
            Self::DidNotSettle { .. } => AgentErrorCode::DidNotSettle,
            Self::WindowOperationFailed { .. } => AgentErrorCode::WindowOperationFailed,
            Self::NotPresentable => AgentErrorCode::NotPresentable,
            Self::AppClosed => AgentErrorCode::AppClosed,
            Self::Internal => AgentErrorCode::Internal,
        }
    }
}

pub(crate) type AgentCommandResult = Result<AgentCommandResponse, AgentCommandError>;

/// 提交命令的失败语义（队列满 / 已关闭 / 窗口不存在 / 回读不可用）。
#[derive(Debug, Clone, PartialEq, Eq)]
// 默认库测试不启动 agent 提交方，但仍需保留提交失败契约。
#[cfg_attr(test, allow(dead_code))]
pub(crate) enum AgentSubmitError {
    WindowNotFound,
    QueueFull,
    AppClosed,
    /// 截屏回读无法安排（无活跃 renderer / 上一请求尚未完成等图形侧原因）。
    #[cfg(any(test, feature = "agent-control"))]
    ReadbackUnavailable { message: String },
}

impl AgentSubmitError {
    #[cfg(feature = "agent-control")]
    pub(crate) fn code(self) -> AgentErrorCode {
        match self {
            Self::WindowNotFound => AgentErrorCode::WindowNotFound,
            Self::QueueFull => AgentErrorCode::Internal,
            Self::AppClosed => AgentErrorCode::AppClosed,
            Self::ReadbackUnavailable { .. } => AgentErrorCode::WindowOperationFailed,
        }
    }
}

/// 提交票据：携带响应通道，可同步等待执行结果。
#[derive(Debug)]
// 默认库测试不启动 agent 提交方，但仍需保留同步响应票据契约。
#[cfg_attr(test, allow(dead_code))]
pub(crate) struct AgentCommandTicket {
    #[cfg_attr(not(any(test, feature = "agent-control")), allow(dead_code))]
    receiver: Receiver<AgentCommandResult>,
    lifecycle: Arc<AgentCommandLifecycle>,
}

const AGENT_COMMAND_PENDING: u8 = 0;
const AGENT_COMMAND_STARTED: u8 = 1;
const AGENT_COMMAND_CANCELLED: u8 = 2;

/// 票据与 UI 队列共享的单向命令生命周期门。
#[derive(Debug)]
struct AgentCommandLifecycle {
    state: AtomicU8,
}

impl AgentCommandLifecycle {
    fn new() -> Self {
        Self {
            state: AtomicU8::new(AGENT_COMMAND_PENDING),
        }
    }

    /// 只有仍待执行的命令能够进入 UI 执行阶段。
    fn try_start(&self) -> bool {
        self.state
            .compare_exchange(
                AGENT_COMMAND_PENDING,
                AGENT_COMMAND_STARTED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }

    /// 只取消尚未开始的命令；已开始时调用方必须按结果未知处理。
    fn cancel_pending(&self) -> bool {
        self.state
            .compare_exchange(
                AGENT_COMMAND_PENDING,
                AGENT_COMMAND_CANCELLED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
    }
}

// 票据等待入口由 agent transport 或专门 GUI 测试按需消费。
#[cfg_attr(test, allow(dead_code))]
impl AgentCommandTicket {
    #[cfg(any(test, feature = "agent-control"))]
    pub(crate) fn recv_timeout(
        &self,
        timeout: Duration,
    ) -> Result<AgentCommandResult, RecvTimeoutError> {
        self.receiver.recv_timeout(timeout)
    }

    /// 超时或连接关闭时尝试取消仍在队列中的命令。
    pub(crate) fn cancel_pending(&self) -> bool {
        self.lifecycle.cancel_pending()
    }
}

impl Drop for AgentCommandTicket {
    fn drop(&mut self) {
        let _ = self.cancel_pending();
    }
}

pub(crate) struct AgentCommandEnvelope {
    pub(crate) request: AgentCommandRequest,
    pub(crate) response: SyncSender<AgentCommandResult>,
    lifecycle: Arc<AgentCommandLifecycle>,
}

impl AgentCommandEnvelope {
    /// 与票据取消竞争；返回 false 表示调用方已放弃且动作不得执行。
    pub(crate) fn try_start(&self) -> bool {
        self.lifecycle.try_start()
    }
}

// 队列容量字段由 agent 提交路径读取，默认库测试不触发该路径。
#[cfg_attr(test, allow(dead_code))]
struct AgentQueueInner {
    pending: VecDeque<AgentCommandEnvelope>,
    capacity: usize,
    closed: bool,
}

/// 每窗口有界命令队列：传输 worker 入队，窗口 UI turn 独占排空。
#[derive(Clone)]
pub(crate) struct AgentCommandQueue {
    inner: Arc<Mutex<AgentQueueInner>>,
}

impl Default for AgentCommandQueue {
    fn default() -> Self {
        Self::new()
    }
}

// 队列提交与消费 API 由 agent transport/窗口循环按需调用。
#[cfg_attr(test, allow(dead_code))]
impl AgentCommandQueue {
    pub(crate) fn new() -> Self {
        Self::with_capacity(DEFAULT_AGENT_COMMAND_QUEUE_CAPACITY)
    }

    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Arc::new(Mutex::new(AgentQueueInner {
                pending: VecDeque::new(),
                capacity: capacity.max(1),
                closed: false,
            })),
        }
    }

    pub(crate) fn submit(
        &self,
        request: AgentCommandRequest,
    ) -> Result<(AgentCommandTicket, bool), AgentSubmitError> {
        let (response, receiver) = sync_channel(1);
        let lifecycle = Arc::new(AgentCommandLifecycle::new());
        let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        if inner.closed {
            return Err(AgentSubmitError::AppClosed);
        }
        if inner.pending.len() >= inner.capacity {
            return Err(AgentSubmitError::QueueFull);
        }
        let should_wake = inner.pending.is_empty();
        inner.pending.push_back(AgentCommandEnvelope {
            request,
            response,
            lifecycle: lifecycle.clone(),
        });
        Ok((
            AgentCommandTicket {
                receiver,
                lifecycle,
            },
            should_wake,
        ))
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .pending
            .is_empty()
    }

    pub(crate) fn len(&self) -> usize {
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .pending
            .len()
    }

    pub(crate) fn pop_front(&self) -> Option<AgentCommandEnvelope> {
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .pending
            .pop_front()
    }

    pub(crate) fn push_front(&self, envelope: AgentCommandEnvelope) {
        let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        if inner.closed {
            drop(inner);
            send_result(envelope.response, Err(AgentCommandError::AppClosed));
        } else {
            inner.pending.push_front(envelope);
        }
    }

    pub(crate) fn close(&self) {
        let pending = {
            let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
            if inner.closed {
                return;
            }
            inner.closed = true;
            std::mem::take(&mut inner.pending)
        };
        for envelope in pending {
            send_result(envelope.response, Err(AgentCommandError::AppClosed));
        }
    }
}

/// 向响应通道发送结果（接收方已关闭时静默丢弃）。
pub(crate) fn send_result(response: SyncSender<AgentCommandResult>, result: AgentCommandResult) {
    let _ = response.send(result);
}
