// 导入应用组合根、ViewNode 与 uix-lang 编译期入口。
// 导入秒级计时器时长。
use std::time::Duration;

use uix::prelude::*;

// 生成 main.uix 中 <Record> 声明的模块级业务模型，供 uix! 与 Rust 侧回调共同引用。
uix_items!("src/main.uix");

// 保存窗口级持久状态与 uix-lang 语言面之外的类型化状态与数据。
// 大部分交互状态已迁入 main.uix 的组件私有 state；这里只保留三类：
// 1) 窗口生命周期状态（页面、首页计数与秒级 tick）；
// 2) 仍由 Rust 构造的私有类型演示数据（VirtualScroll 的 DemoRow 行）。
#[derive(Clone)]
struct DemoStates {
    // 当前页面索引（与 DemoApp 的 State<number> prop 共享同一槽）。
    page: State<f64>,
    // 首页窗口级计数状态。
    count: State<f64>,
    // App.on_start 注册的秒级 tick 状态。
    tick: State<f64>,
    // 虚拟滚动惰性行数据。
    virtual_items: Vec<DemoRow>,
}

// 表示虚拟滚动演示的拥有所有权行数据。
#[derive(Clone)]
struct DemoRow {
    // 保存稳定行身份。
    id: u64,
    // 保存可插值行内容。
    name: String,
}

// 记录 Input @change 的文本载荷。
fn on_name_change(value: &str) {
    // 输出变化文本供演示日志核对。
    tracing::info!(value, "uix-lang Input @change");
}

// 记录 ImageGroup @change 的索引文本载荷。
fn on_gallery_change(index: &str) {
    // 输出变化索引供演示日志核对。
    tracing::info!(index, "uix-lang ImageGroup @change");
}

// 记录 Pagination @change 的页码文本载荷。
fn on_page_change(value: &str) {
    // 输出变化页码供演示日志核对。
    tracing::info!(value, "uix-lang Pagination @change");
}

// 记录 Anchor @change 的 href 文本载荷。
fn on_anchor_change(href: &str) {
    // 输出目标锚点供演示日志核对。
    tracing::info!(href, "uix-lang Anchor @change");
}

// 记录 Alert @close 的关闭载荷。
fn on_alert_closed(value: &str) {
    // 输出关闭载荷供演示日志核对。
    tracing::info!(value, "uix-lang Alert @close");
}

// 提交语言面 record 声明的类型化表单模型并返回空错误。
fn submit_profile(model: Profile) -> Result<(), String> {
    // 输出关键字段供演示日志核对。
    tracing::info!(
        email = %model.email,
        level = %model.level,
        accepted = model.accepted,
        channel = %model.channel,
        notifications = model.notifications,
        volume = model.volume,
        "uix-lang Form @submit"
    );
    // 演示提交始终成功。
    Ok(())
}

// 构造窗口级持久状态与全部演示数据。
impl DemoStates {
    // 创建默认演示状态槽。
    fn new() -> Self {
        Self {
            // 从首页开始。
            page: State::new(0.0_f64),
            // 首页计数从零开始。
            count: State::new(0.0_f64),
            // 全局 tick 从零开始。
            tick: State::new(0.0_f64),
            // 虚拟滚动惰性行数据。
            virtual_items: vec![
                // 首行。
                DemoRow { id: 1, name: "惰性行 1".to_string() },
                // 第二行。
                DemoRow { id: 2, name: "惰性行 2".to_string() },
                // 第三行。
                DemoRow { id: 3, name: "惰性行 3".to_string() },
                // 第四行。
                DemoRow { id: 4, name: "惰性行 4".to_string() },
                // 第五行。
                DemoRow { id: 5, name: "惰性行 5".to_string() },
                // 第六行。
                DemoRow { id: 6, name: "惰性行 6".to_string() },
            ],
        }
    }
}

// 声明接收窗口级持久状态并返回现有 App builder 的语言入口。
fn build_app(states: DemoStates) -> App {
    // 一次按值解构把声明文件引用的全部 Rust 绑定放入函数作用域。
    let DemoStates {
        // 取出窗口级生命周期状态句柄。
        page,
        count,
        tick,
        // 取出仍由 Rust 构造的私有类型演示数据。
        virtual_items,
    } = states;
    // 编译期读取 <App> 根文档并返回尚未运行的现有 App builder。
    uix_app!("src/main.uix")
    // 结束应用构造函数。
}

// 启动由 Rust 持有窗口与运行生命周期的演示应用。
fn main() {
    // 安装环境过滤日志订阅器（UIX 日志域默认 info）。
    init_tracing();
    // 创建窗口级持久状态槽与演示数据。
    let states = DemoStates::new();
    // 克隆 tick 句柄供 on_start 秒级计时器更新。
    let tick = states.tick.clone();
    // 声明式文档展开出较深视图树，按平台入口执行窗口事件循环。
    #[cfg(windows)]
    {
        // Windows 主线程默认栈较小；深层 ViewNode 树的构建、协调与布局递归
        // 需要与 API GUI Demo 一致的有界大栈 UI 线程。
        const WINDOWS_GUI_STACK_BYTES: usize = 8 * 1024 * 1024;
        // 在专用大栈线程上运行声明式演示窗口。
        let ui_thread = match std::thread::Builder::new()
            .name("uix-lang-demo-gui".to_string())
            .stack_size(WINDOWS_GUI_STACK_BYTES)
            .spawn(move || run_gui(states.clone(), tick.clone()))
        {
            // 返回已创建的 UI 线程。
            Ok(ui_thread) => ui_thread,
            // 线程创建失败直接失败退出。
            Err(error) => panic!("spawn Windows UI thread: {error}"),
        };
        // 把子线程 panic 恢复到主线程。
        if let Err(payload) = ui_thread.join() {
            std::panic::resume_unwind(payload);
        }
    }

    // 其他平台沿用主线程入口。
    #[cfg(not(windows))]
    run_gui(states, tick);
    // 结束进程入口。
}

// 组装并运行 uix-lang 演示窗口。
fn run_gui(states: DemoStates, tick: State<f64>) {
    // 由语言面组装演示 App 与根 View。
    let app = build_app(states)
        // 由声明式根视图绘制与 API GUI Demo 一致的自定义标题栏。
        .custom_title_bar(true)
        // 启动后注册秒级计时器。
        .on_start(move |handle| {
            // 秒级计数器驱动声明文件中的 tick 展示。
            let ticks = tick.clone();
            // 注册一秒间隔的演示计时器。
            handle
                .run_interval(Duration::from_secs(1), move || {
                    // 每次间隔推进全局 tick。
                    ticks.update(|value| *value += 1.0);
                })
                .detach();
        });
    // 进入并由同一个现有 App 持有原生窗口事件循环。
    app.run();
}

// 初始化演示程序使用的环境过滤日志订阅器。
fn init_tracing() {
    use tracing_subscriber::EnvFilter;

    // 优先读取 RUST_LOG，缺省时只显示 uix 日志域的 info 级。
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("uix=info"));
    // 订阅器初始化失败不影响演示运行。
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}
