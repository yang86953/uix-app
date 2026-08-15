// 导入应用组合根、ViewNode 与 uix-lang 编译期入口。
// 导入秒级计时器时长。
use std::time::Duration;

use uix::prelude::*;

// 仅在显式测试能力存在时编译专用图形恢复验收组合模块。
#[cfg(feature = "test-harness")]
mod graphics_recovery;
// 编译普通主演示的 Rust 多窗口组合边界。
mod multi_window;

// 保存主演示组合根支持的显式启动选项。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct LaunchOptions {
    // 记录调用方是否请求启用本机 Agent Bridge。
    agent_control: bool,
    // 记录调用方是否请求进入专用图形恢复验收页面。
    graphics_recovery_test: bool,
    // 记录普通主演示是否跟随 Windows 系统明暗主题。
    follow_system_theme: bool,
}

// 解析主演示启动参数，不把控制面选择泄漏到声明式界面。
fn parse_launch_options(args: impl IntoIterator<Item = String>) -> LaunchOptions {
    // 从全部关闭的安全默认值开始。
    let mut options = LaunchOptions::default();
    // 按调用方顺序读取参数。
    for argument in args {
        // 只识别已经登记的显式控制面开关。
        if argument == "--agent-control" {
            // 记录运行时第二道启用门禁。
            options.agent_control = true;
        }
        // 识别专用图形恢复验收页面开关。
        if argument == "--test-graphics-recovery" {
            // 记录 test-harness 的运行时第二道启用门禁。
            options.graphics_recovery_test = true;
        }
        // 识别 App System 已登记的系统主题跟随开关。
        if argument == "--follow-system-theme" {
            // 记录组合根主题策略而不让 UIX 接管系统消息。
            options.follow_system_theme = true;
        }
    }
    // 返回不包含任何运行时资源的纯配置值。
    options
}

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
                DemoRow {
                    id: 1,
                    name: "惰性行 1".to_string(),
                },
                // 第二行。
                DemoRow {
                    id: 2,
                    name: "惰性行 2".to_string(),
                },
                // 第三行。
                DemoRow {
                    id: 3,
                    name: "惰性行 3".to_string(),
                },
                // 第四行。
                DemoRow {
                    id: 4,
                    name: "惰性行 4".to_string(),
                },
                // 第五行。
                DemoRow {
                    id: 5,
                    name: "惰性行 5".to_string(),
                },
                // 第六行。
                DemoRow {
                    id: 6,
                    name: "惰性行 6".to_string(),
                },
            ],
        }
    }
}

// 声明接收窗口级持久状态并返回现有 App builder 的语言入口。
fn build_app(
    // 接收主演示窗口级状态。
    states: DemoStates,
    // 接收唯一多窗口与跨窗主题控制器。
    multi_window: std::sync::Arc<multi_window::MultiWindowController>,
) -> App {
    // 一次按值解构把声明文件引用的全部 Rust 绑定放入函数作用域。
    let DemoStates {
        // 取出窗口级生命周期状态句柄。
        page,
        count,
        tick,
        // 取出仍由 Rust 构造的私有类型演示数据。
        virtual_items,
    } = states;
    // 克隆主窗与子窗共享的主题状态供 UIX props 订阅。
    let theme_status = multi_window.theme_status();
    // 克隆子窗口创建状态供主窗展示。
    let multi_window_status = multi_window.window_status();
    // 创建主窗暗色主题回调并只进入公开 AppHandle 边界。
    let set_dark_theme = {
        // 克隆控制器进入声明事件闭包。
        let controller = multi_window.clone();
        // 返回不暴露 AppHandle 的零参数回调。
        move || controller.set_dark(true)
    };
    // 创建主窗亮色主题回调并只进入公开 AppHandle 边界。
    let set_light_theme = {
        // 克隆控制器进入声明事件闭包。
        let controller = multi_window.clone();
        // 返回不暴露 AppHandle 的零参数回调。
        move || controller.set_dark(false)
    };
    // 创建按需打开主题联动窗口的声明事件回调。
    let open_theme_window = {
        // 移动最后一份局部控制器进入回调。
        let controller = multi_window;
        // 只请求 Application System 创建新窗口。
        move || controller.open_theme_window()
    };
    // 构造数据展示页唯一的静态表格列声明。
    let table_columns = vec![
        // 展示演示能力名称列。
        TableColumn::new("能力", 220.0),
        // 展示演示能力状态列。
        TableColumn::new("状态", 140.0),
    // 结束静态表格列集合。
    ];
    // 构造数据展示页唯一的静态表格行集合。
    let table_rows: Vec<TableRow> = vec![
        // 展示声明式组件映射状态。
        vec!["UIX 组件映射".to_string(), "已登记".to_string()],
        // 展示类型化数据契约状态。
        vec!["TableRow / TableColumn".to_string(), "已复用".to_string()],
    // 结束静态表格行集合。
    ];
    // 编译期读取 <App> 根文档并返回尚未运行的现有 App builder。
    uix_app!("src/main.uix")
    // 结束应用构造函数。
}

