//! 验证无分配文字索引游标与缓存索引表保持完全相同的 Unicode 语义。

use uix::draw::resources::font::text_index::{
    BoundaryBias, CharIndex, TextIndexCursor, TextIndexMap,
};

#[test]
fn streaming_cursor_matches_cached_index_map() {
    // 覆盖空文本、ASCII、多字节标量、组合字符、ZWJ emoji、Indic 与双向文本。
    let samples = [
        "",
        "steady",
        "Aé中",
        "a\u{0301}b",
        "👩🏽‍💻Z",
        "क्ष",
        "Aא\u{05B7}בZ",
    ];
    let biases = [
        BoundaryBias::Backward,
        BoundaryBias::Forward,
        BoundaryBias::Nearest,
    ];

    for text in samples {
        // 现有缓存模型作为稳定语义参考。
        let map = TextIndexMap::new(text);
        // 把两个越界位置也纳入移动、转换与选择对照。
        let limit = map.char_len().0 + 2;
        for index in 0..=limit {
            let cursor = TextIndexCursor::new(text);
            assert_eq!(
                cursor.char_to_byte(CharIndex(index)),
                map.char_to_byte(CharIndex(index)),
                "文本 {text:?} 的字符位置 {index} 字节转换不一致"
            );
            assert_eq!(
                cursor.previous_grapheme_boundary(CharIndex(index)),
                map.previous_grapheme_boundary(CharIndex(index)),
                "文本 {text:?} 的字符位置 {index} 前一边界不一致"
            );
            assert_eq!(
                cursor.next_grapheme_boundary(CharIndex(index)),
                map.next_grapheme_boundary(CharIndex(index)),
                "文本 {text:?} 的字符位置 {index} 后一边界不一致"
            );
            for bias in biases {
                assert_eq!(
                    cursor.normalize_char(CharIndex(index), bias),
                    map.normalize_char(CharIndex(index), bias),
                    "文本 {text:?} 的字符位置 {index} 使用 {bias:?} 归一结果不一致"
                );
            }
            for other in 0..=limit {
                assert_eq!(
                    cursor.normalize_selection(CharIndex(index), CharIndex(other)),
                    map.normalize_selection(CharIndex(index), CharIndex(other)),
                    "文本 {text:?} 的选择 {index}..{other} 归一结果不一致"
                );
            }
        }
    }
}
