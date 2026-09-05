// 声明本文件只编译高级图表文档，不启动窗口或绘制后端。
#![allow(dead_code)]

// 隔离 chart-radar 围栏中的多维雷达图。
mod chart_radar {
    // 引入文档承诺的高级图表公开 prelude。
    use uix::prelude::*;

    // 编译多维多系列与圆形雷达配置。
    fn compile_example() {
        // 构造五个维度的多系列雷达图。
        let _radar = RadarChart::new()
            // 声明各维度量程。
            .axes(vec![
                // 声明速度维度。
                RadarAxis::new("速度", 0.0..=100.0),
                // 声明力量维度。
                RadarAxis::new("力量", 0.0..=100.0),
                // 声明耐力维度。
                RadarAxis::new("耐力", 0.0..=100.0),
                // 声明技巧维度。
                RadarAxis::new("技巧", 0.0..=100.0),
                // 声明协作维度。
                RadarAxis::new("协作", 0.0..=100.0),
            ])
            // 提交两个选手系列。
            .series(vec![
                // 声明选手 A 系列。
                ChartSeries::new(
                    // 提供系列名称。
                    "选手A",
                    // 提供五维数据。
                    vec![
                        // 速度值。
                        RadarData::new(85.0),
                        // 力量值。
                        RadarData::new(70.0),
                        // 耐力值。
                        RadarData::new(90.0),
                        // 技巧值。
                        RadarData::new(60.0),
                        // 协作值。
                        RadarData::new(75.0),
                    ],
                ),
                // 声明选手 B 系列。
                ChartSeries::new(
                    // 提供系列名称。
                    "选手B",
                    // 提供五维数据。
                    vec![
                        // 速度值。
                        RadarData::new(65.0),
                        // 力量值。
                        RadarData::new(85.0),
                        // 耐力值。
                        RadarData::new(60.0),
                        // 技巧值。
                        RadarData::new(80.0),
                        // 协作值。
                        RadarData::new(70.0),
                    ],
                ),
            ])
            // 声明数据面填充透明度。
            .fill_opacity(0.2)
            // 声明图表尺寸。
            .size(350.0);

        // 创建圆形雷达维度。
        let metrics = vec![RadarAxis::new("速度", 0.0..=100.0)];
        // 创建当前值系列。
        let current = ChartSeries::new("当前", vec![RadarData::new(80.0)]);
        // 创建基线系列。
        let baseline = ChartSeries::new("基线", vec![RadarData::new(60.0)]);
        // 构造圆形网格雷达图。
        let _circle = RadarChart::new()
            // 使用圆形网格。
            .shape(RadarShape::Circle)
            // 提交维度。
            .axes(metrics)
            // 提交当前与基线系列。
            .series(vec![current, baseline]);
    }
}

// 隔离 chart-heatmap 围栏中的矩阵与日历热力图。
mod chart_heatmap {
    // 引入文档承诺的热力图公开 prelude。
    use uix::prelude::*;

    // 编译矩阵标签、颜色范围与日历配置。
    fn compile_example() {
        // 构造带标签与数值的矩阵热力图。
        let _matrix = Heatmap::new()
            // 声明水平轴标签。
            .x_labels(vec!["周一", "周二", "周三", "周四", "周五"])
            // 声明垂直轴标签。
            .y_labels(vec!["0点", "6点", "12点", "18点"])
            // 提交矩阵单元格。
            .data(vec![
                // 添加第一格。
                HeatmapCell::new(0, 0, 12.0),
                // 添加第二格。
                HeatmapCell::new(1, 0, 45.0),
            ])
            // 声明从浅到深的颜色范围。
            .color_range(Color::hex("#f7fbff"), Color::hex("#08306b"))
            // 声明图表宽度。
            .width(500.0)
            // 声明图表高度。
            .height(300.0)
            // 声明单元格间距。
            .cell_gap(2.0)
            // 显示单元格数值。
            .show_values(true);

        // 创建日历活动数据。
        let daily_activity = vec![HeatmapCell::new(0, 0, 12.0)];
        // 构造二〇二六年日历热力图。
        let _calendar = Heatmap::new()
            // 启用日历布局。
            .calendar_mode(true)
            // 提交活动数据。
            .data(daily_activity)
            // 声明年份。
            .year(2026)
            // 声明单元格尺寸。
            .cell_size(14.0);
    }
}