// 启动由 Rust 持有窗口与运行生命周期的演示应用。
fn main() {
    // 安装环境过滤日志订阅器（UIX 日志域默认 info）。
    init_tracing();
    // 在创建状态或窗口前解析组合根启动选项。
    let options = parse_launch_options(std::env::args().skip(1));
    // 未编译 Agent 能力时，显式请求必须定向失败而不是静默降级。
    #[cfg(not(feature = "agent-control"))]
    if options.agent_control {
        // 输出可直接执行的 feature 与参数双门禁命令。
        eprintln!(
            "--agent-control 需要同时启用 Cargo feature：\n  cargo run --manifest-path demo/Cargo.toml --features agent-control --bin uix-lang-demo -- --agent-control"
        );
        // 在任何窗口或 Agent 资源创建前返回参数错误。
        std::process::exit(2);
    }
    // 未编译测试能力时，图形恢复请求必须在创建窗口前定向失败。
    #[cfg(not(feature = "test-harness"))]
    if options.graphics_recovery_test {
        // 输出可直接执行的 feature 与参数双门禁命令。
        eprintln!(
            "--test-graphics-recovery 需要同时启用 Cargo feature：\n  cargo run --manifest-path demo/Cargo.toml --features test-harness --bin uix-lang-demo -- --test-graphics-recovery"
        );
        // 返回参数错误，禁止无测试能力的普通主演示静默降级。
        std::process::exit(2);
    }
    // 创建窗口级持久状态槽与演示数据。
    let states = DemoStates::new();
    // 克隆 tick 句柄供 on_start 秒级计时器更新。
    let tick = states.tick.clone();
    // 声明式文档展开出较深视图树；是否换用大栈 UI 线程由平台层决定。
    uix::platform::run_on_ui_thread("uix-lang-demo-gui", move || {
        // 把已经通过前置门禁的全部启动选择交给 UI 入口组合根。
        run_gui(
            states,
            tick,
            options.agent_control,
            options.graphics_recovery_test,
            options.follow_system_theme,
        )
    });
    // 结束进程入口。
}

