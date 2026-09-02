use crate::app::queues::active_work_registry::ActiveWorkRegistry;
use crate::app::queues::agent_command_queue::{AgentCommandQueue, AgentConfirmationRequest};
use crate::app::queues::app_timer::AppTimerQueue;
use crate::app::queues::main_thread_queue::MainThreadQueue;
use crate::app::queues::window_agent_state::{AgentCommandExecutor, WindowAgentState};
use crate::app::window_semantics::{AgentSemanticsPort, WindowSemanticState};
use crate::core::{Error, Rect, WindowId};
use crate::draw::scene::NodeId;
use crate::draw::target::RenderTarget;
use crate::ui::adapter::ViewAdapter;
use crate::ui::view::ViewNode;
use crate::ui::widget_runtime::widget::WidgetCore;
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
    pub(crate) fn build(
        &self,
        store: crate::ui::widget_state::WidgetStateStore,
    ) -> Option<ViewNode> {
        self.factory
            .as_ref()
            // 每次协调都使用所属窗口树的同一状态存储。
            .map(|factory| ViewAdapter::capture_root_with_store(store, || factory()))
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
            view_factory: ViewFactorySlot {
                factory: Some(factory),
            },
            text_input: WindowTextInputState::default(),
            semantic_state,
            agent_commands: WindowAgentState::new(),
        }
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
            // 主关闭路径首失败已由窗口操作边界上报；此处 teardown 重试失败
            // 经边界观察入口记录。
            crate::diagnostics::observe_boundary_error("window_session", &error);
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

    /// 写入窗口聚焦事实的单一入口：同步 IME 门控与树内聚焦投影两个副本。
    ///
    /// 副窗创建、主窗事件路径都必须经此写入，禁止单独改写
    /// `text_input.window_focused` 或 `tree.window_focused` 造成事实漂移；
    /// 树内的最终事实仍以 SystemEvent::WindowFocus/Blur 分发为准，本入口
    /// 只负责创建初期与窗口会话层的同步初值。
    pub(crate) fn set_window_focused(&mut self, focused: bool) {
        self.text_input.window_focused = focused;
        self.tree.window_focused = focused;
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

    /// 组装期注入 Agent 命令执行器（组合根提供，agent Module 实现）。
    pub(crate) fn set_agent_command_executor(
        &mut self,
        executor: std::sync::Arc<dyn AgentCommandExecutor>,
    ) {
        self.agent_commands.set_executor(executor);
    }

    /// 组装期注入 Agent 确认 UI 回调（应用配置，组合根转发）。
    pub(crate) fn set_agent_confirm_ui(
        &mut self,
        handler: Option<std::sync::Arc<dyn Fn(AgentConfirmationRequest) + Send + Sync>>,
    ) {
        self.agent_commands.set_confirm_ui(self.window_id, handler);
    }

    pub(crate) fn bind_agent_window(
        &mut self,
        registration: impl AgentSemanticsPort + 'static,
    ) -> bool {
        if !self
            .semantic_state
            .bind_agent_window(Box::new(registration))
        {
            return false;
        }
        let _ = self.semantic_state.enable(&self.tree);
        true
    }

    pub(crate) fn set_app_state(&mut self, app_state: AppState) {
        self.tree.set_app_state(app_state);
    }
}

impl Drop for WindowSession {
    fn drop(&mut self) {
        self.shutdown();
    }
}