// 隔离 chart-funnel 围栏中的转化漏斗。
mod chart_funnel {
    // 引入文档承诺的漏斗图公开 prelude。
    use uix::prelude::*;

    // 编译转化阶段与对称漏斗配置。
    fn compile_example() {
        // 构造带转化率标签的漏斗图。
        let _funnel = FunnelChart::new()
            // 提交五个转化阶段。
            .data(vec![
                // 添加浏览阶段。
                FunnelData::new("浏览", 5000.0),
                // 添加点击阶段。
                FunnelData::new("点击", 2300.0),
                // 添加加购阶段。
                FunnelData::new("加购", 890.0),
                // 添加下单阶段。
                FunnelData::new("下单", 420.0),
                // 添加支付阶段。
                FunnelData::new("支付", 310.0),
            ])
            // 声明图表宽度。
            .width(400.0)
            // 声明图表高度。
            .height(400.0)
            // 把标签放在右侧。
            .label_position(LabelPosition::Right)
            // 显示相邻阶段转化率。
            .show_conversion_rate(true);

        // 创建对称漏斗数据。
        let funnel_data = vec![FunnelData::new("浏览", 10.0)];
        // 构造左右对称漏斗。
        let _symmetric = FunnelChart::new()
            // 使用对称形状。
            .shape(FunnelShape::Symmetric)
            // 提交转化数据。
            .data(funnel_data);
    }
}

// 隔离 chart-waterfall 围栏中的累计变化图。
mod chart_waterfall {
    // 引入文档承诺的瀑布图公开 prelude。
    use uix::prelude::*;

    // 编译纵向累计分解与横向汇总配置。
    fn compile_example() {
        // 构造利润累计变化瀑布图。
        let _waterfall = WaterfallChart::new()
            // 提交各累计阶段。
            .data(vec![
                // 添加起始汇总。
                WaterfallData::new("起始", 1000.0, WaterfallKind::Total),
                // 添加收入增长。
                WaterfallData::new("收入", 500.0, WaterfallKind::Increase),
                // 添加成本下降。
                WaterfallData::new("成本", -300.0, WaterfallKind::Decrease),
                // 添加税费下降。
                WaterfallData::new("税费", -100.0, WaterfallKind::Decrease),
                // 添加净利汇总。
                WaterfallData::new("净利", 1100.0, WaterfallKind::Total),
            ])
            // 声明图表宽度。
            .width(500.0)
            // 声明图表高度。
            .height(300.0)
            // 声明水平轴标题。
            .x_axis("项目")
            // 声明垂直轴标题。
            .y_axis("金额");

        // 创建横向瀑布数据。
        let waterfall_data = vec![WaterfallData::new("净利", 100.0, WaterfallKind::Total)];
        // 构造横向瀑布图。
        let _horizontal = WaterfallChart::new()
            // 启用横向布局。
            .horizontal(true)
            // 提交汇总数据。
            .data(waterfall_data);
    }
}

// 隔离 chart-combo 围栏中的双轴组合图。
mod chart_combo {
    // 引入文档承诺的组合图公开 prelude。
    use uix::prelude::*;

