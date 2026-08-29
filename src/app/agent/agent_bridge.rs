//! 可选的 Agent Bridge 的进程级路由与窗口发现。
//!
//! 本层与具体传输无关。原生传输层负责连接认证与 JSON 帧划分，随后把已认证
//! 的请求委派给 [`AgentProcessBridge`]。本层永远不会拿到 `WidgetTree` 引用。

#![cfg_attr(not(any(test, feature = "agent-control")), allow(dead_code))]

use std::collections::BTreeMap;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

#[cfg(any(test, feature = "agent-control"))]
use crate::app::queues::agent_command_queue::AgentWindowAction;
use crate::app::queues::agent_command_queue::{
    AgentCommandRequest, AgentCommandTicket, AgentErrorCode, AgentSubmitError,
};
use crate::app::session_runtime::AppRuntime;
use crate::app::window_semantics::{AgentSemanticsPort, AgentWindowState, WindowSemanticSnapshot};
use crate::core::WindowId;
#[cfg(feature = "agent-control")]
use crate::draw::SurfaceReadbackTicket;
use crate::ui::accessibility::semantic_snapshot::SemanticTarget;
use crate::ui::semantic_action::SemanticAction;

// 默认库测试不启动 agent transport，但仍需编译并保留等待协议的上限契约。
#[cfg_attr(test, allow(dead_code))]
pub(crate) const MAX_AGENT_WAIT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentWindowInfo {
    pub(crate) window_id: WindowId,
    pub(crate) generation: u64,
    pub(crate) title: String,
    pub(crate) visible: bool,
    pub(crate) presentable: bool,
    /// 窗口是否持有操作系统输入焦点；Agent 定向输入不借用该焦点。
    pub(crate) focused: bool,
    pub(crate) logical_width: i32,
    pub(crate) logical_height: i32,
    pub(crate) maximized: bool,
    pub(crate) minimized: bool,
    pub(crate) fullscreen: bool,
    pub(crate) revision: u64,
    pub(crate) presented_revision: u64,
    pub(crate) closed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
// 默认库测试不启动 agent transport，但仍需保留跨边界等待条件契约。
#[cfg_attr(test, allow(dead_code))]
pub(crate) enum AgentWaitCondition {
    RevisionAfter(u64),
    PresentedAtLeast(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
// 默认库测试不启动 agent transport，但仍需保留跨边界等待结果契约。
#[cfg_attr(test, allow(dead_code))]
pub(crate) enum AgentWaitOutcome {
    Changed(AgentWindowInfo),
    Presented(AgentWindowInfo),
    Closed(AgentWindowInfo),
}

#[derive(Debug, Clone, PartialEq, Eq)]
// 默认库测试不启动 agent transport，但仍需保留等待错误到协议错误码的映射。
#[cfg_attr(test, allow(dead_code))]
pub(crate) enum AgentWaitError {
    InvalidTimeout { requested: Duration, max: Duration },
    WindowNotFound,
    StaleWindow { expected: u64, actual: u64 },
    Timeout,
    AppClosed,
}

// 仅在 agent transport 或专门 GUI 测试中消费等待错误码。
#[cfg_attr(test, allow(dead_code))]
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

/// 与 Bridge worker 共享的廉价进程元数据。UI 侧语义状态只把标量变更
/// 发布到这里；目录永不遍历 widget 树。
#[derive(Debug, Clone, Default)]
pub(crate) struct AgentBridgeDirectory {
    shared: Arc<AgentBridgeDirectoryShared>,
}

// 目录 API 由 agent transport/GUI 测试按需消费，默认库测试不启动其消费者。
#[cfg_attr(test, allow(dead_code))]
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
        focused: bool,
        logical_width: i32,
        logical_height: i32,
        maximized: bool,
        minimized: bool,
        fullscreen: bool,
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
                focused,
                logical_width,
                logical_height,
                maximized,
                minimized,
                fullscreen,
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

    /// 只阻塞调用它的 Bridge worker。UI 状态变更发布到本目录并通过条件变量
    /// 通知，不调度帧也不唤醒原生事件循环。
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

    fn publish_window_state(&self, window_id: WindowId, generation: u64, update: AgentWindowState) {
        let mut state = self.lock_state();
        let Some(window) = state.windows.get_mut(&window_id) else {
            return;
        };
        if window.generation != generation || window.closed {
            return;
        }
        window.visible = update.visible;
        window.presentable = update.presentable;
        window.focused = update.focused;
        window.logical_width = update.logical_width;
        window.logical_height = update.logical_height;
        window.maximized = update.maximized;
        window.minimized = update.minimized;
        window.fullscreen = update.fullscreen;
    }
}

// 等待结果转换属于 agent transport 的内部契约，默认库测试没有阻塞等待调用。
#[cfg_attr(test, allow(dead_code))]
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

/// 与恰好一个 `(window_id, generation)` 对绑定的 UI 侧发布器。
/// 同一 id 重新注册后，旧发布器会无害地变为过期状态。
#[derive(Debug, Clone)]
pub(crate) struct AgentWindowRegistration {
    directory: AgentBridgeDirectory,
    window_id: WindowId,
    generation: u64,
}

impl AgentSemanticsPort for AgentWindowRegistration {
    fn window_id(&self) -> WindowId {
        self.window_id
    }

    fn generation(&self) -> u64 {
        self.generation
    }

    fn publish_semantics(&self, snapshot: &WindowSemanticSnapshot) {
        self.directory
            .publish_semantics(self.window_id, self.generation, snapshot);
    }

    fn publish_window_state(&self, state: AgentWindowState) {
        self.directory
            .publish_window_state(self.window_id, self.generation, state);
    }
}

/// 一次 Agent 截屏的成对票据：强制出帧命令 + surface 回读结果。
///
/// 协议层先等命令结算，再等回读像素；任何失败路径都应通过
/// `cancel_surface_readback` 释放回读单槽。
#[cfg(feature = "agent-control")]
pub(crate) struct AgentScreenshotSession {
    pub(crate) command: AgentCommandTicket,
    pub(crate) readback: SurfaceReadbackTicket,
}

/// 供已认证的 transport worker 消费的进程适配器。
/// Snapshot/perform 返回的 ticket 由目标 UI turn 完成；wait 只阻塞调用它的
/// transport worker 等待目录通知。
#[derive(Clone)]
// 进程桥由认证 transport 或外部 GUI 测试创建，默认库测试不构造它。
#[cfg_attr(test, allow(dead_code))]
pub(crate) struct AgentProcessBridge {
    runtime: AppRuntime,
}

// 进程桥的快照、动作与等待入口由 agent transport/GUI 测试按需调用。
#[cfg_attr(test, allow(dead_code))]
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

    /// 用户确认流程：AI 确认执行先前命中 `requires_confirmation` 的动作。
    #[cfg(any(test, feature = "agent-control"))]
    pub(crate) fn confirm(
        &self,
        window_id: WindowId,
        confirm_id: u64,
    ) -> Result<AgentCommandTicket, AgentSubmitError> {
        self.submit_for_live_window(window_id, AgentCommandRequest::Confirm { confirm_id })
    }

    /// Agent 截屏：先占用回读单槽，再入队强制出帧命令。
    ///
    /// 命令在窗口 UI turn 强制整树重绘；settle 内的下一次真实 present 完成
    /// 回读票据。命令失败时必须调用 [`Self::cancel_surface_readback`]
    /// 释放单槽，否则同一窗口的后续截屏会被拒绝。
    #[cfg(feature = "agent-control")]
    pub(crate) fn screenshot(
        &self,
        window_id: WindowId,
    ) -> Result<AgentScreenshotSession, AgentSubmitError> {
        if !self.runtime.contains_live_agent_window(window_id) {
            return Err(if self.runtime.is_shutting_down() {
                AgentSubmitError::AppClosed
            } else {
                AgentSubmitError::WindowNotFound
            });
        }
        let readback = self.runtime.request_surface_readback(window_id).map_err(|error| {
            AgentSubmitError::ReadbackUnavailable {
                message: format!("{}", error.what()),
            }
        })?;
        let command = match self.submit_for_live_window(window_id, AgentCommandRequest::Screenshot)
        {
            Ok(ticket) => ticket,
            Err(error) => {
                // 入队失败必须释放回读单槽，避免永久阻塞该窗口的后续截屏。
                self.runtime.cancel_surface_readback(window_id);
                return Err(error);
            }
        };
        Ok(AgentScreenshotSession {
            command,
            readback,
        })
    }

    /// 释放目标窗口仍在排队的回读请求；已被 owner thread 取走的请求无法撤回。
    #[cfg(feature = "agent-control")]
    pub(crate) fn cancel_surface_readback(&self, window_id: WindowId) {
        self.runtime.cancel_surface_readback(window_id);
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

// 目录核心逻辑专项测试（内存内同步操作，不启动线程与 IO）。
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_state_publication_updates_same_generation_without_native_identity() {
        let directory = AgentBridgeDirectory::default();
        assert!(directory.enable());
        let registration = directory
            .register_window(
                WindowId::new(7),
                "fixture".to_owned(),
                true,
                true,
                false,
                800,
                600,
                false,
                false,
                false,
            )
            .expect("enabled directory must register a window");
        registration.publish_window_state(AgentWindowState {
            visible: true,
            presentable: true,
            focused: true,
            logical_width: 1000,
            logical_height: 700,
            maximized: true,
            minimized: false,
            fullscreen: false,
        });

        let windows = directory
            .list_windows()
            .expect("live directory must list windows");
        assert_eq!(windows.len(), 1);
        let window = &windows[0];
        assert_eq!((window.logical_width, window.logical_height), (1000, 700));
        assert!(window.maximized);
        assert!(!window.minimized);
        assert!(!window.fullscreen);
    }
}
