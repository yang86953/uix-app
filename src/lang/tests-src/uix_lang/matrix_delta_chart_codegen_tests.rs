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
        "<Heatmap data={[HeatmapCell(0, 1, 12)]} xLabels={['周一']} yLabels={['上午', '下午']} calendarMode year=\"2026\" cellSize=\"14\" cellGap=\"2\" showValues colorRange={color_range} colorStops={color_stops} animation={animation} />",
    )
    // 合法热力图必须生成。
    .expect("Heatmap 应生成");
    // 索引保持整数、value 规范化为 f32 并精确收集单元类型。
    assert!(
        heatmap.contains("HeatmapCell :: new (0 , 1 , 12.0)")
            && heatmap.contains("Vec < :: uix_app :: prelude :: HeatmapCell >")
            && heatmap.contains("calendar_mode (true)")
            && heatmap.contains("year (2026)")
            && heatmap.contains("show_values (true)")
            && heatmap.contains("color_range")
            && heatmap.contains("Vec < (f32 , :: uix_app :: prelude :: Color) >")
            && heatmap.contains("animation ((animation) . clone ())")
    );
    // 瀑布图覆盖三类变化项、方向与坐标轴标题。
    let waterfall = generate(
        "<WaterfallChart data={[WaterfallData('起始', 1000, 'total'), WaterfallData('收入', 500, 'increase'), WaterfallData('成本', -300, 'decrease')]} horizontal xAxis=\"项目\" yAxis=\"金额\" referenceLines={references} />",
    )
    // 合法瀑布图必须生成。
    .expect("WaterfallChart 应生成");
    // 三类字符串语义必须映射为公开枚举。
    assert!(
        waterfall.contains("Vec < :: uix_app :: prelude :: WaterfallData >")
            && waterfall.contains("WaterfallKind :: Total")
            && waterfall.contains("WaterfallKind :: Increase")
            && waterfall.contains("WaterfallKind :: Decrease")
            && waterfall.contains("horizontal (true)")
            && waterfall.contains("reference_line")
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
    // 颜色范围不接受字符串冒充类型化颜色元组。
    let color_range = generate("<Heatmap data={items} colorRange=\"#ffffff,#000000\" />")
        // 字符串颜色范围必须失败。
        .expect_err("colorRange 字符串不得映射");
    // 诊断必须保留具体属性名和表达式要求。
    assert!(
        color_range.message.contains("colorRange")
            && color_range.message.contains("类型化颜色表达式")
    );
    // 修复建议必须使用受限表达式可接受的预构造变量。
    assert!(color_range.suggestion.contains("color_range"));
    // 多段色阶同样只接受类型化集合表达式。
    let color_stops = generate("<Heatmap data={items} colorStops=\"red,blue\" />")
        // 字符串色阶必须失败。
        .expect_err("colorStops 字符串不得映射");
    // 诊断必须保留具体属性名。
    assert!(color_stops.message.contains("colorStops"));
}
