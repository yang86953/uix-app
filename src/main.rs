// ============================================================================
// UIX Framework Demo Entry Point
// ============================================================================
// 运行方式：
//   cargo run --bin uix-demo          → GUI demo
//   cargo run --bin uix-demo -- --cli → CLI demo
// ============================================================================

mod demos;

use uix::diag::log::{info_fn, Level, Logger};

fn main() {
    Logger::instance().set_level(Level::Info);

    let args: Vec<String> = std::env::args().collect();
    let is_cli = args.iter().any(|a| a == "--cli");

    if is_cli {
        info_fn("UIX CLI Demo starting...");
        if let Err(e) = demos::cli::run_all_cli() {
            eprintln!("CLI demo error: {}", e.short_what());
            std::process::exit(1);
        }
    } else {
        #[cfg(windows)]
        {
            info_fn("UIX Windows GUI Demo starting...");
            demos::gui::run_gui_demo();
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            info_fn("UIX Linux GUI Demo starting...");
            demos::gui_linux::run_gui_demo();
        }
        #[cfg(target_os = "macos")]
        {
            compile_error!("macOS is not yet supported by UIX");
        }
    }
}
