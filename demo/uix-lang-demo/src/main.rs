// 导入应用组合根与 uix-lang 编译期入口。
use uix::prelude::*;

// 把主演示正文与中文统一到随应用分发的同一字体字节。
const DETERMINISTIC_UI_FONT: &[u8] =
    include_bytes!("../../../assets/fonts/NotoSansCJKsc-Regular.otf");

// 仅在显式测试能力存在时编译专用图形恢复验收组合模块。
#[cfg(feature = "test-harness")]
mod graphics_recovery;
// 仅在显式测试能力存在时编译主演示 surface 像素验收。
#[cfg(feature = "test-harness")]
mod graphics_readback;

// 只把语言面 Record 投影为提交回调的模块级业务输入类型。
uix_items!("src/main.uix");

// 真实终端演示组合：会话生命周期由 Rust 宿主拥有，声明式页面经 KernelView 投影。
mod live_terminal;
// 声明式页面按 external 名字引用真实终端演示入口。
use live_terminal::{live_terminal_toggle, live_terminal_toggle_width, live_terminal_view};

// 接收 Form 已完成字段校验与状态写回后的业务提交事实。
fn submit_profile(model: Profile) -> Result<(), String> {
    tracing::info!(
        email = %model.email,
        level = %model.level,
        accepted = model.accepted,
        "uix-lang Form @submit"
    );
    Ok(())
}

// 保存 Rust Application System 必须拥有的显式启动选项。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct LaunchOptions {
    // 记录调用方是否请求启用本机 Agent Bridge。
    agent_control: bool,
    // 记录调用方是否请求进入专用图形恢复验收页面。
    graphics_recovery_test: bool,
    // 记录调用方是否请求对主演示首帧执行真实 surface 像素验收。
    graphics_readback_test: bool,
    // 记录普通主演示是否跟随 Windows 系统明暗主题。
    follow_system_theme: bool,
}

// 解析应用生命周期选项，不把进程参数或平台能力泄漏到声明式界面。
fn parse_launch_options(args: impl IntoIterator<Item = String>) -> LaunchOptions {
    let mut options = LaunchOptions::default();
    for argument in args {
        match argument.as_str() {
            "--agent-control" => options.agent_control = true,
            "--test-graphics-recovery" => options.graphics_recovery_test = true,
            "--test-graphics-readback" => options.graphics_readback_test = true,
            "--follow-system-theme" => options.follow_system_theme = true,
            _ => {}
        }
    }
    options
}

// 启动由 Rust 持有进程、窗口和平台生命周期的 uix-lang 演示应用。
fn main() {
    init_tracing();
    let options = parse_launch_options(std::env::args().skip(1));

    // 未编译 Agent 能力时，显式请求必须定向失败而不是静默降级。
    #[cfg(not(feature = "agent-control"))]
    if options.agent_control {
        eprintln!(
            "--agent-control 需要同时启用 Cargo feature：\n  cargo run --manifest-path demo/Cargo.toml --features agent-control --bin uix-lang-demo -- --agent-control"
        );
        std::process::exit(2);
    }

    // 未编译测试能力时，两种图形验收请求都必须在创建窗口前定向失败。
    #[cfg(not(feature = "test-harness"))]
    if options.graphics_recovery_test || options.graphics_readback_test {
        eprintln!(
            "图形验收参数需要同时启用 Cargo feature：\n  cargo run --manifest-path demo/Cargo.toml --features test-harness --bin uix-lang-demo -- --test-graphics-recovery"
        );
        std::process::exit(2);
    }

    // 声明文件展开出较深视图树；是否换用大栈 UI 线程由平台层决定。
    uix::platform::run_on_ui_thread("uix-lang-demo-gui", move || run_gui(options));
}

// 选择普通全组件文档或显式测试页面，并进入唯一应用事件循环。
fn run_gui(options: LaunchOptions) {
    // Agent 确认回调只取得 Application System 交付的公开句柄。
    let agent_handle_slot: std::sync::Arc<std::sync::Mutex<Option<uix::app::AppHandle>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    // 保存可 clone 的诊断句柄，供应用退出后消费复现清单与报告摘要。
    let diagnostics_slot: std::sync::Arc<std::sync::Mutex<Option<uix::diagnostics::Diagnostics>>> =
        std::sync::Arc::new(std::sync::Mutex::new(None));
    // 持有报告即时订阅句柄：订阅随演示会话存活，drop 即注销。
    let report_subscription_slot: std::sync::Arc<
        std::sync::Mutex<Option<uix::diagnostics::ReportSubscription>>,
    > = std::sync::Arc::new(std::sync::Mutex::new(None));

    // 测试能力存在时按运行时开关选择独立验收组合或普通主演示。
    #[cfg(feature = "test-harness")]
    let app = if options.graphics_recovery_test {
        graphics_recovery::build_app()
    } else if options.graphics_readback_test {
        graphics_readback::build_app()
    } else {
        build_demo_app(
            options.follow_system_theme,
            agent_handle_slot.clone(),
            diagnostics_slot.clone(),
            report_subscription_slot.clone(),
        )
    };

    // 测试能力缺失时只保留全组件主演示组合路径。
    #[cfg(not(feature = "test-harness"))]
    let app = {
        debug_assert!(!options.graphics_recovery_test);
        debug_assert!(!options.graphics_readback_test);
        build_demo_app(
            options.follow_system_theme,
            agent_handle_slot.clone(),
            diagnostics_slot.clone(),
            report_subscription_slot.clone(),
        )
    };

    // 所有启动模式都在窗口创建前安装同一正文与 CJK 字体包。
    let app = with_deterministic_fonts(app);

    // feature 与运行参数同时存在时才开放 Agent Bridge。
    #[cfg(feature = "agent-control")]
    let app = if options.agent_control {
        app.enable_agent_control()
            // 专用后台根不捕获主演示的导航、草稿、终端或窗口句柄。
            .agent_root(|| uix::uix!("src/agent.uix"))
            .agent_require_confirm("demo-popconfirm-trigger")
            .agent_confirm_ui(move |request: uix::app::AgentConfirmationRequest| {
                tracing::warn!(
                    window_id = ?request.window_id,
                    confirm_id = request.confirm_id,
                    target = %request.target,
                    action = %request.action,
                    "agent confirmation requested (demo auto-rejects)"
                );
                if let Some(handle) = agent_handle_slot.lock().unwrap().clone() {
                    let _ = handle.resolve_agent_confirmation(
                        request.window_id,
                        request.confirm_id,
                        false,
                    );
                }
            })
    } else {
        app
    };

    // feature 缺失时保留同一 App 类型并锁定前置参数门禁。
    #[cfg(not(feature = "agent-control"))]
    let app = {
        debug_assert!(!options.agent_control);
        app
    };

    let _ = app.run();

    // 宿主最终消费：应用循环结束后落盘复现清单并输出报告摘要。
    finalize_diagnostics(&diagnostics_slot);
}

