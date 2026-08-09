//! 验证 RichText 跨样式段共享段落级 UAX #9 结果。

// 引入估算布局入口。
use super::rich_text_layout::layout_rich_text;
// 引入富文本段与文本样式。
use super::{RichTextSegment, RichTextStyle};
// 引入稳定测试颜色。
use crate::draw::Color;

// 验证样式边界不会破坏段落级混排视觉顺序与逻辑源索引。
#[test]
fn mixed_direction_across_style_segments_uses_one_paragraph_order() {
    // Latin 前缀使用普通文本样式。
    let latin = RichTextSegment::Text {
        // 保留段落 LTR 首强字符与中性空格。
        content: "abc ".to_owned(),
        // 使用默认样式。
        style: RichTextStyle::default(),
    };
    // Hebrew 中段故意放入链接样式段。
    let hebrew = RichTextSegment::Link {
        // 三个 Hebrew 字符形成 RTL run。
        content: "אבג".to_owned(),
        // 使用稳定非空链接目标。
        url: "https://example.invalid".to_owned(),
    };
    // 数字尾段使用代码样式，验证跨样式中性字符解析。
    let numbers = RichTextSegment::Code {
        // 空格与欧洲数字必须保留内部 LTR 顺序。
        content: " 123".to_owned(),
    };
    // 执行不依赖窗口和字体文件的完整富文本布局。
    let (lines, _, _) = layout_rich_text(
        // 传入跨样式段混排。
        &[latin, hebrew, numbers],
        // 使用足够宽的单行约束。
        4096.0,
        // 使用稳定默认字号。
        16.0,
        // 使用稳定默认颜色。
        Color::BLACK,
    );
    // 宽约束下必须只形成一个视觉行。
    assert_eq!(lines.len(), 1);
    // 按实际 x 观察视觉字符顺序，而不是依赖 run 内逻辑存储顺序。
    let mut visual_glyphs = lines[0].glyphs.iter().collect::<Vec<_>>();
    // 使用布局生成的有限坐标排序。
    visual_glyphs.sort_by(|left, right| left.x.total_cmp(&right.x));
    // 提取视觉到逻辑源字符映射。
    let visual_indices = visual_glyphs
        // 遍历视觉字符。
        .iter()
        // 读取全局逻辑字符索引。
        .map(|glyph| glyph.global_char_idx)
        // 收集映射。
        .collect::<Vec<_>>();
    // 数字保持 LTR，Hebrew 字符按视觉顺序反向，样式边界不改变段落结果。
    assert_eq!(visual_indices, vec![0, 1, 2, 3, 8, 9, 10, 7, 6, 5, 4]);
    // 链接 RTL run 内仍按逻辑文本顺序保存字符，供 rustybuzz 正确 shaping。
    let rtl_logical_indices = lines[0]
        // 遍历内部 run 顺序字符。
        .glyphs
        // 借用字符字形。
        .iter()
        // 只保留链接样式段。
        .filter(|glyph| glyph.segment_idx == 1)
        // 读取逻辑源索引。
        .map(|glyph| glyph.global_char_idx)
        // 收集 RTL run 内部顺序。
        .collect::<Vec<_>>();
    // RTL 绘制 run 必须保留逻辑字符顺序，视觉反向由 x 与 shaping 共同表达。
    assert_eq!(rtl_logical_indices, vec![4, 5, 6]);
    // 同一链接 run 的逻辑字符 x 坐标必须从右向左递减。
    let rtl_x = lines[0]
        // 遍历链接字符。
        .glyphs
        // 借用字符字形。
        .iter()
        // 只保留链接样式段。
        .filter(|glyph| glyph.segment_idx == 1)
        // 提取视觉左缘。
        .map(|glyph| glyph.x)
        // 收集坐标。
        .collect::<Vec<_>>();
    // 三个逻辑字符必须按 RTL 方向定位。
    assert!(rtl_x.windows(2).all(|pair| pair[0] > pair[1]));
}
