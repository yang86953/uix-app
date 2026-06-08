// ============================================================================
// UIX Framework Dashboard Demo — Ant Design 5 Style GUI
// ============================================================================
// 运行方式：cargo run -p uix-demo
// ============================================================================

mod demos;

use demos::gui::run_gui_demo;
use uix::platform::log::{info_fn, Level, Logger};

fn main() {
    Logger::instance().set_level(Level::Info);

    info_fn("UIX Dashboard Demo starting...");
    run_gui_demo();
}
