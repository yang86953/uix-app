//! Widget 展示辅助 — 面板 / 样例块 / 提示条，统一各页视觉节奏。

use uix::prelude::*;
use uix::ui::WidgetComponent;

use super::page::demo_row;

/// 组件名小字标签（固定宽度列）。
/// 用于纵向堆叠的 `labeled_row`，使多行标签列对齐。
pub fn widget_caption(_tk: &DesignTokens, name: &str) -> Label {
    Label::new(name)
        .style(Style {
            color: ColorValue::Neutral(NeutralRole::TextTertiary),
            ..Style::default()
        })
        .font_size(11.0)
        .size(148.0, 18.0)
}

/// 内容面板：抬升底 + 边框，随父级 Stretch 拉满。
pub fn panel(tk: &DesignTokens, body: ViewNode) -> ViewNode {
    column_fit([body])
        .padding(EdgeInsets::uniform(16.0))
        .bg(ColorValue::Neutral(NeutralRole::BgElevated))
        .radius(tk.border_radius)
        .border(1.0, ColorValue::Neutral(NeutralRole::BorderSecondary))
}

/// 可换行的水平样例流（不写死 INNER_W；flex_grow 吃满父级宽，窄窗下 wrap）。
pub fn flow_row(min_h: f32) -> Space {
    Space::new()
        .size(SpaceSize::Middle)
        .height(min_h)
        .direction(FlexDirection::Row)
        .wrap(true)
        .flex_grow(1.0)
        .align(AlignItems::Start)
}

/// 垂直样例块：上标签、下控件（画廊主模式）。
pub fn sample_block(
    _tk: &DesignTokens,
    name: &str,
    widget: impl WidgetComponent + 'static,
) -> Space {
    Space::new()
        .size(SpaceSize::Small)
        .direction(FlexDirection::Column)
        .align(AlignItems::Start)
        .child(
            Label::new(name)
                .style(Style {
                    color: ColorValue::Neutral(NeutralRole::TextTertiary),
                    ..Style::default()
                })
                .font_size(11.0),
        )
        .child(widget)
}

/// 水平样例对：短标签 + 控件（intrinsic 宽）。
#[allow(dead_code)]
pub fn sample_pair(
    _tk: &DesignTokens,
    name: &str,
    widget: impl WidgetComponent + 'static,
) -> Space {
    Space::new()
        .size(SpaceSize::Small)
        .direction(FlexDirection::Row)
        .align(AlignItems::Center)
        .child(
            Label::new(name)
                .style(Style {
                    color: ColorValue::Neutral(NeutralRole::TextTertiary),
                    ..Style::default()
                })
                .font_size(11.0),
        )
        .child(widget)
}

/// 单行：左标签 + 右 widget（纵向列表对齐用）。
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

/// 信息提示条。
pub fn info_note(tk: &DesignTokens, text: &str) -> ViewNode {
    row([
        embed(Icon::new("info").size(16.0)),
        label(text)
            .color(ColorValue::Palette(PaletteColor::Info))
            .font_size(12.0),
    ])
    .align(AlignItems::Center)
    .gap(10.0)
    .padding(EdgeInsets::new(12.0, 10.0, 12.0, 10.0))
    .bg(ColorValue::Palette(PaletteColor::InfoBg))
    .radius(tk.border_radius)
    .border(1.0, ColorValue::Palette(PaletteColor::InfoBorder))
}

/// 警告提示条（未 demo 的 backlog 项）。
pub fn backlog_note(tk: &DesignTokens, text: &str) -> ViewNode {
    row([
        embed(Icon::new("alert-triangle").size(16.0)),
        label(text)
            .color(ColorValue::Palette(PaletteColor::Warning))
            .font_size(12.0),
    ])
    .align(AlignItems::Center)
    .gap(10.0)
    .padding(EdgeInsets::new(12.0, 10.0, 12.0, 10.0))
    .bg(ColorValue::Palette(PaletteColor::WarningBg))
    .radius(tk.border_radius)
    .border(1.0, ColorValue::Palette(PaletteColor::WarningBorder))
}

/// 带标题的内容卡片（固定宽高样例）。
pub fn demo_card(
    tk: &DesignTokens,
    title: &str,
    width: f32,
    height: f32,
    body: ViewNode,
) -> ViewNode {
    column_fit([
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
