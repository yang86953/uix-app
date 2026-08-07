//! 应用运行时辅助：图形选择、副窗编排与平台事件映射。

use super::*;
use crate::diagnostics::{Diagnostics, PendingFailureQueue};
use crate::draw::renderer::{RebuildRequest, RecoveryAction};

/// Drains native callback failures at the application owner-thread boundary.
///
/// Native callbacks only enqueue typed errors. This function is intentionally
/// called by the app loop, where reporting or a future domain recovery action
/// is allowed to run. Registered recovery handlers run first at this safe
/// point; only errors that remain unhandled reach the final report, matching
/// the "报告不能恢复的" runtime guarantee.
pub(crate) fn drain_platform_pending_failures(
    platform: &mut dyn Platform,
    diagnostics: &Diagnostics,
) -> usize {
    let mut drained = 0;
    while let Some(error) = platform.take_pending_failure() {
        if let Some(unresolved) = diagnostics.attempt_recovery(error).into_error() {
            diagnostics.report(unresolved);
        }
        drained += 1;
    }
    drained
}

pub(crate) fn resolve_graphics_backend(
    builder: Option<NativeGraphicsBackend>,
    env_value: Option<&str>,
    settings: Option<&SettingsService>,
) -> NativeGraphicsBackend {
    if let Some(backend) = builder {
        return backend;
    }

    if let Some(value) = env_value {
        if let Some(backend) = parse_graphics_backend_config(GRAPHICS_BACKEND_ENV, value) {
            return backend;
        }
    }

    if let Some(settings) = settings {
        for key in GRAPHICS_BACKEND_SETTING_KEYS {
            if let Some(value) = settings.get(key) {
                if let Some(backend) = parse_graphics_backend_config(key, &value) {
                    return backend;
                }
            }
        }
    }

    NativeGraphicsBackend::Auto
}

fn parse_graphics_backend_config(source: &str, value: &str) -> Option<NativeGraphicsBackend> {
    match value.parse::<NativeGraphicsBackend>() {
        Ok(backend) => Some(backend),
        Err(err) => {
            tracing::warn!(
                "graphics backend config {source} ignored: {}",
                err.short_what()
            );
            None
        }
    }
}

// 测试目标保留默认图形后端的 pending-window 便捷入口，供外部 GUI 测试按需调用。
#[cfg_attr(test, allow(dead_code))]
#[cfg(test)]
pub(crate) fn drain_pending_open_windows(
    platform: &mut dyn Platform,
    runtime: &AppRuntime,
    app_state: &AppState,
    container: &Container,
    on_window_start: Option<&Arc<dyn Fn(AppHandle) + Send + Sync>>,
    secondary_windows: &mut Vec<SecondaryWindowSession>,
) -> usize {
    drain_pending_open_windows_with_backend(
        platform,
        runtime,
        app_state,
        container,
        NativeGraphicsBackend::Auto,
        RebuildRequest::default(),
        on_window_start,
        secondary_windows,
    )
}

pub(crate) fn drain_pending_open_windows_with_backend(
    platform: &mut dyn Platform,
    runtime: &AppRuntime,
    app_state: &AppState,
    container: &Container,
    graphics_backend: NativeGraphicsBackend,
    recovery_request: RebuildRequest,
    on_window_start: Option<&Arc<dyn Fn(AppHandle) + Send + Sync>>,
    secondary_windows: &mut Vec<SecondaryWindowSession>,
) -> usize {
    let mut created = 0;
    while let Some(request) = runtime.take_next_open_window() {
        if let Some(mut window) = create_secondary_window(
            platform,
            runtime,
            app_state,
            container,
            graphics_backend,
            recovery_request.clone(),
            request,
        ) {
            if let Some(callback) = on_window_start {
                callback(window.handle.clone());
            }
            window.drain_main_thread_work();
            secondary_windows.push(window);
            created += 1;
        }
    }
    created
}

