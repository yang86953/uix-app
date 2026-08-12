// 导入应用组合根、ViewNode 与 uix-lang 编译期入口。
use uix::prelude::*;

// 声明由应用组合根调用且接收窗口级持久状态的界面构造函数。
fn build_view(page: State<f64>, count: State<f64>) -> ViewNode {
    // 编译期读取相对当前 crate 清单目录的 uix-lang 文件。
    uix!("src/main.uix")
    // 结束界面构造函数。
}

// 启动由 Rust 持有窗口与运行生命周期的演示应用。
fn main() {
    // 安装环境过滤日志订阅器（UIX 日志域默认 info）。
    init_tracing();
    // 页面选择由窗口 root factory 持有，reconcile 只能克隆同一状态槽。
    let page = State::new(0.0_f64);
    // 计数器采用同一窗口级生命周期，避免声明式重建恢复初始值。
    let count = State::new(0.0_f64);
    // 组装并运行 uix-lang 演示窗口。
    App::new()
        // 设置演示窗口标题。
        .title("UIX Demo")
        // 使用与 API GUI Demo 一致的初始窗口尺寸。
        .size(1200, 800)
        // 由声明式根视图绘制与 API GUI Demo 一致的自定义标题栏。
        .custom_title_bar(true)
        // 把捕获持久状态句柄的 ViewNode 工厂交给应用组合根。
        .root(move || {
            // 每次 reconcile 只复制句柄，底层页面与计数状态槽保持不变。
            build_view(page.clone(), count.clone())
            // 结束声明式根构造闭包。
        })
        // 进入并由 App 持有原生窗口事件循环。
        .run();
    // 结束进程入口。
}

// 初始化演示程序使用的环境过滤日志订阅器。
fn init_tracing() {
    use tracing_subscriber::EnvFilter;

    // 优先读取 RUST_LOG，缺省时只显示 uix 日志域的 info 级。
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("uix=info"));
    // 订阅器初始化失败不影响演示运行。
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}
