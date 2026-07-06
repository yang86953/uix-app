use std::collections::BTreeMap;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

use crate::app::app_timer::{AppTimerQueue, TimerHandle};
use crate::app::main_thread_queue::{MainThreadContext, MainThreadQueue};
use crate::core::WindowId;

#[derive(Clone, Default)]
pub(crate) struct AppRuntime {
    sessions: Arc<Mutex<BTreeMap<WindowId, SessionRuntime>>>,
}

#[derive(Clone)]
struct SessionRuntime {
    app_timers: AppTimerQueue,
    main_thread_queue: MainThreadQueue,
    alive: Arc<AtomicBool>,
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
        }
    }

    pub(crate) fn enqueue_with_context<F>(&self, window_id: WindowId, f: F)
    where
        F: for<'a> FnOnce(&mut MainThreadContext<'a>) + Send + 'static,
    {
        if let Some(session) = self.session(window_id) {
            session.main_thread_queue.enqueue_with_context(f);
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
}

#[cfg(test)]
#[path = "../tests/app/session_runtime.rs"]
mod tests;
