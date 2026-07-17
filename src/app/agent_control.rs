//! Bounded, per-window command ingress for the Agent Bridge.
//!
//! Transport workers may enqueue requests and wait for a response, but only
//! the owning window's UI turn drains this queue and touches `WidgetTree`.

use std::collections::VecDeque;
#[cfg(any(test, feature = "agent-control"))]
use std::sync::mpsc::RecvTimeoutError;
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::{Arc, Mutex};
#[cfg(any(test, feature = "agent-control"))]
use std::time::Duration;

use crate::app::window_semantics::{WindowSemanticSnapshot, WindowSemanticState};
#[cfg(any(test, feature = "agent-control"))]
use crate::core::Point;
use crate::core::{ComponentId, WindowId};
use crate::ui::semantic_action::{SemanticAction, SemanticActionError, SemanticActionKind};
use crate::ui::semantic_snapshot::SemanticTarget;
use crate::ui::WidgetTree;
#[cfg(any(test, feature = "agent-control"))]
use crate::ui::{KeyCode, KeyMod, MouseButton, SystemEvent};

pub(crate) const DEFAULT_AGENT_COMMAND_QUEUE_CAPACITY: usize = 64;
pub(crate) const MAX_AGENT_SETTLE_PASSES: usize = 32;

#[cfg(any(test, feature = "agent-control"))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum AgentWindowAction {
    PressKey { key: KeyCode, modifiers: KeyMod },
    ClickAt { position: Point },
    PointerMove { position: Point },
    PointerDown { position: Point },
    PointerUp { position: Point },
}

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

struct AgentCommandEnvelope {
    request: AgentCommandRequest,
    response: SyncSender<AgentCommandResult>,
}

struct AgentQueueInner {
    pending: VecDeque<AgentCommandEnvelope>,
    capacity: usize,
    closed: bool,
}

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

    fn pop_front(&self) -> Option<AgentCommandEnvelope> {
        self.inner
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .pending
            .pop_front()
    }

    fn push_front(&self, envelope: AgentCommandEnvelope) {
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

struct PendingAgentResponse {
    response: SyncSender<AgentCommandResult>,
}

pub(crate) struct WindowAgentState {
    queue: AgentCommandQueue,
    in_flight: Vec<PendingAgentResponse>,
    settle_passes: usize,
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
        }
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
                } => match perform(
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
                } => match perform_window_action(
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

fn perform(
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
fn perform_window_action(
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

fn send_result(response: SyncSender<AgentCommandResult>, result: AgentCommandResult) {
    let _ = response.send(result);
}
