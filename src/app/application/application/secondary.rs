//! 次要窗口会话。

use super::*;

pub(crate) struct SecondaryWindowSession {
    // session 必须先于原生窗口析构，确保 engine/GL 资源先释放。
    pub(crate) session: WindowSession,
    pub(super) _window: Box<dyn PlatformWindow>,
    pub(crate) handle: AppHandle,
    pub(super) driver: WindowDriver,
    pub(crate) last_frame: Option<Instant>,
}

impl SecondaryWindowSession {
    pub(super) fn window_id(&self) -> WindowId {
        self.session.window_id()
    }

    pub(super) fn handle_event(&mut self, platform: &mut dyn Platform, event: &UiEvent) -> bool {
        let window_id = self.window_id();
        let native_window = self._window.native_handle().native_window();
        if event.type_ == UiEventType::WindowClose {
            self.handle.mark_closed();
            let parts = self.session.parts_mut();
            parts.text_input.window_focused = false;
            sync_window_text_input(
                parts.tree,
                parts.active_work,
                parts.text_input,
                window_id,
                native_window,
                platform,
            );
            return false;
        }

        let parts = self.session.parts_mut();
        self.driver.handle_window_event(
            event,
            parts.tree,
            parts.engine,
            self._window.as_mut(),
            platform,
            parts.text_input,
        );

        if let Some(system_event) = map_ui_event(event) {
            parts.tree.dispatch_event(&system_event);
            if let Err(error) = apply_pending_window_actions(parts.tree, self._window.as_mut()) {
                tracing::error!("secondary window action failed: {}", error.short_what());
                return false;
            }
        }
        platform.event_bus().publish(event);
        sync_window_text_input(
            parts.tree,
            parts.active_work,
            parts.text_input,
            window_id,
            native_window,
            platform,
        );
        true
    }

    pub(super) fn drain_main_thread_work(&mut self) -> bool {
        let parts = self.session.parts_mut();
        let mut main_thread_context =
            MainThreadContext::new(parts.pending_root, parts.reconcile_pending);
        let had_main_thread_work = parts.main_thread_queue.drain(&mut main_thread_context);
        let had_app_state_focus_work = parts.tree.drain_app_state_focus_requests();
        let had_app_state_semantic_work = parts.tree.drain_app_state_semantic_events();

        had_main_thread_work
            || had_app_state_focus_work
            || had_app_state_semantic_work
            || *parts.reconcile_pending
    }

    pub(super) fn has_frame_work(&mut self, now: Instant) -> bool {
        let parts = self.session.parts_mut();
        self.driver.has_frame_work(
            now,
            parts.tree,
            parts.active_work,
            &parts.app_timers,
            &parts.main_thread_queue,
            parts.agent_commands,
            parts.pending_root,
            parts.reconcile_pending,
        )
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "frame draining preserves the established application service boundary"
    )]
    pub(super) fn drain_frame(
        &mut self,
        font_service: &FontService,
        image_service: &ImageService,
        theme: &RefCell<Theme>,
        debug_mode: &Cell<bool>,
        cursor_pos: &Cell<Point>,
        clock: &dyn AppClock,
        platform: Option<&mut dyn Platform>,
    ) -> bool {
        let now = clock.now();
        let Self {
            session,
            _window,
            driver,
            last_frame,
            ..
        } = self;
        let parts = session.parts_mut();
        let mut no_runtime_tasks = |_platform: &mut dyn Platform, _tree: &mut WidgetTree| {};
        let no_frame = |_tree: &mut WidgetTree,
                        _engine: &mut dyn RenderTarget,
                        _platform: &mut dyn Platform| {};
        let result = driver.drive_frame(WindowFrameContext {
            tree: parts.tree,
            engine: parts.engine,
            active_work: parts.active_work,
            app_timers: &parts.app_timers,
            main_thread_queue: &parts.main_thread_queue,
            agent_commands: parts.agent_commands,
            view_factory: Some(parts.view_factory),
            pending_root: parts.pending_root,
            reconcile_pending: parts.reconcile_pending,
            loop_state: parts.loop_state,
            text_input: parts.text_input,
            semantic_state: parts.semantic_state,
            platform_window: _window.as_mut(),
            platform,
            font_service,
            image_service,
            theme,
            debug_mode,
            cursor_pos,
            metrics: None,
            now,
            had_events: false,
            had_layout_event: false,
            input_us: 0,
            next_external_deadline: None,
            on_runtime_tasks: &mut no_runtime_tasks,
            on_frame: &no_frame,
        });
        if let Err(error) = apply_pending_window_actions(parts.tree, _window.as_mut()) {
            tracing::error!(
                "secondary window action failed after runtime work: {}",
                error.short_what()
            );
            report_window_operation_error(
                "secondary window action failure close request failed",
                _window.request_close(),
            );
        }
        *last_frame = driver.last_frame();
        result.did_work
    }

    pub(super) fn next_deadline(&mut self) -> Option<Instant> {
        let parts = self.session.parts_mut();
        self.driver.next_deadline(
            Instant::now(),
            parts.tree,
            parts.active_work,
            &parts.app_timers,
            parts.agent_commands,
            parts.pending_root,
            *parts.reconcile_pending,
        )
    }

    pub(super) fn close(mut self) {
        report_window_operation_error(
            "secondary graphics shutdown failed",
            self.session.try_shutdown(),
        );
        // Drop retries a failed checked shutdown while the native surface is
        // still alive. Successful shutdown is idempotent.
        drop(self.session);
        report_window_operation_error("secondary close failed", self._window.close());
        self.handle.mark_closed();
    }
}

// ════════════════════════════════════════════════════════════════════════════
// App — 统一应用入口
// ════════════════════════════════════════════════════════════════════════════