// 只组装语言文档无法拥有的应用生命周期策略；全部界面来自 main.uix。
fn build_demo_app(
    follow_system_theme: bool,
    agent_handle_slot: std::sync::Arc<std::sync::Mutex<Option<uix::app::AppHandle>>>,
    diagnostics_slot: std::sync::Arc<std::sync::Mutex<Option<uix::diagnostics::Diagnostics>>>,
    report_subscription_slot: std::sync::Arc<
        std::sync::Mutex<Option<uix::diagnostics::ReportSubscription>>,
    >,
) -> App {
    // 宿主显式配置诊断：崩溃报告与复现清单写入固定临时目录。
    uix_app!("src/main.uix")
        .custom_title_bar(true)
        .follow_system_theme(follow_system_theme)
        .diagnostics(
            uix::diagnostics::DiagnosticsConfig::default()
                .crash_report_directory(demo_diagnostics_directory()),
        )
        .on_start(move |handle| {
            *agent_handle_slot.lock().unwrap() = Some(handle.clone());
            // 真实终端的读线程唤醒经主窗口句柄 post_to_ui 回 UI 线程。
            live_terminal::install_ui_handle(&handle);
            // 诊断句柄可 clone 且在窗口关闭后仍可用，供退出路径消费。
            let diagnostics = handle.diagnostics();
            // 报告即时订阅：任何报告入库的同时实时输出摘要，错误发生的
            // 第一时间控制台可见，无需事后翻快照。
            let subscription = diagnostics.on_report(|report| {
                tracing::warn!(
                    code = %report.code(),
                    severity = ?report.severity(),
                    origin = report.origin_target(),
                    operation = report.operation().unwrap_or(""),
                    summary = report.summary(),
                    "framework or application report emitted in real time"
                );
            });
            *report_subscription_slot.lock().unwrap() = Some(subscription);
            *diagnostics_slot.lock().unwrap() = Some(diagnostics);
        })
}

// demo 诊断落盘目录：固定在系统临时目录下，便于人工检视后整体清理。
fn demo_diagnostics_directory() -> std::path::PathBuf {
    std::env::temp_dir().join("uix-lang-demo-diagnostics")
}

// 应用退出后的宿主诊断消费：写出复现清单并汇总留存报告。
fn finalize_diagnostics(
    diagnostics_slot: &std::sync::Arc<std::sync::Mutex<Option<uix::diagnostics::Diagnostics>>>,
) {
    // 测试验收组合不经过 demo 组装根，没有可消费的诊断句柄。
    let Some(diagnostics) = diagnostics_slot.lock().unwrap().clone() else {
        return;
    };
    let snapshot = diagnostics.snapshot();
    tracing::info!(
        total_reports = snapshot.total_reports(),
        evicted_reports = snapshot.evicted_reports(),
        retained_reports = snapshot.reports().len(),
        "demo diagnostics summary"
    );
    match diagnostics.write_debug_repro_manifest(demo_diagnostics_directory()) {
        Ok(path) => tracing::info!(manifest = %path.display(), "demo debug repro manifest written"),
        Err(error) => tracing::warn!("demo debug repro manifest write failed: {}", error.what()),
    }
}

// 将确定性正文资源注入 Application 组合根，不让声明页面接触字体句柄。
fn with_deterministic_fonts(app: App) -> App {
    app.font_bundle(uix::draw::FontBundle::from_static(
        "Noto Sans CJK SC",
        DETERMINISTIC_UI_FONT,
    ))
}

// 初始化演示程序使用的环境过滤日志订阅器。
fn init_tracing() {
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("uix=info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

// 保存二进制组合根的近邻启动参数回归测试。
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_lifecycle_gates_together() {
        let options = parse_launch_options([
            "--agent-control".to_string(),
            "--unknown".to_string(),
            "--test-graphics-recovery".to_string(),
            "--test-graphics-readback".to_string(),
            "--follow-system-theme".to_string(),
        ]);

        assert!(options.agent_control);
        assert!(options.graphics_recovery_test);
        assert!(options.graphics_readback_test);
        assert!(options.follow_system_theme);
    }
}
