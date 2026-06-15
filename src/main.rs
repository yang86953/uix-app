// ============================================================================
// UIX 框架演示入口
// ============================================================================
// 运行方式：
//   cargo run --bin uix-demo          → GUI 演示
//   cargo run --bin uix-demo -- --cli → CLI 演示
// ============================================================================

mod demos;

use uix::diag::log::{info_fn, Level, Logger};

fn main() {
    Logger::instance().set_level(Level::Info);

    let args: Vec<String> = std::env::args().collect();
    let is_cli = args.iter().any(|a| a == "--cli");

    if is_cli {
        info_fn("UIX CLI 演示启动中...");
        if let Err(e) = demos::cli::run_all_cli() {
            eprintln!("CLI 演示出错: {}", e.short_what());
            std::process::exit(1);
        }
    } else {
        info_fn("UIX GUI 演示启动中...");
        demos::dashboard::run_gui_demo();
    }
}