pub(crate) fn drain_secondary_window_queues(
    secondary_windows: &mut [SecondaryWindowSession],
) -> bool {
    let mut drained = false;
    for window in secondary_windows {
        drained |= window.drain_main_thread_work();
    }
    drained
}

// 测试目标保留无平台参数的 secondary-window 帧便捷入口，供外部 GUI 测试按需调用。
#[cfg_attr(test, allow(dead_code))]
#[cfg(test)]
pub(crate) fn drain_secondary_window_frames(
    secondary_windows: &mut [SecondaryWindowSession],
    font_service: &FontService,
    image_service: &ImageService,
    theme: &RefCell<Theme>,
    debug_mode: &Cell<bool>,
    cursor_pos: &Cell<Point>,
    clock: &dyn AppClock,
) -> bool {
    drain_secondary_window_frames_impl(
        None,
        secondary_windows,
        font_service,
        image_service,
        theme,
        debug_mode,
        cursor_pos,
        clock,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "secondary window draining mirrors the application service boundary"
)]
pub(super) fn drain_secondary_window_frames_with_platform(
    platform: &mut dyn Platform,
    secondary_windows: &mut [SecondaryWindowSession],
    font_service: &FontService,
    image_service: &ImageService,
    theme: &RefCell<Theme>,
    debug_mode: &Cell<bool>,
    cursor_pos: &Cell<Point>,
    clock: &dyn AppClock,
) -> bool {
    drain_secondary_window_frames_impl(
        Some(platform),
        secondary_windows,
        font_service,
        image_service,
        theme,
        debug_mode,
        cursor_pos,
        clock,
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "secondary window draining mirrors the application service boundary"
)]
fn drain_secondary_window_frames_impl(
    platform: Option<&mut dyn Platform>,
    secondary_windows: &mut [SecondaryWindowSession],
    font_service: &FontService,
    image_service: &ImageService,
    theme: &RefCell<Theme>,
    debug_mode: &Cell<bool>,
    cursor_pos: &Cell<Point>,
    clock: &dyn AppClock,
) -> bool {
    let mut drained = false;
    let now = clock.now();
    match platform {
        Some(platform) => {
            for window in secondary_windows {
                if window.has_frame_work(now) {
                    drained |= window.drain_frame(
                        font_service,
                        image_service,
                        theme,
                        debug_mode,
                        cursor_pos,
                        clock,
                        Some(&mut *platform),
                    );
                }
            }
        }
        None => {
            for window in secondary_windows {
                if window.has_frame_work(now) {
                    drained |= window.drain_frame(
                        font_service,
                        image_service,
                        theme,
                        debug_mode,
                        cursor_pos,
                        clock,
                        None,
                    );
                }
            }
        }
    }
    drained
}

pub(crate) fn secondary_windows_next_deadline(
    secondary_windows: &mut [SecondaryWindowSession],
) -> Option<Instant> {
    secondary_windows
        .iter_mut()
        .filter_map(SecondaryWindowSession::next_deadline)
        .min()
}

pub(crate) fn dispatch_secondary_window_event(
    secondary_windows: &mut Vec<SecondaryWindowSession>,
    platform: &mut dyn Platform,
    event: &UiEvent,
) -> bool {
    let Some(window_id) = event.window_id else {
        return false;
    };
    let Some(index) = secondary_windows
        .iter()
        .position(|window| window.window_id() == window_id)
    else {
        return false;
    };

    if secondary_windows[index].handle_event(platform, event) {
        true
    } else {
        secondary_windows.remove(index).close();
        true
    }
}

pub(crate) fn dispatch_secondary_system_theme_changed(
    secondary_windows: &mut [SecondaryWindowSession],
    is_dark: bool,
) -> bool {
    let mut dispatched = false;
    for window in secondary_windows {
        window
            .session
            .parts_mut()
            .tree
            .dispatch_event(&SystemEvent::ThemeChanged { is_dark });
        dispatched = true;
    }
    dispatched
}

