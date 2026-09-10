// 声明本文件只编译基础图表文档，不启动窗口或绘制后端。
#![allow(dead_code)]

// 隔离 dynamic-chart 围栏中的响应式数据入口。
mod dynamic_chart {
    // 引入文档承诺的图表与状态公开 prelude。
    use uix_app::prelude::*;

    // 编译业务 State 到图表数据快照的公开数据流。
    fn compile_example() {
        // 创建由业务拥有的柱状图数据状态。
        let data = State::new(vec![
            // 添加第一季度数据。
            BarData::new("Q1", 120.0, Color::hex("#1677ff")),
            // 添加第二季度数据。
            BarData::new("Q2", 200.0, Color::hex("#52c41a")),
        ]);

        // 构造消费当前数据快照的柱状图组件。
        let _chart = BarChart::new()
            // 从业务 State 读取拥有型数据快照。
            .data(data.get())
            // 声明图表宽度。
            .width(400.0)
            // 声明图表高度。
            .height(300.0);

        // 在业务侧原子追加第三季度数据。
        data.update(|values| {
            // 向唯一数据真值追加新条目。
            values.push(BarData::new("Q3", 150.0, Color::hex("#faad14")));
        });
    }
}

// 隔离 bar-chart 围栏中的基础柱状图。
mod bar_chart {
    // 引入文档承诺的柱状图公开 prelude。
    use uix_app::prelude::*;

    // 编译固定数据与尺寸配置。
    fn compile_example() {
        // 构造三季度柱状图。
        let _chart = BarChart::new()
            // 提交拥有型数据列表。
            .data(vec![
                // 添加第一季度数据。
                BarData::new("Q1", 120.0, Color::hex("#1677ff")),
                // 添加第二季度数据。
                BarData::new("Q2", 200.0, Color::hex("#52c41a")),
                // 添加第三季度数据。
                BarData::new("Q3", 150.0, Color::hex("#faad14")),
            ])
            // 声明图表宽度。
            .width(400.0)
            // 声明图表高度。
            .height(300.0);
    }
}

// 隔离 chart-bar-advanced 围栏中的柱状图高级布局。
mod chart_bar_advanced {
    // 引入文档承诺的柱状图与系列公开 prelude。
    use uix_app::prelude::*;

    // 编译分组、堆叠与横向柱状图配置。
    fn compile_example() {
        // 构造多系列并排的分组柱状图。
        let _grouped = BarChart::new()
            // 启用分组布局。
            .grouped(true)
            // 提交两个年份系列。
            .series(vec![
                // 声明二〇二四系列。
                ChartSeries::new(
                    // 提供系列名称。
                    "2024",
                    // 提供系列数据。
                    vec![
                        // 添加第一季度数据。
                        BarData::new("Q1", 100.0, Color::BLUE),
                        // 添加第二季度数据。
                        BarData::new("Q2", 130.0, Color::BLUE),
                    ],
                ),
                // 声明二〇二五系列。
                ChartSeries::new(
                    // 提供系列名称。
                    "2025",
                    // 提供系列数据。
                    vec![
                        // 添加第一季度数据。
                        BarData::new("Q1", 120.0, Color::GREEN),
                        // 添加第二季度数据。
                        BarData::new("Q2", 160.0, Color::GREEN),
                    ],
                ),
            ])
            // 声明图表宽度。
            .width(500.0)
            // 声明图表高度。
            .height(300.0);

        // 构造收入与成本上下堆叠的柱状图。
        let _stacked = BarChart::new()
            // 启用堆叠布局。
            .stacked(true)
            // 提交两个业务系列。
            .series(vec![
                // 声明收入系列。
                ChartSeries::new(
                    // 提供收入系列名称。
                    "收入",
                    // 提供收入数据。
                    vec![BarData::new("Q1", 100.0, Color::BLUE)],
                ),
                // 声明成本系列。
                ChartSeries::new(
                    // 提供成本系列名称。
                    "成本",
                    // 提供成本数据。
                    vec![BarData::new("Q1", 40.0, Color::RED)],
                ),
            ]);

        // 构造适合长标签的横向柱状图。
        let _horizontal = BarChart::new()
            // 启用横向布局。
            .horizontal(true)
            // 提交长标签数据。
            .data(vec![BarData::new("长标签", 80.0, Color::BLUE)])
            // 声明横向条形总空间。
            .height(400.0);
    }
}

// 隔离 line-chart 围栏中的基础折线图。
mod line_chart {
    // 引入文档承诺的折线图公开 prelude。
    use uix_app::prelude::*;

    // 编译固定数据、尺寸与数据点半径配置。
    fn compile_example() {
        // 构造三个月份的折线图。
        let _chart = LineChart::new()
            // 提交拥有型折线数据。
            .data(vec![
                // 添加一月数据。
                LineData::new("Jan", 100.0),
                // 添加二月数据。
                LineData::new("Feb", 150.0),
                // 添加三月数据。
                LineData::new("Mar", 130.0),
            ])
            // 声明图表宽度。
            .width(400.0)
            // 声明图表高度。
            .height(300.0)
            // 声明数据点半径。
            .dot_radius(4.0);
    }
}

// 隔离 chart-line-advanced 围栏中的折线与面积高级配置。
mod chart_line_advanced {
    // 引入文档承诺的折线、面积与系列公开 prelude。
    use uix_app::prelude::*;

