// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证面积、散点与漏斗图的类型化静态映射。
#[test]
fn generates_area_scatter_and_funnel_charts() {
    // 面积图覆盖 LineData、填充与曲线属性。
    let area = generate(
        "<AreaChart data={[LineData('Jan', 10)]} stacked smooth fillOpacity=\"0.3\" legend={legend} referenceLines={references} width=\"420px\" />",
    )
    // 合法面积图必须生成。
    .expect("AreaChart 应生成");
    // 面积图必须精确收集 LineData 并应用配置。
    assert!(
        area.contains("AreaChart :: new")
            && area.contains("Vec < :: uix_app :: prelude :: LineData >")
            && area.contains("stacked (true)")
            && area.contains("fill_opacity (0.3)")
            && area.contains("legend ((legend) . clone ())")
            && area.contains("reference_line")
    );
    // 散点图覆盖坐标数据、轴标题与点样式。
    let scatter = generate(
        "<ScatterChart data={[ScatterData('A', 2, 6)]} xAxis=\"宽度\" yAxis=\"高度\" pointSize=\"6\" pointStyle=\"diamond\" referenceLines={references} />",
    )
    // 合法散点图必须生成。
    .expect("ScatterChart 应生成");
    // 散点图必须精确收集 ScatterData 并映射公开枚举。
    assert!(
        scatter.contains("Vec < :: uix_app :: prelude :: ScatterData >")
            && scatter.contains("x_axis (\"宽度\")")
            && scatter.contains("PointStyle :: Diamond")
            && scatter.contains("reference_line")
    );
    // 气泡图覆盖显式 BubbleData 入口与大小缩放。
    let bubble = generate(
        "<ScatterChart bubbleData={[BubbleData('中国', 1.4, 12, 80)]} bubbleScale=\"0.8\" xAxis=\"人口\" yAxis=\"GDP\" />",
    )
    // 合法气泡图必须生成。
    .expect("ScatterChart bubbleData 应生成");
    // 气泡图必须精确收集 BubbleData、规范化数值并应用缩放。
    assert!(
        bubble.contains("BubbleData :: new (\"中国\" , 1.4 , 12.0 , 80.0)")
            && bubble.contains("Vec < :: uix_app :: prelude :: BubbleData >")
            && bubble.contains("bubble_scale (0.8)")
    );
    // 漏斗图覆盖阶段数据、枚举、布尔与间隙。
    let funnel = generate(
        "<FunnelChart data={[FunnelData('浏览', 500)]} showConversionRate labelPosition=\"right\" align=\"left\" shape=\"symmetric\" gap=\"4\" />",
    )
    // 合法漏斗图必须生成。
    .expect("FunnelChart 应生成");
    // 漏斗图必须精确收集 FunnelData 并映射全部配置。
    assert!(
        funnel.contains("Vec < :: uix_app :: prelude :: FunnelData >")
            && funnel.contains("show_conversion_rate (true)")
            && funnel.contains("LabelPosition :: Right")
            && funnel.contains("FunnelAlign :: Left")
            && funnel.contains("FunnelShape :: Symmetric")
    );
}

// 验证面积图与散点图登记精确的 ChartSeries 多系列入口。
#[test]
fn generates_static_chart_multi_series() {
    // 面积图内联多系列使用 ChartSeries<Vec<LineData>>。
    let area = generate(
        "<AreaChart series={[ChartSeries('收入', [LineData('Jan', 10)]), ChartSeries('成本', [LineData('Jan', 4)])]} stacked />",
    )
    // 合法面积图多系列必须生成。
    .expect("AreaChart series 应生成");
    // 生成代码必须固定嵌套 LineData 泛型并保留面积类型。
    assert!(
        area.contains("ChartSeries < :: std :: vec :: Vec < :: uix_app :: prelude :: LineData")
            && area.contains("series")
            && area.contains("stacked (true)")
    );
    // 散点图动态多系列表达式交给 Rust 核对精确泛型。
    let scatter = generate(
        "<ScatterChart series={scatter_series} xAxis=\"宽度\" yAxis=\"高度\" legend=\"bottom\" />",
    )
    // 合法散点图多系列必须生成。
    .expect("ScatterChart series 应生成");
    // 生成代码必须克隆声明快照并固定嵌套 ScatterData 泛型。
    assert!(
        scatter.contains("(scatter_series) . clone ()")
            && scatter
                .contains("ChartSeries < :: std :: vec :: Vec < :: uix_app :: prelude :: ScatterData")
            && scatter.contains("x_axis (\"宽度\")")
    );
}

