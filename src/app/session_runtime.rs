use std::collections::BTreeMap;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;

#[cfg(any(test, feature = "agent-control"))]
use crate::app::agent::agent_bridge::AgentProcessBridge;
use crate::app::agent::agent_bridge::{
    AgentBridgeDirectory, AgentWaitCondition, AgentWaitError, AgentWaitOutcome, AgentWindowInfo,
    AgentWindowRegistration,
};
use crate::app::agent::agent_control::AgentCommandExecutorImpl;
use crate::app::agent::agent_policy::AgentPolicy;
#[cfg(feature = "agent-control")]
use crate::app::agent::agent_transport::{
    AgentTransportError, AgentTransportHandle, AgentTransportInfo,
};
use crate::app::queues::agent_command_queue::{
    AgentCommandQueue, AgentCommandRequest, AgentCommandTicket, AgentConfirmationRequest,
    AgentSubmitError,
};
use crate::app::queues::app_timer::{AppTimerQueue, TimerHandle};
use crate::app::queues::main_thread_queue::{MainThreadContext, MainThreadQueue};
use crate::app::queues::window_agent_state::AgentCommandExecutor;
use crate::app::window::window_config::WindowConfig;
use crate::app::window::window_session::TextInputCoordinator;
use crate::core::WindowId;
use crate::diagnostics::{Diagnostics, DiagnosticsConfig};
#[cfg(any(feature = "test-harness", feature = "agent-control"))]
use crate::draw::renderer::test_harness::{GraphicsFaultSignal, SurfaceReadbackTicket};
use crate::platform::windowing::event::EventLoopWaker;
use crate::ui::Theme;
use std::collections::VecDeque;

#[derive(Clone)]
pub(crate) struct AppRuntime {
    diagnostics: Diagnostics,
    pub(crate) sessions: Arc<Mutex<BTreeMap<WindowId, SessionRuntime>>>,
    pending_open_windows: Arc<Mutex<VecDeque<OpenWindowRequest>>>,
    pending_theme: Arc<Mutex<Option<Theme>>>,
    shutting_down: Arc<AtomicBool>,
    next_window_id: Arc<Mutex<u64>>,
    event_loop_waker: Arc<Mutex<EventLoopWaker>>,
    text_input_coordinator: TextInputCoordinator,
    agent_bridge: AgentBridgeDirectory,
    agent_policy: Arc<AgentPolicy>,
    /// 应用注入的 Agent 确认 UI 回调（默认空操作：无确认 UI 时确认请求直接失败）。
    agent_confirm_ui: Arc<dyn Fn(AgentConfirmationRequest) + Send + Sync>,
    #[cfg(feature = "agent-control")]
    agent_transport: Arc<Mutex<Option<AgentTransportHandle>>>,
}

#[derive(Clone)]
pub(crate) struct SessionRuntime {
    app_timers: AppTimerQueue,
    main_thread_queue: MainThreadQueue,
    agent_commands: AgentCommandQueue,
    alive: Arc<AtomicBool>,
    #[cfg(any(feature = "test-harness", feature = "agent-control"))]
    graphics_faults: GraphicsFaultSignal,
}

#[allow(dead_code)]
pub(crate) struct OpenWindowRequest {
    pub(crate) window_id: WindowId,
    pub(crate) config: WindowConfig,
    pub(crate) app_timers: AppTimerQueue,
    pub(crate) main_thread_queue: MainThreadQueue,
    pub(crate) alive: Arc<AtomicBool>,
}

pub(crate) struct ReservedWindowSession {
    pub(crate) window_id: WindowId,
    pub(crate) alive: Arc<AtomicBool>,
}

impl AppRuntime {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// 组装期默认：Agent 命令执行器由组合根按当前动作策略创建，注入每窗口状态机。
    pub(crate) fn agent_command_executor(&self) -> Arc<dyn AgentCommandExecutor> {
        Arc::new(AgentCommandExecutorImpl::new(self.agent_policy.clone()))
    }

    /// 组装期设置 Agent 动作策略（授权第二层）；窗口创建前调用。
    pub(crate) fn set_agent_policy(&mut self, policy: AgentPolicy) {
        self.agent_policy = Arc::new(policy);
    }

