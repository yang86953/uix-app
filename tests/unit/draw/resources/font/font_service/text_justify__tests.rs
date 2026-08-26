// 引入被测入口和视觉行包装类型。
use super::{LineGlyph, justify_line};
// 引入可构造的中性字形与字体句柄。
use crate::draw::{FontHandle, resources::font::text_backend::PositionedGlyph};

// 按固定坐标构造一个视觉字形。
fn glyph(index: usize, source_char: char) -> LineGlyph {
    // 返回六像素等宽测试字形。
    LineGlyph {
        // 保存中性定位字形。
        glyph: PositionedGlyph {
            // 按字符下标建立稳定横坐标。
            x: index as f32 * 6.0,
            // 测试字形位于行顶。
            y: 0.0,
            // 使用固定六像素 advance。
            width: 6.0,
            // 使用固定十像素字形高度。
            height: 10.0,
            // 使用下标作为稳定字形编号。
            glyph_id: index as u32,
            // 保存逻辑字符起点。
            char_index: index,
            // 每个字形覆盖一个字符。
            char_end: index + 1,
            // 测试行使用从左向右级别。
            bidi_level: 0,
            // 使用唯一有效测试字体句柄。
            font: FontHandle::new(0),
        },
        // 保存用于识别可折叠空白的源字符。
        source_char,
    }
}

// 非末行必须把剩余宽度放入空白 advance。
#[test]
fn text_align_justify_expands_gap_and_following_position() {
    // 构造“a b”的稳定视觉行。
    let mut glyphs = vec![glyph(0, 'a'), glyph(1, ' '), glyph(2, 'b')];
    // 从十八像素自然宽度扩展到二十四像素容器。
    let width = justify_line(&mut glyphs, 18.0, 24.0, false);
    // 行宽必须精确占满容器。
    assert_eq!(width, 24.0);
    // 空白 advance 必须包含新增六像素。
    assert_eq!(glyphs[1].glyph.width, 12.0);
    // 后续正文必须同步平移，不能与扩展空白重叠。
    assert_eq!(glyphs[2].glyph.x, 18.0);
    // 段落末行必须保持自然宽度与坐标。
    assert_eq!(justify_line(&mut glyphs, 24.0, 30.0, true), 24.0);
}
