//! 组件库页面 — page_charts（Bar / Line / Pie 全覆盖 + Advanced Charts 场景矩阵）。

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
        .push(
            tree! { Container::new().size(inner_w, 220.0).dir(FlexDirection::Row).gap(16.0) => [
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
            ]},
        )
        .section("AreaChart — 磁盘占用趋势")
        .push(
            AreaChart::new()
                .width(inner_w)
                .height(170.0)
                .title("磁盘占用趋势")
                .data(vec![
                    LineData::new("周一", 320.0),
                    LineData::new("周二", 410.0),
                    LineData::new("周三", 380.0),
                    LineData::new("周四", 520.0),
                    LineData::new("周五", 610.0),
                ]),
        )
        .section("ScatterChart — 散点 / 气泡")
        .push(
            tree! { Container::new().size(inner_w, 210.0).dir(FlexDirection::Row).gap(16.0) => [
                ScatterChart::new().width(inner_w * 0.5 - 8.0).height(210.0)
                    .title("散点分布")
                    .data(vec![
                        ScatterData::new("A", 1.0, 3.0),
                        ScatterData::new("B", 2.0, 5.0),
                        ScatterData::new("C", 3.0, 4.0),
                        ScatterData::new("D", 4.0, 8.0),
                        ScatterData::new("E", 5.0, 7.0),
                    ]).into_node(),
                ScatterChart::new().width(inner_w * 0.5 - 8.0).height(210.0)
                    .title("气泡分布")
                    .data(vec![
                        BubbleData::new("A", 1.0, 3.0, 12.0),
                        BubbleData::new("B", 2.0, 5.0, 26.0),
                        BubbleData::new("C", 3.0, 4.0, 18.0),
                        BubbleData::new("D", 4.0, 8.0, 34.0),
                    ]).into_node(),
            ]},
        )
        .section("RadarChart — 能力雷达")
        .push(
            RadarChart::new()
                .width(inner_w)
                .height(220.0)
                .title("能力雷达")
                .axes(vec![
                    RadarAxis::new("性能", 0.0..=10.0),
                    RadarAxis::new("易用", 0.0..=10.0),
                    RadarAxis::new("稳定", 0.0..=10.0),
                    RadarAxis::new("安全", 0.0..=10.0),
                    RadarAxis::new("扩展", 0.0..=10.0),
                ])
                .series(vec![
                    ChartSeries::new(
                        "当前",
                        vec![
                            RadarData::new(8.0),
                            RadarData::new(7.0),
                            RadarData::new(9.0),
                            RadarData::new(6.0),
                            RadarData::new(8.0),
                        ],
                    ),
                    ChartSeries::new(
                        "目标",
                        vec![
                            RadarData::new(9.0),
                            RadarData::new(8.0),
                            RadarData::new(9.0),
                            RadarData::new(8.0),
                            RadarData::new(9.0),
                        ],
                    ),
                ]),
        )
        .section("Heatmap — 热力矩阵")
        .push(
            Heatmap::new()
                .width(inner_w)
                .height(170.0)
                .title("热力矩阵")
                .data(vec![
                    HeatmapCell::new(0, 0, 2.0),
                    HeatmapCell::new(0, 1, 5.0),
                    HeatmapCell::new(0, 2, 8.0),
                    HeatmapCell::new(1, 0, 4.0),
                    HeatmapCell::new(1, 1, 6.0),
                    HeatmapCell::new(1, 2, 3.0),
                    HeatmapCell::new(2, 0, 7.0),
                    HeatmapCell::new(2, 1, 2.0),
                    HeatmapCell::new(2, 2, 9.0),
                    HeatmapCell::new(3, 0, 5.0),
                    HeatmapCell::new(3, 1, 8.0),
                    HeatmapCell::new(3, 2, 1.0),
                ]),
        )
        .section("FunnelChart — 转化漏斗")
        .push(
            FunnelChart::new()
                .width(inner_w)
                .height(200.0)
                .title("转化漏斗")
                .data(vec![
                    FunnelData::new("访问", 1000.0),
                    FunnelData::new("注册", 600.0),
                    FunnelData::new("下单", 320.0),
                    FunnelData::new("支付", 180.0),
                ]),
        )
        .section("WaterfallChart — 月度盈亏")
        .push(
            WaterfallChart::new()
                .width(inner_w)
                .height(200.0)
                .title("月度盈亏")
                .data(vec![
                    WaterfallData::new("期初", 100.0, WaterfallKind::Total),
                    WaterfallData::new("收入", 60.0, WaterfallKind::Increase),
                    WaterfallData::new("成本", -30.0, WaterfallKind::Decrease),
                    WaterfallData::new("费用", -20.0, WaterfallKind::Decrease),
                    WaterfallData::new("期末", 110.0, WaterfallKind::Total),
                ]),
        )
        .section("ComboChart — 双轴组合")
        .push(
            ComboChart::new()
                .width(inner_w)
                .height(200.0)
                .title("双轴组合")
                .series(vec![
                    ComboSeries::new("销售额", vec![
                        LineData::new("Q1", 120.0),
                        LineData::new("Q2", 180.0),
                        LineData::new("Q3", 150.0),
                        LineData::new("Q4", 240.0),
                    ]).chart_type(ChartType::Bar),
                    ComboSeries::new("增长率", vec![
                        LineData::new("Q1", 8.0),
                        LineData::new("Q2", 15.0),
                        LineData::new("Q3", 12.0),
                        LineData::new("Q4", 22.0),
                    ]).chart_type(ChartType::Line),
                ]),
        )
        .section("Treemap — 磁盘占用")
        .push(
            Treemap::new()
                .width(inner_w)
                .height(190.0)
                .title("磁盘占用")
                .data(vec![
                    TreemapNode::new("文档", 40.0).children(vec![
                        TreemapNode::new("设计稿", 18.0),
                        TreemapNode::new("合同", 12.0),
                        TreemapNode::new("备份", 10.0),
                    ]),
                    TreemapNode::new("媒体", 35.0).children(vec![
                        TreemapNode::new("视频", 22.0),
                        TreemapNode::new("图片", 13.0),
                    ]),
                    TreemapNode::new("代码", 25.0),
                ]),
        )
        .section("Gauge — 仪表盘")
        .push(
            tree! { Container::new().size(inner_w, 210.0).dir(FlexDirection::Row).gap(16.0) => [
                Gauge::new().size(190.0).value(72.0).min(0.0).max(100.0)
                    .title("完成度")
                    .range_colors(vec![
                        GaugeRange::new(0.0, 60.0, tk.color_warning),
                        GaugeRange::new(60.0, 90.0, tk.color_success),
                        GaugeRange::new(90.0, 100.0, tk.color_error),
                    ]).into_node(),
                Gauge::new().size(190.0).value(45.0).min(0.0).max(100.0)
                    .title("预算使用")
                    .range_colors(vec![
                        GaugeRange::new(0.0, 50.0, tk.color_primary),
                        GaugeRange::new(50.0, 100.0, tk.color_warning),
                    ]).into_node(),
            ]},
        )
        .build()
}
