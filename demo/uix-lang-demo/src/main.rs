// 导入应用组合根、ViewNode 与 uix-lang 编译期入口。
use uix::prelude::*;

// 声明由应用组合根调用的界面构造函数。
fn build_view() -> ViewNode {
    // 编译期读取相对当前 crate 清单目录的 uix-lang 文件。
    uix!("src/main.uix")
    // 结束界面构造函数。
}

// 启动由 Rust 持有窗口与运行生命周期的演示应用。
fn main() {
    // 组装并运行 uix-lang 演示窗口。
    App::new()
        // 设置演示窗口标题。
        .title("UIX Lang Demo")
        // 设置适合首个声明式示例的窗口尺寸。
        .size(640, 360)
        // 把编译期生成的 ViewNode 工厂交给应用组合根。
        .root(build_view)
        // 进入并由 App 持有原生窗口事件循环。
        .run();
    // 结束进程入口。
}
