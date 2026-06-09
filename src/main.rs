// ============================================================================
// UIX Framework Dashboard Demo — Ant Design 5 Style GUI
// ============================================================================
// 运行方式：cargo run -p uix-demo
// ============================================================================

mod demos;

use uix::diag::log::{info_fn, Level, Logger};

#[cfg(windows)]
fn main() {
    Logger::instance().set_level(Level::Info);

    info_fn("UIX Dashboard Demo starting...");
    demos::gui::run_gui_demo();
}

#[cfg(all(unix, not(target_os = "macos")))]
fn main() {
    Logger::instance().set_level(Level::Info);

    info_fn("UIX Linux Demo starting...");

    // Platform-agnostic: Window::create() handles init, center, show, raise.
    let mut app = uix::app::App::new();
    app.init();
    app.create_window("UIX on Linux", 1024, 768);
    app.run();
}
