// ============================================================================
// platform/linux/platform.rs — Linux Platform 统一实现（组合模式）
//
// LinuxPlatform 通过组合持有 WaylandBackend 和各子系统 struct。
// 所有子 trait 的访问统一通过 Platform 的访问器方法委托。
// ============================================================================

use crate::diag::Error;
use crate::platform::event::*;
use crate::platform::*;

use crate::platform::linux::console::LinuxConsole;
use crate::platform::linux::file_dialog::LinuxFileDialog;
use crate::platform::linux::filesystem::LinuxFileSystem;
use crate::platform::linux::notification::LinuxNotification;
use crate::platform::linux::system_info::LinuxSystemInfo;
use crate::platform::linux::text_input::LinuxTextInput;
use crate::platform::linux::timer::LinuxTimer;
use crate::platform::linux::wayland::WaylandBackend;

// ════════════════════════════════════════════════════════════════════════════
// LinuxPlatform — Wayland 平台实现
//
// 统一通过组合持有各子系统，Platform 访问器委托给对应的子系统。
// ════════════════════════════════════════════════════════════════════════════

pub struct LinuxPlatform {
    // ── Wayland 后端（窗口管理 + 呈现 + 光标/键盘/显示/剪贴板 + 事件循环）─
    backend: WaylandBackend,

    // ── 独立子系统（各自独立 struct，非 Wayland 协议相关）─────────
    console_subsys: LinuxConsole,
    file_dialog_subsys: LinuxFileDialog,
    file_system_subsys: LinuxFileSystem,
    notification_subsys: LinuxNotification,
    system_info_subsys: LinuxSystemInfo,
    text_input_subsys: LinuxTextInput,
    timer_subsys: LinuxTimer,
}

// ════════════════════════════════════════════════════════════════════════════
// 构造
// ════════════════════════════════════════════════════════════════════════════

impl LinuxPlatform {
    pub fn new() -> Result<Self, Error> {
        let backend = match WaylandBackend::new() {
            Ok(wl) => {
                log::info!("Wayland backend initialized");
                wl
            }
            Err(e) => {
                return Err(Error::new(
                    crate::diag::Errc::PlatformError,
                    format!("Wayland backend failed: {}", e),
                ));
            }
        };

        let timer_eq = backend.event_queue_handle();

        Ok(Self {
            backend,
            console_subsys: LinuxConsole::new(),
            file_dialog_subsys: LinuxFileDialog::new(),
            file_system_subsys: LinuxFileSystem::new(),
            notification_subsys: LinuxNotification::new(),
            system_info_subsys: LinuxSystemInfo::new(),
            text_input_subsys: LinuxTextInput::new(),
            timer_subsys: LinuxTimer::new(timer_eq),
        })
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Platform — 统一访问器
//
// 所有子系统通过访问器暴露，不直接实现子 trait。
// ════════════════════════════════════════════════════════════════════════════

impl Platform for LinuxPlatform {
    // ── 核心窗口访问器 ─────────────────────────────────────────────
    fn window_manager(&mut self) -> &mut dyn IWindowManager {
        &mut self.backend
    }
    fn window_properties(&self) -> &dyn IWindowProperties {
        &self.backend
    }
    fn event_loop(&mut self) -> &mut dyn IEventLoop {
        &mut self.backend
    }
    fn native_handle(&self) -> &dyn INativeHandle {
        &self.backend
    }
    fn presenter(&mut self) -> &mut dyn IPresenter {
        &mut self.backend
    }

    // ── 子系统访问器 ──────────────────────────────────────────────
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
        &mut self.text_input_subsys
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
// Drop — 清理资源
// ════════════════════════════════════════════════════════════════════════════

impl Drop for LinuxPlatform {
    fn drop(&mut self) {
        self.backend.destroy_window();
    }
}
