// 导入应用构建器与 uix-lang 编译期入口。
use uix::prelude::*;

// 启动 Popconfirm 独立真窗验收应用。
fn main() {
    // 编译期读取验收声明并进入现有原生窗口事件循环。
    uix_app!("src/main.uix").run();
}
