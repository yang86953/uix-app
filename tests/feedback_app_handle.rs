//! 逐窗反馈公开 API 的类型契约。

// 引入核心错误与结果类型，验证生命周期失败公开面。
use uix::core::{Error, Result};
// 引入应用句柄和两类反馈条目。
use uix::prelude::{AppHandle, MessageItem, NotificationItem};

// 验证反馈入口只能通过显式窗口 AppHandle 调用。
#[test]
fn app_handle_exposes_typed_window_feedback_contract() {
    // Message 投递返回真实稳定 ID 或类型化失败。
    let _: fn(&AppHandle, MessageItem) -> Result<u64> = AppHandle::push_message;
    // Notification 投递返回真实稳定 ID 或类型化失败。
    let _: fn(&AppHandle, NotificationItem) -> Result<u64> = AppHandle::push_notification;
    // Message 关闭明确区分生命周期失败与未找到条目。
    let _: fn(&AppHandle, u64) -> Result<bool> = AppHandle::dismiss_message;
    // 普通 Notification 关闭使用本地稳定 ID 命名空间。
    let _: fn(&AppHandle, u64) -> Result<bool> = AppHandle::dismiss_notification;
    // 原生错误 Notification 关闭使用外部稳定 ID 命名空间。
    let _: fn(&AppHandle, u64) -> Result<bool> = AppHandle::dismiss_error_notification;
    // 原生错误桥区分合法过滤与类型化生命周期失败。
    let _: fn(&AppHandle, &Error) -> Result<Option<u64>> = AppHandle::notify_error;
}
