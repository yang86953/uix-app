// ============================================================================
// UIX 演示入口 — GUI（默认）| CLI（--cli）
// ============================================================================
//   cargo run --bin uix-demo          → GUI 多页应用
//   cargo run --bin uix-demo -- --cli → CLI 功能域演示
// ============================================================================

mod cli;
mod common;
mod demos;
mod gui;

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

    if std::env::args().any(|a| a == "--cli") {
        info_fn("UIX CLI 演示启动中...");
        if let Err(e) = cli::run() {
            eprintln!("CLI 演示出错: {}", e.short_what());
            std::process::exit(1);
        }
    } else {
        info_fn("UIX GUI 演示启动中...");
        gui::run();
    }
}