    /// 组装期注入 Agent 确认 UI 回调（授权第三层）；窗口创建前调用。
    pub(crate) fn set_agent_confirm_ui(
        &mut self,
        handler: impl Fn(AgentConfirmationRequest) + Send + Sync + 'static,
    ) {
        self.agent_confirm_ui = Arc::new(handler);
    }

    /// 窗口组装期取确认 UI 回调。
    pub(crate) fn agent_confirm_ui(
        &self,
    ) -> Option<Arc<dyn Fn(AgentConfirmationRequest) + Send + Sync>> {
        Some(self.agent_confirm_ui.clone())
    }

    pub(crate) fn set_diagnostics(&mut self, config: DiagnosticsConfig) {
        self.diagnostics = Diagnostics::new(config);
    }

    pub(crate) fn set_diagnostics_runtime(&mut self, diagnostics: Diagnostics) {
        self.diagnostics = diagnostics;
    }

    pub(crate) fn diagnostics(&self) -> Diagnostics {
        self.diagnostics.clone()
    }

    pub(crate) fn register_session(
        &self,
        window_id: WindowId,
        app_timers: AppTimerQueue,
        main_thread_queue: MainThreadQueue,
        alive: Arc<AtomicBool>,
    ) {
        self.register_session_inner(
            window_id,
            app_timers,
            main_thread_queue,
            alive,
            #[cfg(any(feature = "test-harness", feature = "agent-control"))]
            GraphicsFaultSignal::default(),
        );
    }

    #[cfg(any(feature = "test-harness", feature = "agent-control"))]
    pub(crate) fn register_session_with_graphics_faults(
        &self,
        window_id: WindowId,
        app_timers: AppTimerQueue,
        main_thread_queue: MainThreadQueue,
        alive: Arc<AtomicBool>,
        graphics_faults: GraphicsFaultSignal,
    ) {
        self.register_session_inner(
            window_id,
            app_timers,
            main_thread_queue,
            alive,
            graphics_faults,
        );
    }

    fn register_session_inner(
        &self,
        window_id: WindowId,
        app_timers: AppTimerQueue,
        main_thread_queue: MainThreadQueue,
        alive: Arc<AtomicBool>,
        #[cfg(any(feature = "test-harness", feature = "agent-control"))] graphics_faults: GraphicsFaultSignal,
    ) {
        self.reserve_after(window_id);
        let agent_commands = AgentCommandQueue::new();
        let event_loop_waker = self.event_loop_waker.clone();
        app_timers.set_removal_waker(Arc::new(move || {
            let waker = {
                event_loop_waker
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .clone()
            };
            waker.wake();
        }));
        let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        if self.shutting_down.load(Ordering::Acquire) {
            crate::app::queues::release_window_scheduling_resources(
                &alive,
                &app_timers,
                &main_thread_queue,
                &agent_commands,
            );
            return;
        }
        alive.store(true, Ordering::Release);
        let replaced = sessions.insert(
            window_id,
            SessionRuntime {
                app_timers,
                main_thread_queue,
                agent_commands,
                alive,
                #[cfg(any(feature = "test-harness", feature = "agent-control"))]
                graphics_faults,
            },
        );
        if let Some(replaced) = replaced {
            replaced.agent_commands.close();
            self.agent_bridge.close_window(window_id);
        }
    }

    // 测试目标保留 agent 控制开关，供 transport/GUI 契约测试按需启动。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(any(test, feature = "agent-control"))]
    pub(crate) fn enable_agent_control(&self) -> bool {
        self.agent_bridge.enable()
    }

    // 测试目标保留进程桥构造入口，供 transport/GUI 契约测试按需获取。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(any(test, feature = "agent-control"))]
    pub(crate) fn agent_bridge(&self) -> Option<AgentProcessBridge> {
        self.agent_bridge
            .is_enabled()
            .then(|| AgentProcessBridge::new(self.clone()))
    }

    #[cfg(feature = "agent-control")]
    pub(crate) fn start_agent_transport(
        &self,
        display_name: &str,
    ) -> Result<AgentTransportInfo, AgentTransportError> {
        let bridge = self
            .agent_bridge()
            .ok_or(AgentTransportError::BridgeDisabled)?;
        let mut transport = self
            .agent_transport
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(transport) = transport.as_ref() {
            return Ok(transport.info().clone());
        }
        let handle = AgentTransportHandle::start(bridge, display_name)?;
        let info = handle.info().clone();
        *transport = Some(handle);
        Ok(info)
    }

