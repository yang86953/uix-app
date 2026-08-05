// 故意导入未启用 capability 的基础与弹层组件，门禁要求此行无法编译。
use uix::prelude::{Alert, Modal};
// 故意导入未启用 capability 的全局门面函数，证明注册表公开面同步收缩。
use uix::ui::{message, notify};

// 提供 compile-fail fixture 的稳定入口。
fn main() {
    // 若基础反馈组件意外泄漏，构造值会使门禁检测到编译成功并失败。
    let alert = Alert::success("不应可用");
    // 若弹层反馈组件意外泄漏，构造值会阻止部分门控静默通过。
    let modal = Modal::new("不应可用");
    // 若全局消息门面意外泄漏，调用会参与公开面门禁。
    let message_available = message().is_available();
    // 若全局通知门面意外泄漏，调用会参与公开面门禁。
    let notification_available = notify().is_available();
    // 显式消费全部值，避免公开面泄漏时只产生未使用警告。
    drop((alert, modal, message_available, notification_available));
}