// 组装并运行 uix-lang 演示窗口。
fn run_gui(
    // 接收窗口级持久状态与演示数据。
    states: DemoStates,
    // 接收秒级计时状态句柄。
    tick: State<f64>,
    // 接收已经通过启动参数选择的 Agent 控制开关。
    agent_control: bool,
    // 接收已经通过启动参数选择的图形恢复验收开关。
    graphics_recovery_test: bool,
    // 接收普通主演示的系统主题跟随策略。
    follow_system_theme: bool,
) {
    // 测试能力存在时按运行时开关选择独立验收组合或普通主演示。
    #[cfg(feature = "test-harness")]
    let app = if graphics_recovery_test {
        // 使用狭窄的图形恢复验收页面，不把测试控件混入产品演示文档。
        graphics_recovery::build_app()
    } else {
        // 普通启动继续使用原有主演示组合。
        build_demo_app(states, tick, follow_system_theme)
    };
    // 测试能力缺失时锁定前置门禁已经拒绝图形恢复请求。
    #[cfg(not(feature = "test-harness"))]
    let app = {
        // 调试构建核对参数 Gate 没有被后续改动绕过。
        debug_assert!(!graphics_recovery_test);
        // 保留唯一普通主演示组合路径。
        build_demo_app(states, tick, follow_system_theme)
    };
    // feature 存在且启动参数显式请求时才开放 Agent Bridge。
    #[cfg(feature = "agent-control")]
    let app = if agent_control {
        // 复用 App System 唯一的 Agent Module 组装入口。
        app.enable_agent_control()
    } else {
        // 普通主演示保持控制面关闭。
        app
    };
    // feature 缺失时保留同一 App 类型，并锁定前置门禁已经拒绝请求。
    #[cfg(not(feature = "agent-control"))]
    let app = {
        // 调试构建核对参数 Gate 没有被后续改动绕过。
        debug_assert!(!agent_control);
        // 返回未启用控制面的应用构建器。
        app
    };
    // 进入并由同一个现有 App 持有原生窗口事件循环。
    app.run();
}

// 组装普通主演示的窗口配置与秒级生命周期回调。
fn build_demo_app(
    // 接收窗口级持久状态与演示数据。
    states: DemoStates,
    // 接收秒级计时状态句柄。
    tick: State<f64>,
    // 接收是否把 Windows 系统主题变化交给 App System。
    follow_system_theme: bool,
) -> App {
    // 创建普通主演示唯一多窗口控制器。
    let multi_window = std::sync::Arc::new(multi_window::MultiWindowController::new(
        // 初始状态文本只报告真实主题策略。
        follow_system_theme,
    ));
    // 克隆控制器供 on_start 安装 Application System 句柄。
    let start_multi_window = multi_window.clone();
    // 由语言面组装演示 App 与根 View。
    build_app(states, multi_window)
        // 由声明式根视图绘制与 API GUI Demo 一致的自定义标题栏。
        .custom_title_bar(true)
        // 由 Application System 唯一处理 Windows 主题消息与 token 切换。
        .follow_system_theme(follow_system_theme)
        // 启动后注册秒级计时器。
        .on_start(move |handle| {
            // 先把 Application System 句柄安装到多窗口控制器。
            start_multi_window.attach_handle(handle.clone());
            // 秒级计数器驱动声明文件中的 tick 展示。
            let ticks = tick.clone();
            // 注册一秒间隔的演示计时器。
            handle
                .run_interval(Duration::from_secs(1), move || {
                    // 每次间隔推进全局 tick。
                    ticks.update(|value| *value += 1.0);
                })
                .detach();
        })
}

// 初始化演示程序使用的环境过滤日志订阅器。
fn init_tracing() {
    use tracing_subscriber::EnvFilter;

    // 优先读取 RUST_LOG，缺省时只显示 uix 日志域的 info 级。
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("uix=info"));
    // 订阅器初始化失败不影响演示运行。
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

// 保存二进制组合根的近邻启动参数回归测试。
#[cfg(test)]
mod tests {
    // 导入当前模块的私有启动解析入口。
    use super::*;

    // 验证两项运行时门禁可以同时被显式选择。
    #[test]
    fn parses_agent_graphics_recovery_and_system_theme_gates_together() {
        // 按真实命令行顺序解析三项已登记参数与一个无关参数。
        let options = parse_launch_options([
            // 选择 Agent Bridge。
            "--agent-control".to_string(),
            // 保留未知参数不影响已登记开关。
            "--unknown".to_string(),
            // 选择专用图形恢复页面。
            "--test-graphics-recovery".to_string(),
            // 选择普通主演示系统主题跟随策略。
            "--follow-system-theme".to_string(),
        ]);
        // Agent 运行时门禁必须开启。
        assert!(options.agent_control);
        // 图形恢复运行时门禁必须同时开启。
        assert!(options.graphics_recovery_test);
        // 系统主题跟随策略必须同时开启。
        assert!(options.follow_system_theme);
    }
}
