//! Widget 展示辅助 — 统一「标签 + 实例」行模式，减少各页样板代码。

use uix::prelude::*;
use uix::ui::WidgetComponent;

use super::page::demo_row;

/// 组件名小字标签（固定宽度列）。
pub fn widget_caption(_tk: &DesignTokens, name: &str) -> Label {
    Label::new(name)
        .style(Style {
            color: ColorValue::Neutral(NeutralRole::TextTertiary),
            ..Style::default()
        })
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
/// 不写死宽度：父级 Column 默认 Stretch 会拉满内容区。
pub fn info_note(tk: &DesignTokens, text: &str) -> ViewNode {
    row([
        embed(Icon::new("info").size(16.0)),
        space(10.0),
        label(text)
            .color(ColorValue::Palette(PaletteColor::Info))
            .font_size(12.0),
    ])
    .padding(EdgeInsets::new(12.0, 10.0, 12.0, 10.0))
    .bg(ColorValue::Palette(PaletteColor::InfoBg))
    .radius(tk.border_radius)
    .border(1.0, ColorValue::Palette(PaletteColor::InfoBorder))
}

/// 警告提示条（未 demo 的 backlog 项）。
pub fn backlog_note(tk: &DesignTokens, text: &str) -> ViewNode {
    row([
        embed(Icon::new("alert-triangle").size(16.0)),
        space(10.0),
        label(text)
            .color(ColorValue::Palette(PaletteColor::Warning))
            .font_size(12.0),
    ])
    .padding(EdgeInsets::new(12.0, 10.0, 12.0, 10.0))
    .bg(ColorValue::Palette(PaletteColor::WarningBg))
    .radius(tk.border_radius)
    .border(1.0, ColorValue::Palette(PaletteColor::WarningBorder))
}

/// 带标题的内容卡片（用于首页等展示区块）。
pub fn demo_card(
    tk: &DesignTokens,
    title: &str,
    width: f32,
    height: f32,
    body: ViewNode,
) -> ViewNode {
    column([
        label(title)
            .color(ColorValue::Neutral(NeutralRole::Text))
            .font_size(14.0),
        space(10.0),
        body,
    ])
    .width(width)
    .height(height)
    .padding(EdgeInsets::uniform(16.0))
    .bg(ColorValue::Neutral(NeutralRole::BgElevated))
    .radius(tk.border_radius_lg)
    .border(1.0, ColorValue::Neutral(NeutralRole::BorderSecondary))
}
