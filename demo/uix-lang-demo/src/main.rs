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
    // 安装环境过滤日志订阅器（UIX 日志域默认 info）。
    init_tracing();
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

// 初始化演示程序使用的环境过滤日志订阅器。
fn init_tracing() {
    use tracing_subscriber::EnvFilter;

    // 优先读取 RUST_LOG，缺省时只显示 uix 日志域的 info 级。
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("uix=info"));
    // 订阅器初始化失败不影响演示运行。
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}
