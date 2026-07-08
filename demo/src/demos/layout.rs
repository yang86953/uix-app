//! 组件库页面 — page_layout。

use uix::prelude::*;

use crate::common::page::{demo_row, PageBuilder, INNER_W};

pub fn page_layout(tk: &DesignTokens) -> ViewNode {
    let c4 = (INNER_W - 24.0) / 4.0;

    // ── 调试辅助：简单 Flex Row ──
    let r = |label: &str, color: Color| {
        tree! { Container::new().size(80.0, 28.0).bg(color).rounded(4.0) => [
            Label::new(label).color(Color::white()).font_size(11.0),
        ]}
    };

    PageBuilder::new(tk)
        .gap()
        .section("Flex Row — 水平排列（SpaceBetween）")
        .push(tree! { Container::new().w(INNER_W).h(70.0).flex_grow(1.0).dir(FlexDirection::Row)
            .gap(12.0).rounded(4.0).align(AlignItems::Center)
            .justify(JustifyContent::SpaceBetween)
            .bg(tk.color_fill)
            .border(tk.color_border, 1.5) => [
            r("A", Color::from_rgb(64,150,255)),
            r("B", Color::from_rgb(82,196,26)),
            r("C", Color::from_rgb(250,173,20)),
            r("D", Color::from_rgb(114,46,209)),
            r("E", Color::from_rgb(245,34,45)),
            r("F", Color::from_rgb(19,194,194)),
            r("G", Color::from_rgb(250,140,22)),
        ]})
        .section("Flex Column — 垂直排列")
        .push(tree! { Container::new().w(INNER_W).h(230.0).flex_grow(1.0).dir(FlexDirection::Column)
            .gap(8.0).rounded(4.0).align(AlignItems::Center)
            .bg(tk.color_fill)
            .border(tk.color_border, 1.5) => [
            r("壹", Color::from_rgb(64,150,255)),
            r("贰", Color::from_rgb(82,196,26)),
            r("叁", Color::from_rgb(250,173,20)),
            r("肆", Color::from_rgb(114,46,209)),
            r("伍", Color::from_rgb(245,34,45)),
            r("陆", Color::from_rgb(19,194,194)),
            r("柒", Color::from_rgb(250,140,22)),
        ]})
        .section("Padding 对比 — 各 pad 值下 child x 坐标")
        .push(tree! { Container::new().size(INNER_W, 70.0).dir(FlexDirection::Row)
            .gap(12.0).rounded(4.0) => [
            tree! { Container::new().size(150.0, 50.0).bg(tk.color_bg_raised)
                .dir(FlexDirection::Row).align(AlignItems::Start).rounded(4.0) => [
                r("pad=0", Color::from_rgb(64,150,255).with_alpha(150)),
            ]},
            tree! { Container::new().size(150.0, 50.0).bg(tk.color_bg_raised)
                .dir(FlexDirection::Row).align(AlignItems::Start).rounded(4.0)
                .pad(EdgeInsets::uniform(8.0)) => [
                r("pad=8", Color::from_rgb(255,77,79).with_alpha(150)),
            ]},
            tree! { Container::new().size(150.0, 50.0).bg(tk.color_bg_raised)
                .dir(FlexDirection::Row).align(AlignItems::Start).rounded(4.0)
                .pad(EdgeInsets::new(16.0, 0.0, 0.0, 0.0)) => [
                r("padL=16", Color::from_rgb(250,173,20).with_alpha(150)),
            ]},
            tree! { Container::new().size(150.0, 50.0).bg(tk.color_bg_raised)
                .dir(FlexDirection::Row).align(AlignItems::Start).rounded(4.0)
                .pad(EdgeInsets::new(0.0, 0.0, 8.0, 8.0)) => [
                r("padBR=8", Color::from_rgb(82,196,26).with_alpha(150)),
            ]},
        ]})
        .section("网格 Grid — 2 列")
        .push(tree! { Grid::two_columns().gap(8.0).pad(EdgeInsets::uniform(4.0)).size(INNER_W, 70.0) => [
            tree! { Container::new().size(100.0, 60.0).bg(tk.color_primary_bg).rounded(tk.border_radius_sm) =>
                [Label::new("Column 1").color(tk.color_primary).font_size(13.0)]},
            tree! { Container::new().size(100.0, 60.0).bg(tk.color_success_bg).rounded(tk.border_radius_sm) =>
                [Label::new("Column 2").color(tk.color_success).font_size(13.0)]},
        ]})
        .section("网格 Grid — 3 列")
        .push(tree! { Grid::three_columns().gap(8.0).pad(EdgeInsets::uniform(4.0)).size(INNER_W, 70.0) => [
            tree! { Container::new().size(100.0, 60.0).bg(tk.color_primary_bg).rounded(tk.border_radius_sm) =>
                [Label::new("Cell A").color(tk.color_primary).font_size(13.0)]},
            tree! { Container::new().size(100.0, 60.0).bg(tk.color_warning_bg).rounded(tk.border_radius_sm) =>
                [Label::new("Cell B").color(tk.color_warning).font_size(13.0)]},
            tree! { Container::new().size(100.0, 60.0).bg(tk.color_success_bg).rounded(tk.border_radius_sm) =>
                [Label::new("Cell C").color(tk.color_success).font_size(13.0)]},
        ]})
        .section("网格 Grid — 自定义 (1fr 2fr)")
        .push(tree! { Grid::new()
            .columns(vec![GridTrack::Fr(1.0), GridTrack::Fr(2.0)])
            .gap(8.0).pad(EdgeInsets::uniform(4.0)).size(INNER_W, 70.0) => [
            tree! { Container::new().size(100.0, 60.0).bg(tk.color_primary_bg).rounded(tk.border_radius_sm) =>
                [Label::new("1fr").color(tk.color_primary).font_size(13.0)]},
            tree! { Container::new().size(200.0, 60.0).bg(tk.color_info_bg).rounded(tk.border_radius_sm) =>
                [Label::new("2fr").color(tk.color_info).font_size(13.0)]},
        ]})
        .section("间距 Space")
        .push({
            let mut sp = Space::new().size(SpaceSize::Small).width(INNER_W).height(0.0)
                .direction(FlexDirection::Column);
            for (sz, label, h) in [
                (SpaceSize::Small, "Small (8px)", 32.0),
                (SpaceSize::Middle, "Middle (16px)", 56.0),
            ] {
                sp = sp.child(
                    Space::new().size(sz).width(INNER_W).height(h)
                        .direction(FlexDirection::Row).align(AlignItems::Center)
                        .child(Label::new(label).color(tk.color_text_tertiary).font_size(11.0))
                        .child(button("甲").widget())
                        .child(button("乙").widget()),
                );
            }
            sp
        })
        .section("分割线 Divider")
        .push(
            demo_row(36.0)
                .child(Label::new("左侧").color(tk.color_text).font_size(14.0))
                .child(Divider::new().vertical().color(tk.color_border))
                .child(Label::new("中间").color(tk.color_text).font_size(14.0))
                .child(Divider::new().vertical().color(tk.color_border))
                .child(Label::new("右侧").color(tk.color_text).font_size(14.0)),
        )
        .section("折叠面板 Collapse")
        .push(
            Space::new().size(SpaceSize::Small).width(INNER_W)
                .direction(FlexDirection::Column).align(AlignItems::Stretch)
                .child(Collapse::new().panels(vec![
                    CollapsePanel::new("面板 1：常规", "面板 1 的内容。").expanded(),
                    CollapsePanel::new("面板 2：设置", "配置选项与偏好设置。"),
                    CollapsePanel::new("面板 3：高级", "面向高级用户的设置。"),
                ])),
        )
        .section("分段器 Segmented")
        .push(
            demo_row(36.0)
                .child(Segmented::new().options(vec!["每日", "每周", "每月", "每年"]).selected(2)),
        )
        .section("分割面板 Splitter")
        .push(
            Space::new().size(SpaceSize::Custom(200.0)).height(140.0)
                .child(Splitter::new().panels(3).vertical(false)),
        )
        .section("卡片 Card — 阴影层级 0~3")
        .push(
            Space::new().size(SpaceSize::Middle).width(INNER_W).height(130.0)
                .direction(FlexDirection::Row).align(AlignItems::Stretch)
                .child(Card::new().title("Elevation 0").elevation(0).bordered(true).size(c4, 120.0)
                    .child(Label::new("有边框，无阴影").color(tk.color_text_tertiary).font_size(12.0)))
                .child(Card::new().title("Elevation 1").elevation(1).size(c4, 120.0)
                    .child(Label::new("柔和阴影").color(tk.color_text_tertiary).font_size(12.0)))
                .child(Card::new().title("Elevation 2").elevation(2).size(c4, 120.0)
                    .child(Label::new("中等阴影").color(tk.color_text_tertiary).font_size(12.0)))
                .child(Card::new().title("Elevation 3").elevation(3).size(c4, 120.0)
                    .child(Label::new("深阴影").color(tk.color_text_tertiary).font_size(12.0))),
        )
        .build()

}
