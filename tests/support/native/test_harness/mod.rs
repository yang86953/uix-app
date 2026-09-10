//! # native 测试框架
//!
//! 提供所有平台子系统的 **Fake 实现**，可在测试中替代真实的 linux/windows 实现。
//!
//! ## 设计原则
//!
//! 每个 Fake 是**真正的内存实现**（不是哑巴 Mock），写入后读出一致。
//! 每个 Fake **记录所有调用**，测试可直接断言调用历史。
//! `FakePlatform` 聚合所有 Fake，实现 `Platform` trait。
//!
//! ## 使用方式
//!
//! ```ignore
//! use uix_app::native::test_harness::FakePlatform;
//!
//! let mut pf = FakePlatform::new();
//! pf.clipboard.set_text("hello");
//! assert_eq!(pf.clipboard.text(), "hello");
//! assert_eq!(pf.clipboard.last_set_text(), Some("hello"));
//! ```
//!
//! 代码路径通过 `Platform` trait 访问，测试路径直接读 Fake 的公开字段。
//! 双重访问路径无需 cast 或 unwrap。

pub mod fake_clipboard;
pub mod fake_console;
pub mod fake_cursor;
pub mod fake_display;
pub mod fake_event_source;
pub mod fake_file_dialog;
pub mod fake_file_system;
pub mod fake_graphics_context;
pub mod fake_keyboard;
pub mod fake_notification;
pub mod fake_presenter;
pub mod fake_system_info;
pub mod fake_text_input;
pub mod fake_timer;
pub mod fake_window;

// ── Convenience re-exports ─────────────────────────────────────────────
pub use fake_clipboard::FakeClipboard;
pub use fake_console::FakeConsole;
pub use fake_cursor::FakeCursor;
pub use fake_display::FakeDisplay;
pub use fake_event_source::FakeEventSource;
pub use fake_file_dialog::FakeFileDialog;
pub use fake_file_system::FakeFileSystem;
pub use fake_graphics_context::FakeGraphicsContext;
pub use fake_keyboard::FakeKeyboard;
pub use fake_notification::FakeNotification;
pub use fake_presenter::FakePresenter;
pub use fake_system_info::FakeSystemInfo;
pub use fake_text_input::FakeTextInput;
pub use fake_timer::FakeTimer;
pub use fake_window::{FakeNativeHandle, FakeWindow, FakeWindowManager, FakeWindowProperties};

use std::time::Duration;

use crate::core::Error;
use crate::diagnostics::{PendingFailureQueue, PendingFailureSource};
use crate::native::windowing::shared::OsEventSource;
use crate::platform::display::IDisplay;
use crate::platform::platform::Platform;
use crate::platform::system::console::IConsole;
use crate::platform::system::filesystem::IFileSystem;
use crate::platform::system::info::ISystemInfo;
use crate::platform::system::{IFileDialog, INotification, ITimer};
use crate::platform::windowing::event::{EventBus, EventLoopWaker, IEventLoop, UiEvent};
use crate::platform::windowing::window::IWindowManager;
use crate::platform::windowing::{IClipboard, ICursor, IKeyboard, ITextInput};

// ════════════════════════════════════════════════════════════════════════════
// FakePlatform — 聚合所有 Fake 子系统
// ════════════════════════════════════════════════════════════════════════════

/// 聚合所有 Fake 子系统的测试平台。
///
/// 所有字段公开，测试可直接读取断言：
/// ```ignore
/// let mut pf = FakePlatform::new();
/// pf.clipboard.set_text("hello");
/// assert_eq!(pf.clipboard.state.text, "hello");
/// ```
///
/// 同时实现 `Platform` trait，代码路径通过 trait 接口访问。
pub struct FakePlatform {
    pub clipboard: FakeClipboard,
    pub console: FakeConsole,
    pub cursor: FakeCursor,
    pub display: FakeDisplay,
    pub event_source: FakeEventSource,
    pub file_dialog: FakeFileDialog,
    pub file_system: FakeFileSystem,
    pub graphics_context: FakeGraphicsContext,
    pub keyboard: FakeKeyboard,
    pub notification: FakeNotification,
    pub presenter: FakePresenter,
    pub system_info: FakeSystemInfo,
    pub text_input: FakeTextInput,
    pub timer: FakeTimer,
    pub window_manager: FakeWindowManager,
    pub event_bus: EventBus,
    pub pending_failures: PendingFailureSource,
}

