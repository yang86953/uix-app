//! 标准窗口控制组的 UIX 声明壳。

use crate::ui::view::ViewNode;

use super::window_chrome::{WindowControl, window_control};

// 构造由 UIX 声明调用的最小化交互内核。
fn standard_minimize_control(content: ViewNode) -> ViewNode {
    window_control(WindowControl::Minimize, content)
}

// 构造由 UIX 声明调用的最大化或还原交互内核。
fn standard_maximize_control(content: ViewNode) -> ViewNode {
    window_control(WindowControl::MaximizeRestore, content)
}

// 构造由 UIX 声明调用的关闭交互内核。
fn standard_close_control(content: ViewNode) -> ViewNode {
    window_control(WindowControl::Close, content)
}

/// 构造标准最小化、最大化或还原与关闭窗口控制组合。
pub fn window_controls(show_minimize: bool, show_maximize: bool, show_close: bool) -> ViewNode {
    // 条件结构与排列由同目录 UIX 声明拥有，平台动作由 Rust 内核执行。
    crate::uix!("src/ui/widgets/window_controls/window_controls.uix")
}
