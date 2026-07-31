#![allow(dead_code)]

use crate::app::active_work_registry::ActiveWorkRegistry;
use crate::app::agent_bridge::AgentWindowRegistration;
use crate::app::agent_control::{AgentCommandQueue, WindowAgentState};
use crate::app::app_timer::AppTimerQueue;
use crate::app::main_thread_queue::MainThreadQueue;
use crate::app::window_semantics::{WindowSemanticSnapshot, WindowSemanticState};
use crate::core::{Error, Rect, WindowId};
use crate::draw::scene::NodeId;
use crate::draw::target::RenderTarget;
use crate::ui::adapter::ViewAdapter;
use crate::ui::component::widget::WidgetCore;
use crate::ui::view::ViewNode;
use crate::ui::{AppState, WidgetTree};
use std::sync::{Arc, Mutex};

type ViewFactory = Arc<dyn Fn() -> ViewNode + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowLoopState {
    DeepIdle,
    RegisteredActive,
    Active,
}

/// Coordinates the single native IME service without moving per-window IME
/// state out of its owning `WindowSession`.
#[derive(Clone, Default)]
pub(crate) struct TextInputCoordinator {
    active_window: Arc<Mutex<Option<WindowId>>>,
}

impl TextInputCoordinator {
    pub(crate) fn active_window(&self) -> Option<WindowId> {
        *self
            .active_window
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    pub(crate) fn activate(&self, window_id: WindowId) {
        *self
            .active_window
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(window_id);
    }

    pub(crate) fn deactivate(&self, window_id: WindowId) {
        let mut active = self
            .active_window
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if *active == Some(window_id) {
            *active = None;
        }
    }
}

pub(crate) struct WindowTextInputState {
    pub(crate) window_focused: bool,
    pub(crate) ime_session: Option<NodeId>,
    pub(crate) cursor_rect: Option<Rect>,
    pub(crate) coordinator: TextInputCoordinator,
}

impl Default for WindowTextInputState {
    fn default() -> Self {
        Self {
            window_focused: true,
            ime_session: None,
            cursor_rect: None,
            coordinator: TextInputCoordinator::default(),
        }
    }
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
    engine: Box<dyn RenderTarget>,
    engine_shutdown: bool,
    loop_state: WindowLoopState,
    active_work: ActiveWorkRegistry,
    app_timers: AppTimerQueue,
    main_thread_queue: MainThreadQueue,
    pending_root: Option<ViewNode>,
    reconcile_pending: bool,
    window_visible: bool,
    view_factory: ViewFactorySlot,
    text_input: WindowTextInputState,
    semantic_state: WindowSemanticState,
    agent_commands: WindowAgentState,
}

pub(crate) struct WindowSessionParts<'a> {
    pub(crate) tree: &'a mut WidgetTree,
    pub(crate) engine: &'a mut dyn RenderTarget,
    pub(crate) active_work: &'a mut ActiveWorkRegistry,
    pub(crate) app_timers: AppTimerQueue,
    pub(crate) main_thread_queue: MainThreadQueue,
    pub(crate) view_factory: &'a ViewFactorySlot,
    pub(crate) pending_root: &'a mut Option<ViewNode>,
    pub(crate) reconcile_pending: &'a mut bool,
    pub(crate) loop_state: &'a mut WindowLoopState,
    pub(crate) text_input: &'a mut WindowTextInputState,
    pub(crate) semantic_state: &'a mut WindowSemanticState,
    pub(crate) agent_commands: &'a mut WindowAgentState,
}

impl WindowSession {
    pub(crate) fn from_root(
        root_node: ViewNode,
        engine: Box<dyn RenderTarget>,
        width: i32,
        height: i32,
    ) -> Self {
        Self::from_root_for_window(WindowId::ROOT, root_node, engine, width, height)
    }