// 验证未登记数据、属性、枚举与子树都得到编译诊断。
#[test]
fn rejects_unregistered_static_chart_contracts() {
    // 普通散点与气泡数据入口不能同时决定载荷。
    let duplicate = generate(
        "<ScatterChart data={[ScatterData('A', 1, 2)]} bubbleData={[BubbleData('B', 2, 3, 4)]} />",
    )
    // 两个入口必须互斥。
    .expect_err("data 与 bubbleData 同时使用必须失败");
    // 诊断必须点名两个入口。
    assert!(duplicate.message.contains("data") && duplicate.message.contains("bubbleData"));
    // bubbleData 不能借用普通散点构造器。
    let mismatched = generate("<ScatterChart bubbleData={[ScatterData('A', 1, 2)]} />")
        // 类型错配必须在宏展开期失败。
        .expect_err("bubbleData 类型错配必须失败");
    // 诊断必须同时点名实际与目标类型。
    assert!(
        mismatched.message.contains("ScatterData") && mismatched.message.contains("BubbleData")
    );
    // 普通散点不能携带无效气泡缩放。
    let scale = generate("<ScatterChart data={items} bubbleScale=\"0.8\" />")
        // 无效配置必须失败。
        .expect_err("普通散点 bubbleScale 必须失败");
    // 诊断必须指出 bubbleData 前提。
    assert!(scale.message.contains("bubbleData"));
    // 气泡图不能携带无效普通点大小。
    let point = generate("<ScatterChart bubbleData={items} pointSize=\"6\" />")
        // 无效配置必须失败。
        .expect_err("气泡图 pointSize 必须失败");
    // 诊断必须指向气泡大小契约。
    assert!(point.suggestion.contains("bubbleScale"));
    // 单集合与多系列不能同时决定面积图载荷。
    let area_series = generate("<AreaChart data={items} series={series} />")
        // 两个入口必须互斥。
        .expect_err("AreaChart data 与 series 同时使用必须失败");
    // 诊断必须点名两个互斥入口。
    assert!(area_series.message.contains("data") && area_series.message.contains("series"));
    // 气泡单集合与普通散点多系列不能同时决定载荷。
    let scatter_series = generate("<ScatterChart bubbleData={items} series={series} />")
        // 两个入口必须互斥。
        .expect_err("ScatterChart bubbleData 与 series 同时使用必须失败");
    // 诊断必须点名三类互斥入口。
    assert!(
        scatter_series.message.contains("bubbleData") && scatter_series.message.contains("series")
    );
    // 多系列内联数组不能借用自定义组合系列构造器。
    let mismatched_series =
        generate("<ScatterChart series={[ComboSeries('样本', [LineData('Jan', 10)])]} />")
            // 构造器错配必须在宏展开期失败。
            .expect_err("ScatterChart series 构造器错配必须失败");
    // 诊断必须点名实际与目标构造器。
    assert!(
        mismatched_series.message.contains("ComboSeries")
            && mismatched_series.message.contains("ChartSeries")
    );
    // 非法枚举必须在宏展开期拒绝。
    let style = generate("<ScatterChart data={items} pointStyle=\"square\" />")
        // 未登记点形状必须失败。
        .expect_err("非法 pointStyle 必须失败");
    // 诊断必须包含非法值。
    assert!(style.message.contains("square"));
    // 图表叶组件不能接收可见子树。
    let child = generate("<FunnelChart data={items}><Text>非法</Text></FunnelChart>")
        // 子树必须失败。
        .expect_err("图表子节点必须失败");
    // 诊断必须说明叶边界。
    assert!(child.message.contains("不接受子节点"));
}