    // 测试目标保留 transport 信息观测入口，供带 transport 的 GUI 契约测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(all(feature = "agent-control", test))]
    pub(crate) fn agent_transport_info(&self) -> Option<AgentTransportInfo> {
        self.agent_transport
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
            .map(|transport| transport.info().clone())
    }

    pub(crate) fn register_agent_window(
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
        self.agent_bridge.register_window(
            window_id,
            title,
            visible,
            presentable,
            focused,
            logical_width,
            logical_height,
            maximized,
            minimized,
            fullscreen,
        )
    }

    pub(crate) fn list_agent_windows(&self) -> Result<Vec<AgentWindowInfo>, AgentSubmitError> {
        self.agent_bridge.list_windows()
    }

    pub(crate) fn wait_agent_window(
        &self,
        window_id: WindowId,
        generation: u64,
        condition: AgentWaitCondition,
        timeout: Duration,
    ) -> Result<AgentWaitOutcome, AgentWaitError> {
        self.agent_bridge
            .wait(window_id, generation, condition, timeout)
    }

    pub(crate) fn contains_live_agent_window(&self, window_id: WindowId) -> bool {
        self.agent_bridge.contains_live_window(window_id)
    }

    pub(crate) fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::Acquire)
    }

    pub(crate) fn text_input_coordinator(&self) -> TextInputCoordinator {
        self.text_input_coordinator.clone()
    }

    pub(crate) fn request_open_window(&self, config: WindowConfig) -> ReservedWindowSession {
        let window_id = self.next_window_id();
        let app_timers = AppTimerQueue::new();
        let main_thread_queue = MainThreadQueue::new();
        let alive = Arc::new(AtomicBool::new(true));
        self.register_session(
            window_id,
            app_timers.clone(),
            main_thread_queue.clone(),
            alive.clone(),
        );
        let mut pending = self
            .pending_open_windows
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if alive.load(Ordering::Acquire) && !self.shutting_down.load(Ordering::Acquire) {
            pending.push_back(OpenWindowRequest {
                window_id,
                config,
                app_timers: app_timers.clone(),
                main_thread_queue: main_thread_queue.clone(),
                alive: alive.clone(),
            });
        }
        drop(pending);
        if alive.load(Ordering::Acquire) {
            self.wake_event_loop();
        } else {
            self.close_session(window_id);
        }
        ReservedWindowSession { window_id, alive }
    }

    pub(crate) fn take_next_open_window(&self) -> Option<OpenWindowRequest> {
        self.pending_open_windows
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pop_front()
    }

    pub(crate) fn set_theme(&self, theme: Theme) {
        let mut pending = self.pending_theme.lock().unwrap_or_else(|e| e.into_inner());
        let should_wake = pending.is_none();
        *pending = Some(theme);
        drop(pending);
        if should_wake {
            self.wake_event_loop();
        }
    }

    pub(crate) fn take_pending_theme(&self) -> Option<Theme> {
        self.pending_theme
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
    }

    pub(crate) fn set_event_loop_waker(&self, waker: EventLoopWaker) {
        *self
            .event_loop_waker
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = waker;
    }

    pub(crate) fn close_session(&self, window_id: WindowId) {
        let session = self
            .sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&window_id);
        if let Some(session) = session {
            crate::app::queues::release_window_scheduling_resources(
                &session.alive,
                &session.app_timers,
                &session.main_thread_queue,
                &session.agent_commands,
            );
        }
        self.agent_bridge.close_window(window_id);
        self.pending_open_windows
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|request| request.window_id != window_id);
    }

    /// Terminal app shutdown: no window session, queued open request, or pending
    /// theme update may outlive the native event loop.
    pub(crate) fn shutdown_all(&self) {
        self.shutting_down.store(true, Ordering::Release);
        let sessions =
            std::mem::take(&mut *self.sessions.lock().unwrap_or_else(|e| e.into_inner()));
        for session in sessions.into_values() {
            crate::app::queues::release_window_scheduling_resources(
                &session.alive,
                &session.app_timers,
                &session.main_thread_queue,
                &session.agent_commands,
            );
        }
        self.pending_open_windows
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        self.pending_theme
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        self.agent_bridge.close_all();
        #[cfg(feature = "agent-control")]
        if let Some(mut transport) = self
            .agent_transport
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
        {
            // close_all 已发布窗口终态；有界等待在途 wait 写回后再强制拆除传输。
            transport.shutdown_after_terminal_reply();
        }
    }

    pub(crate) fn run_after<F>(&self, window_id: WindowId, delay: Duration, f: F) -> TimerHandle
    where
        F: FnOnce() + Send + 'static,
    {
        let Some(session) = self.session(window_id) else {
            return TimerHandle::inactive();
        };
        let handle = session.app_timers.run_after(delay, f);
        self.wake_event_loop();
        handle
    }

    pub(crate) fn run_interval<F>(
        &self,
        window_id: WindowId,
        interval: Duration,
        f: F,
    ) -> TimerHandle
    where
        F: FnMut() + Send + 'static,
    {
        let Some(session) = self.session(window_id) else {
            return TimerHandle::inactive();
        };
        let handle = session.app_timers.run_interval(interval, f);
        self.wake_event_loop();
        handle
    }

    pub(crate) fn post_to_ui<F>(&self, window_id: WindowId, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        if let Some(session) = self.session(window_id) {
            session.main_thread_queue.enqueue(f);
            self.wake_event_loop();
        }
    }

    pub(crate) fn enqueue_with_context<F>(&self, window_id: WindowId, f: F)
    where
        F: for<'a> FnOnce(&mut MainThreadContext<'a>) + Send + 'static,
    {
        if let Some(session) = self.session(window_id) {
            session.main_thread_queue.enqueue_with_context(f);
            self.wake_event_loop();
        }
    }

    pub(crate) fn submit_agent_command(
        &self,
        window_id: WindowId,
        request: AgentCommandRequest,
    ) -> Result<AgentCommandTicket, AgentSubmitError> {
        if self.shutting_down.load(Ordering::Acquire) {
            return Err(AgentSubmitError::AppClosed);
        }
        let session = self
            .session(window_id)
            .ok_or(AgentSubmitError::WindowNotFound)?;
        let (ticket, should_wake) = session.agent_commands.submit(request)?;
        if should_wake {
            self.wake_event_loop();
        }
        Ok(ticket)
    }

    pub(crate) fn agent_command_queue(&self, window_id: WindowId) -> Option<AgentCommandQueue> {
        self.session(window_id)
            .map(|session| session.agent_commands)
    }

    #[cfg(any(feature = "test-harness", feature = "agent-control"))]
    pub(crate) fn graphics_fault_signal(&self, window_id: WindowId) -> Option<GraphicsFaultSignal> {
        self.session(window_id)
            .map(|session| session.graphics_faults)
    }

    #[cfg(feature = "test-harness")]
    pub(crate) fn inject_graphics_device_lost_for_test(
        &self,
        window_id: WindowId,
    ) -> crate::core::Result<()> {
        let session = self.session(window_id).ok_or_else(|| {
            crate::core::Error::new(
                crate::core::Errc::NotFound,
                "cannot inject a graphics fault into a closed window session",
            )
        })?;
        session.graphics_faults.arm_device_lost()?;
        self.wake_event_loop();
        Ok(())
    }

    // 在目标窗口下一次 owner-thread 帧边界安排 surface-lost 注入。
    #[cfg(feature = "test-harness")]
    pub(crate) fn inject_graphics_surface_lost_for_test(
        &self,
        window_id: WindowId,
    ) -> crate::core::Result<()> {
        // 关闭窗口后不允许向已释放的图形会话投递故障。
        let session = self.session(window_id).ok_or_else(|| {
            crate::core::Error::new(
                crate::core::Errc::NotFound,
                "cannot inject a surface fault into a closed window session",
            )
        })?;
        // 复用同一恢复信号，实际 surface 错误仍在 owner thread 产生。
        session.graphics_faults.arm_surface_lost()?;
        // 唤醒事件循环，让下一帧及时消费测试信号。
        self.wake_event_loop();
        Ok(())
    }

    // 为目标窗口安排下一次成功 GPU 帧的规范 surface 回读。
    #[cfg(any(feature = "test-harness", feature = "agent-control"))]
    pub(crate) fn request_surface_readback_for_test(
        // 借用应用运行时控制面。
        &self,
        // 指定持有真实图形 owner 的窗口。
        window_id: WindowId,
        // 返回可由非 UI 线程有界等待的一次性票据。
    ) -> crate::core::Result<SurfaceReadbackTicket> {
        // 关闭窗口后不得向已经释放的 Renderer 投递请求。
        let session = self.session(window_id).ok_or_else(|| {
            // 使用稳定的窗口不存在错误。
            crate::core::Error::new(
                // 会话已经不存在。
                crate::core::Errc::NotFound,
                // 诊断不暴露内部会话表。
                "cannot read back a closed window session",
            )
        })?;
        // 在共享图形测试信号上保留唯一待处理请求。
        let ticket = session.graphics_faults.request_surface_readback()?;
        // 唤醒事件循环，让已有脏帧及时进入 owner-thread 绘制边界。
        self.wake_event_loop();
        // 把接收端交给调用方，发送端继续由 Renderer 持有。
        Ok(ticket)
    }

    // 为 Agent 截屏安排目标窗口下一次成功帧的规范 surface 回读。
    #[cfg(feature = "agent-control")]
    pub(crate) fn request_surface_readback(
        &self,
        window_id: WindowId,
    ) -> crate::core::Result<SurfaceReadbackTicket> {
        // 关闭窗口后不得向已经释放的 Renderer 投递请求。
        let session = self.session(window_id).ok_or_else(|| {
            crate::core::Error::new(
                crate::core::Errc::NotFound,
                "cannot read back a closed window session",
            )
        })?;
        // 在共享信号上保留唯一待处理请求；实际回读仍在图形 owner thread 执行。
        let ticket = session.graphics_faults.request_surface_readback()?;
        // 唤醒事件循环，让截屏命令强制出的帧及时进入 owner-thread 绘制边界。
        self.wake_event_loop();
        Ok(ticket)
    }

    // Agent 截屏失败路径取消仍在排队的回读请求，释放单槽避免阻塞后续截屏。
    #[cfg(feature = "agent-control")]
    pub(crate) fn cancel_surface_readback(&self, window_id: WindowId) {
        if let Some(session) = self.session(window_id) {
            session.graphics_faults.cancel_surface_readback();
        }
    }

    fn session(&self, window_id: WindowId) -> Option<SessionRuntime> {
        let session = self
            .sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .get(&window_id)
            .cloned()?;
        if session.alive.load(Ordering::Acquire) {
            Some(session)
        } else {
            None
        }
    }

    fn next_window_id(&self) -> WindowId {
        let mut next = self
            .next_window_id
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let id = (*next).max(1);
        *next = id.wrapping_add(1).max(1);
        WindowId::new(id)
    }

    fn reserve_after(&self, window_id: WindowId) {
        let mut next = self
            .next_window_id
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let candidate = window_id.raw().wrapping_add(1).max(1);
        if *next < candidate {
            *next = candidate;
        }
    }

    fn wake_event_loop(&self) {
        let waker = {
            self.event_loop_waker
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
        };
        waker.wake();
    }
}

impl Default for AppRuntime {
    fn default() -> Self {
        Self {
            diagnostics: Diagnostics::default(),
            sessions: Arc::new(Mutex::new(BTreeMap::new())),
            pending_open_windows: Arc::new(Mutex::new(VecDeque::new())),
            pending_theme: Arc::new(Mutex::new(None)),
            shutting_down: Arc::new(AtomicBool::new(false)),
            next_window_id: Arc::new(Mutex::new(0)),
            event_loop_waker: Arc::new(Mutex::new(EventLoopWaker::default())),
            text_input_coordinator: TextInputCoordinator::default(),
            agent_bridge: AgentBridgeDirectory::default(),
            agent_policy: Arc::new(AgentPolicy::default()),
            agent_confirm_ui: Arc::new(|_: AgentConfirmationRequest| {}),
            #[cfg(feature = "agent-control")]
            agent_transport: Arc::new(Mutex::new(None)),
        }
    }
}
