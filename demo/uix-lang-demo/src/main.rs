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

    // 测试能力存在时按运行时开关选择独立验收组合或普通主演示。
    #[cfg(feature = "test-harness")]
    let app = if options.graphics_recovery_test {
        graphics_recovery::build_app()
    } else if options.graphics_readback_test {
        graphics_readback::build_app()
    } else {
        build_demo_app(options.follow_system_theme, agent_handle_slot.clone())
    };

    // 测试能力缺失时只保留全组件主演示组合路径。
    #[cfg(not(feature = "test-harness"))]
    let app = {
        debug_assert!(!options.graphics_recovery_test);
        debug_assert!(!options.graphics_readback_test);
        build_demo_app(options.follow_system_theme, agent_handle_slot.clone())
    };

    // 所有启动模式都在窗口创建前安装同一正文与 CJK 字体包。
    let app = with_deterministic_fonts(app);

    // feature 与运行参数同时存在时才开放 Agent Bridge。
    #[cfg(feature = "agent-control")]
    let app = if options.agent_control {
        app.enable_agent_control()
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

    app.run();
}

// 只组装语言文档无法拥有的应用生命周期策略；全部界面来自 main.uix。
fn build_demo_app(
    follow_system_theme: bool,
    agent_handle_slot: std::sync::Arc<std::sync::Mutex<Option<uix::app::AppHandle>>>,
) -> App {
    uix_app!("src/main.uix")
        .custom_title_bar(true)
        .follow_system_theme(follow_system_theme)
        .on_start(move |handle| {
            *agent_handle_slot.lock().unwrap() = Some(handle.clone());
        })
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
