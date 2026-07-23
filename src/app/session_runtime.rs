use std::collections::BTreeMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

#[cfg(any(test, feature = "agent-control"))]
use crate::app::agent_bridge::AgentProcessBridge;
use crate::app::agent_bridge::{
    AgentBridgeDirectory, AgentWaitCondition, AgentWaitError, AgentWaitOutcome, AgentWindowInfo,
    AgentWindowRegistration,
};
use crate::app::agent_control::{
    AgentCommandQueue, AgentCommandRequest, AgentCommandTicket, AgentSubmitError,
};
#[cfg(feature = "agent-control")]
use crate::app::agent_transport::{AgentTransportError, AgentTransportHandle, AgentTransportInfo};
use crate::app::app_timer::{AppTimerQueue, TimerHandle};
use crate::app::main_thread_queue::{MainThreadContext, MainThreadQueue};
use crate::app::window_config::WindowConfig;
use crate::app::window_session::TextInputCoordinator;
use crate::core::WindowId;
#[cfg(feature = "test-harness")]
use crate::draw::renderer::test_harness::GraphicsFaultSignal;
use crate::native::traits::event::EventLoopWaker;
use crate::ui::Theme;
use std::collections::VecDeque;

#[derive(Clone, Default)]
pub(crate) struct AppRuntime {
    pub(crate) sessions: Arc<Mutex<BTreeMap<WindowId, SessionRuntime>>>,
    pending_open_windows: Arc<Mutex<VecDeque<OpenWindowRequest>>>,
    pending_theme: Arc<Mutex<Option<Theme>>>,
    shutting_down: Arc<AtomicBool>,
    next_window_id: Arc<Mutex<u64>>,
    event_loop_waker: Arc<Mutex<EventLoopWaker>>,
    text_input_coordinator: TextInputCoordinator,
    agent_bridge: AgentBridgeDirectory,
    #[cfg(feature = "agent-control")]
    agent_transport: Arc<Mutex<Option<AgentTransportHandle>>>,
}

#[derive(Clone)]
pub(crate) struct SessionRuntime {
    app_timers: AppTimerQueue,
    main_thread_queue: MainThreadQueue,
    agent_commands: AgentCommandQueue,
    alive: Arc<AtomicBool>,
    #[cfg(feature = "test-harness")]
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
            #[cfg(feature = "test-harness")]
            GraphicsFaultSignal::default(),
        );
    }

    #[cfg(feature = "test-harness")]
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
        #[cfg(feature = "test-harness")] graphics_faults: GraphicsFaultSignal,
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
            alive.store(false, Ordering::Release);
            app_timers.cancel_all();
            main_thread_queue.clear();
            agent_commands.close();
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
                #[cfg(feature = "test-harness")]
                graphics_faults,
            },
        );
        if let Some(replaced) = replaced {
            replaced.agent_commands.close();
            self.agent_bridge.close_window(window_id);
        }
    }

    #[cfg(any(test, feature = "agent-control"))]
    pub(crate) fn enable_agent_control(&self) -> bool {
        self.agent_bridge.enable()
    }

    #[cfg(any(test, feature = "agent-control"))]
    pub(crate) fn agent_bridge(&self) -> Option<AgentProcessBridge> {
        self.agent_bridge
            .is_enabled()
            .then(|| AgentProcessBridge::new(self.clone()))
    }

    #[cfg(feature = "agent-control")]
    pub(crate) fn start_agent_transport(&self) -> Result<AgentTransportInfo, AgentTransportError> {
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
        let handle = AgentTransportHandle::start(bridge)?;
        let info = handle.info().clone();
        *transport = Some(handle);
        Ok(info)
    }

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
    ) -> Option<AgentWindowRegistration> {
        self.agent_bridge
            .register_window(window_id, title, visible, presentable)
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
            session.alive.store(false, Ordering::Release);
            session.app_timers.cancel_all();
            session.main_thread_queue.clear();
            session.agent_commands.close();
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
            session.alive.store(false, Ordering::Release);
            session.app_timers.cancel_all();
            session.main_thread_queue.clear();
            session.agent_commands.close();
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
            transport.shutdown();
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

    #[cfg(feature = "test-harness")]
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
