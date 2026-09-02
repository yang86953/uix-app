//! 次要窗口会话。
//!
//! 与主窗事件路径（event_loop）的有意差异，改动前先确认是否漂移：
//! - 主题富化：副窗的 `ThemeChanged` 只经主循环 foreign 路径
//!   （`dispatch_secondary_system_theme_changed` + `publish_theme_applied`）
//!   处理一次，本分发器不重复富化；
//! - debug 输入关联、指针光标同步与 Ctrl+Shift+D 热键仅主窗携带；
//! - 动作失败语义：副窗摘除自身窗口（返回 `false`），主窗终止应用循环；
//! - 装配段与聚焦写入经 `window_assembly` / `set_window_focused` 共享。

use super::*;

pub(crate) struct SecondaryWindowSession {
    // session 必须先于原生窗口析构，确保 engine/GL 资源先释放。
    pub(crate) session: WindowSession,
    pub(super) _window: Box<dyn PlatformWindow>,
    pub(crate) handle: AppHandle,
    pub(super) driver: WindowDriver,
    pub(crate) last_frame: Option<Instant>,
    // 副窗生命周期的窗口操作失败经此句柄进入框架报告。
    pub(super) diagnostics: crate::diagnostics::Diagnostics,
}

impl SecondaryWindowSession {
    pub(super) fn window_id(&self) -> WindowId {
        self.session.window_id()
    }

    pub(super) fn handle_event(&mut self, platform: &mut dyn PlatformSystem, event: &UiEvent) -> bool {
        let window_id = self.window_id();
        let native_window = self._window.native_handle().native_window();
        if event.type_ == UiEventType::WindowClose {
            self.handle.mark_closed();
            let parts = self.session.parts_mut();
            // 关闭前同步 IME 门控与树内聚焦投影两个副本，禁止单侧漂移。
            parts.text_input.window_focused = false;
            parts.tree.window_focused = false;
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
            parts.text_input,
            &self.diagnostics,
        );

        if let Some(system_event) = map_ui_event(event) {
            parts.tree.dispatch_event(&system_event);
            // 副窗口同样只消费当前原生事件携带的指针激活身份。
            if let Err(error) = apply_pending_window_actions(
                // 转交副窗口独占的 UI 树。
                parts.tree,
                // 转交事件所属的副原生窗口。
                self._window.as_mut(),
                // 保留该次 PointerDown 的平台激活因果关系。
                event.pointer_activation(),
                // 结束副窗口当前事件动作参数。
            ) {
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
        debug_mode: &crate::diagnostics::Diagnostics,
        cursor_pos: &Cell<Point>,
        clock: &dyn AppClock,
        platform: Option<&mut dyn PlatformSystem>,
    ) -> bool {
        let now = clock.now();
        let Self {
            session,
            _window,
            driver,
            last_frame,
            diagnostics,
            ..
        } = self;
        let parts = session.parts_mut();
        let mut no_runtime_tasks = |_platform: &mut dyn PlatformSystem, _tree: &mut WidgetTree| {};
        let no_frame = |_tree: &mut WidgetTree,
                        _engine: &mut dyn RenderTarget,
                        _platform: &mut dyn PlatformSystem| {};
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
            debug_correlation_id: debug_mode
                .debug_mode()
                .then(|| debug_mode.next_debug_correlation_id()),
            // 副窗事件路径不携带 debug 指针拦截，HUD 保持固定右上角形态。
            hud_state: None,
            cursor_pos,
            metrics: None,
            now,
            had_events: false,
            had_layout_event: false,
            next_external_deadline: None,
            on_runtime_tasks: &mut no_runtime_tasks,
            on_frame: &no_frame,
        });
        // 帧与运行时工作不能复用先前原生事件的激活身份。
        if let Err(error) = apply_pending_window_actions(
            // 转交副窗口独占的 UI 树。
            parts.tree,
            // 转交副窗口原生命令接收者。
            _window.as_mut(),
            // 明确声明该路径没有原生 PointerDown 上下文。
            None,
            // 结束副窗口运行时动作参数。
        ) {
            tracing::error!(
                "secondary window action failed after runtime work: {}",
                error.short_what()
            );
            report_window_operation_error(
                diagnostics,
                "secondary window action failure close request failed",
                _window.request_close(),
            );
        }
        *last_frame = driver.last_frame();
        result.did_work
    }

    pub(super) fn next_deadline(&mut self, now: Instant) -> Option<Instant> {
        let parts = self.session.parts_mut();
        self.driver.next_deadline(
            // 与 drain_frame 的注入 clock 保持同一时刻来源，虚拟时钟下不漂移。
            now,
            parts.tree,
            parts.active_work,
            &parts.app_timers,
            // 副窗口剩余队列以即时外部 deadline 唤醒应用主循环。
            &parts.main_thread_queue,
            parts.agent_commands,
            parts.pending_root,
            *parts.reconcile_pending,
        )
    }

    pub(super) fn close(mut self) {
        report_window_operation_error(
            &self.diagnostics,
            "secondary graphics shutdown failed",
            self.session.try_shutdown(),
        );
        // Drop retries a failed checked shutdown while the native surface is
        // still alive. Successful shutdown is idempotent.
        drop(self.session);
        report_window_operation_error(
            &self.diagnostics,
            "secondary close failed",
            self._window.close(),
        );
        self.handle.mark_closed();
    }
}

// ════════════════════════════════════════════════════════════════════════════
// App — 统一应用入口
// ════════════════════════════════════════════════════════════════════════════
