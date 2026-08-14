use std::path::PathBuf;

use crate::core::{Errc, Error, Result};
use crate::platform::hardware::{DisplayInfo, MemoryInfo, OsInfo};
// 引入跨平台通知身份与能力状态契约。
use crate::platform::services::{
    AppUserModelId, SpecialDir, SystemNotification, SystemNotificationCapability,
};

pub(crate) struct State;

impl State {
    pub(crate) fn new() -> Result<Self> {
        Err(unsupported("Platform::new"))
    }
}

pub(crate) fn is_main_thread() -> Result<bool> {
    Err(unsupported("Platform::new"))
}

pub(crate) fn os_info() -> Result<OsInfo> {
    Err(unsupported("Platform::os_info"))
}

pub(crate) fn cpu_metadata() -> (Option<String>, Option<String>) {
    (None, None)
}

pub(crate) fn memory_info() -> Result<MemoryInfo> {
    Err(unsupported("Platform::memory_info"))
}

pub(crate) fn displays() -> Result<Box<[DisplayInfo]>> {
    Err(unsupported("Platform::displays"))
}

pub(crate) fn special_dir(_directory: SpecialDir) -> Result<PathBuf> {
    Err(unsupported("Platform::special_dir"))
}

// 没有平台 Provider 的目标返回显式 Unsupported 状态。
pub(crate) fn system_notification_capability(
    // Unsupported provider 不消费 Windows AUMID。
    _app_user_model_id: Option<&AppUserModelId>,
) -> Result<SystemNotificationCapability> {
    // 能力查询本身成功，状态表达不可用原因。
    Ok(SystemNotificationCapability::Unsupported)
}

pub(crate) fn show_notification(
    // Unsupported provider 不消费 Windows AUMID。
    _app_user_model_id: Option<&AppUserModelId>,
    // 保留统一通知值签名。
    _notification: &SystemNotification,
) -> Result<()> {
    Err(unsupported("Platform::show_notification"))
}

fn unsupported(operation: &str) -> Error {
    Error::new(
        Errc::NotImplemented,
        format!("{operation}: this target has no UIX platform provider"),
    )
}