    // 编译多系列、面积、堆叠与平滑配置。
    fn compile_example() {
        // 创建 CPU 折线数据。
        let cpu_data = vec![LineData::new("Jan", 40.0), LineData::new("Feb", 60.0)];
        // 创建内存折线数据。
        let mem_data = vec![LineData::new("Jan", 30.0), LineData::new("Feb", 50.0)];
        // 构造带顶部图例的多系列折线图。
        let _multiple = LineChart::new()
            // 提交 CPU 与内存系列。
            .series(vec![
                // 克隆 CPU 数据供系列拥有。
                ChartSeries::new("CPU", cpu_data.clone()),
                // 移交内存数据所有权。
                ChartSeries::new("内存", mem_data),
            ])
            // 声明图表宽度。
            .width(500.0)
            // 声明图表高度。
            .height(300.0)
            // 把图例放在顶部。
            .legend(LegendPosition::Top);

        // 构造带半透明填充的面积图。
        let _area = AreaChart::new()
            // 提交单点面积数据。
            .data(vec![LineData::new("Jan", 100.0)])
            // 声明填充透明度。
            .fill_opacity(0.3)
            // 声明图表宽度。
            .width(400.0)
            // 声明图表高度。
            .height(300.0);

        // 构造单系列堆叠面积图。
        let _stacked = AreaChart::new()
            // 提交面积系列。
            .series(vec![ChartSeries::new(
                // 提供系列名称。
                "A",
                // 提供系列数据。
                vec![LineData::new("Jan", 1.0)],
            )])
            // 启用堆叠模式。
            .stacked(true);

        // 构造隐藏数据点的平滑折线图。
        let _smooth = LineChart::new()
            // 提交两个采样点。
            .data(vec![LineData::new("S1", 1.0), LineData::new("S2", 2.0)])
            // 启用平滑曲线。
            .smooth(true)
            // 隐藏数据点标记。
            .dot_radius(0.0);
    }
}

// 隔离 pie-chart 围栏中的基础饼图。
mod pie_chart {
    // 引入文档承诺的饼图公开 prelude。
    use uix_app::prelude::*;

    // 编译产品占比环形图配置。
    fn compile_example() {
        // 构造三个产品占比的环形图。
        let _chart = PieChart::new()
            // 提交拥有型扇区数据。
            .data(vec![
                // 添加产品 A 扇区。
                PieData::new("产品A", 40.0, Color::hex("#1677ff")),
                // 添加产品 B 扇区。
                PieData::new("产品B", 30.0, Color::hex("#52c41a")),
                // 添加产品 C 扇区。
                PieData::new("产品C", 30.0, Color::hex("#faad14")),
            ])
            // 声明图表直径。
            .size(300.0)
            // 声明内环比例。
            .donut(0.55);
    }
}

// 隔离 chart-pie-advanced 围栏中的饼图高级配置。
mod chart_pie_advanced {
    // 引入文档承诺的饼图公开 prelude。
    use uix_app::prelude::*;

    // 编译玫瑰图与半圆环配置。
    fn compile_example() {
        // 构造按半径表达数值的玫瑰图。
        let _rose = PieChart::new()
            // 提交单扇区数据。
            .data(vec![PieData::new("A", 40.0, Color::BLUE)])
            // 启用玫瑰模式。
            .rose(true)
            // 使用半径映射样式。
            .rose_style(RoseStyle::Radius);

        // 构造仪表盘风格半圆环。
        let _semicircle = PieChart::new()
            // 提交完成度数据。
            .data(vec![PieData::new("完成", 68.0, Color::GREEN)])
            // 声明图表直径。
            .size(200.0)
            // 声明内环比例。
            .donut(0.7)
            // 从左半弧开始。
            .start_angle(180.0)
            // 声明用于数值归一化的总量。
            .total(360.0);
    }
}

// 隔离 chart-scatter 围栏中的散点与气泡图。
mod chart_scatter {
    // 引入文档承诺的散点、气泡与系列公开 prelude。
    use uix_app::prelude::*;

    // 编译单系列、多系列与气泡尺寸配置。
    fn compile_example() {
        // 构造双变量散点图。
        let _scatter = ScatterChart::new()
            // 提交两个散点。
            .data(vec![
                // 添加 A 散点。
                ScatterData::new("A", 2.5, 6.3),
                // 添加 B 散点。
                ScatterData::new("B", 3.1, 5.8),
            ])
            // 声明水平轴标题。
            .x_axis("宽度 (cm)")
            // 声明垂直轴标题。
            .y_axis("高度 (cm)")
            // 声明图表宽度。
            .width(400.0)
            // 声明图表高度。
            .height(300.0);

        // 创建雄性样本数据。
        let male_data = vec![ScatterData::new("A", 1.0, 2.0)];
        // 创建雌性样本数据。
        let female_data = vec![ScatterData::new("B", 2.0, 3.0)];
        // 构造带底部图例的多系列散点图。
        let _series = ScatterChart::new()
            // 提交两个样本系列。
            .series(vec![
                // 提交雄性系列。
                ChartSeries::new("雄性", male_data),
                // 提交雌性系列。
                ChartSeries::new("雌性", female_data),
            ])
            // 把图例放在底部。
            .legend(LegendPosition::Bottom);

        // 构造第三维由气泡大小表达的图表。
        let _bubble = ScatterChart::new()
            // 提交气泡数据。
            .data(vec![
                // 添加中国数据。
                BubbleData::new("中国", 1.4, 12.0, 80.0),
                // 添加美国数据。
                BubbleData::new("美国", 0.33, 21.0, 50.0),
            ])
            // 声明气泡缩放比例。
            .bubble_scale(0.8)
            // 声明水平轴标题。
            .x_axis("人口 (十亿)")
            // 声明垂直轴标题。
            .y_axis("GDP (万亿美元)");
    }
}
