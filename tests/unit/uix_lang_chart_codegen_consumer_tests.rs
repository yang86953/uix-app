// 引入生成代码承诺调用的公开 prelude。
use crate::prelude::*;

// 验证图表共同高级配置在真实公开宏消费者中通过类型检查。
#[test]
fn chart_advanced_config_compiles_against_public_uix_api() {
    // 提供图表公开动画配置快照。
    let chart_animation = AnimationConfig::fade_in(0.2);
    // 提供图表公开点击、缩放、平移与十字线配置快照。
    let chart_interaction = InteractionConfig {
        // 启用滚轮缩放。
        zoom: true,
        // 启用拖拽平移。
        pan: true,
        // 启用十字参考线。
        crosshair: true,
        // 本编译证明不触发业务点击回调。
        on_click: None,
    };
    // 提供图表公开刷选配置快照。
    let chart_brush = BrushConfig {
        // 启用刷选。
        enabled: true,
        // 本编译证明不触发业务刷选回调。
        on_select: None,
    };
    // 提供图表公开 tooltip 模板配置快照。
    let chart_tooltip = TooltipConfig::new().template("{label}: {value}");
    // 提供动态公开图例位置。
    let chart_legend = LegendPosition::Right;
    // 展开五项配置并要求结果类型为公开 ViewNode。
    let _chart: ViewNode = crate::uix!(
        r##"<BarChart data={[BarData('Q1', 12, Color('#1677ff'))]} legend={chart_legend} animation={chart_animation} interactive={chart_interaction} brush={chart_brush} tooltip={chart_tooltip} />"##
    );
    // 展开气泡数据入口并要求精确 BubbleData 集合通过公开 API 类型检查。
    let _bubble: ViewNode = crate::uix!(
        r#"<ScatterChart bubbleData={[BubbleData('A', 1, 2, 3)]} bubbleScale="0.8" />"#
    );
    // 提供热力图两端颜色的精确元组。
    let heatmap_range = (Color::hex("#f7fbff"), Color::hex("#08306b"));
    // 提供热力图带归一化位置的精确色阶集合。
    let heatmap_stops = vec![
        // 声明色阶起点。
        (0.0, Color::hex("#f7fbff")),
        // 声明色阶中点。
        (0.5, Color::hex("#6baed6")),
        // 声明色阶终点。
        (1.0, Color::hex("#08306b")),
    ];
    // 展开两类热力图颜色配置并要求公开 API 完成类型检查。
    let _heatmap: ViewNode = crate::uix!(
        r#"<Heatmap data={[HeatmapCell(0, 0, 12)]} colorRange={heatmap_range} colorStops={heatmap_stops} />"#
    );
    // 提供坐标图可复用的精确参考线集合。
    let chart_references = vec![
        // 声明目标实线。
        (80.0, "目标".to_owned(), LineStyle::Solid),
        // 声明警戒虚线。
        (60.0, "警戒".to_owned(), LineStyle::Dashed),
    ];
    // 展开参考线集合并要求公开 builder 完成类型检查。
    let _references: ViewNode = crate::uix!(
        r#"<AreaChart data={[LineData('Q1', 72)]} referenceLines={chart_references} />"#
    );
    // 提供可克隆的无捕获仪表盘格式化器。
    let gauge_format = |value: f32| format!("{value:.1}%");
    // 展开格式化器并要求公开 Gauge builder 完成类型检查。
    let _gauge: ViewNode = crate::uix!(r#"<Gauge value="68" format={gauge_format} />"#);
    // 提供使用既有公开枚举配置的自定义组合系列。
    let combo_series = vec![
        // 柱系列复用 LineData 载荷并绑定左轴。
        ComboSeries::new("访问量", vec![LineData::new("Jan", 100.0)])
            // 指定柱状绘制类型。
            .chart_type(ChartType::Bar)
            // 指定左侧坐标轴。
            .y_axis(AxisSide::Left),
        // 面积系列绑定右轴并使用虚线。
        ComboSeries::new("转化率", vec![LineData::new("Jan", 10.0)])
            // 指定面积绘制类型。
            .chart_type(ChartType::Area)
            // 指定右侧坐标轴。
            .y_axis(AxisSide::Right)
            // 指定虚线样式。
            .line_style(LineStyle::Dashed),
    ];
    // 展开自定义组合系列并要求公开 ComboChart builder 完成类型检查。
    let _combo: ViewNode = crate::uix!(r#"<ComboChart series={combo_series} />"#);
}

// 验证基础坐标图多系列生成代码通过真实公开 API 类型检查。
#[test]
fn basic_chart_multi_series_compiles_against_public_uix_api() {
    // 提供柱状图精确多系列声明快照。
    let bar_series = vec![
        // 声明 2025 年柱状数据系列。
        ChartSeries::new(
            // 声明系列名称。
            "2025",
            // 声明系列数据项。
            vec![BarData::new("Q1", 120.0, Color::BLUE)],
        ),
    ];
    // 展开柱状图多系列并要求公开 builder 完成类型检查。
    let _bar: ViewNode = crate::uix!(r#"<BarChart series={bar_series} grouped />"#);
    // 提供折线与面积图共享的精确多系列声明快照。
    let line_series = vec![
        // 声明收入折线系列。
        ChartSeries::new(
            // 声明系列名称。
            "收入",
            // 声明系列数据项。
            vec![LineData::new("Jan", 100.0)],
        ),
    ];
    // 展开折线图多系列并要求公开 builder 完成类型检查。
    let _line: ViewNode = crate::uix!(r#"<LineChart series={line_series} smooth />"#);
    // 为面积图创建独立快照，证明生成代码按值克隆且不转移前一声明。
    let area_series = vec![
        // 声明成本面积系列。
        ChartSeries::new(
            // 声明系列名称。
            "成本",
            // 声明系列数据项。
            vec![LineData::new("Jan", 40.0)],
        ),
    ];
    // 展开面积图多系列并要求 ChartPlaceholder 公开 builder 完成类型检查。
    let _area: ViewNode = crate::uix!(r#"<AreaChart series={area_series} stacked />"#);
    // 提供散点图精确多系列声明快照。
    let scatter_series = vec![
        // 声明实验组散点系列。
        ChartSeries::new(
            // 声明系列名称。
            "实验组",
            // 声明系列数据项。
            vec![ScatterData::new("A", 1.0, 2.0)],
        ),
    ];
    // 展开散点图多系列并要求 ChartPlaceholder 公开 builder 完成类型检查。
    let _scatter: ViewNode =
        crate::uix!(r#"<ScatterChart series={scatter_series} xAxis="宽度" yAxis="高度" />"#);
}
