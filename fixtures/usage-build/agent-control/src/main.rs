// 导入应用公开入口，覆盖 Agent 控制 capability 的 builder 方法。
use uix::prelude::App;

// 提供可执行 fixture 的稳定入口。
fn main() {
    // 显式启用进程级 Agent 控制，证明 feature 开启后公开方法可达。
    let app = App::new().enable_agent_control();
    // 消费应用值，避免无意义的未使用警告。
    drop(app);
}
