//! 组件库页面 — page_layout（布局 / 容器全覆盖）。

use uix::prelude::*;

use crate::common::page::{PageBuilder, INNER_W};
use crate::common::showcase::labeled_row;
use crate::demos::context::DemoCtx;

pub fn page_layout(ctx: &DemoCtx<'_>) -> ViewNode {
    let tk = ctx.tk;

    let r = |label: &str, color: Color| {
        tree! { Container::new().size(80.0, 28.0).bg(color).rounded(4.0) => [
            Label::new(label).color(Color::white()).font_size(11.0),
        ]}
    };

    PageBuilder::new(tk)
        .gap()
        .section("Container — Flex Row")
        .push(tree! { Container::new().w(INNER_W).h(70.0).dir(FlexDirection::Row)
            .gap(12.0).rounded(4.0).align(AlignItems::Center)
            .justify(JustifyContent::SpaceBetween)
            .bg(tk.color_fill).border(tk.color_border, 1.5) => [
            r("A", Color::from_rgb(64, 150, 255)),
            r("B", Color::from_rgb(82, 196, 26)),
            r("C", Color::from_rgb(250, 173, 20)),
            r("D", Color::from_rgb(114, 46, 209)),
        ]})
        .section("Container — Flex Column")
        .push(tree! { Container::new().w(INNER_W).h(160.0).dir(FlexDirection::Column)
            .gap(8.0).rounded(4.0).align(AlignItems::Center)
            .bg(tk.color_fill).border(tk.color_border, 1.5) => [
            r("壹", Color::from_rgb(64, 150, 255)),
            r("贰", Color::from_rgb(82, 196, 26)),
            r("叁", Color::from_rgb(250, 173, 20)),
        ]})
        .section("Grid — 2 / 3 / 自定义列")
        .push(tree! { Grid::two_columns().gap(8.0).pad(EdgeInsets::uniform(4.0)).size(INNER_W, 70.0) => [
            tree! { Container::new().size(100.0, 60.0).bg(tk.color_primary_bg).rounded(tk.border_radius_sm) =>
                [Label::new("2 col A").color(tk.color_primary).font_size(13.0)]},
            tree! { Container::new().size(100.0, 60.0).bg(tk.color_success_bg).rounded(tk.border_radius_sm) =>
                [Label::new("2 col B").color(tk.color_success).font_size(13.0)]},
        ]})
        .push(tree! { Grid::three_columns().gap(8.0).pad(EdgeInsets::uniform(4.0)).size(INNER_W, 70.0) => [
            tree! { Container::new().size(80.0, 60.0).bg(tk.color_primary_bg).rounded(tk.border_radius_sm) =>
                [Label::new("A").color(tk.color_primary).font_size(13.0)]},
            tree! { Container::new().size(80.0, 60.0).bg(tk.color_warning_bg).rounded(tk.border_radius_sm) =>
                [Label::new("B").color(tk.color_warning).font_size(13.0)]},
            tree! { Container::new().size(80.0, 60.0).bg(tk.color_success_bg).rounded(tk.border_radius_sm) =>
                [Label::new("C").color(tk.color_success).font_size(13.0)]},
        ]})
        .push(tree! { Grid::new()
            .columns(vec![GridTrack::Fr(1.0), GridTrack::Fr(2.0)])
            .gap(8.0).size(INNER_W, 70.0) => [
            tree! { Container::new().size(100.0, 60.0).bg(tk.color_primary_bg).rounded(tk.border_radius_sm) =>
                [Label::new("1fr").color(tk.color_primary).font_size(13.0)]},
            tree! { Container::new().size(200.0, 60.0).bg(tk.color_info_bg).rounded(tk.border_radius_sm) =>
                [Label::new("2fr").color(tk.color_info).font_size(13.0)]},
        ]})
        .section("Layout — Header / Sider / Content / Footer")
        .push(tree! { Layout::new().bg(tk.color_bg_layout) => [
            Header::new(36.0).bg(tk.color_primary_bg),
            tree! { Container::new().size(INNER_W, 100.0).dir(FlexDirection::Row) => [
                Sider::new(100.0).bg(tk.color_fill_secondary),
                Content::new().bg(tk.color_bg_container),
            ]},
            Footer::new(28.0).bg(tk.color_fill_tertiary),
        ]})
        .section("Splitter — 水平 / 垂直")
        .push(labeled_row(
            tk,
            120.0,
            "Horizontal",
            Splitter::new().panels(3).vertical(false),
        ))
        .push(labeled_row(
            tk,
            120.0,
            "Vertical",
            Splitter::new().panels(2).vertical(true),
        ))
        .section("ScrollView — Vertical")
        .push(
            ScrollView::new(ScrollDirection::Vertical)
                .size(INNER_W, 120.0)
                .child(Label::new("ScrollView 行 1 — 滚轮滚动").font_size(12.0))
                .child(Label::new("ScrollView 行 2").font_size(12.0))
                .child(Label::new("ScrollView 行 3").font_size(12.0))
                .child(Label::new("ScrollView 行 4").font_size(12.0))
                .child(Label::new("ScrollView 行 5").font_size(12.0))
                .child(Label::new("ScrollView 行 6").font_size(12.0)),
        )
        .section("Affix / BackTop")
        .push(labeled_row(tk, 40.0, "Affix", Affix::new(12.0)))
        .push(labeled_row(
            tk,
            40.0,
            "BackTop",
            BackTop::new().visibility_height(100.0),
        ))
        .build()
}
