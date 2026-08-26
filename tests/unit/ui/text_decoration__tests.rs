// 引入被测线段生成入口。
use super::*;
// 引入可构造的字体句柄和布局结构。
use crate::draw::{
    FontHandle,
    resources::font::text_backend::{LineInfo, PositionedGlyph},
};

// 构造包含逆序视觉字形的一行布局。
fn text_decoration_test_layout() -> TextLayout {
    // 返回只用于几何测试的中性布局。
    TextLayout {
        // 逆序横坐标验证边界聚合不依赖首尾顺序。
        glyphs: vec![
            // 首个视觉字形位于右侧。
            PositionedGlyph {
                x: 8.0,
                y: 0.0,
                width: 4.0,
                height: 8.0,
                glyph_id: 1,
                char_index: 0,
                char_end: 1,
                bidi_level: 1,
                font: FontHandle(0),
            },
            // 第二个视觉字形位于左侧。
            PositionedGlyph {
                x: 2.0,
                y: 0.0,
                width: 5.0,
                height: 8.0,
                glyph_id: 2,
                char_index: 1,
                char_end: 2,
                bidi_level: 1,
                font: FontHandle(0),
            },
        ],
        // 单行高度用于验证三种垂直位置。
        lines: vec![LineInfo {
            y: 3.0,
            height: 20.0,
            width: 10.0,
            start_char: 0,
            end_char: 2,
            glyph_start: 0,
            glyph_count: 2,
        }],
        // 布局总宽度不参与线段边界计算。
        width: 12.0,
        // 布局总高度不参与线段边界计算。
        height: 20.0,
    }
}

// 三种装饰必须共享视觉字形边界并使用各自纵坐标。
#[test]
fn text_decoration_segments_follow_visual_line_geometry() {
    // 构造带绝对偏移的测试布局。
    let layout = text_decoration_test_layout();
    // 生成下划线线段。
    let underline = segments(
        &layout,
        Point::new(10.0, 20.0),
        10.0,
        TextDecoration::Underline,
    );
    // 生成上划线线段。
    let overline = segments(
        &layout,
        Point::new(10.0, 20.0),
        10.0,
        TextDecoration::Overline,
    );
    // 生成删除线线段。
    let line_through = segments(
        &layout,
        Point::new(10.0, 20.0),
        10.0,
        TextDecoration::LineThrough,
    );
    // 混排边界必须从最左二像素延伸到最右十二像素。
    assert_eq!((underline[0].start_x, underline[0].end_x), (12.0, 22.0));
    // 下划线位于行框下缘内侧。
    assert_eq!(underline[0].y, 42.0);
    // 上划线位于行框上缘内侧。
    assert_eq!(overline[0].y, 24.0);
    // 删除线位于行框中部。
    assert_eq!(line_through[0].y, 33.4);
    // 显式 none 不产生线段。
    assert!(segments(&layout, Point::new(10.0, 20.0), 10.0, TextDecoration::None).is_empty());
}
