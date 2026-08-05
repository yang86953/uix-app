// 导入基础反馈组件与弹层组件，覆盖单 capability 的 prelude 公开面。
use uix::prelude::{Alert, Modal};
// 导入全局消息与通知门面，覆盖 ui 扁平公开面。
use uix::ui::{message, notify};

// 提供可执行 fixture 的稳定入口。
fn main() {
    // 构造基础警告提示，证明普通反馈组件公开面可达。
    let alert = Alert::success("已保存");
    // 构造带遮罩的对话框，证明弹层反馈组件公开面可达。
    let modal = Modal::new("确认操作").closable(true).overlay(true);
    // 读取全局消息门面的可用状态，证明函数与门面类型均可达。
    let message_available = message().is_available();
    // 读取全局通知门面的可用状态，证明通知注册表公开面可达。
    let notification_available = notify().is_available();
    // 显式消费全部值，避免无意义的未使用警告。
    drop((alert, modal, message_available, notification_available));
}
