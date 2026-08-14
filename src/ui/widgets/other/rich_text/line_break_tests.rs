//! 覆盖 RichText 估算布局的 UAX #14 断行契约。

// 引入被测布局入口与富文本段类型。
use super::{RichTextSegment, RichTextStyle, layout_rich_text};
// 引入稳定测试颜色。
use crate::draw::Color;

// 构造单个普通文本段并返回每个视觉行的字符内容。
fn layout_lines(text: &str, max_width: f32) -> Vec<String> {
    // 使用默认样式隔离断行行为。
    let segments = vec![RichTextSegment::Text {
        // 保存调用方测试文本。
        content: text.to_owned(),
        // 使用默认富文本样式。
        style: RichTextStyle::default(),
        // 结束文本段构造。
    }];
    // 使用十像素字号执行纯内存估算布局。
    let (lines, _, _) = layout_rich_text(
        // 传入单段测试内容。
        &segments,
        // 传入测试宽度约束。
        max_width,
        // 使用稳定字号。
        10.0,
        // 颜色不参与断行。
        Color::black(),
    );
    // 将行字形还原为便于断言的字符串。
    lines
        // 消费全部视觉行。
        .into_iter()
        // 拼接每行逻辑字符。
        .map(|line| line.glyphs.into_iter().map(|glyph| glyph.ch).collect())
        // 收集最终行字符串。
        .collect()
}

// 验证 CJK 可以逐字折行但行首不能出现闭标点。
#[test]
fn cjk_wrap_keeps_close_punctuation_with_previous_character() {
    // 十像素宽度不足以同时容纳汉字与全角逗号。
    let lines = layout_lines("天，地", 10.0);
    // UAX 禁则要求逗号随前一汉字留在溢出首行。
    assert_eq!(lines, vec!["天，".to_owned(), "地".to_owned()]);
}

// 验证 UAX 边界可以跨 Text 与 Link 样式段复用。
#[test]
fn cjk_break_opportunity_crosses_style_segments() {
    // 将两个相邻 CJK 字符故意拆到不同样式段。
    let segments = vec![
        // 首字符使用普通文本样式。
        RichTextSegment::Text {
            // 保存首个汉字。
            content: "天".to_owned(),
            // 使用默认文本样式。
            style: RichTextStyle::default(),
            // 结束普通文本段构造。
        },
        // 次字符使用链接样式以验证跨段边界。
        RichTextSegment::Link {
            // 保存第二个汉字。
            content: "地".to_owned(),
            // 使用稳定测试 URL。
            url: "https://example.test".to_owned(),
            // 结束链接段构造。
        },
        // 结束测试段列表。
    ];
    // 十像素宽度每行只能容纳一个 CJK 字符。
    let (lines, _, _) = layout_rich_text(&segments, 10.0, 10.0, Color::black());
    // 完整源 UAX 边界必须在样式段交界处触发折行。
    assert_eq!(lines.len(), 2);
    // 第一行只包含普通文本段字符。
    assert_eq!(lines[0].glyphs[0].ch, '天');
    // 第二行只包含链接段字符。
    assert_eq!(lines[1].glyphs[0].ch, '地');
}

// 验证不可断序列宁可溢出也不会被宽度兜底拆开。
#[test]
fn nbsp_combining_and_emoji_sequences_do_not_split() {
    // NBSP 两侧正文必须保持在同一视觉行。
    assert_eq!(layout_lines("a\u{00a0}b", 6.0).len(), 1);
    // 基字与组合音标必须保持在同一视觉行。
    assert_eq!(layout_lines("a\u{0301}", 6.0).len(), 1);
    // ZWJ 家庭 emoji 内部不能出现紧急断行。
    assert_eq!(layout_lines("👩‍👩‍👧‍👦", 6.0).len(), 1);
}

// 验证超长字母数字词仍保留逐 cluster 紧急折行能力。
#[test]
fn long_word_uses_controlled_emergency_breaks() {
    // 六像素宽度每行只能容纳一个估算字母。
    let lines = layout_lines("abcd", 6.0);
    // 四个字母应形成四个稳定视觉行。
    assert_eq!(lines, vec!["a", "b", "c", "d"]);
    // 将同一长单词拆到不同样式段以验证紧急边界跨段复用。
    let segments = vec![
        // 前半词使用普通文本段。
        RichTextSegment::Text {
            // 保存前两个字母。
            content: "ab".to_owned(),
            // 使用默认文本样式。
            style: RichTextStyle::default(),
            // 结束普通文本段构造。
        },
        // 后半词使用代码段改变样式。
        RichTextSegment::Code {
            // 保存后两个字母。
            content: "cd".to_owned(),
            // 结束代码段构造。
        },
        // 结束测试段列表。
    ];
    // 十一像素恰好容纳两个默认估算字母。
    let (lines, _, _) = layout_rich_text(&segments, 11.0, 10.0, Color::black());
    // 跨段长单词应在样式边界的紧急机会折为两行。
    assert_eq!(lines.len(), 2);
}

// 验证 CRLF 与单独 CR 都形成一个强制换行而不进入字形流。
#[test]
fn crlf_and_cr_are_single_mandatory_breaks() {
    // CRLF 只产生两行正文。
    assert_eq!(layout_lines("a\r\nb", 100.0), vec!["a", "b"]);
    // 单独 CR 同样产生两行正文。
    assert_eq!(layout_lines("a\rb", 100.0), vec!["a", "b"]);
    // 构造直接携带 CRLF 的文本段以验证全文字符索引。
    let segments = vec![RichTextSegment::Text {
        // 保留完整 CRLF 源序列。
        content: "a\r\nb".to_owned(),
        // 使用默认文本样式。
        style: RichTextStyle::default(),
        // 结束文本段构造。
    }];
    // 执行宽约束充足的估算布局。
    let (lines, _, _) = layout_rich_text(&segments, 100.0, 10.0, Color::black());
    // 第二行字符必须从完整 CRLF 后的源索引三开始。
    assert_eq!(lines[1].glyphs[0].global_char_idx, 3);
}