impl FakePlatform {
    pub fn new() -> Self {
        Self {
            clipboard: FakeClipboard::new(),
            console: FakeConsole::new(),
            cursor: FakeCursor::new(),
            display: FakeDisplay::new(),
            event_source: FakeEventSource::new(),
            file_dialog: FakeFileDialog::new(),
            file_system: FakeFileSystem::new(),
            graphics_context: FakeGraphicsContext::new(),
            keyboard: FakeKeyboard::new(),
            notification: FakeNotification::new(),
            presenter: FakePresenter::new(),
            system_info: FakeSystemInfo::new(),
            text_input: FakeTextInput::new(),
            timer: FakeTimer::new(),
            window_manager: FakeWindowManager::new(),
            event_bus: EventBus::new(),
            pending_failures: PendingFailureQueue::new().source(),
        }
    }

    /// Enqueues one typed callback-style failure, mirroring the native
    /// callback contract: only enqueue here, never report or recover.
    pub fn enqueue_pending_failure(&self, error: Error) {
        let _ = self.pending_failures.enqueue(error);
    }
}

impl Default for FakePlatform {
    fn default() -> Self {
        Self::new()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// OsEventSource — 事件源（blanket impl 自动提供 IEventLoop）
// ════════════════════════════════════════════════════════════════════════════

impl OsEventSource for FakePlatform {
    fn waker(&self) -> EventLoopWaker {
        OsEventSource::waker(&self.event_source)
    }

    fn dispatch_pending(&mut self) -> bool {
        self.event_source.dispatch_pending()
    }

    fn dispatch_blocking(&mut self) -> bool {
        self.event_source.dispatch_blocking()
    }

    fn dispatch_timeout(&mut self, timeout: Duration) -> bool {
        self.event_source.dispatch_timeout(timeout)
    }

    fn next_event(&mut self) -> Option<UiEvent> {
        self.event_source.next_event()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Platform — 统一访问器
// ════════════════════════════════════════════════════════════════════════════

impl Platform for FakePlatform {
    fn take_pending_failure(&mut self) -> Option<Error> {
        self.pending_failures.take()
    }

    fn window_manager(&mut self) -> &mut dyn IWindowManager {
        &mut self.window_manager
    }

    fn event_loop(&mut self) -> &mut dyn IEventLoop {
        // FakePlatform 实现了 OsEventSource → blanket impl 提供 IEventLoop
        self
    }

    fn event_bus(&mut self) -> &mut EventBus {
        &mut self.event_bus
    }

    fn clipboard(&mut self) -> &mut dyn IClipboard {
        &mut self.clipboard
    }

    fn cursor(&mut self) -> &mut dyn ICursor {
        &mut self.cursor
    }

    fn display(&self) -> &dyn IDisplay {
        &self.display
    }

    fn file_dialog(&mut self) -> &mut dyn IFileDialog {
        &mut self.file_dialog
    }

    fn keyboard(&self) -> &dyn IKeyboard {
        &self.keyboard
    }

    fn text_input(&mut self) -> &mut dyn ITextInput {
        &mut self.text_input
    }

    fn timer(&mut self) -> &mut dyn ITimer {
        &mut self.timer
    }

    fn notification(&mut self) -> &mut dyn INotification {
        &mut self.notification
    }

    fn console(&mut self) -> &mut dyn IConsole {
        &mut self.console
    }

    fn file_system(&self) -> &dyn IFileSystem {
        &self.file_system
    }

    fn system_info(&self) -> &dyn ISystemInfo {
        &self.system_info
    }
}