    pub(crate) fn from_root_for_window(
        window_id: WindowId,
        root_node: ViewNode,
        engine: Box<dyn RenderTarget>,
        width: i32,
        height: i32,
    ) -> Self {
        let mut tree = ViewAdapter::build_nodes(root_node);
        if let Some(root) = tree.root_mut() {
            root.set_frame(Rect::new(0.0, 0.0, width as f32, height as f32));
        }
        tree.layout();
        tree.mark_full_frame_dirty();
        #[cfg(feature = "test-harness")]
        tree.configure_automation_from_env(window_id);

        #[allow(unused_mut)]
        let mut semantic_state = WindowSemanticState::new(window_id);
        #[cfg(feature = "test-harness")]
        if tree.automation_snapshot_configured() {
            semantic_state.enable(&tree);
        }

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
            text_input: WindowTextInputState::default(),
            semantic_state,
            agent_commands: WindowAgentState::new(),
        }
    }

    pub(crate) fn from_root_factory<F>(
        build_root: F,
        engine: Box<dyn RenderTarget>,
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
        engine: Box<dyn RenderTarget>,
        width: i32,
        height: i32,
    ) -> Self
    where
        F: Fn() -> ViewNode + Send + Sync + 'static,
    {
        let factory: ViewFactory = Arc::new(build_root);
        let root = ViewAdapter::capture_root(|| factory());
        let mut tree = ViewAdapter::build_nodes(root);
        if let Some(r) = tree.root_mut() {
            r.set_frame(Rect::new(0.0, 0.0, width as f32, height as f32));
        }
        tree.layout();
        tree.mark_full_frame_dirty();
        #[cfg(feature = "test-harness")]
        tree.configure_automation_from_env(window_id);
        #[allow(unused_mut)]
        let mut semantic_state = WindowSemanticState::new(window_id);
        #[cfg(feature = "test-harness")]
        if tree.automation_snapshot_configured() {
            semantic_state.enable(&tree);
        }
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
            view_factory: ViewFactorySlot {
                factory: Some(factory),
            },
            text_input: WindowTextInputState::default(),
            semantic_state,
            agent_commands: WindowAgentState::new(),
        }
    }

    pub(crate) fn tree_and_engine_mut(&mut self) -> (&mut WidgetTree, &mut dyn RenderTarget) {
        (&mut self.tree, self.engine.as_mut())
    }

    pub(crate) fn try_shutdown(&mut self) -> Result<(), Error> {
        if self.engine_shutdown {
            return Ok(());
        }
        self.agent_commands.close();
        self.semantic_state.close();
        #[cfg(feature = "test-harness")]
        if let Some(snapshot) = self.semantic_state.snapshot() {
            self.tree.close_automation_snapshot(
                snapshot.generation,
                snapshot.revision,
                snapshot.presented_revision,
            );
        }
        self.tree.shutdown();
        self.engine.try_shutdown()?;
        self.engine_shutdown = true;
        Ok(())
    }

    pub(crate) fn shutdown(&mut self) {
        if let Err(error) = self.try_shutdown() {
            tracing::error!(
                "WindowSession checked shutdown failed: {}",
                error.short_what()
            );
        }
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
            text_input: &mut self.text_input,
            semantic_state: &mut self.semantic_state,
            agent_commands: &mut self.agent_commands,
        }
    }

    pub(crate) fn set_text_input_coordinator(&mut self, coordinator: TextInputCoordinator) {
        self.text_input.coordinator = coordinator;
    }

    pub(crate) fn set_window_focused(&mut self, focused: bool) {
        self.text_input.window_focused = focused;
    }

    pub(crate) fn set_app_timers(&mut self, app_timers: AppTimerQueue) {
        self.app_timers = app_timers;
    }

    pub(crate) fn set_main_thread_queue(&mut self, queue: MainThreadQueue) {
        self.main_thread_queue = queue;
    }

    pub(crate) fn set_agent_command_queue(&mut self, queue: AgentCommandQueue) {
        self.agent_commands.replace_queue(queue);
    }

    pub(crate) fn bind_agent_window(&mut self, registration: AgentWindowRegistration) -> bool {
        if !self.semantic_state.bind_agent_window(registration) {
            return false;
        }
        let _ = self.semantic_state.enable(&self.tree);
        true
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

    pub(crate) fn enable_semantic_tracking(&mut self) -> bool {
        self.semantic_state.enable(&self.tree)
    }

    pub(crate) fn semantic_snapshot(&self) -> Option<&WindowSemanticSnapshot> {
        self.semantic_state.snapshot()
    }

    pub(crate) fn agent_command_queue(&self) -> AgentCommandQueue {
        self.agent_commands.queue()
    }
}

impl Drop for WindowSession {
    fn drop(&mut self) {
        self.shutdown();
    }
}
