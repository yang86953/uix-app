// ============================================================================
// UIX 框架演示入口
// ============================================================================
// 运行方式：
//   cargo run --bin uix-demo                   → 完整 GUI 演示
//   cargo run --bin uix-demo -- --cli          → CLI 演示
//   cargo run --bin uix-demo -- --simple       → 简化 API 演示
// ============================================================================

mod demos;

use uix::platform::log::{info_fn, Level, Logger};

fn main() {
    let log_level = std::env::var("RUST_LOG")
        .ok()
        .and_then(|s| match s.to_lowercase().as_str() {
            "trace" => Some(Level::Trace),
            "debug" => Some(Level::Debug),
            "warn" => Some(Level::Warn),
            "error" => Some(Level::Error),
            _ => None,
        })
        .unwrap_or(Level::Info);
    Logger::instance().set_level(log_level);

    let args: Vec<String> = std::env::args().collect();
    let is_cli = args.iter().any(|a| a == "--cli");
    let is_simple = args.iter().any(|a| a == "--simple");

    if is_cli {
        info_fn("UIX CLI 演示启动中...");
        if let Err(e) = demos::cli::run_all_cli() {
            eprintln!("CLI 演示出错: {}", e.short_what());
            std::process::exit(1);
        }
    } else if is_simple {
        info_fn("UIX 简化 API 演示启动中...");
        demos::simplified::run_simplified_demo();
    } else {
        info_fn("UIX GUI 演示启动中...");
        demos::dashboard::run_gui_demo();
    }
}
