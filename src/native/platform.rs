//! 平台聚合协议 — 统一访问各功能域子系统。

use crate::core::Error;
use crate::native::capabilities::system::IFileSystem;
use crate::platform::display::IDisplay;
use crate::platform::system::console::IConsole;
use crate::platform::system::info::ISystemInfo;
use crate::platform::system::{IFileDialog, INotification, ITimer};
use crate::platform::windowing::event::{EventBus, IEventLoop};
use crate::platform::windowing::{IClipboard, ICursor, IKeyboard, ITextInput};
use crate::platform::windowing::window::IWindowManager;

/// 平台根接口 — 持有并暴露所有 OS 抽象子系统。
pub trait Platform {
    /// Takes one native callback failure at an owner-thread boundary.
    ///
    /// Callback code must only enqueue typed failures. The application owns
    /// the decision to recover, return, or report after taking the failure.
    fn take_pending_failure(&mut self) -> Option<Error> {
        None
    }

    fn window_manager(&mut self) -> &mut dyn IWindowManager;
    fn event_loop(&mut self) -> &mut dyn IEventLoop;
    fn event_bus(&mut self) -> &mut EventBus;
    fn clipboard(&mut self) -> &mut dyn IClipboard;
    fn cursor(&mut self) -> &mut dyn ICursor;
    fn display(&self) -> &dyn IDisplay;
    fn file_dialog(&mut self) -> &mut dyn IFileDialog;
    fn keyboard(&self) -> &dyn IKeyboard;
    fn text_input(&mut self) -> &mut dyn ITextInput;
    fn timer(&mut self) -> &mut dyn ITimer;
    fn notification(&mut self) -> &mut dyn INotification;
    fn console(&mut self) -> &mut dyn IConsole;
    fn file_system(&self) -> &dyn IFileSystem;
    fn system_info(&self) -> &dyn ISystemInfo;
}
