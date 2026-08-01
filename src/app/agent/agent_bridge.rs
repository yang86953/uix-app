//! Process-wide routing and window discovery for the opt-in Agent Bridge.
//!
//! This layer is transport-neutral. The native transport owns connection
//! authentication and JSON framing, then delegates authenticated requests to
//! [`AgentProcessBridge`]. It never receives a `WidgetTree` reference.

#![cfg_attr(not(any(test, feature = "agent-control")), allow(dead_code))]

use std::collections::BTreeMap;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

#[cfg(any(test, feature = "agent-control"))]
use crate::app::agent::agent_control::AgentWindowAction;
use crate::app::agent::agent_control::{
    AgentCommandRequest, AgentCommandTicket, AgentErrorCode, AgentSubmitError,
};
use crate::app::session_runtime::AppRuntime;
use crate::app::window_semantics::WindowSemanticSnapshot;
use crate::core::WindowId;
use crate::ui::accessibility::semantic_snapshot::SemanticTarget;
use crate::ui::semantic_action::SemanticAction;

pub(crate) const MAX_AGENT_WAIT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentWindowInfo {
    pub(crate) window_id: WindowId,
    pub(crate) generation: u64,
    pub(crate) title: String,
    pub(crate) visible: bool,
    pub(crate) presentable: bool,
    pub(crate) revision: u64,
    pub(crate) presented_revision: u64,
    pub(crate) closed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentWaitCondition {
    RevisionAfter(u64),
    PresentedAtLeast(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentWaitOutcome {
    Changed(AgentWindowInfo),
    Presented(AgentWindowInfo),
    Closed(AgentWindowInfo),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AgentWaitError {
    InvalidTimeout { requested: Duration, max: Duration },
    WindowNotFound,
    StaleWindow { expected: u64, actual: u64 },
    Timeout,
    AppClosed,
}

impl AgentWaitError {
    pub(crate) const fn code(&self) -> AgentErrorCode {
        match self {
            Self::InvalidTimeout { .. } => AgentErrorCode::InvalidRequest,
            Self::WindowNotFound => AgentErrorCode::WindowNotFound,
            Self::StaleWindow { .. } => AgentErrorCode::StaleWindow,
            Self::Timeout => AgentErrorCode::Timeout,
            Self::AppClosed => AgentErrorCode::AppClosed,
        }
    }
}

#[derive(Debug, Default)]
struct AgentBridgeDirectoryState {
    enabled: bool,
    app_closed: bool,
    windows: BTreeMap<WindowId, AgentWindowInfo>,
    generations: BTreeMap<WindowId, u64>,
    waiters: BTreeMap<WindowId, Arc<Condvar>>,
}

#[derive(Debug, Default)]
struct AgentBridgeDirectoryShared {
    state: Mutex<AgentBridgeDirectoryState>,
}

/// Cheap process metadata shared with Bridge workers. UI-owned semantic state
/// publishes scalar changes here; the directory never traverses a widget tree.
#[derive(Debug, Clone, Default)]
pub(crate) struct AgentBridgeDirectory {
    shared: Arc<AgentBridgeDirectoryShared>,
}

impl AgentBridgeDirectory {
    fn lock_state(&self) -> MutexGuard<'_, AgentBridgeDirectoryState> {
        self.shared
            .state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    pub(crate) fn enable(&self) -> bool {
        let mut state = self.lock_state();
        if state.app_closed || state.enabled {
            return false;
        }
        state.enabled = true;
        true
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.lock_state().enabled
    }

    pub(crate) fn register_window(
        &self,
        window_id: WindowId,
        title: String,
        visible: bool,
        presentable: bool,
    ) -> Option<AgentWindowRegistration> {
        let mut state = self.lock_state();
        if !state.enabled || state.app_closed {
            return None;
        }

        let generation = state
            .generations
            .get(&window_id)
            .copied()
            .unwrap_or(0)
            .saturating_add(1)
            .max(1);
        state.generations.insert(window_id, generation);
        let replaced_waiter = state.waiters.insert(window_id, Arc::new(Condvar::new()));
        state.windows.insert(
            window_id,
            AgentWindowInfo {
                window_id,
                generation,
                title,
                visible,
                presentable,
                revision: 0,
                presented_revision: 0,
                closed: false,
            },
        );
        drop(state);
        if let Some(replaced_waiter) = replaced_waiter {
            replaced_waiter.notify_all();
        }

        Some(AgentWindowRegistration {
            directory: self.clone(),
            window_id,
            generation,
        })
    }

    pub(crate) fn list_windows(&self) -> Result<Vec<AgentWindowInfo>, AgentSubmitError> {
        let state = self.lock_state();
        if state.app_closed {
            return Err(AgentSubmitError::AppClosed);
        }
        Ok(state
            .windows
            .values()
            .filter(|window| !window.closed)
            .cloned()
            .collect())
    }

    pub(crate) fn contains_live_window(&self, window_id: WindowId) -> bool {
        let state = self.lock_state();
        !state.app_closed
            && state
                .windows
                .get(&window_id)
                .is_some_and(|window| !window.closed)
    }

    /// Blocks only the calling Bridge worker. UI state changes publish into
    /// this directory and notify the condition variable without scheduling a
    /// frame or waking the native event loop.
    pub(crate) fn wait(
        &self,
        window_id: WindowId,
        generation: u64,
        condition: AgentWaitCondition,
        timeout: Duration,
    ) -> Result<AgentWaitOutcome, AgentWaitError> {
        if timeout > MAX_AGENT_WAIT_TIMEOUT {
            return Err(AgentWaitError::InvalidTimeout {
                requested: timeout,
                max: MAX_AGENT_WAIT_TIMEOUT,
            });
        }

        let deadline = Instant::now() + timeout;
        let mut state = self.lock_state();
        if state.app_closed {
            return Err(AgentWaitError::AppClosed);
        }
        let waiter = state
            .waiters
            .get(&window_id)
            .cloned()
            .ok_or(AgentWaitError::WindowNotFound)?;

        loop {
            let window = state
                .windows
                .get(&window_id)
                .ok_or(AgentWaitError::WindowNotFound)?;
            if window.generation != generation {
                return Err(AgentWaitError::StaleWindow {
                    expected: generation,
                    actual: window.generation,
                });
            }
            if let Some(outcome) = wait_outcome(window, condition) {
                return Ok(outcome);
            }
            if state.app_closed {
                return Err(AgentWaitError::AppClosed);
            }

            let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                return Err(AgentWaitError::Timeout);
            };
            if remaining.is_zero() {
                return Err(AgentWaitError::Timeout);
            }
            let (next_state, _) = waiter
                .wait_timeout(state, remaining)
                .unwrap_or_else(|error| error.into_inner());
            state = next_state;
        }
    }

    pub(crate) fn close_window(&self, window_id: WindowId) -> bool {
        let mut state = self.lock_state();
        let waiter = state.waiters.get(&window_id).cloned();
        let Some(window) = state.windows.get_mut(&window_id) else {
            return false;
        };
        if window.closed {
            return false;
        }
        window.closed = true;
        window.presentable = false;
        window.revision = window.revision.saturating_add(1).max(1);
        drop(state);
        if let Some(waiter) = waiter {
            waiter.notify_all();
        }
        true
    }

    pub(crate) fn close_all(&self) {
        let mut state = self.lock_state();
        if state.app_closed {
            return;
        }
        state.app_closed = true;
        let waiters = state.waiters.values().cloned().collect::<Vec<_>>();
        for window in state.windows.values_mut() {
            if !window.closed {
                window.closed = true;
                window.presentable = false;
                window.revision = window.revision.saturating_add(1).max(1);
            }
        }
        drop(state);
        for waiter in waiters {
            waiter.notify_all();
        }
    }

    fn publish_semantics(
        &self,
        window_id: WindowId,
        generation: u64,
        snapshot: &WindowSemanticSnapshot,
    ) {
        let mut state = self.lock_state();
        let waiter = state.waiters.get(&window_id).cloned();
        let Some(window) = state.windows.get_mut(&window_id) else {
            return;
        };
        if window.generation != generation
            || snapshot.window_id != window_id
            || snapshot.generation != generation
        {
            return;
        }
        let changed = window.revision != snapshot.revision
            || window.presented_revision != snapshot.presented_revision
            || window.closed != snapshot.closed;
        window.revision = snapshot.revision;
        window.presented_revision = snapshot.presented_revision;
        window.closed = snapshot.closed;
        if snapshot.closed {
            window.presentable = false;
        }
        drop(state);
        if changed {
            if let Some(waiter) = waiter {
                waiter.notify_all();
            }
        }
    }

    fn publish_availability(
        &self,
        window_id: WindowId,
        generation: u64,
        visible: bool,
        presentable: bool,
    ) {
        let mut state = self.lock_state();
        let Some(window) = state.windows.get_mut(&window_id) else {
            return;
        };
        if window.generation != generation || window.closed {
            return;
        }
        window.visible = visible;
        window.presentable = presentable;
    }
}

fn wait_outcome(
    window: &AgentWindowInfo,
    condition: AgentWaitCondition,
) -> Option<AgentWaitOutcome> {
    if window.closed {
        return Some(AgentWaitOutcome::Closed(window.clone()));
    }
    match condition {
        AgentWaitCondition::RevisionAfter(revision) if window.revision > revision => {
            Some(AgentWaitOutcome::Changed(window.clone()))
        }
        AgentWaitCondition::PresentedAtLeast(revision) if window.presented_revision >= revision => {
            Some(AgentWaitOutcome::Presented(window.clone()))
        }
        AgentWaitCondition::RevisionAfter(_) | AgentWaitCondition::PresentedAtLeast(_) => None,
    }
}

/// UI-side publisher tied to exactly one `(window_id, generation)` pair.
/// Re-registering the same id makes old publishers harmlessly stale.
#[derive(Debug, Clone)]
pub(crate) struct AgentWindowRegistration {
    directory: AgentBridgeDirectory,
    window_id: WindowId,
    generation: u64,
}

impl AgentWindowRegistration {
    pub(crate) const fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub(crate) const fn generation(&self) -> u64 {
        self.generation
    }

    pub(crate) fn publish_semantics(&self, snapshot: &WindowSemanticSnapshot) {
        self.directory
            .publish_semantics(self.window_id, self.generation, snapshot);
    }

    pub(crate) fn publish_availability(&self, visible: bool, presentable: bool) {
        self.directory
            .publish_availability(self.window_id, self.generation, visible, presentable);
    }
}

/// Process adapter consumed by authenticated transport workers.
/// Snapshot/perform return tickets completed by the target UI turn; wait
/// blocks only its calling transport worker on directory notifications.
#[derive(Clone)]
pub(crate) struct AgentProcessBridge {
    runtime: AppRuntime,
}

impl AgentProcessBridge {
    pub(crate) fn new(runtime: AppRuntime) -> Self {
        Self { runtime }
    }

    pub(crate) fn list_windows(&self) -> Result<Vec<AgentWindowInfo>, AgentSubmitError> {
        self.runtime.list_agent_windows()
    }

    pub(crate) fn snapshot(
        &self,
        window_id: WindowId,
    ) -> Result<AgentCommandTicket, AgentSubmitError> {
        self.submit_for_live_window(window_id, AgentCommandRequest::Snapshot)
    }

    pub(crate) fn perform(
        &self,
        window_id: WindowId,
        generation: u64,
        expected_revision: Option<u64>,
        target: SemanticTarget,
        action: SemanticAction,
    ) -> Result<AgentCommandTicket, AgentSubmitError> {
        self.submit_for_live_window(
            window_id,
            AgentCommandRequest::Perform {
                generation,
                expected_revision,
                target,
                action,
            },
        )
    }

    #[cfg(any(test, feature = "agent-control"))]
    pub(crate) fn perform_window(
        &self,
        window_id: WindowId,
        generation: u64,
        expected_revision: Option<u64>,
        action: AgentWindowAction,
    ) -> Result<AgentCommandTicket, AgentSubmitError> {
        self.submit_for_live_window(
            window_id,
            AgentCommandRequest::PerformWindow {
                generation,
                expected_revision,
                action,
            },
        )
    }

    pub(crate) fn wait(
        &self,
        window_id: WindowId,
        generation: u64,
        condition: AgentWaitCondition,
        timeout: Duration,
    ) -> Result<AgentWaitOutcome, AgentWaitError> {
        self.runtime
            .wait_agent_window(window_id, generation, condition, timeout)
    }

    fn submit_for_live_window(
        &self,
        window_id: WindowId,
        request: AgentCommandRequest,
    ) -> Result<AgentCommandTicket, AgentSubmitError> {
        if !self.runtime.contains_live_agent_window(window_id) {
            return Err(if self.runtime.is_shutting_down() {
                AgentSubmitError::AppClosed
            } else {
                AgentSubmitError::WindowNotFound
            });
        }
        self.runtime.submit_agent_command(window_id, request)
    }
}
