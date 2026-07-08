// ============================================================================
// UIX 演示入口 — 模式分派
// ============================================================================
//   cargo run --bin uix-demo              → 入门演示（App + View DSL）
//   cargo run --bin uix-demo -- --dashboard → 组件库全景
//   cargo run --bin uix-demo -- --cli     → CLI 功能域演示
//   cargo run --bin uix-demo -- --dashboard --gpu → 同上（App 默认优先 GPU）
// ============================================================================

mod common;
mod demos;
mod modes;

use uix::core::log::{info_fn, Level, Logger};

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

    let args: Vec<String> = std::env::args().collect();
    let is_cli = args.iter().any(|a| a == "--cli");
    let is_dashboard = args.iter().any(|a| a == "--dashboard");

    if is_cli {
        info_fn("UIX CLI 演示启动中...");
        if let Err(e) = modes::cli::run() {
            eprintln!("CLI 演示出错: {}", e.short_what());
            std::process::exit(1);
        }
    } else if is_dashboard {
        info_fn("UIX 组件库演示启动中...");
        modes::dashboard::run();
    } else {
        info_fn("UIX 入门演示启动中...");
        modes::default::run();
    }
}
