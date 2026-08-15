// 引入解析与核心生成入口。
use super::{Diagnostic, generate_view, parse_document};

// 生成测试源码的稳定令牌快照。
fn generate(source: &str) -> Result<String, Diagnostic> {
    // 先解析完整 UIX 文档。
    let document = parse_document(source)?;
    // 再生成公开 Rust View 表达式。
    generate_view(&document.root).map(|tokens| tokens.to_string())
}

// 验证热力图与瀑布图的类型化静态映射。
#[test]
fn generates_heatmap_and_waterfall_charts() {
    // 热力图覆盖混合数值数据、标签、日历与单元配置。
    let heatmap = generate(
        "<Heatmap data={[HeatmapCell(0, 1, 12)]} xLabels={['周一']} yLabels={['上午', '下午']} calendarMode year=\"2026\" cellSize=\"14\" cellGap=\"2\" showValues animation={animation} />",
    )
    // 合法热力图必须生成。
    .expect("Heatmap 应生成");
    // 索引保持整数、value 规范化为 f32 并精确收集单元类型。
    assert!(
        heatmap.contains("HeatmapCell :: new (0 , 1 , 12.0)")
            && heatmap.contains("Vec < :: uix :: prelude :: HeatmapCell >")
            && heatmap.contains("calendar_mode (true)")
            && heatmap.contains("year (2026)")
            && heatmap.contains("show_values (true)")
            && heatmap.contains("animation ((animation) . clone ())")
    );
    // 瀑布图覆盖三类变化项、方向与坐标轴标题。
    let waterfall = generate(
        "<WaterfallChart data={[WaterfallData('起始', 1000, 'total'), WaterfallData('收入', 500, 'increase'), WaterfallData('成本', -300, 'decrease')]} horizontal xAxis=\"项目\" yAxis=\"金额\" />",
    )
    // 合法瀑布图必须生成。
    .expect("WaterfallChart 应生成");
    // 三类字符串语义必须映射为公开枚举。
    assert!(
        waterfall.contains("Vec < :: uix :: prelude :: WaterfallData >")
            && waterfall.contains("WaterfallKind :: Total")
            && waterfall.contains("WaterfallKind :: Increase")
            && waterfall.contains("WaterfallKind :: Decrease")
            && waterfall.contains("horizontal (true)")
    );
}

// 验证混合参数、枚举和未登记属性得到编译诊断。
#[test]
fn rejects_invalid_matrix_and_delta_contracts() {
    // HeatmapCell 参数数量必须精确。
    let cell = generate("<Heatmap data={[HeatmapCell(0, 12)]} />")
        // 缺少参数必须失败。
        .expect_err("HeatmapCell 参数数量错误必须失败");
    // 诊断必须点名构造器。
    assert!(cell.message.contains("HeatmapCell"));
    // WaterfallData 类型只接受三个语义值。
    let kind = generate("<WaterfallChart data={[WaterfallData('A', 10, 'unknown')]} />")
        // 未知 kind 必须失败。
        .expect_err("WaterfallData kind 错误必须失败");
    // 诊断必须包含非法值。
    assert!(kind.message.contains("unknown"));
    // 热力图标签不接受单个字符串。
    let labels = generate("<Heatmap data={items} xLabels=\"周一\" />")
        // 非集合标签必须失败。
        .expect_err("Heatmap 字符串标签必须失败");
    // 诊断必须说明集合要求。
    assert!(labels.message.contains("字符串集合"));
    // 颜色范围仍未登记，不能静默忽略。
    let color = generate("<Heatmap data={items} colorRange={colors} />")
        // 未登记颜色范围必须失败。
        .expect_err("colorRange 不得静默映射");
    // 诊断必须保留具体属性名。
    assert!(color.message.contains("colorRange"));
}
