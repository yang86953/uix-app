// 引入当前布局模块中的测试辅助函数和几何类型。
use super::*;

// 验证顶部锚点保持不动且下缘向右倾斜。
#[test]
fn italic_transform_keeps_top_edge_and_slants_lower_edge() {
    // 构造一个以 y=10 为顶部轴的斜体变换。
    let transform = italic_transform(10.0);
    // 计算顶部点经过变换后的坐标。
    let top = transform.transform_point(crate::core::Point::new(5.0, 10.0));
    // 计算下方点经过变换后的坐标。
    let lower = transform.transform_point(crate::core::Point::new(5.0, 20.0));
    // 顶部点的水平坐标应保持不变。
    assert!((top.x - 5.0).abs() < f32::EPSILON);
    // 顶部点的垂直坐标应保持不变。
    assert!((top.y - 10.0).abs() < f32::EPSILON);
    // 下方点应按 0.18 的倾斜比例向右移动。
    assert!((lower.x - 6.8).abs() < 0.0001);
    // 下方点的垂直坐标应保持不变。
    assert!((lower.y - 20.0).abs() < f32::EPSILON);
}

// 验证 Unicode 空白作为独立 token 时，窄宽度换行不会拆散后续单词。
#[test]
fn unicode_whitespace_wraps_as_a_separate_token() {
    // 构造前后两个普通文本段，模拟内联元素边界后的空白正文。
    let segments = vec![
        RichTextSegment::Text {
            content: "a".into(),
            style: Default::default(),
        },
        RichTextSegment::Text {
            content: "\u{2003}bb".into(),
            style: Default::default(),
        },
    ];
    // 使用窄宽度使空白可以留在第一行而后续单词换到第二行。
    let (lines, _, _) = layout_rich_text(&segments, 12.0, 10.0, Color::black());
    // 断行结果应保持为两行而不是把空白与单词一起推入逐字回退。
    assert_eq!(lines.len(), 2);
    // 第一行应保留源文本中的字母和 Unicode 空白。
    let first: String = lines[0].glyphs.iter().map(|glyph| glyph.ch).collect();
    // 第二行应只包含后续单词。
    let second: String = lines[1].glyphs.iter().map(|glyph| glyph.ch).collect();
    // 两行字符顺序应与源文本一致。
    assert_eq!(first, "a\u{2003}");
    // 后续单词不应被空白 token 牵连到上一行。
    assert_eq!(second, "bb");
}

// 验证真实 advance 使用源字符索引，而不是依赖 glyph 数组槽位。
#[test]
fn real_advance_uses_source_char_index() {
    // 构造跳过一个源字符索引的后端字形序列。
    let glyphs = vec![
        PositionedGlyph {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 12.0,
            glyph_id: 1,
            char_index: 0,
            // 测试字形覆盖第一个源字符。
            char_end: 1,
            // 测试字形默认使用 LTR 嵌入级别。
            bidi_level: 0,
            font: FontHandle::new(0),
        },
        PositionedGlyph {
            x: 10.0,
            y: 0.0,
            width: 20.0,
            height: 12.0,
            glyph_id: 2,
            char_index: 2,
            // 测试字形覆盖第三个源字符。
            char_end: 3,
            // 测试字形默认使用 LTR 嵌入级别。
            bidi_level: 0,
            font: FontHandle::new(0),
        },
        PositionedGlyph {
            x: 30.0,
            y: 0.0,
            width: 0.0,
            height: 12.0,
            glyph_id: 3,
            char_index: 3,
            // 测试字形覆盖第四个源字符。
            char_end: 4,
            // 测试字形默认使用 LTR 嵌入级别。
            bidi_level: 0,
            font: FontHandle::new(0),
        },
    ];
    // 索引 2 应读取第二个字形的真实宽度，而不是索引 1 的槽位。
    assert_eq!(measured_advance_for_char(&glyphs, 2, 7.0), 20.0);
    // 后端明确返回的零宽字形不能被误回退为可见宽度。
    assert_eq!(measured_advance_for_char(&glyphs, 3, 7.0), 0.0);
    // 没有字形的索引应回退到估算宽度。
    assert_eq!(measured_advance_for_char(&glyphs, 1, 7.0), 7.0);
    // 构造相邻字形的负 kerning，验证实际 x 间距会缩短当前 advance。
    let kerned_glyphs = vec![
        // 第一个字形的 advance 应由下一个源字形的 x 坐标决定。
        PositionedGlyph {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 12.0,
            glyph_id: 4,
            char_index: 0,
            // 测试字形覆盖第一个源字符。
            char_end: 1,
            // 测试字形默认使用 LTR 嵌入级别。
            bidi_level: 0,
            font: FontHandle::new(0),
        },
        // 下一个字形左移到 9px，模拟字体后端返回的 kerning。
        PositionedGlyph {
            x: 9.0,
            y: 0.0,
            width: 8.0,
            height: 12.0,
            glyph_id: 5,
            char_index: 1,
            // 测试字形覆盖第二个源字符。
            char_end: 2,
            // 测试字形默认使用 LTR 嵌入级别。
            bidi_level: 0,
            font: FontHandle::new(0),
        },
    ];
    // 富文本游标应与最终 draw_text 的下一个字形起点保持一致。
    assert_eq!(measured_advance_for_char(&kerned_glyphs, 0, 7.0), 9.0);
}
