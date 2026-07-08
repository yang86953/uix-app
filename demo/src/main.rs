// ============================================================================
// UIX 演示入口
// ============================================================================
//   cargo run --bin uix-demo              → 简化 API（prelude + App）
//   cargo run --bin uix-demo -- --dashboard → 完整 GUI 组件库（高级 API）
//   cargo run --bin uix-demo -- --cli     → CLI 功能域演示
//   cargo run --bin uix-demo -- --dashboard --gpu → GUI 组件库 + GPU 引擎
// ============================================================================

mod demos;

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
        if let Err(e) = demos::cli::run_all_cli() {
            eprintln!("CLI 演示出错: {}", e.short_what());
            std::process::exit(1);
        }
    } else if is_dashboard {
        info_fn("UIX GUI 组件库演示启动中...");
        demos::dashboard::run_gui_demo();
    } else {
        info_fn("UIX 简化 API 演示启动中...");
        demos::simplified::run_simplified_demo();
    }
}
