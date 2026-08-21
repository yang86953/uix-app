//! Platform System 内部使用的基础系统服务协议。

use crate::core::Result;

/// 低层原生文件对话框端口；公开门面负责值转换与输入验证。
pub(crate) trait IFileDialog {
    /// 打开文件选择对话框。`Ok(None)` 表示用户取消；`Err` 表示对话框本身失败。
    fn open(&mut self, title: &str, filters: &str) -> Result<Option<Vec<String>>>;
    /// 打开保存对话框。`Ok(None)` 表示用户取消；`Err` 表示对话框本身失败。
    fn save(&mut self, title: &str, filters: &str) -> Result<Option<String>>;
    /// 打开目录选择对话框。`Ok(None)` 表示用户取消；`Err` 表示对话框本身失败。
    fn open_folder(&mut self, title: &str) -> Result<Option<String>>;
}

/// 低层原生系统通知端口，不包含应用身份与能力探测策略。
pub(crate) trait INotification {
    /// 显示系统通知。失败时返回 typed error（如通知区域不可用、notify-send 缺失）。
    fn show(&mut self, title: &str, message: &str) -> Result<()>;
}

/// 低层原生定时器端口，ID、重复与清理语义由各平台实现保持。
pub(crate) trait ITimer {
    fn set(&mut self, interval_ms: u32, repeating: bool) -> Result<u32>;
    fn clear(&mut self, id: u32) -> Result<()>;
}
