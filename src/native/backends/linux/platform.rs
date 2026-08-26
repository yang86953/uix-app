// ============================================================================
// platform/linux/platform.rs — LinuxPlatform（OsEventSource + Platform 实现）
//
// LinuxPlatform 通过组合持有 WaylandBackend 和各子系统。
// 实现 OsEventSource（获得 IEventLoop 的 blanket impl）。
// Platform trait 访问器委托给对应的子系统。
// ============================================================================

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use crate::core::Error;
use crate::diagnostics::PendingFailureQueue;
use crate::native::windowing::shared::{OsEventSource, WindowState};
use crate::platform::display::IDisplay;
use crate::platform::platform::Platform;
use crate::platform::system::console::IConsole;
use crate::platform::system::filesystem::IFileSystem;
use crate::platform::system::info::ISystemInfo;
use crate::platform::system::{IFileDialog, INotification, ITimer};
use crate::platform::windowing::event::{EventBus, EventLoopWaker, IEventLoop, UiEvent};
use crate::platform::windowing::window::IWindowManager;
use crate::platform::windowing::{IClipboard, ICursor, IKeyboard, ITextInput};

use crate::native::backends::linux::console::LinuxConsole;
use crate::native::backends::linux::file_dialog::LinuxFileDialog;
use crate::native::backends::linux::filesystem::LinuxFileSystem;
use crate::native::backends::linux::notification::LinuxNotification;
use crate::native::backends::linux::system_info::LinuxSystemInfo;
use crate::native::backends::linux::timer::LinuxTimer;
use crate::native::backends::linux::wayland::WaylandBackend;

// ════════════════════════════════════════════════════════════════════════════
// LinuxPlatform
// ════════════════════════════════════════════════════════════════════════════

pub(crate) struct LinuxPlatform {
    // ── 共享窗口状态（与 PlatformWindowCore 共享）─────────
    window: Rc<RefCell<WindowState>>,

    // ── Wayland 后端（连接管理 + 事件分发 + 输入/显示/剪贴板）─
    backend: WaylandBackend,

    // ── 事件总线（其他层通过订阅接收事件）───────────────────
    event_bus: EventBus,

    // ── 独立子系统 ──────────────────────────────────────────
    console_subsys: LinuxConsole,
    file_dialog_subsys: LinuxFileDialog,
    file_system_subsys: LinuxFileSystem,
    notification_subsys: LinuxNotification,
    system_info_subsys: LinuxSystemInfo,
    timer_subsys: LinuxTimer,
}

// ════════════════════════════════════════════════════════════════════════════
// 构造
// ════════════════════════════════════════════════════════════════════════════

impl LinuxPlatform {
    pub(crate) fn new(pending_failures: PendingFailureQueue) -> Result<Self, Error> {
        let pending_source = pending_failures.source();
        let backend = match WaylandBackend::new(pending_source.clone()) {
            Ok(wl) => {
                tracing::info!("Wayland backend initialized");
                wl
            }
            Err(e) => {
                return Err(Error::new(
                    crate::native::Errc::PlatformError,
                    format!("Wayland backend failed: {}", e),
                ));
            }
        };

        let timer_eq = backend.event_queue_handle();

        Ok(Self {
            window: Rc::new(RefCell::new(WindowState::default())),
            backend,
            console_subsys: LinuxConsole::new(),
            file_dialog_subsys: LinuxFileDialog::new(),
            file_system_subsys: LinuxFileSystem::new(),
            notification_subsys: LinuxNotification::new(),
            system_info_subsys: LinuxSystemInfo::new(),
            event_bus: EventBus::new(),
            timer_subsys: LinuxTimer::new(timer_eq, pending_source),
        })
    }
}

// ════════════════════════════════════════════════════════════════════════════
// OsEventSource — 事件分发（委托给 WaylandBackend）
//
// 通过实现了 OsEventSource，LinuxPlatform 自动获得 IEventLoop（blanket impl）。
// ════════════════════════════════════════════════════════════════════════════

impl OsEventSource for LinuxPlatform {
    fn dispatch_pending(&mut self) -> bool {
        self.backend.try_dispatch()
    }

    fn dispatch_blocking(&mut self) -> bool {
        self.backend.dispatch_blocking()
    }

    fn dispatch_timeout(&mut self, timeout: Duration) -> bool {
        self.backend.dispatch_timeout(timeout)
    }

    fn next_event(&mut self) -> Option<UiEvent> {
        self.backend.next_event()
    }

    fn waker(&self) -> EventLoopWaker {
        self.backend.waker()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Platform — 统一访问器
//
// 核心变更：event_loop() 返回 self（由 OsEventSource blanket impl 提供 IEventLoop），
// 而非委托给 WaylandBackend。
// ════════════════════════════════════════════════════════════════════════════

impl Platform for LinuxPlatform {
    fn take_pending_failure(&mut self) -> Option<Error> {
        self.backend.take_pending_failure()
    }

    fn window_manager(&mut self) -> &mut dyn IWindowManager {
        &mut self.backend
    }

    fn event_loop(&mut self) -> &mut dyn IEventLoop {
        self
    }

    fn event_bus(&mut self) -> &mut EventBus {
        &mut self.event_bus
    }

    // ── 子系统访问器 ──────────────────────────────────────────
    fn clipboard(&mut self) -> &mut dyn IClipboard {
        &mut self.backend
    }
    fn cursor(&mut self) -> &mut dyn ICursor {
        &mut self.backend
    }
    fn display(&self) -> &dyn IDisplay {
        &self.backend
    }
    fn keyboard(&self) -> &dyn IKeyboard {
        &self.backend
    }
    fn file_dialog(&mut self) -> &mut dyn IFileDialog {
        &mut self.file_dialog_subsys
    }
    fn text_input(&mut self) -> &mut dyn ITextInput {
        &mut self.backend
    }
    fn timer(&mut self) -> &mut dyn ITimer {
        &mut self.timer_subsys
    }
    fn notification(&mut self) -> &mut dyn INotification {
        &mut self.notification_subsys
    }
    fn console(&mut self) -> &mut dyn IConsole {
        &mut self.console_subsys
    }
    fn file_system(&self) -> &dyn IFileSystem {
        &self.file_system_subsys
    }
    fn system_info(&self) -> &dyn ISystemInfo {
        &self.system_info_subsys
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Drop
// ════════════════════════════════════════════════════════════════════════════

impl Drop for LinuxPlatform {
    fn drop(&mut self) {
        // WaylandBackend 的连接由 Rust 所有权自动管理
    }
}
