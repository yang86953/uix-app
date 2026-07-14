//! Widget 展示辅助 — 面板 / 样例块 / 提示条，统一各页视觉节奏。

use uix::prelude::*;

/// 组件名小字标签（固定宽度列）。
/// 用于纵向堆叠的 `labeled_row`，使多行标签列对齐。
pub fn widget_caption(tk: &DesignTokens, name: &str) -> ViewNode {
    label(name)
        .fg(tk.color_text_tertiary)
        .font_size(11.0)
        .size(148.0, 18.0)
}

/// 内容面板：抬升底 + 边框，随父级 Stretch 拉满。
pub fn panel(tk: &DesignTokens, body: ViewNode) -> ViewNode {
    column((body,))
        .padding(16.0)
        .bg(tk.color_bg_elevated)
        .radius(tk.border_radius)
        .border(1.0, tk.color_border_secondary)
}

/// 可换行的水平样例流。
pub fn flow_row(min_h: f32) -> ViewNode {
    row(())
        .height(min_h)
        .wrap(true)
        .flex_grow(1.0)
        .align(AlignItems::Start)
}

/// 垂直样例块：上标签、下控件（画廊主模式）。
pub fn sample_block(tk: &DesignTokens, name: &str, widget: impl IntoViewChildren) -> ViewNode {
    column((
        label(name).font_size(11.0).fg(tk.color_text_tertiary),
        column(widget),
    ))
    .align(AlignItems::Start)
    .gap(4.0)
}

/// 水平样例对：短标签 + 控件（intrinsic 宽）。
pub fn sample_pair(tk: &DesignTokens, name: &str, widget: impl IntoViewChildren) -> ViewNode {
    row((
        label(name).font_size(11.0).fg(tk.color_text_tertiary),
        column(widget),
    ))
    .align(AlignItems::Center)
    .gap(8.0)
}

/// 单行：左标签 + 右 widget（纵向列表对齐用）。
pub fn labeled_row(
    tk: &DesignTokens,
    height: f32,
    name: &str,
    widget: impl IntoViewChildren,
) -> ViewNode {
    row((widget_caption(tk, name), column(widget)))
        .height(height)
        .align(AlignItems::Center)
        .gap(8.0)
}

/// 信息提示条。
pub fn info_note(tk: &DesignTokens, text: &str) -> ViewNode {
    row((
        Icon::new("info").size(16.0),
        label(text).fg(tk.color_info).font_size(12.0),
    ))
    .align(AlignItems::Center)
    .gap(10.0)
    .padding(EdgeInsets::new(12.0, 10.0, 12.0, 10.0))
    .bg(tk.color_info_bg)
    .radius(tk.border_radius)
    .border(1.0, tk.color_info_border)
}

/// 警告提示条（未 demo 的 backlog 项）。
pub fn backlog_note(tk: &DesignTokens, text: &str) -> ViewNode {
    row((
        Icon::new("alert-triangle").size(16.0),
        label(text).fg(tk.color_warning).font_size(12.0),
    ))
    .align(AlignItems::Center)
    .gap(10.0)
    .padding(EdgeInsets::new(12.0, 10.0, 12.0, 10.0))
    .bg(tk.color_warning_bg)
    .radius(tk.border_radius)
    .border(1.0, tk.color_warning_border)
}

/// 带标题的内容卡片（固定宽高样例）。
pub fn demo_card(
    tk: &DesignTokens,
    title: &str,
    width: f32,
    height: f32,
    body: ViewNode,
) -> ViewNode {
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
