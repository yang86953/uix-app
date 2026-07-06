use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Duration;

use crate::app::app_timer::TimerHandle;
use crate::app::session_runtime::AppRuntime;
use crate::app::shell::di::Container;
pub use crate::core::WindowId;
use crate::ui::view::{View, ViewAdapter, ViewNode};
use crate::ui::AppState;

#[derive(Clone)]
pub struct AppHandle {
    window_id: WindowId,
    app_state: AppState,
    runtime: AppRuntime,
    container: Container,
    alive: Arc<AtomicBool>,
}

impl AppHandle {
    pub(crate) fn new(
        window_id: WindowId,
        app_state: AppState,
        runtime: AppRuntime,
        container: Container,
        alive: Arc<AtomicBool>,
    ) -> Self {
        Self {
            window_id,
            app_state,
            runtime,
            container,
            alive,
        }
    }

    pub fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub fn app_state(&self) -> AppState {
        self.app_state.clone()
    }

    pub fn resolve<T: 'static + Send + Clone>(&self) -> Option<T> {
        self.container.resolve_clone::<T>()
    }

    pub fn run_after<F>(&self, delay: Duration, f: F) -> TimerHandle
    where
        F: FnOnce() + Send + 'static,
    {
        if !self.alive.load(Ordering::Acquire) {
            return TimerHandle::inactive();
        }
        self.runtime.run_after(self.window_id, delay, f)
    }

    pub fn run_interval<F>(&self, interval: Duration, f: F) -> TimerHandle
    where
        F: FnMut() + Send + 'static,
    {
        if !self.alive.load(Ordering::Acquire) {
            return TimerHandle::inactive();
        }
        self.runtime.run_interval(self.window_id, interval, f)
    }

    pub fn post_to_ui<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        if self.alive.load(Ordering::Acquire) {
            self.runtime.post_to_ui(self.window_id, f);
        }
    }

    pub fn update_view<F>(&self, build_root: F)
    where
        F: FnOnce() -> ViewNode + Send + 'static,
    {
        if self.alive.load(Ordering::Acquire) {
            self.runtime
                .enqueue_with_context(self.window_id, move |ctx| {
                    let root = ViewAdapter::capture_root(build_root);
                    ctx.update_root(root);
                });
        }
    }

    pub fn set_root<V>(&self, view: V)
    where
        V: View + Send + 'static,
    {
        self.update_view(move || view.build());
    }

    pub(crate) fn mark_closed(&self) {
        self.alive.store(false, Ordering::Release);
        self.runtime.close_session(self.window_id);
    }
}

#[cfg(test)]
#[path = "../tests/app/app_handle.rs"]
mod tests;
