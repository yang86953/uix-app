// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证雷达图与组合图的精确泛型系列映射。
#[test]
fn generates_radar_and_combo_charts() {
    // 雷达图覆盖闭区间轴、泛型系列、形状与网格配置。
    let radar = generate(
        "<RadarChart axes={[RadarAxis('速度', 0, 100), RadarAxis('力量', 0, 100)]} series={[ChartSeries('当前', [RadarData(80), RadarData(70)])]} shape=\"circle\" gridLevels=\"5\" fillOpacity=\"0.2\" interactive={interaction} brush={brush} />",
    )
    // 合法雷达图必须生成。
    .expect("RadarChart 应生成");
    // 必须精确收集轴与 ChartSeries<Vec<RadarData>>。
    assert!(
        radar.contains("RadarAxis :: new (\"速度\" , (0.0) ..= (100.0))")
            && radar.contains("Vec < :: uix_app :: prelude :: RadarAxis >")
            && radar
                .contains("ChartSeries < :: std :: vec :: Vec < :: uix_app :: prelude :: RadarData")
            && radar.contains("RadarShape :: Circle")
            && radar.contains("grid_levels (5)")
            && radar.contains("interactive ((interaction) . clone ())")
            && radar.contains("brush ((brush) . clone ())")
    );
    // 组合图覆盖柱/线两类精确泛型系列与双轴标题。
    let combo = generate(
        r##"<ComboChart barSeries={[ChartSeries('销售额', [BarData('Jan', 100, Color('#1677ff'))])]} lineSeries={[ChartSeries('利润率', [LineData('Jan', 20)])]} yAxisLeft="销售额" yAxisRight="利润率" referenceLines={references} />"##,
    )
    // 合法组合图必须生成。
    .expect("ComboChart 应生成");
    // 两类系列必须分别精确收集，不能依赖运行时猜测。
    assert!(
        combo.contains("Vec < :: uix_app :: prelude :: ChartSeries < :: std :: vec :: Vec < :: uix_app :: prelude :: BarData")
            && combo.contains("Vec < :: uix_app :: prelude :: ChartSeries < :: std :: vec :: Vec < :: uix_app :: prelude :: LineData")
            && combo.contains("y_axis_left (\"销售额\")")
            && combo.contains("y_axis_right (\"利润率\")")
            && combo.contains("reference_line")
    );
    // 自定义组合系列覆盖精确泛型集合与既有 series builder。
    let custom =
        generate("<ComboChart series={[ComboSeries('访问量', [LineData('Jan', 100)])]} />")
            // 合法自定义组合系列必须生成。
            .expect("ComboChart 自定义系列应生成");
    // 自定义入口必须固定 ComboSeries<Vec<LineData>>，不能运行时猜测。
    assert!(
        custom.contains(
            "Vec < :: uix_app :: prelude :: ComboSeries < :: std :: vec :: Vec < :: uix_app :: prelude :: LineData",
        ) && custom.contains("series")
    );
}

// 验证必需集合、形状和高级系列边界得到编译诊断。
#[test]
fn rejects_invalid_series_chart_contracts() {
    // 雷达图缺少轴定义不能生成。
    let axes = generate("<RadarChart series={series} />")
        // 缺少 axes 必须失败。
        .expect_err("RadarChart 缺少 axes 必须失败");
    // 诊断必须点名缺失属性。
    assert!(axes.message.contains("axes"));
    // 组合图至少需要一类系列。
    let combo = generate("<ComboChart />").expect_err("ComboChart 缺少系列必须失败");
    // 诊断必须说明三个可选入口。
    assert!(
        combo.message.contains("series")
            && combo.message.contains("barSeries")
            && combo.message.contains("lineSeries")
    );
    // 雷达图形状只允许两个公开值。
    let shape = generate("<RadarChart axes={axes} series={series} shape=\"square\" />")
        // 非法形状必须失败。
        .expect_err("RadarChart 非法 shape 必须失败");
    // 诊断必须包含非法值。
    assert!(shape.message.contains("square"));
    // 自定义 ComboSeries 与专用系列入口不能同时拥有数据。
    let custom = generate("<ComboChart barSeries={bars} series={series} />")
        // 互斥入口必须失败。
        .expect_err("ComboChart 互斥系列入口必须失败");
    // 诊断必须同时点名冲突入口。
    assert!(custom.message.contains("series") && custom.message.contains("barSeries"));
    // 自定义入口的内联数组不能借用通用 ChartSeries 构造器。
    let mismatched =
        generate("<ComboChart series={[ChartSeries('访问量', [LineData('Jan', 100)])]} />")
            // 错配构造器必须失败。
            .expect_err("ComboChart 自定义系列构造器错配必须失败");
    // 诊断必须同时点名实际与目标构造器。
    assert!(
        mismatched.message.contains("ChartSeries") && mismatched.message.contains("ComboSeries")
    );
}
