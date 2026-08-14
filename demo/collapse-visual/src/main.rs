// 导入应用构建器与 uix-lang 编译期入口。
use uix::prelude::*;

// 接收 UIX Change 载荷以在编译期核对稳定 key 处理器签名。
fn record_panel(_panel_key: &str) {
    // 真窗验收只观察受控展开状态，不产生额外副作用。
}

// 启动 Collapse 独立真窗验收应用。
fn main() {
    // 编译期读取验收声明并进入现有原生窗口事件循环。
    uix_app!("src/main.uix")
        // 专用验收程序显式开放同用户本机 Agent Adapter。
        .enable_agent_control()
        // 进入现有原生窗口事件循环。
        .run();
}
