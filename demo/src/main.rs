// ============================================================================
// UIX 演示入口 — GUI（默认）| CLI（--cli）
// ============================================================================
//   cargo run --bin uix-demo          → GUI 多页应用
//   cargo run --bin uix-demo -- --cli → CLI 功能域演示
//   cargo run --bin uix-demo -- --follow-system-theme
//                                      → GUI 跟随系统主题
//   cargo run --features agent-control --bin uix-demo -- --agent-control
//                                      → 启用 Agent Bridge 的 GUI
//   cargo run --features test-harness --bin uix-demo -- --graphics-recovery-acceptance
//                                      → 显示图形故障恢复验收路径
//   cargo run --release --bin uix-demo -- --g5-release-scenario
//                                      → 显式启动内部 G5 生产测量场景
// ============================================================================

mod cli;
mod common;
mod demos;
mod gui;

#[derive(Debug, Default, PartialEq, Eq)]
struct LaunchOptions {
    cli: bool,
    agent_control: bool,
    follow_system_theme: bool,
    graphics_recovery_acceptance: bool,
    component_qa: bool,
    g5_release_scenario: bool,
}

fn parse_launch_options(args: impl IntoIterator<Item = String>) -> LaunchOptions {
    let mut options = LaunchOptions::default();
    for argument in args {
        match argument.as_str() {
            "--cli" => options.cli = true,
            "--agent-control" => options.agent_control = true,
            "--follow-system-theme" => options.follow_system_theme = true,
            "--graphics-recovery-acceptance" => options.graphics_recovery_acceptance = true,
            "--component-qa" => options.component_qa = true,
            "--g5-release-scenario" => options.g5_release_scenario = true,
            _ => {}
        }
    }
    options
}

fn init_tracing() {
    use tracing_subscriber::EnvFilter;

    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("uix=info,uix_demo=info"));
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
    init_tracing();
    let options = parse_launch_options(std::env::args().skip(1));

    if options.cli && options.agent_control {
        eprintln!("--agent-control 只适用于 GUI 模式，不能与 --cli 同时使用");
        std::process::exit(2);
    }
    if options.cli && options.follow_system_theme {
        eprintln!("--follow-system-theme 只适用于 GUI 模式，不能与 --cli 同时使用");
        std::process::exit(2);
    }
    if options.cli && options.graphics_recovery_acceptance {
        eprintln!("--graphics-recovery-acceptance 只适用于 GUI 模式，不能与 --cli 同时使用");
        std::process::exit(2);
    }
    if options.cli && options.component_qa {
        eprintln!("--component-qa 只适用于 GUI 模式，不能与 --cli 同时使用");
        std::process::exit(2);
    }
    if options.cli && options.g5_release_scenario {
        eprintln!("--g5-release-scenario 只适用于 GUI 模式，不能与 --cli 同时使用");
        std::process::exit(2);
    }
    if options.g5_release_scenario
        && (options.agent_control
            || options.follow_system_theme
            || options.graphics_recovery_acceptance
            || options.component_qa)
    {
        eprintln!("--g5-release-scenario 必须独占启动，不能与其他 GUI 验收模式组合");
        std::process::exit(2);
    }
    #[cfg(not(feature = "agent-control"))]
    if options.agent_control {
        eprintln!("--agent-control 需要同时启用 Cargo feature：--features agent-control");
        std::process::exit(2);
    }
    #[cfg(not(feature = "test-harness"))]
    if options.graphics_recovery_acceptance {
        eprintln!(
            "--graphics-recovery-acceptance 需要同时启用 Cargo feature：--features test-harness"
        );
        std::process::exit(2);
    }

    if options.cli {
        tracing::info!("UIX CLI 演示启动中...");
        if let Err(e) = cli::run() {
            eprintln!("CLI 演示出错: {}", e.short_what());
            std::process::exit(1);
        }
    } else {
        tracing::info!("UIX GUI 演示启动中...");
        run_gui(options);
    }
}
