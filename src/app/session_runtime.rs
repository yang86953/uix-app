use std::collections::BTreeMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

use crate::app::app_timer::{AppTimerQueue, TimerHandle};
use crate::app::main_thread_queue::{MainThreadContext, MainThreadQueue};
use crate::app::window_config::WindowConfig;
use crate::core::WindowId;
use crate::native::traits::event::EventLoopWaker;
use std::collections::VecDeque;

#[derive(Clone, Default)]
pub(crate) struct AppRuntime {
    sessions: Arc<Mutex<BTreeMap<WindowId, SessionRuntime>>>,
    pending_open_windows: Arc<Mutex<VecDeque<OpenWindowRequest>>>,
    next_window_id: Arc<Mutex<u64>>,
    event_loop_waker: Arc<Mutex<EventLoopWaker>>,
}

#[derive(Clone)]
struct SessionRuntime {
    app_timers: AppTimerQueue,
    main_thread_queue: MainThreadQueue,
    alive: Arc<AtomicBool>,
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
        alive.store(true, Ordering::Release);
        self.reserve_after(window_id);
        self.sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(
                window_id,
                SessionRuntime {
                    app_timers,
                    main_thread_queue,
                    alive,
                },
            );
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
        self.pending_open_windows
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push_back(OpenWindowRequest {
                window_id,
                config,
                app_timers: app_timers.clone(),
                main_thread_queue: main_thread_queue.clone(),
                alive: alive.clone(),
            });
        ReservedWindowSession { window_id, alive }
    }

    pub(crate) fn take_next_open_window(&self) -> Option<OpenWindowRequest> {
        self.pending_open_windows
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pop_front()
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
        }
        self.pending_open_windows
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .retain(|request| request.window_id != window_id);
    }

    pub(crate) fn run_after<F>(&self, window_id: WindowId, delay: Duration, f: F) -> TimerHandle
    where
        F: FnOnce() + Send + 'static,
    {
        let Some(session) = self.session(window_id) else {
            return TimerHandle::inactive();
        };
        session.app_timers.run_after(delay, f)
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
        session.app_timers.run_interval(interval, f)
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
        self.event_loop_waker
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .wake();
    }
}

#[cfg(test)]
#[path = "../tests/app/session_runtime.rs"]
mod tests;
