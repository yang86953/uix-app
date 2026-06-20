// ============================================================================
// uix-platform/src/windows/platform_impl.rs — INativeHandle / Platform / Drop 实现
// ============================================================================

use super::platform::WindowsPlatform;
use crate::diag::Error;
use crate::platform::*;

// ════════════════════════════════════════════════════════════════════════════
// IPresenter — 像素呈现（委托给内部 presenter 字段）
// ════════════════════════════════════════════════════════════════════════════

impl IPresenter for WindowsPlatform {
    fn present(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        dirty_rect: Option<(i32, i32, i32, i32)>,
    ) -> Result<(), Error> {
        self.presenter.present(pixels, width, height, dirty_rect)
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.presenter.resize(width, height)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// INativeHandle — 原生窗口句柄
// ════════════════════════════════════════════════════════════════════════════

impl INativeHandle for WindowsPlatform {
    fn native_window(&self) -> *mut std::ffi::c_void {
        self.hwnd
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Platform — 聚合接口实现（子系统访问器委托给组合实例）
// ════════════════════════════════════════════════════════════════════════════

impl Platform for WindowsPlatform {
    fn presenter(&mut self) -> &mut dyn IPresenter {
        self.presenter.as_mut()
    }
    fn clipboard(&mut self) -> &mut dyn IClipboard {
        &mut self.clipboard_subsys
    }
    fn cursor(&mut self) -> &mut dyn ICursor {
        &mut self.cursor_subsys
    }
    fn display(&self) -> &dyn IDisplay {
        &self.display_subsys
    }
    fn file_dialog(&mut self) -> &mut dyn IFileDialog {
        &mut self.file_dialog_subsys
    }
    fn keyboard(&self) -> &dyn IKeyboard {
        &self.keyboard_subsys
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

impl Drop for WindowsPlatform {
    fn drop(&mut self) {
        // 移除通知图标
        self.notification_subsys.remove_icon();
        // 销毁窗口
        self.destroy_window();
    }
}