    // 编译柱线组合与逐系列类型配置。
    fn compile_example() {
        // 创建销售额柱数据。
        let sales_data = vec![BarData::new("Jan", 100.0, Color::BLUE)];
        // 创建利润率折线数据。
        let profit_rate_data = vec![LineData::new("Jan", 20.0)];
        // 构造带左右轴标题的柱线组合图。
        let _combo = ComboChart::new()
            // 提交柱状系列。
            .bar_series(vec![ChartSeries::new("销售额", sales_data)])
            // 提交折线系列。
            .line_series(vec![ChartSeries::new("利润率", profit_rate_data)])
            // 声明左轴标题。
            .y_axis_left("销售额 (万元)")
            // 声明右轴标题。
            .y_axis_right("利润率 (%)")
            // 声明图表宽度。
            .width(500.0)
            // 声明图表高度。
            .height(300.0)
            // 把图例放在顶部。
            .legend(LegendPosition::Top);

        // 创建访问量系列数据。
        let visit_data = vec![LineData::new("Jan", 100.0)];
        // 创建转化率系列数据。
        let rate_data = vec![LineData::new("Jan", 10.0)];
        // 创建目标线系列数据。
        let target_data = vec![LineData::new("Jan", 15.0)];
        // 构造逐系列配置类型和轴侧的组合图。
        let _custom = ComboChart::new().series(vec![
            // 把访问量声明为左轴柱系列。
            ComboSeries::new("访问量", visit_data)
                // 声明柱状类型。
                .chart_type(ChartType::Bar)
                // 使用左轴。
                .y_axis(AxisSide::Left),
            // 把转化率声明为右轴折线系列。
            ComboSeries::new("转化率", rate_data)
                // 声明折线类型。
                .chart_type(ChartType::Line)
                // 使用右轴。
                .y_axis(AxisSide::Right),
            // 把目标线声明为虚线折线系列。
            ComboSeries::new("目标线", target_data)
                // 声明折线类型。
                .chart_type(ChartType::Line)
                // 使用虚线样式。
                .line_style(LineStyle::Dashed),
        ]);
    }
}

// 隔离 chart-treemap 围栏中的层级占比图。
mod chart_treemap {
    // 引入文档承诺的矩形树图公开 prelude。
    use uix::prelude::*;

    // 编译嵌套层级、尺寸、间距与标签配置。
    fn compile_example() {
        // 构造部门层级占比矩形树图。
        let _treemap = Treemap::new()
            // 提交顶级部门及子部门。
            .data(vec![
                // 声明技术部门及其子部门。
                TreemapNode::new("技术", 45.0).children(vec![
                    // 添加前端子部门。
                    TreemapNode::new("前端", 20.0),
                    // 添加后端子部门。
                    TreemapNode::new("后端", 18.0),
                    // 添加运维子部门。
                    TreemapNode::new("运维", 7.0),
                ]),
                // 声明市场部门及其子部门。
                TreemapNode::new("市场", 30.0).children(vec![
                    // 添加线上子部门。
                    TreemapNode::new("线上", 20.0),
                    // 添加线下子部门。
                    TreemapNode::new("线下", 10.0),
                ]),
                // 添加行政部门。
                TreemapNode::new("行政", 15.0),
            ])
            // 声明图表宽度。
            .width(500.0)
            // 声明图表高度。
            .height(350.0)
            // 声明矩形间距。
            .gap(4.0)
            // 显示节点标签。
            .label_visible(true);
    }
}

// 隔离 chart-gauge 围栏中的仪表盘。
mod chart_gauge {
    // 引入文档承诺的仪表盘公开 prelude。
    use uix::prelude::*;