pub(crate) fn apply_runtime_theme_change(
    theme: &RefCell<Theme>,
    root_tree: &mut WidgetTree,
    secondary_windows: &mut [SecondaryWindowSession],
    next_theme: Theme,
) {
    let is_dark = next_theme.is_dark();
    *theme.borrow_mut() = next_theme;
    root_tree.dispatch_event(&SystemEvent::ThemeChanged { is_dark });
    dispatch_secondary_system_theme_changed(secondary_windows, is_dark);
}

fn format_probe_failures(report: &ProbeReport) -> String {
    if report.failures.is_empty() {
        return "none".to_string();
    }

    report
        .failures
        .iter()
        .enumerate()
        .map(|(index, failure)| {
            format!(
                "failure[{index}]={{candidate={}, detail={:?}}}",
                failure.backend, failure.message
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

pub(crate) fn format_gpu_probe_fallback(
    request: NativeGraphicsBackend,
    report: &ProbeReport,
) -> String {
    format!(
        "GPU probe exhausted; request={request}; platform={}; fallback=software_cpu; failures=[{}]",
        graphics_runtime_platform(),
        format_probe_failures(report)
    )
}

// ════════════════════════════════════════════════════════════════════════════
// UiEvent → SystemEvent 映射（唯一实现）
// ════════════════════════════════════════════════════════════════════════════

/// 将平台 `UiEvent` 转换为 `SystemEvent`。
pub fn map_ui_event(ev: &UiEvent) -> Option<SystemEvent> {
    match ev.type_ {
        UiEventType::PointerDown => {
            if let UiEventPayload::PointerButton(ref d) = ev.payload {
                Some(SystemEvent::PointerDown {
                    pos: d.pos,
                    button: d.btn,
                    mods: d.mods,
                })
            } else {
                None
            }
        }
        UiEventType::PointerDoubleClick => {
            if let UiEventPayload::PointerButton(ref d) = ev.payload {
                Some(SystemEvent::PointerDoubleClick {
                    pos: d.pos,
                    button: d.btn,
                    mods: d.mods,
                })
            } else {
                None
            }
        }
        UiEventType::PointerUp => {
            if let UiEventPayload::PointerButton(ref d) = ev.payload {
                Some(SystemEvent::PointerUp {
                    pos: d.pos,
                    button: d.btn,
                    mods: d.mods,
                })
            } else {
                None
            }
        }
        UiEventType::PointerMove => {
            if let UiEventPayload::PointerMove(ref d) = ev.payload {
                Some(SystemEvent::PointerMove {
                    pos: d.pos,
                    mods: d.mods,
                })
            } else {
                None
            }
        }
        UiEventType::Wheel => {
            if let UiEventPayload::Wheel(ref d) = ev.payload {
                Some(SystemEvent::Wheel {
                    pos: d.pos,
                    delta: Point::new(d.delta_x, d.delta_y),
                })
            } else {
                None
            }
        }
        UiEventType::KeyDown => {
            if let UiEventPayload::Key(ref d) = ev.payload {
                Some(SystemEvent::KeyDown {
                    key: d.key,
                    mods: d.mods,
                })
            } else {
                None
            }
        }
        UiEventType::KeyUp => {
            if let UiEventPayload::Key(ref d) = ev.payload {
                Some(SystemEvent::KeyUp {
                    key: d.key,
                    mods: d.mods,
                })
            } else {
                None
            }
        }
        UiEventType::Copy => Some(SystemEvent::Copy),
        UiEventType::Cut => Some(SystemEvent::Cut),
        UiEventType::Paste => {
            if let UiEventPayload::Clipboard(ref d) = ev.payload {
                Some(SystemEvent::Paste {
                    text: d.text.clone(),
                })
            } else {
                None
            }
        }
        UiEventType::TextInput => {
            if let UiEventPayload::TextInput(ref d) = ev.payload {
                Some(SystemEvent::TextInput {
                    text: d.text.clone(),
                })
            } else {
                None
            }
        }
        UiEventType::ImeCompositionStart => Some(SystemEvent::ImeCompositionStart),
        UiEventType::ImeCompositionUpdate => {
            if let UiEventPayload::ImeComposition(ref d) = ev.payload {
                Some(SystemEvent::ImeCompositionUpdate {
                    text: d.text.clone(),
                })
            } else {
                None
            }
        }
        UiEventType::ImeCompositionEnd => {
            if let UiEventPayload::ImeComposition(ref d) = ev.payload {
                Some(SystemEvent::ImeCompositionEnd {
                    text: d.text.clone(),
                })
            } else {
                None
            }
        }
        UiEventType::ThemeChanged => {
            if let UiEventPayload::ThemeChanged(ref d) = ev.payload {
                Some(SystemEvent::ThemeChanged { is_dark: d.is_dark })
            } else {
                None
            }
        }
        UiEventType::LocaleChanged => {
            if let UiEventPayload::LocaleChanged(ref d) = ev.payload {
                Some(SystemEvent::LocaleChanged {
                    locale: d.locale.clone(),
                })
            } else {
                None
            }
        }
        UiEventType::WindowResize => {
            if let UiEventPayload::Resize(ref d) = ev.payload {
                Some(SystemEvent::Resize {
                    width: d.width as f32,
                    height: d.height as f32,
                })
            } else {
                None
            }
        }
        UiEventType::WindowMaximize => Some(SystemEvent::WindowMaximize),
        UiEventType::WindowMinimize => Some(SystemEvent::WindowMinimize),
        UiEventType::WindowRestore => Some(SystemEvent::WindowRestore),
        UiEventType::WindowFocus => Some(SystemEvent::WindowFocus),
        UiEventType::WindowBlur => Some(SystemEvent::WindowBlur),
        UiEventType::Timer => {
            if let UiEventPayload::Timer(ref d) = ev.payload {
                Some(SystemEvent::Timer { id: d.timer_id })
            } else {
                None
            }
        }
        UiEventType::FileDrop => {
            if let UiEventPayload::FileDrop(ref d) = ev.payload {
                Some(SystemEvent::FileDrop {
                    files: d.files.clone(),
                    position: d.position,
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Errc;
    use crate::diagnostics::{Diagnostics, DiagnosticsConfig, RecoveryAction};
    use crate::native::capabilities::*;
    use crate::native::platform::Platform;
    use crate::native::windowing::event::EventBus;
    use crate::native::windowing::*;
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn new_diagnostics() -> Diagnostics {
        Diagnostics::new(DiagnosticsConfig::default())
    }

    /// 最小 owner-thread 平台桩：只提供 pending failure 取出，其余能力不可达。
    struct PendingPlatform {
        pending: VecDeque<Error>,
    }

    impl PendingPlatform {
        fn enqueue(&mut self, error: Error) {
            self.pending.push_back(error);
        }
    }

    impl Platform for PendingPlatform {
        fn take_pending_failure(&mut self) -> Option<Error> {
            self.pending.pop_front()
        }

        fn window_manager(&mut self) -> &mut dyn IWindowManager {
            unimplemented!("not needed by the drain safe-point test")
        }

        fn event_loop(&mut self) -> &mut dyn IEventLoop {
            unimplemented!("not needed by the drain safe-point test")
        }

        fn event_bus(&mut self) -> &mut EventBus {
            unimplemented!("not needed by the drain safe-point test")
        }

        fn clipboard(&mut self) -> &mut dyn IClipboard {
            unimplemented!("not needed by the drain safe-point test")
        }

        fn cursor(&mut self) -> &mut dyn ICursor {
            unimplemented!("not needed by the drain safe-point test")
        }

        fn display(&self) -> &dyn IDisplay {
            unimplemented!("not needed by the drain safe-point test")
        }

        fn file_dialog(&mut self) -> &mut dyn IFileDialog {
            unimplemented!("not needed by the drain safe-point test")
        }

        fn keyboard(&self) -> &dyn IKeyboard {
            unimplemented!("not needed by the drain safe-point test")
        }

        fn text_input(&mut self) -> &mut dyn ITextInput {
            unimplemented!("not needed by the drain safe-point test")
        }

        fn timer(&mut self) -> &mut dyn ITimer {
            unimplemented!("not needed by the drain safe-point test")
        }

        fn notification(&mut self) -> &mut dyn INotification {
            unimplemented!("not needed by the drain safe-point test")
        }

        fn console(&mut self) -> &mut dyn IConsole {
            unimplemented!("not needed by the drain safe-point test")
        }

        fn file_system(&self) -> &dyn IFileSystem {
            unimplemented!("not needed by the drain safe-point test")
        }

        fn system_info(&self) -> &dyn ISystemInfo {
            unimplemented!("not needed by the drain safe-point test")
        }
    }

    #[test]
    fn drain_recovered_failure_is_not_reported_and_unhandled_is_reported_once() {
        let diagnostics = new_diagnostics();
        let mut platform = PendingPlatform {
            pending: VecDeque::new(),
        };
        let handled = Arc::new(AtomicUsize::new(0));

        let _subscription = diagnostics.on_error(Errc::PlatformError, {
            let handled = Arc::clone(&handled);
            move |_| {
                handled.fetch_add(1, Ordering::SeqCst);
                RecoveryAction::Recovered
            }
        });

        platform.enqueue(Error::new(
            Errc::PlatformError,
            "recoverable callback failure",
        ));
        platform.enqueue(Error::new(
            Errc::WindowCreationFailed,
            "unrecoverable callback failure",
        ));

        let drained = drain_platform_pending_failures(&mut platform, &diagnostics);
        assert_eq!(drained, 2);
        assert_eq!(handled.load(Ordering::SeqCst), 1);

        let snapshot = diagnostics.snapshot();
        let reports = snapshot.reports();
        assert_eq!(reports.len(), 1);
        assert_eq!(reports[0].code, Errc::WindowCreationFailed);
    }

    #[test]
    fn report_never_runs_registered_recovery_handlers() {
        let diagnostics = new_diagnostics();
        let handled = Arc::new(AtomicUsize::new(0));

        let _subscription = diagnostics.on_error(Errc::PlatformError, {
            let handled = Arc::clone(&handled);
            move |_| {
                handled.fetch_add(1, Ordering::SeqCst);
                RecoveryAction::Recovered
            }
        });

        diagnostics.report(Error::new(Errc::PlatformError, "reported failure"));

        assert_eq!(handled.load(Ordering::SeqCst), 0);
        assert_eq!(diagnostics.snapshot().reports().len(), 1);
    }

    #[test]
    fn device_lost_recovery_registration_requests_rebuild_on_attempt() {
        let diagnostics = new_diagnostics();
        let request = RebuildRequest::default();
        let _subscription = diagnostics.on_error(Errc::GraphicsDeviceLost, {
            let request = request.clone();
            move |_| {
                request.request_rebuild();
                RecoveryAction::Recovered
            }
        });

        let outcome =
            diagnostics.attempt_recovery(Error::new(Errc::GraphicsDeviceLost, "graphics device lost"));
        assert!(outcome.is_recovered());
        assert!(request.is_requested());

        // 未注册代码的 device-lost 仍以 Unhandled 保留原错误。
        let unregistered = new_diagnostics();
        let outcome = unregistered.attempt_recovery(Error::new(
            Errc::GraphicsDeviceLost,
            "unregistered device lost",
        ));
        assert!(matches!(
            outcome,
            crate::diagnostics::RecoveryOutcome::Unhandled(_)
        ));
    }
}

mod create;

pub(crate) use self::create::*;
