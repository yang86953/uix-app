//! Widget 展示辅助 — 统一「标签 + 实例」行模式，减少各页样板代码。

use uix::prelude::*;
use uix::ui::WidgetComponent;

use super::page::{demo_row, INNER_W};

/// 组件名小字标签（固定宽度列）。
pub fn widget_caption(tk: &DesignTokens, name: &str) -> Label {
    Label::new(name)
        .color(tk.color_text_tertiary)
        .font_size(11.0)
        .size(120.0, 18.0)
}

/// 单行：左标签 + 右 widget。
pub fn labeled_row(
    tk: &DesignTokens,
    height: f32,
    name: &str,
    widget: impl WidgetComponent + 'static,
) -> Space {
    demo_row(height)
        .child(widget_caption(tk, name))
        .child(widget)
}

/// 信息提示条（用于说明 backlog / 平台限制）。
pub fn info_note(tk: &DesignTokens, text: &str) -> impl IntoWidgetNode {
    tree! { Container::new().size(INNER_W, 40.0)
        .bg(tk.color_info_bg)
        .rounded(tk.border_radius_sm)
        .pad(EdgeInsets::uniform(8.0)) => [
        Label::new(text).color(tk.color_info).font_size(12.0),
    ]}
}

/// 警告提示条（未 demo 的 backlog 项）。
pub fn backlog_note(tk: &DesignTokens, text: &str) -> impl IntoWidgetNode {
    tree! { Container::new().size(INNER_W, 40.0)
        .bg(tk.color_warning_bg)
        .rounded(tk.border_radius_sm)
        .pad(EdgeInsets::uniform(8.0)) => [
        Label::new(text).color(tk.color_warning).font_size(12.0),
    ]}
}
