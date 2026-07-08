//! 组件库页面 — page_charts（Bar / Line / Pie 全覆盖）。

use uix::prelude::*;

use crate::common::page::PageBuilder;
use crate::demos::context::DemoCtx;

pub fn page_charts(ctx: &DemoCtx<'_>) -> ViewNode {
    let tk = ctx.tk;
    let inner_w = crate::common::page::INNER_W;

    PageBuilder::new(tk)
        .gap()
        .section("BarChart — 月活跃用户")
        .push(
            BarChart::new()
                .width(inner_w)
                .height(180.0)
                .show_value(true)
                .data(vec![
                    BarData::new("Jan", 420.0, tk.color_primary),
                    BarData::new("Feb", 380.0, tk.color_primary),
                    BarData::new("Mar", 530.0, tk.color_success),
                    BarData::new("Apr", 490.0, tk.color_primary),
                    BarData::new("May", 620.0, tk.color_success),
                    BarData::new("Jun", 580.0, tk.color_warning),
                ]),
        )
        .section("LineChart — CPU 温度")
        .push(
            LineChart::new()
                .width(inner_w)
                .height(160.0)
                .line_color(tk.color_error)
                .show_dots(true)
                .show_grid(true)
                .line_width(2.0)
                .data(vec![
                    LineData::new("00:00", 42.0),
                    LineData::new("02:00", 41.0),
                    LineData::new("04:00", 46.0),
                    LineData::new("06:00", 52.0),
                    LineData::new("08:00", 63.0),
                    LineData::new("10:00", 67.0),
                ]),
        )
        .section("PieChart — 实心 / 环形")
        .push(tree! { Container::new().size(inner_w, 220.0).dir(FlexDirection::Row).gap(16.0) => [
            PieChart::new().size(180.0).data(vec![
                PieData::new("Chrome", 65.0, tk.color_primary),
                PieData::new("Firefox", 15.0, tk.color_success),
                PieData::new("Safari", 10.0, tk.color_warning),
                PieData::new("Other", 10.0, tk.color_fill_tertiary),
            ]).into_node(),
            PieChart::new().size(180.0).donut(0.45).data(vec![
                PieData::new("A", 40.0, tk.color_primary),
                PieData::new("B", 30.0, tk.color_success),
                PieData::new("C", 20.0, tk.color_warning),
                PieData::new("D", 10.0, tk.color_error),
            ]).into_node(),
        ]})
        .build()
}
