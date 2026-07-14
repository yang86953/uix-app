//! Widget 展示辅助 — 使用 [`使用.md`](../../docs/使用.md) 风格。
//! 以下为模板代码，展示正确 API 用法。

use uix::prelude::*;

/// 内容面板：抬升底 + 边框。
pub fn panel(tk: &DesignTokens, body: impl View) -> impl View {
    column((body,))
        .padding(16.0)
        .bg(tk.color_bg_elevated)
        .radius(tk.border_radius)
        .border(1.0, tk.color_border_secondary)
}

/// 信息提示条。
pub fn info_note(tk: &DesignTokens, text: &str) -> impl View {
    row((
        icon("info").size(16.0),
        label(text).fg(tk.color_info).font_size(12.0),
    ))
    .align(AlignItems::Center)
    .gap(10.0)
    .padding(EdgeInsets::new(12.0, 10.0, 12.0, 10.0))
    .bg(tk.color_info_bg)
    .radius(tk.border_radius)
    .border(1.0, tk.color_info_border)
}

/// 警告提示条。
pub fn backlog_note(tk: &DesignTokens, text: &str) -> impl View {
    row((
        icon("alert-triangle").size(16.0),
        label(text).fg(tk.color_warning).font_size(12.0),
    ))
    .align(AlignItems::Center)
    .gap(10.0)
    .padding(EdgeInsets::new(12.0, 10.0, 12.0, 10.0))
    .bg(tk.color_warning_bg)
    .radius(tk.border_radius)
    .border(1.0, tk.color_warning_border)
}

/// 带标题的内容卡片。
pub fn demo_card(
    tk: &DesignTokens,
    title: &str,
    width: f32,
    height: f32,
    body: impl View,
) -> impl View {
    column((
        label(title).fg(tk.color_text).font_size(14.0),
        space(10.0),
        body,
    ))
    .width(width)
    .height(height)
    .padding(16.0)
    .bg(tk.color_bg_elevated)
    .radius(tk.border_radius_lg)
    .border(1.0, tk.color_border_secondary)
}
