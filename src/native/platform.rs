//! 平台聚合协议 — 统一访问各功能域子系统。

use crate::core::{Error, Result};
use crate::native::capabilities::display::IDisplay;
use crate::native::capabilities::system::{
    CpuInfo, IConsole, IFileDialog, IFileSystem, INotification, ISystemInfo, ITimer, MemoryInfo,
    OsInfo,
};
use crate::native::windowing::event::{EventBus, IEventLoop};
use crate::native::windowing::input::{IClipboard, ICursor, IKeyboard, ITextInput};
use crate::native::windowing::window::IWindowManager;

/// 平台根接口 — 持有并暴露所有 OS 抽象子系统。
pub trait Platform {
    /// Takes one native callback failure at an owner-thread boundary.
    ///
    /// Callback code must only enqueue typed failures. The application owns
    /// the decision to recover, return, or report after taking the failure.
    fn take_pending_failure(&mut self) -> Option<Error> {
        None
    }

    /// 便捷：操作系统信息（走 `system_info()` 契约）。
    fn os_info(&self) -> Result<OsInfo> {
        self.system_info().os_info()
    }

    /// 便捷：CPU 架构与逻辑核心数。
    fn cpu_info(&self) -> Result<CpuInfo> {
        let system = self.system_info();
        Ok(CpuInfo::new(std::env::consts::ARCH, system.cpu_count()?))
    }

    /// 便捷：内存总量与可用量（走 `system_info()` 契约）。
    fn memory_info(&self) -> Result<MemoryInfo> {
        self.system_info().memory_info()
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