    // 编译范围颜色、格式化与仪表盘类型配置。
    fn compile_example() {
        // 构造带三段颜色的 CPU 仪表盘。
        let _gauge = Gauge::new()
            // 声明当前值。
            .value(68.0)
            // 声明最小值。
            .min(0.0)
            // 声明最大值。
            .max(100.0)
            // 声明三段区间颜色。
            .range_colors(vec![
                // 添加红色低区间。
                GaugeRange::new(0.0, 30.0, Color::hex("#ff4d4f")),
                // 添加黄色中区间。
                GaugeRange::new(30.0, 70.0, Color::hex("#faad14")),
                // 添加绿色高区间。
                GaugeRange::new(70.0, 100.0, Color::hex("#52c41a")),
            ])
            // 声明仪表盘尺寸。
            .size(200.0)
            // 声明中心标题。
            .title("CPU 使用率")
            // 声明数值格式化策略。
            .format(|value| format!("{value:.1}%"));

        // 构造默认半圆仪表盘。
        let _dashboard = Gauge::new()
            // 选择半圆类型。
            .gauge_type(GaugeType::Dashboard)
            // 声明当前值。
            .value(75.0)
            // 声明尺寸。
            .size(180.0);

        // 构造全圆仪表盘。
        let _full = Gauge::new()
            // 选择全圆类型。
            .gauge_type(GaugeType::Full)
            // 声明当前值。
            .value(82.0)
            // 声明尺寸。
            .size(220.0);
    }
}

// 隔离 chart-common 围栏中的响应式与交互配置。
mod chart_common {
    // 引入文档承诺的图表交互公开 prelude。
    use uix::prelude::*;

    // 编译响应式尺寸、点击、缩放、平移、十字线与刷选配置。
    fn compile_example() {
        // 创建营收柱数据。
        let revenue = vec![BarData::new("Jan", 100.0, Color::BLUE)];
        // 构造跟随容器约束的柱状图。
        let _responsive = BarChart::new()
            // 提交营收数据。
            .data(revenue)
            // 启用响应式尺寸。
            .responsive(true);

        // 创建时间序列数据。
        let timeseries = vec![LineData::new("Jan", 100.0)];
        // 构造带交互与刷选的折线图。
        let _interactive = LineChart::new()
            // 提交时间序列数据。
            .data(timeseries)
            // 声明图表交互配置。
            .interactive(InteractionConfig {
                // 启用滚轮缩放。
                zoom: true,
                // 启用拖拽平移。
                pan: true,
                // 启用十字准线。
                crosshair: true,
                // 注册只消费数据标签的点击函数指针。
                on_click: Some(|_item| {}),
            })
            // 声明刷选配置。
            .brush(BrushConfig {
                // 启用刷选。
                enabled: true,
                // 注册只消费选区范围的函数指针。
                on_select: Some(|_range| {}),
            });
    }
}

// 隔离 chart-state-driven 围栏中的响应式图表组合。
mod chart_state_driven {
    // 引入文档承诺的图表、状态与视图公开 prelude。
    use uix::prelude::*;

    // 编译 State 数据快照到响应式图表节点的映射。
    fn compile_example() {
        // 创建由业务拥有的系列状态。
        let series = State::new(vec![BarData::new("Q1", 120.0, Color::BLUE)]);

        // 把当前数据快照映射为响应式图表节点。
        let _chart = series.map(|data| {
            // 嵌入只消费拥有型数据克隆的柱状图。
            embed(
                // 构造柱状图组件。
                BarChart::new()
                    // 克隆当前快照交给组件拥有。
                    .data(data.clone())
                    // 跟随父容器尺寸。
                    .responsive(true),
            )
        });
    }
}

// 隔离 chart-empty-state 围栏中的数据空态决策。
mod chart_empty_state {
    // 引入文档承诺的图表、状态与视图公开 prelude。
    use uix::prelude::*;

    // 把非空业务数据构造成图表节点。
    fn chart_view(data: &[BarData]) -> ViewNode {
        // 克隆数据快照并嵌入响应式柱状图。
        embed(BarChart::new().data(data.to_vec()).responsive(true))
    }

    // 编译由数据是否为空决定的可选图表节点。
    fn compile_example() {
        // 创建由业务拥有的可空系列状态。
        let series = State::new(vec![BarData::new("Q1", 120.0, Color::BLUE)]);
        // 只在数据非空时构造图表节点。
        let _optional = series.map_opt(|data| (!data.is_empty()).then(|| chart_view(data)));
    }
}
