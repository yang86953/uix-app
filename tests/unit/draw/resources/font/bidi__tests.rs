// 引入被测共享分析类型。
use super::BidiAnalysis;

// 验证每个段落独立解析首个强字符形成基准级别。
#[test]
fn paragraph_base_levels_follow_first_strong_character() {
    // 同时构造 LTR 与 RTL 两个段落。
    let analysis = BidiAnalysis::new("abc\nאבג");
    // 两个显式段落必须分别登记。
    assert_eq!(analysis.paragraphs().len(), 2);
    // Latin 段落基准级别必须为偶数零级。
    assert_eq!(analysis.paragraphs()[0].base_level, 0);
    // Hebrew 段落基准级别必须为奇数一级。
    assert_eq!(analysis.paragraphs()[1].base_level, 1);
}

// 验证 LTR 段落中的 RTL 文本和数字按解析级别形成视觉顺序。
#[test]
fn mixed_text_preserves_number_order_and_reverses_rtl_letters() {
    // 构造包含 Latin、Hebrew、空格与欧洲数字的混排行。
    let text = "abc אבג 123";
    // 执行完整段落分析。
    let analysis = BidiAnalysis::new(text);
    // 每个字符作为一个逻辑布局对象输入。
    let logical_indices = (0..text.chars().count()).collect::<Vec<_>>();
    // 取得完整单行的视觉映射。
    let order = analysis.line_order(0..logical_indices.len(), &logical_indices);
    // 按视觉顺序重建仅用于断言的字符序列。
    let logical_chars = text.chars().collect::<Vec<_>>();
    // 应用视觉到逻辑映射。
    let visual_text = order
        // 遍历视觉对象位置。
        .visual_to_logical
        // 消费映射以便直接索引逻辑字符。
        .iter()
        // 读取对应逻辑字符。
        .map(|logical_index| logical_chars[*logical_index])
        // 收集为可读视觉字符串。
        .collect::<String>();
    // 数字内部保持 LTR，Hebrew 字母按视觉顺序反向。
    assert_eq!(visual_text, "abc 123 גבא");
    // 数字段落内解析为偶数级，而 Hebrew 文本保持奇数级。
    assert!(order.levels[8..11].iter().all(|level| level % 2 == 0));
}

// 验证视觉行尾空白通过 L1 重置到段落基准级别。
#[test]
fn trailing_neutral_uses_paragraph_level_for_line_reordering() {
    // RTL 段落末尾保留一个中性空格。
    let text = "אבג ";
    // 执行段落分析。
    let analysis = BidiAnalysis::new(text);
    // 每个字符作为独立逻辑对象。
    let logical_indices = (0..text.chars().count()).collect::<Vec<_>>();
    // 对完整视觉行执行 L1 与 L2。
    let order = analysis.line_order(0..logical_indices.len(), &logical_indices);
    // 行尾空格必须重置为 RTL 段落的一级。
    assert_eq!(order.levels[3], 1);
    // RTL 行中的逻辑尾随空格位于最左侧视觉位置。
    assert_eq!(order.visual_to_logical[0], 3);
}
