//! uix-app 窗口层集成测试 —— Window 状态方法
//!
//! 只测试不依赖平台交互的状态方法。create / show / run 需要真实平台窗口，跳过。

use uix::runtime::window::Window;
use uix::platform::Point;

// ════════════════════════════════════════════════════════════════════════════
// Window 构造
// ════════════════════════════════════════════════════════════════════════════

fn new_window() -> Option<Window> {
    match uix::platform::create_platform() {
        Ok(p) => Some(Window::new(p)),
        Err(_) => None,
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 调试模式
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn debug_mode_default() {
    if let Some(window) = new_window() {
        let default = window.debug_mode();
        // 默认值取决于 UIX_DEBUG 环境变量是否设置
        let expected = std::env::var("UIX_DEBUG").is_ok();
        assert_eq!(default, expected);
    }
}

#[test]
fn set_debug_mode_enables_debug() {
    if let Some(window) = new_window() {
        window.set_debug_mode(true);
        assert!(window.debug_mode());
    }
}

#[test]
fn set_debug_mode_disables_debug() {
    if let Some(window) = new_window() {
        window.set_debug_mode(true);
        window.set_debug_mode(false);
        assert!(!window.debug_mode());
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 光标位置
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn cursor_pos_default_is_origin() {
    if let Some(window) = new_window() {
        assert_eq!(window.cursor_pos(), Point::new(0.0, 0.0));
    }
}

#[test]
fn set_cursor_pos_updates_position() {
    if let Some(window) = new_window() {
        window.set_cursor_pos(Point::new(100.0, 200.0));
        assert_eq!(window.cursor_pos(), Point::new(100.0, 200.0));
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 运行状态
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn is_running_starts_false() {
    if let Some(window) = new_window() {
        assert!(!window.is_running());
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 窗口尺寸
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn size_returns_initial_size() {
    if let Some(window) = new_window() {
        assert_eq!(window.size(), (800, 600));
    }
}
