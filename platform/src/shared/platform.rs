// ============================================================================
// platform/shared/platform.rs — Platform 聚合接口
//
// 组合模式：将共享资源（事件循环、输入、系统服务等）聚合到 Platform trait。
// 窗口专属操作在 PlatformWindow trait 中。
// ============================================================================

use crate::{
    EventBus, IWindowManager, IEventLoop, IClipboard, ICursor, IDisplay,
    IFileDialog, IKeyboard, ITextInput, ITimer, INotification,
    IConsole, IFileSystem, ISystemInfo,
};

// ════════════════════════════════════════════════════════════════════════════
// Platform — 聚合接口（组合模式）
// ════════════════════════════════════════════════════════════════════════════

pub trait Platform {
    // ── 窗口工厂 ─────────────────────────────────────────────
    fn window_manager(&mut self) -> &mut dyn IWindowManager;

    // ── 事件循环（平台共享）─────────────────────────────────
    fn event_loop(&mut self) -> &mut dyn IEventLoop;

    // ── 事件总线（平台共享）─────────────────────────────────
    /// 返回平台层的事件总线，其他层通过订阅接收事件。
    fn event_bus(&mut self) -> &mut EventBus;

    // ── 子系统访问器（平台共享）─────────────────────────────
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
