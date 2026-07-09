#![allow(dead_code)]

use crate::app::active_work_registry::ActiveWorkRegistry;
use crate::app::app_timer::AppTimerQueue;
use crate::app::main_thread_queue::MainThreadQueue;
use crate::core::{Rect, WindowId};
use crate::draw::traits::GraphicsEngine;
use crate::ui::core::widget::WidgetCore;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::{AppState, WidgetTree};
use std::sync::Arc;

type ViewFactory = Arc<dyn Fn() -> ViewNode + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowLoopState {
    DeepIdle,
    RegisteredActive,
    Active,
}

#[derive(Default)]
pub(crate) struct ViewFactorySlot {
    factory: Option<ViewFactory>,
}

impl ViewFactorySlot {
    pub(crate) fn is_installed(&self) -> bool {
        self.factory.is_some()
    }

    pub(crate) fn build(&self) -> Option<ViewNode> {
        self.factory
            .as_ref()
            .map(|factory| ViewAdapter::capture_root(|| factory()))
    }
}

pub(crate) struct WindowSession {
    window_id: WindowId,
    tree: WidgetTree,
    engine: Box<dyn GraphicsEngine>,
    engine_shutdown: bool,
    loop_state: WindowLoopState,
    active_work: ActiveWorkRegistry,
    app_timers: AppTimerQueue,
    main_thread_queue: MainThreadQueue,
    pending_root: Option<ViewNode>,
    reconcile_pending: bool,
    window_visible: bool,
    view_factory: ViewFactorySlot,
}

pub(crate) struct WindowSessionParts<'a> {
    pub(crate) tree: &'a mut WidgetTree,
    pub(crate) engine: &'a mut dyn GraphicsEngine,
    pub(crate) active_work: &'a mut ActiveWorkRegistry,
    pub(crate) app_timers: AppTimerQueue,
    pub(crate) main_thread_queue: MainThreadQueue,
    pub(crate) view_factory: &'a ViewFactorySlot,
    pub(crate) pending_root: &'a mut Option<ViewNode>,
    pub(crate) reconcile_pending: &'a mut bool,
    pub(crate) loop_state: &'a mut WindowLoopState,
}

impl WindowSession {
    pub(crate) fn from_root(
        root_node: ViewNode,
        engine: Box<dyn GraphicsEngine>,
        width: i32,
        height: i32,
    ) -> Self {
        Self::from_root_for_window(WindowId::ROOT, root_node, engine, width, height)
    }

    pub(crate) fn from_root_for_window(
        window_id: WindowId,
        root_node: ViewNode,
        engine: Box<dyn GraphicsEngine>,
        width: i32,
        height: i32,
    ) -> Self {
        let mut tree = ViewAdapter::build_nodes(root_node);
        if let Some(root) = tree.root_mut() {
            root.set_frame(Rect::new(0.0, 0.0, width as f32, height as f32));
        }
        tree.layout();
        tree.mark_full_frame_dirty();

        Self {
            window_id,
            tree,
            engine,
            engine_shutdown: false,
            loop_state: WindowLoopState::Active,
            active_work: ActiveWorkRegistry::new(),
            app_timers: AppTimerQueue::new(),
            main_thread_queue: MainThreadQueue::new(),
            pending_root: None,
            reconcile_pending: false,
            window_visible: true,
            view_factory: ViewFactorySlot::default(),
        }
    }

    pub(crate) fn from_root_factory<F>(
        build_root: F,
        engine: Box<dyn GraphicsEngine>,
        width: i32,
        height: i32,
    ) -> Self
    where
        F: Fn() -> ViewNode + Send + Sync + 'static,
    {
        Self::from_root_factory_for_window(WindowId::ROOT, build_root, engine, width, height)
    }

    pub(crate) fn from_root_factory_for_window<F>(
        window_id: WindowId,
        build_root: F,
        engine: Box<dyn GraphicsEngine>,
        width: i32,
        height: i32,
    ) -> Self
    where
        F: Fn() -> ViewNode + Send + Sync + 'static,
    {
        let factory: ViewFactory = Arc::new(build_root);
        let root = ViewAdapter::capture_root(|| factory());
        let mut session = Self::from_root_for_window(window_id, root, engine, width, height);
        session.view_factory = ViewFactorySlot {
            factory: Some(factory),
        };
        session
    }

    pub(crate) fn tree_and_engine_mut(&mut self) -> (&mut WidgetTree, &mut dyn GraphicsEngine) {
        (&mut self.tree, self.engine.as_mut())
    }

    pub(crate) fn shutdown(&mut self) {
        if self.engine_shutdown {
            return;
        }
        self.engine_shutdown = true;
        self.engine.shutdown();
    }

    pub(crate) fn window_id(&self) -> WindowId {
        self.window_id
    }

    pub(crate) fn parts_mut(&mut self) -> WindowSessionParts<'_> {
        WindowSessionParts {
            tree: &mut self.tree,
            engine: self.engine.as_mut(),
            active_work: &mut self.active_work,
            app_timers: self.app_timers.clone(),
            main_thread_queue: self.main_thread_queue.clone(),
            view_factory: &self.view_factory,
            pending_root: &mut self.pending_root,
            reconcile_pending: &mut self.reconcile_pending,
            loop_state: &mut self.loop_state,
        }
    }

    pub(crate) fn set_app_timers(&mut self, app_timers: AppTimerQueue) {
        self.app_timers = app_timers;
    }

    pub(crate) fn set_main_thread_queue(&mut self, queue: MainThreadQueue) {
        self.main_thread_queue = queue;
    }

    pub(crate) fn set_app_state(&mut self, app_state: AppState) {
        self.tree.set_app_state(app_state);
    }

    pub(crate) fn active_work_mut(&mut self) -> &mut ActiveWorkRegistry {
        &mut self.active_work
    }

    pub(crate) fn loop_state(&self) -> WindowLoopState {
        self.loop_state
    }

    pub(crate) fn active_work(&self) -> &ActiveWorkRegistry {
        &self.active_work
    }

    pub(crate) fn main_thread_queue(&self) -> MainThreadQueue {
        self.main_thread_queue.clone()
    }

    pub(crate) fn request_reconcile(&mut self) {
        self.reconcile_pending = true;
    }

    pub(crate) fn reconcile_pending(&self) -> bool {
        self.reconcile_pending
    }

    pub(crate) fn take_pending_root(&mut self) -> Option<ViewNode> {
        self.pending_root.take()
    }

    pub(crate) fn window_visible(&self) -> bool {
        self.window_visible
    }

    pub(crate) fn view_factory(&self) -> &ViewFactorySlot {
        &self.view_factory
    }
}

impl Drop for WindowSession {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
#[path = "../tests/app/window_session.rs"]
mod tests;
