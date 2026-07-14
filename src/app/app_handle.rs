use std::collections::HashMap;
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};

use crate::app::app_timer::TimerHandle;
use crate::app::session_runtime::AppRuntime;
use crate::app::shell::di::Container;
use crate::app::window_config::WindowConfig;
pub use crate::core::WindowId;
use crate::core::{ComponentId, Constraints, Errc, Error, Rect, Result, Size};
use crate::impl_widget_component;
use crate::native::notification::ToastEntry;
use crate::ui::traits::WidgetLayout;
use crate::ui::view::{View, ViewAdapter, ViewNode};
use crate::ui::widgets::feedback::notification::{
    Notification, NotificationHandle, NotificationItem,
};
use crate::ui::{AppState, Theme};

#[derive(Default)]
pub(crate) struct AppOverlayRoot;

impl_widget_component!(AppOverlayRoot; Layout);

impl WidgetLayout for AppOverlayRoot {
    fn measure(&self, constraints: Constraints) -> Size {
        constraints.definite.unwrap_or_default()
    }

    fn layout_children(
        &self,
        frame: Rect,
        children: &[crate::ui::LayoutChild],
        _tree: &crate::ui::WidgetTree,
    ) -> Vec<(ComponentId, Rect)> {
        children.iter().map(|child| (child.id, frame)).collect()
    }
}

#[derive(Clone)]
pub(crate) struct AppNotificationState {
    windows: Arc<Mutex<HashMap<WindowId, NotificationHandle>>>,
    next_id: Arc<AtomicU64>,
    max_visible: usize,
}

impl AppNotificationState {
    pub(crate) fn new() -> Self {
        Self {
            windows: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(AtomicU64::new(0)),
            max_visible: 5,
        }
    }

    pub(crate) fn handle(&self, window_id: WindowId) -> NotificationHandle {
        self.windows
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .entry(window_id)
            .or_insert_with(NotificationHandle::new)
            .clone()
    }

    pub(crate) fn notify_error(&self, window_id: WindowId, error: &Error) -> Option<u64> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let toast = ToastEntry::from_error(id, error, Instant::now())?;
        let item = NotificationItem::from_toast_entry(&toast);
        let handle = self.handle(window_id);
        handle.push_external(id, item);
        handle.retain_latest(self.max_visible);
        Some(id)
    }

    pub(crate) fn remove_window(&self, window_id: WindowId) {
        self.windows
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(&window_id);
    }
}

pub(crate) fn wrap_root_with_notification_overlay(
    root: ViewNode,
    notifications: AppNotificationState,
    window_id: WindowId,
) -> ViewNode {
    let notification = Notification::from_handle(notifications.handle(window_id));
    ViewNode::new(
        AppOverlayRoot,
        vec![root, ViewNode::leaf(notification).z_index(10_000)],
    )
}

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

    pub fn resolve<T: 'static + Send + Sync + Clone>(&self) -> Option<T> {
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

    /// Replaces the app-wide theme and schedules palette-only repaint work for
    /// every live window.
    pub fn set_theme(&self, theme: Theme) -> Result<()> {
        if !self.alive.load(Ordering::Acquire) {
            return Err(Error::new(
                Errc::InvalidState,
                "cannot set the theme from a closed AppHandle",
            ));
        }
        self.runtime.set_theme(theme);
        Ok(())
    }

    pub fn notify_error(&self, error: &Error) -> Option<u64> {
        if !self.alive.load(Ordering::Acquire) {
            return None;
        }
        let notifications = self.container.resolve_clone::<AppNotificationState>()?;
        let id = notifications.notify_error(self.window_id, error)?;
        self.runtime.post_to_ui(self.window_id, || {});
        Some(id)
    }

    pub fn dismiss_notification(&self, id: u64) -> bool {
        if !self.alive.load(Ordering::Acquire) {
            return false;
        }
        let Some(notifications) = self.container.resolve_clone::<AppNotificationState>() else {
            return false;
        };
        let dismissed = notifications.handle(self.window_id).dismiss_external(id);
        if dismissed {
            self.runtime.post_to_ui(self.window_id, || {});
        }
        dismissed
    }

    pub fn update_view<F>(&self, build_root: F)
    where
        F: FnOnce() -> ViewNode + Send + 'static,
    {
        if self.alive.load(Ordering::Acquire) {
            let notifications = self.container.resolve_clone::<AppNotificationState>();
            let window_id = self.window_id;
            self.runtime
                .enqueue_with_context(self.window_id, move |ctx| {
                    let root = ViewAdapter::capture_root(build_root);
                    let root = match notifications {
                        Some(state) => wrap_root_with_notification_overlay(root, state, window_id),
                        None => root,
                    };
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

    pub fn open_window(&self, config: WindowConfig) -> Result<AppHandle> {
        if !self.alive.load(Ordering::Acquire) {
            return Err(Error::new(
                Errc::InvalidState,
                "cannot open a window from a closed AppHandle",
            ));
        }
        let session = self.runtime.request_open_window(config);
        Ok(Self::new(
            session.window_id,
            self.app_state.clone(),
            self.runtime.clone(),
            self.container.clone(),
            session.alive,
        ))
    }

    pub(crate) fn mark_closed(&self) {
        self.alive.store(false, Ordering::Release);
        if let Some(notifications) = self.container.resolve_clone::<AppNotificationState>() {
            notifications.remove_window(self.window_id);
        }
        self.runtime.close_session(self.window_id);
    }
}
