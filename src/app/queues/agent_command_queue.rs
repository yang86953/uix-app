//! Agent 命令契约与有界队列 — app System 私有边界（SMC-06 P1 修复上移）。
//!
//! 本模块拥有跨 Module 共享的 Agent 命令协议契约（请求 / 响应 / 错误 / 票据）
//! 与有界命令队列。`agent` Module 是实现者（消费本契约并注入执行逻辑），
//! `event_loop` / `window` 与组合根 `session_runtime` 消费本契约，任何 Module
//! 不拥有队列实例（由组合根创建并注入）。

use std::collections::VecDeque;
#[cfg(any(test, feature = "agent-control"))]
use std::sync::mpsc::RecvTimeoutError;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
#[cfg(any(test, feature = "agent-control"))]
use std::time::Duration;

use crate::app::window_semantics::WindowSemanticSnapshot;
#[cfg(any(test, feature = "agent-control"))]
use crate::core::Point;
use crate::core::{ComponentId, WindowId};
use crate::ui::accessibility::semantic_snapshot::SemanticTarget;
use crate::ui::semantic_action::{SemanticAction, SemanticActionKind};
#[cfg(any(test, feature = "agent-control"))]
use crate::ui::{KeyCode, KeyMod};

/// 每窗口 Agent 命令队列默认容量。
pub(crate) const DEFAULT_AGENT_COMMAND_QUEUE_CAPACITY: usize = 64;
/// in-flight 动作允许的最大 settle 轮数，超限判为 DidNotSettle。
pub(crate) const MAX_AGENT_SETTLE_PASSES: usize = 32;

/// 窗口级自动化动作（测试 / agent-control 专用命令载荷）。
#[cfg(any(test, feature = "agent-control"))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum AgentWindowAction {
    PressKey { key: KeyCode, modifiers: KeyMod },
    ClickAt { position: Point },
    PointerMove { position: Point },
    PointerDown { position: Point },
    PointerUp { position: Point },
}

/// 命令请求契约：快照或执行语义动作。
#[derive(Debug, Clone, PartialEq)]
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
}

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
    #[cfg(any(test, feature = "agent-control"))]
    NodeNotFound,
    #[cfg(any(test, feature = "agent-control"))]
    AmbiguousTarget,
    #[cfg(any(test, feature = "agent-control"))]
    UnsupportedAction,
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
    Timeout,
    AppClosed,
    #[cfg(any(test, feature = "agent-control"))]
    Internal,
}

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
            Self::NodeNotFound => "node_not_found",
            Self::AmbiguousTarget => "ambiguous_target",
            Self::UnsupportedAction => "unsupported_action",
            Self::InvalidValue => "invalid_value",
            Self::NotInteractable => "not_interactable",
            Self::Blocked => "blocked",
            Self::DidNotSettle => "did_not_settle",
            Self::NotPresentable => "not_presentable",
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
    InvalidValue {
        target: String,
        action: SemanticActionKind,
    },
    NotInteractable(String),
    Blocked {
        target: String,
        blocker: ComponentId,
    },
    DidNotSettle {
        passes: usize,
    },
    NotPresentable,
    AppClosed,
    Internal,
}

impl AgentCommandError {
    #[cfg(any(test, feature = "agent-control"))]
    pub(crate) const fn code(&self) -> AgentErrorCode {
        match self {
            Self::StaleWindow { .. } => AgentErrorCode::StaleWindow,
            Self::StaleRevision { .. } => AgentErrorCode::StaleRevision,
            Self::NodeNotFound(_) => AgentErrorCode::NodeNotFound,
            Self::AmbiguousTarget { .. } => AgentErrorCode::AmbiguousTarget,
            Self::UnsupportedAction { .. } => AgentErrorCode::UnsupportedAction,
            Self::InvalidValue { .. } => AgentErrorCode::InvalidValue,
            Self::NotInteractable(_) => AgentErrorCode::NotInteractable,
            Self::Blocked { .. } => AgentErrorCode::Blocked,
            Self::DidNotSettle { .. } => AgentErrorCode::DidNotSettle,
            Self::NotPresentable => AgentErrorCode::NotPresentable,
            Self::AppClosed => AgentErrorCode::AppClosed,
            Self::Internal => AgentErrorCode::Internal,
        }
    }
}

pub(crate) type AgentCommandResult = Result<AgentCommandResponse, AgentCommandError>;

/// 提交命令的失败语义（队列满 / 已关闭 / 窗口不存在）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentSubmitError {
    WindowNotFound,
    QueueFull,
    AppClosed,
}

impl AgentSubmitError {
    #[cfg(feature = "agent-control")]
    pub(crate) const fn code(self) -> AgentErrorCode {
        match self {
            Self::WindowNotFound => AgentErrorCode::WindowNotFound,
            Self::QueueFull => AgentErrorCode::Internal,
            Self::AppClosed => AgentErrorCode::AppClosed,
        }
    }
}

/// 提交票据：携带响应通道，可同步等待执行结果。
#[derive(Debug)]
pub(crate) struct AgentCommandTicket {
    #[cfg_attr(not(any(test, feature = "agent-control")), allow(dead_code))]
    receiver: Receiver<AgentCommandResult>,
}

impl AgentCommandTicket {
    #[cfg(any(test, feature = "agent-control"))]
    pub(crate) fn recv_timeout(
        self,
        timeout: Duration,
    ) -> Result<AgentCommandResult, RecvTimeoutError> {
        self.receiver.recv_timeout(timeout)
    }
}

pub(crate) struct AgentCommandEnvelope {
    pub(crate) request: AgentCommandRequest,
    pub(crate) response: SyncSender<AgentCommandResult>,
}

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
        let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        if inner.closed {
            return Err(AgentSubmitError::AppClosed);
        }
        if inner.pending.len() >= inner.capacity {
            return Err(AgentSubmitError::QueueFull);
        }
        let should_wake = inner.pending.is_empty();
        inner
            .pending
            .push_back(AgentCommandEnvelope { request, response });
        Ok((AgentCommandTicket { receiver }, should_wake))
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
