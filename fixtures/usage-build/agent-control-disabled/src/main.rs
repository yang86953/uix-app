// 导入基础应用公开入口；该类型本身不随 Agent 控制 capability 裁剪。
use uix::prelude::App;

// 提供 compile-fail fixture 的稳定入口。
fn main() {
    // 故意调用未启用 capability 的方法，门禁要求此链无法编译。
    let app = App::new().enable_agent_control();
    // 若方法意外泄漏，消费应用值会使门禁检测到编译成功并失败。
    drop(app);
}
