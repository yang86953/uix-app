//! 组件库页面 — page_charts。

use uix::prelude::*;

use crate::common::page::{PageBuilder, INNER_W};

pub fn page_charts(tk: &DesignTokens) -> ViewNode {
    PageBuilder::new(tk)
        .gap()
        .section("柱状图 — 月活跃用户")
        .push(
            BarChart::new()
                .width(INNER_W)
                .height(180.0)
                .show_value(true)
                .data(vec![
                    BarData::new("Jan", 420.0, tk.color_primary),
                    BarData::new("Feb", 380.0, tk.color_primary),
                    BarData::new("Mar", 530.0, tk.color_success),
                    BarData::new("Apr", 490.0, tk.color_primary),
                    BarData::new("May", 620.0, tk.color_success),
                    BarData::new("Jun", 580.0, tk.color_warning),
                    BarData::new("Jul", 710.0, tk.color_success),
                    BarData::new("Aug", 680.0, tk.color_primary),
                ]),
        )
        .section("折线图 — CPU 温度")
        .push(
            LineChart::new()
                .width(INNER_W)
                .height(160.0)
                .line_color(tk.color_error)
                .show_dots(true)
                .show_grid(true)
                .line_width(2.0)
                .data(vec![
                    LineData::new("00:00", 42.0),
                    LineData::new("01:00", 44.0),
                    LineData::new("02:00", 41.0),
                    LineData::new("03:00", 48.0),
                    LineData::new("04:00", 46.0),
                    LineData::new("05:00", 43.0),
                    LineData::new("06:00", 52.0),
                    LineData::new("07:00", 58.0),
                    LineData::new("08:00", 63.0),
                    LineData::new("09:00", 67.0),
                ]),
        )
        .section("饼图 — 浏览器市场份额")
        .push(
            tree! { Container::new().size(INNER_W, 220.0).dir(FlexDirection::Row) => [
                PieChart::new().size(180.0).data(vec![
                    PieData::new("Chrome", 65.0, tk.color_primary),
                    PieData::new("Firefox", 15.0, tk.color_success),
                    PieData::new("Safari", 10.0, tk.color_warning),
                    PieData::new("Edge", 8.0, tk.color_error),
                    PieData::new("Other", 2.0, tk.color_fill_tertiary),
                ]).into_node(),
                tree! { Container::new().size(20.0, 0.0) },
                PieChart::new().size(180.0).donut(0.45).data(vec![
                    PieData::new("Chrome", 65.0, tk.color_primary),
                    PieData::new("Firefox", 15.0, tk.color_success),
                    PieData::new("Safari", 10.0, tk.color_warning),
                    PieData::new("Edge", 8.0, tk.color_error),
                    PieData::new("Other", 2.0, tk.color_fill_tertiary),
                ]).into_node(),
            ]},
        )
        .build()

}
