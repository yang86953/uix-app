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
// ============================================================================

mod cli;
mod common;
mod demos;
mod gui;

use uix::core::log::{info_fn, Level, Logger};

#[derive(Debug, Default, PartialEq, Eq)]
struct LaunchOptions {
    cli: bool,
    agent_control: bool,
    follow_system_theme: bool,
    graphics_recovery_acceptance: bool,
}

fn parse_launch_options(args: impl IntoIterator<Item = String>) -> LaunchOptions {
    let mut options = LaunchOptions::default();
    for argument in args {
        match argument.as_str() {
            "--cli" => options.cli = true,
            "--agent-control" => options.agent_control = true,
            "--follow-system-theme" => options.follow_system_theme = true,
            "--graphics-recovery-acceptance" => options.graphics_recovery_acceptance = true,
            _ => {}
        }
    }
    options
}

fn parse_log_level() -> Level {
    std::env::var("RUST_LOG")
        .ok()
        .and_then(|s| match s.to_lowercase().as_str() {
            "trace" => Some(Level::Trace),
            "debug" => Some(Level::Debug),
            "warn" => Some(Level::Warn),
            "error" => Some(Level::Error),
            _ => None,
        })
        .unwrap_or(Level::Info)
}

fn main() {
    Logger::instance().set_level(parse_log_level());
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
        info_fn("UIX CLI 演示启动中...");
        if let Err(e) = cli::run() {
            eprintln!("CLI 演示出错: {}", e.short_what());
            std::process::exit(1);
        }
    } else {
        info_fn("UIX GUI 演示启动中...");
        gui::run(
            options.agent_control,
            options.follow_system_theme,
            options.graphics_recovery_acceptance,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_launch_options, LaunchOptions};

    #[test]
    fn launch_options_keep_gui_capabilities_explicit() {
        assert_eq!(
            parse_launch_options([
                "--cli".to_owned(),
                "--agent-control".to_owned(),
                "--follow-system-theme".to_owned(),
                "--graphics-recovery-acceptance".to_owned(),
            ]),
            LaunchOptions {
                cli: true,
                agent_control: true,
                follow_system_theme: true,
                graphics_recovery_acceptance: true,
            }
        );
        assert_eq!(
            parse_launch_options(["--unknown".to_owned()]),
            LaunchOptions::default()
        );
    }
}
