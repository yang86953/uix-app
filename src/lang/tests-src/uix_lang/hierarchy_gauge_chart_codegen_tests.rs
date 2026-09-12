// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证矩形树图与仪表盘的类型化静态映射。
#[test]
fn generates_treemap_and_gauge_charts() {
    // 矩形树图覆盖递归节点、间隙与标签配置。
    let treemap = generate(
        "<Treemap data={[TreemapNode('技术', 45).children([TreemapNode('前端', 20), TreemapNode('后端', 25)])]} gap=\"4\" labelVisible tooltip={tooltip} title=\"部门占比\" />",
    )
    // 合法矩形树图必须生成。
    .expect("Treemap 应生成");
    // 必须精确收集 TreemapNode 并保留 children 构造链。
    assert!(
        treemap.contains("Vec < :: uix_app :: prelude :: TreemapNode >")
            && treemap.contains("children")
            && treemap.contains("gap (4.0)")
            && treemap.contains("label_visible (true)")
            && treemap.contains("tooltip ((tooltip) . clone ())")
    );
    // 仪表盘覆盖色带、类型、指针、格式化器与数值范围。
    let gauge = generate(
        r##"<Gauge value="68" min="0" max="100" gaugeType="ring" rangeColors={[GaugeRange(0, 30, Color('#ff4d4f')), GaugeRange(30, 100, Color('#52c41a'))]} pointerWidth="4" pointerColor={Color('#1677ff')} format={gauge_format} />"##,
    )
    // 合法仪表盘必须生成。
    .expect("Gauge 应生成");
    // 必须精确收集 GaugeRange 并映射公开枚举与颜色。
    assert!(
        gauge.contains("Gauge :: new")
            && gauge.contains("value (68.0)")
            && gauge.contains("Vec < :: uix_app :: prelude :: GaugeRange >")
            && gauge.contains("GaugeType :: Ring")
            && gauge.contains("pointer_color")
            && gauge.contains("format ((gauge_format) . clone ())")
    );
}

// 验证缺失值、错配数据与未登记配置得到编译诊断。
#[test]
fn rejects_invalid_hierarchy_and_gauge_contracts() {
    // 仪表盘没有当前值就没有明确语义。
    let missing = generate("<Gauge />").expect_err("Gauge 缺少 value 必须失败");
    // 诊断必须点名必需属性。
    assert!(missing.message.contains("value"));
    // 矩形树图不能借用漏斗数据构造器。
    let mismatched = generate("<Treemap data={[FunnelData('A', 10)]} />")
        // 图表数据错配必须失败。
        .expect_err("Treemap 数据错配必须失败");
    // 诊断必须同时点名实际与目标类型。
    assert!(
        mismatched.message.contains("FunnelData") && mismatched.message.contains("TreemapNode")
    );
    // 仪表盘指针颜色不接受无类型字符串。
    let color = generate("<Gauge value=\"10\" pointerColor=\"#fff\" />")
        // 字符串颜色必须失败。
        .expect_err("pointerColor 字符串必须失败");
    // 诊断必须说明 Color 表达式。
    assert!(color.message.contains("Color 表达式"));
    // 仪表盘格式化器不接受无类型字符串。
    let formatter = generate("<Gauge value=\"10\" format=\"percent\" />")
        // 字符串格式化器必须失败。
        .expect_err("format 字符串必须失败");
    // 诊断必须说明类型化格式化器契约。
    assert!(formatter.message.contains("类型化格式化器"));
    // 动画字符串不能伪装成类型化配置对象。
    let animation = generate("<Treemap data={items} animation=\"fade\" />")
        // 非表达式配置必须失败。
        .expect_err("animation 字符串必须失败");
    // 诊断必须说明类型化表达式契约。
    assert!(animation.message.contains("类型化配置表达式"));
}
