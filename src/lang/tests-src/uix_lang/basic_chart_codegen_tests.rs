// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证三类基础图表与类型化数据构造器映射。
#[test]
fn generates_bar_line_and_pie_charts() {
    // 柱状图覆盖类型化数据、布尔和数值属性。
    let bar = generate(r##"<BarChart data={[BarData('Q1', 12, Color('#1677ff'))]} grouped showValue barRadius="3" legend="bottom" animation={animation} interactive={interaction} brush={brush} tooltip={tooltip} width="320px" />"##)
        // 合法柱状图必须生成。
        .expect("BarChart 应生成");
    // 必须收集成公开 BarData 并调用现有构建器。
    assert!(
        bar.contains("BarChart :: new") && bar.contains("Vec < :: uix_app :: prelude :: BarData >")
    );
    // 专有和公共属性必须各自映射。
    assert!(
        bar.contains("grouped (true)")
            && bar.contains("bar_radius (3.0)")
            && bar.contains("LegendPosition :: Bottom")
            && bar.contains("animation ((animation) . clone ())")
            && bar.contains("interactive ((interaction) . clone ())")
            && bar.contains("brush ((brush) . clone ())")
            && bar.contains("tooltip ((tooltip) . clone ())")
            && bar.contains("width (320.0)")
    );
    // 折线图覆盖基础线形配置。
    let line = generate(
        "<LineChart data={[LineData('Jan', 10)]} smooth showDots={dots} lineWidth=\"2\" />",
    )
    // 合法折线图必须生成。
    .expect("LineChart 应生成");
    // 必须使用公开 LineData 与配置方法。
    assert!(
        line.contains("Vec < :: uix_app :: prelude :: LineData >")
            && line.contains("smooth (true)")
            && line.contains("show_dots (dots)")
    );
    // 饼图覆盖颜色数据、环形比例和标签开关。
    let pie = generate(r##"<PieChart data={[PieData('A', 40, Color('#52c41a'))]} donut="0.5" labelVisible={labels} />"##)
        // 合法饼图必须生成。
        .expect("PieChart 应生成");
    // 必须使用公开 PieData 与环形配置。
    assert!(
        pie.contains("Vec < :: uix_app :: prelude :: PieData >")
            && pie.contains("donut (0.5)")
            && pie.contains("label_visible (labels)")
    );
}

// 验证柱状图与折线图登记精确的 ChartSeries 多系列入口。
#[test]
fn generates_basic_chart_multi_series() {
    // 柱状图内联多系列使用 ChartSeries<Vec<BarData>>。
    let bar = generate(
        "<BarChart series={[ChartSeries('2024', [BarData('Q1', 10, Color('#1677ff'))]), ChartSeries('2025', [BarData('Q1', 12, Color('#52c41a'))])]} grouped />",
    )
    // 合法柱状图多系列必须生成。
    .expect("BarChart series 应生成");
    // 生成代码必须固定嵌套 BarData 泛型并调用 series builder。
    assert!(
        bar.contains("ChartSeries < :: std :: vec :: Vec < :: uix_app :: prelude :: BarData")
            && bar.contains("series")
            && bar.contains("grouped (true)")
    );
    // 折线图动态多系列表达式交给 Rust 核对精确泛型。
    let line = generate("<LineChart series={line_series} smooth legend=\"top\" />")
        // 合法折线图多系列必须生成。
        .expect("LineChart series 应生成");
    // 生成代码必须克隆声明快照并固定嵌套 LineData 泛型。
    assert!(
        line.contains("(line_series) . clone ()")
            && line.contains("ChartSeries < :: std :: vec :: Vec < :: uix_app :: prelude :: LineData")
            && line.contains("smooth (true)")
    );
}

// 验证数据、子树与高级配置诊断。
#[test]
fn rejects_invalid_basic_chart_contracts() {
    // 缺少 data 时没有图表数据源。
    let missing = generate("<BarChart />").expect_err("缺少 data 必须失败");
    // 诊断必须点名 data。
    assert!(missing.message.contains("data"));
    // 字符串不能伪装成 LineData 集合。
    let literal = generate("<LineChart data=\"items\" />").expect_err("字符串 data 必须失败");
    // 诊断必须说明类型化数据。
    assert!(literal.message.contains("类型化数据"));
    // 图表叶组件不能接收可见子树。
    let child = generate("<PieChart data={items}><Text>非法</Text></PieChart>")
        // 子树必须失败。
        .expect_err("图表子节点必须失败");
    // 诊断必须说明叶边界。
    assert!(child.message.contains("不接受子节点"));
    // tooltip 字符串不能伪装成类型化配置对象。
    let tooltip = generate("<BarChart data={items} tooltip=\"hover\" />")
        // 非表达式配置必须失败。
        .expect_err("tooltip 字符串必须失败");
    // 诊断必须说明类型化表达式契约。
    assert!(tooltip.message.contains("类型化配置表达式"));
    // 静态图例只接受完整公开枚举集合。
    let legend = generate("<BarChart data={items} legend=\"center\" />")
        // 未登记图例位置必须失败。
        .expect_err("非法 legend 必须失败");
    // 诊断必须包含非法值。
    assert!(legend.message.contains("center"));
    // 单集合与多系列不能同时决定柱状图载荷。
    let duplicate = generate("<BarChart data={items} series={series} />")
        // 两个入口必须互斥。
        .expect_err("BarChart data 与 series 同时使用必须失败");
    // 诊断必须点名两个互斥入口。
    assert!(duplicate.message.contains("data") && duplicate.message.contains("series"));
    // 多系列内联数组不能借用自定义组合系列构造器。
    let mismatched =
        generate("<LineChart series={[ComboSeries('访问量', [LineData('Jan', 10)])]} />")
            // 构造器错配必须在宏展开期失败。
            .expect_err("LineChart series 构造器错配必须失败");
    // 诊断必须点名实际与目标构造器。
    assert!(
        mismatched.message.contains("ComboSeries") && mismatched.message.contains("ChartSeries")
    );
    // 基础专用柱图没有高级 ChartPlaceholder 的参考线 builder。
    let bar_reference = generate("<BarChart data={items} referenceLines={references} />")
        // 无效果配置必须在宏展开期失败。
        .expect_err("基础 BarChart referenceLines 必须失败");
    // 诊断必须保留具体属性名。
    assert!(bar_reference.message.contains("referenceLines"));
}
