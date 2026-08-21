use super::*;
use crate::core::Errc;
use crate::diagnostics::{Diagnostics, DiagnosticsConfig, RecoveryAction};
use crate::native::capabilities::*;
use crate::native::platform::Platform;
use crate::platform::display::IDisplay;
use crate::platform::windowing::event::{EventBus, IEventLoop};
use crate::platform::windowing::window::IWindowManager;
use crate::platform::windowing::{IClipboard, ICursor, IKeyboard, ITextInput};
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

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
// 验证公开 builder 的具体 API 优先级高于环境自动配置。
fn builder_backend_resolves_to_explicit_private_selection() {
    // 用 auto 环境值对照 builder 的显式 D3D11 请求。
    let resolved = resolve_graphics_backend(Some(GraphicsApi::D3d11), Some("auto"), None);
    // builder 必须保持显式 API，不能被后续自动策略覆盖。
    assert_eq!(resolved, GraphicsSelection::Explicit(GraphicsApi::D3d11));
    // 完全省略配置时才启用自动策略。
    let automatic = resolve_graphics_backend(None, None, None);
    // 默认结果必须是 crate-private Automatic 策略。
    assert_eq!(automatic, GraphicsSelection::Automatic);
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
