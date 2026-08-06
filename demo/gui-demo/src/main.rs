// ============================================================================
// UIX GUI 演示入口（CLI 演示见独立项目 demo/cli-demo）
// ============================================================================
//   cargo run --manifest-path demo/Cargo.toml --bin uix-demo
//                                      → GUI 多页应用
//   ... -- --follow-system-theme       → GUI 跟随系统主题
//   ... --features agent-control -- --agent-control
//                                      → 启用 Agent Bridge 的 GUI
//   ... --features test-harness -- --graphics-recovery-acceptance
//                                      → 显示图形故障恢复验收路径
//   ... --release -- --g5-release-scenario
//                                      → 显式启动内部 G5 生产测量场景
// ============================================================================

mod common;
mod demos;
mod gui;

use std::path::PathBuf;

#[derive(Debug, Default, PartialEq, Eq)]
struct LaunchOptions {
    agent_control: bool,
    follow_system_theme: bool,
    graphics_recovery_acceptance: bool,
    component_qa: bool,
    g5_release_scenario: bool,
    empty: bool,
    crash_dir: Option<PathBuf>,
}

fn parse_launch_options(args: impl IntoIterator<Item = String>) -> LaunchOptions {
    let mut options = LaunchOptions::default();
    let mut args = args.into_iter();
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--cli" => {
                eprintln!("--cli 已迁移到独立项目，请运行：cargo run --manifest-path demo/Cargo.toml --bin uix-cli-demo");
                std::process::exit(2);
            }
            "--agent-control" => options.agent_control = true,
            "--follow-system-theme" => options.follow_system_theme = true,
            "--graphics-recovery-acceptance" => options.graphics_recovery_acceptance = true,
            "--component-qa" => options.component_qa = true,
            "--g5-release-scenario" => options.g5_release_scenario = true,
            "--empty" => options.empty = true,
            "--crash-dir" => {
                if let Some(directory) = args.next() {
                    options.crash_dir = Some(PathBuf::from(directory));
                }
            }
            _ if argument.starts_with("--crash-dir=") => {
                options.crash_dir = Some(PathBuf::from(&argument["--crash-dir=".len()..]));
            }
            _ => {}
        }
    }
    options
}

// 演示日志 capability 关闭时不编译订阅器初始化实现。
#[cfg(feature = "demo-logging")]
// 初始化演示程序使用的环境过滤日志订阅器。
fn init_tracing() {
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("uix=info,uix_demo=info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

fn run_gui(options: LaunchOptions) {
    let run = move || {
        gui::run(
            options.agent_control,
            options.follow_system_theme,
            options.graphics_recovery_acceptance,
            options.component_qa,
            options.g5_release_scenario,
            options.empty,
            options.crash_dir,
        );
    };

    #[cfg(windows)]
    {
        // Windows executable main threads default to a small stack. The component
        // acceptance page intentionally constructs deeply nested, realistic view
        // trees, so keep the Win32 message loop on a dedicated UI thread with a
        // bounded stack large enough for build/reconcile/layout recursion.
        const WINDOWS_GUI_STACK_BYTES: usize = 8 * 1024 * 1024;
        let ui_thread = match std::thread::Builder::new()
            .name("uix-demo-gui".to_string())
            .stack_size(WINDOWS_GUI_STACK_BYTES)
            .spawn(run)
        {
            Ok(ui_thread) => ui_thread,
            Err(error) => panic!("spawn Windows UI thread: {error}"),
        };
        if let Err(payload) = ui_thread.join() {
            std::panic::resume_unwind(payload);
        }
    }

    #[cfg(not(windows))]
    run();
}

fn main() {
    // 仅在演示日志 capability 启用时安装全局订阅器。
    #[cfg(feature = "demo-logging")]
    // 安装订阅器后再解析并执行演示启动参数。
    init_tracing();
    let options = parse_launch_options(std::env::args().skip(1));

    if options.g5_release_scenario
        && (options.agent_control
            || options.follow_system_theme
            || options.graphics_recovery_acceptance
            || options.component_qa
            || options.empty)
    {
        eprintln!("--g5-release-scenario 必须独占启动，不能与其他 GUI 验收模式组合");
        std::process::exit(2);
    }
    #[cfg(not(feature = "agent-control"))]
    if options.agent_control {
        eprintln!(
            "--agent-control 需要同时启用 Cargo feature：\n  cargo run --manifest-path demo/Cargo.toml --features agent-control --bin uix-demo -- --agent-control"
        );
        std::process::exit(2);
    }
    #[cfg(not(feature = "test-harness"))]
    if options.graphics_recovery_acceptance {
        eprintln!(
            "--graphics-recovery-acceptance 需要同时启用 Cargo feature：\n  cargo run --manifest-path demo/Cargo.toml --features test-harness --bin uix-demo -- --graphics-recovery-acceptance"
        );
        std::process::exit(2);
    }

    tracing::info!("UIX GUI 演示启动中...");
    run_gui(options);
}
