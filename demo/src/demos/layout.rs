//! 组件库页面 — page_layout（布局 / 容器全覆盖）。

use uix::prelude::*;

use crate::common::page::{PageBuilder, INNER_W};
use crate::common::showcase::labeled_row;
use crate::demos::context::DemoCtx;

pub fn page_layout(ctx: &DemoCtx<'_>) -> ViewNode {
    let tk = ctx.tk;

    /// Helper: colored label box for container demos.
    fn box_label(label_text: &str, color: Color) -> impl View {
        row((label(label_text).fg(Color::white()).font_size(11.0),))
            .width(80.0)
            .height(28.0)
            .bg(color)
            .radius(4.0)
    }

    PageBuilder::new(tk)
        .gap()
        .section("Container — Flex Row")
        .push(
            row((
                box_label("A", Color::from_rgb(64, 150, 255)),
                box_label("B", Color::from_rgb(82, 196, 26)),
                box_label("C", Color::from_rgb(250, 173, 20)),
                box_label("D", Color::from_rgb(114, 46, 209)),
            ))
            .width(INNER_W)
            .height(70.0)
            .gap(12.0)
            .radius(4.0)
            .align(AlignItems::Center)
            .justify(JustifyContent::SpaceBetween)
            .bg(tk.color_fill)
            .border(1.5, tk.color_border),
        )
        .section("Container — Flex Column")
        .push(
            column((
                box_label("壹", Color::from_rgb(64, 150, 255)),
                box_label("贰", Color::from_rgb(82, 196, 26)),
                box_label("叁", Color::from_rgb(250, 173, 20)),
            ))
            .width(INNER_W)
            .height(160.0)
            .gap(8.0)
            .radius(4.0)
            .align(AlignItems::Center)
            .bg(tk.color_fill)
            .border(1.5, tk.color_border),
        )
        .section("Grid — 2 / 3 / 自定义列")
        .push(
            Grid::new()
                .columns(2)
                .gap(8.0, 8.0)
                .size(INNER_W, 70.0)
                .children((
                    row((label("2 col A").fg(tk.color_primary).font_size(13.0),))
                        .width(100.0)
                        .height(60.0)
                        .bg(tk.color_primary_bg)
                        .radius(tk.border_radius_sm),
                    row((label("2 col B").fg(tk.color_success).font_size(13.0),))
                        .width(100.0)
                        .height(60.0)
                        .bg(tk.color_success_bg)
                        .radius(tk.border_radius_sm),
                )),
        )
        .push(
            Grid::new()
                .columns(3)
                .gap(8.0, 8.0)
                .size(INNER_W, 70.0)
                .children((
                    row((label("A").fg(tk.color_primary).font_size(13.0),))
                        .width(80.0)
                        .height(60.0)
                        .bg(tk.color_primary_bg)
                        .radius(tk.border_radius_sm),
                    row((label("B").fg(tk.color_warning).font_size(13.0),))
                        .width(80.0)
                        .height(60.0)
                        .bg(tk.color_warning_bg)
                        .radius(tk.border_radius_sm),
                    row((label("C").fg(tk.color_success).font_size(13.0),))
                        .width(80.0)
                        .height(60.0)
                        .bg(tk.color_success_bg)
                        .radius(tk.border_radius_sm),
                )),
        )
        .push(
            Grid::new()
                .column_widths(&[100.0, -1.0])
                .gap(8.0, 8.0)
                .size(INNER_W, 70.0)
                .children((
                    row((label("1fr").fg(tk.color_primary).font_size(13.0),))
                        .width(100.0)
                        .height(60.0)
                        .bg(tk.color_primary_bg)
                        .radius(tk.border_radius_sm),
                    row((label("2fr").fg(tk.color_info).font_size(13.0),))
                        .width(200.0)
                        .height(60.0)
                        .bg(tk.color_info_bg)
                        .radius(tk.border_radius_sm),
                )),
        )
        .section("Grid — 响应式")
        .push(
            Grid::responsive()
                .breakpoints(Breakpoints::antd())
                .cols(vec![
                    Col::new().span(24).sm(12).md(8).lg(6),
                    Col::new().span(24).sm(12).md(8).lg(6),
                    Col::new().span(24).sm(12).md(8).lg(6),
                    Col::new().span(24).sm(12).md(8).lg(6),
                ])
                .children((
                    row((label("Col 1").fg(tk.color_primary).font_size(12.0),))
                        .bg(tk.color_primary_bg)
                        .radius(tk.border_radius_sm),
                    row((label("Col 2").fg(tk.color_success).font_size(12.0),))
                        .bg(tk.color_success_bg)
                        .radius(tk.border_radius_sm),
                    row((label("Col 3").fg(tk.color_warning).font_size(12.0),))
                        .bg(tk.color_warning_bg)
                        .radius(tk.border_radius_sm),
                    row((label("Col 4").fg(tk.color_info).font_size(12.0),))
                        .bg(tk.color_info_bg)
                        .radius(tk.border_radius_sm),
                )),
        )
        .section("Layout — Header / Sider / Content / Footer")
        .push(
            Layout::new()
                .header(
                    Header::new()
                        .height(36.0)
                        .child(label("Header").fg(tk.color_primary).font_size(12.0)),
                )
                .sider(
                    Sider::new()
                        .child(
                            column((
                                label("Sider").fg(tk.color_text_secondary).font_size(11.0),
                                label("100px").fg(tk.color_text_tertiary).font_size(10.0),
                            ))
                            .align(AlignItems::Center)
                            .padding(12.0),
                        )
                        .width(100.0),
                )
                .content(
                    column((
                        label("Content").fg(tk.color_text).font_size(13.0),
                        label("主内容区域")
                            .fg(tk.color_text_tertiary)
                            .font_size(11.0),
                    ))
                    .width(-1.0)
                    .height(80.0)
                    .align(AlignItems::Center)
                    .bg(tk.color_bg_container),
                )
                .footer(
                    Footer::new()
                        .height(28.0)
                        .child(label("Footer").fg(tk.color_text_tertiary).font_size(11.0)),
                )
                .bg(tk.color_bg_layout),
        )
        .section("Splitter — 水平 / 垂直")
        .push(labeled_row(
            tk,
            120.0,
            "Horizontal",
            Splitter::horizontal()
                .first(
                    column((
                        label("面板 A").fg(tk.color_text).font_size(13.0),
                        label("240px 初宽")
                            .fg(tk.color_text_tertiary)
                            .font_size(11.0),
                    ))
                    .align(AlignItems::Center)
                    .bg(tk.color_primary_bg)
                    .radius(tk.border_radius_sm),
                    240.0,
                )
                .second(
                    column((
                        label("面板 B").fg(tk.color_text).font_size(13.0),
                        label("弹性填充").fg(tk.color_text_tertiary).font_size(11.0),
                    ))
                    .align(AlignItems::Center)
                    .bg(tk.color_success_bg)
                    .radius(tk.border_radius_sm),
                )
                .min_first(180.0),
        ))
        .push(labeled_row(
            tk,
            120.0,
            "Vertical",
            Splitter::horizontal()
                .first(
                    column((
                        label("上").fg(tk.color_text).font_size(13.0),
                        label("拖拽分界").fg(tk.color_text_tertiary).font_size(10.0),
                    ))
                    .align(AlignItems::Center)
                    .bg(tk.color_primary_bg)
                    .radius(tk.border_radius_sm),
                    80.0,
                )
                .second(
                    column((
                        label("下").fg(tk.color_text).font_size(13.0),
                        label("弹性填充").fg(tk.color_text_tertiary).font_size(10.0),
                    ))
                    .align(AlignItems::Center)
                    .bg(tk.color_success_bg)
                    .radius(tk.border_radius_sm),
                )
                .min_first(50.0),
        ))
        .section("ScrollView")
        .push(
            ScrollView::new().size(-1.0, 120.0).child(
                column((
                    label("ScrollView 行 1 — 滚轮滚动").font_size(12.0),
                    label("ScrollView 行 2 — 内容可超出视口").font_size(12.0),
                    label("ScrollView 行 3").font_size(12.0),
                    label("ScrollView 行 4").font_size(12.0),
                    label("ScrollView 行 5").font_size(12.0),
                    label("ScrollView 行 6 — 末端").font_size(12.0),
                ))
                .gap(6.0),
            ),
        )
        .section("VirtualScroll — 虚拟列表")
        .push(
            VirtualScroll::new()
                .item_count(10000)
                .item_height(32.0)
                .size(INNER_W, 180.0)
                .render(|i| {
                    row((label(format!("row-{i}")).fg(tk.color_text).font_size(13.0),))
                        .width(INNER_W - 16.0)
                        .height(28.0)
                }),
        )
        .section("Affix / BackTop")
        .push(labeled_row(
            tk,
            48.0,
            "Affix",
            Affix::new().top(0.0).child(
                row((label("吸顶导航栏").fg(tk.color_text).font_size(12.0),))
                    .bg(tk.color_primary_bg)
                    .radius(tk.border_radius_sm)
                    .padding(EdgeInsets::new(4.0, 12.0, 4.0, 12.0)),
            ),
        ))
        .push(labeled_row(
            tk,
            40.0,
            "BackTop",
            BackTop::new().visibility_height(100.0),
        ))
        .build()
}
