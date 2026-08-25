//! Input 字素簇光标、选择、删除与长度限制回归测试。

// 引入被测 Input 及其私有交互辅助。
use super::{Input, LogicalLineCursor, logical_line_count};
// 使用缓存索引模型复现优化前的字素簇归一语义。
use crate::draw::resources::font::text_index::{BoundaryBias, CharIndex, TextIndexMap};

/// 线性行游标必须保持旧渲染路径的 Unicode 字符区间和耗尽回退语义。
#[test]
fn logical_line_cursor_preserves_unicode_and_empty_line_ranges() {
    // 跳过首行后，空行仍必须占用一个换行字符位置。
    let mut cursor = LogicalLineCursor::new("甲🙂\n\n尾");
    cursor.skip_lines(1);
    assert_eq!(cursor.next_line(), ("", 3, 3));
    assert_eq!(cursor.next_line(), ("尾", 4, 5));
    // composition 产生额外显示行时，耗尽值行仍按空行继续推进。
    assert_eq!(cursor.next_line(), ("", 6, 6));
}

/// 左右移动和 Shift 选择必须按完整扩展字素簇推进。
#[test]
// 同时覆盖组合音标、emoji ZWJ 肤色序列与 Indic 连写。
fn cursor_movement_and_shift_selection_skip_whole_graphemes() {
    // 构造三个连续复杂字素簇，共九个 Unicode 标量。
    let mut input = Input::new("").with_value("a\u{0301}👩🏽‍💻क्ष");
    // 从文本起点开始移动。
    input.cursor_char = 0;
    // 初始化选择锚点。
    input.sel_anchor.set(0);
    // 普通右移越过组合音标字素簇。
    input.move_cursor_right(false, false);
    // 光标不得停在组合音标内部的字符一。
    assert_eq!(input.cursor_char, 2);
    // Shift 右移越过完整 emoji 序列。
    input.move_cursor_right(false, true);
    // 光标抵达四标量 emoji 之后。
    assert_eq!(input.cursor_char, 6);
    // 选择只使用合法字素簇边界。
    assert_eq!(input.selection.get(), Some((2, 6)));
    // 再次 Shift 右移越过完整 Indic 连写。
    input.move_cursor_right(false, true);
    // 光标抵达文本末尾。
    assert_eq!(input.cursor_char, 9);
    // 选择扩展到完整 Indic 字素簇之后。
    assert_eq!(input.selection.get(), Some((2, 9)));
    // 普通左移先把选择折叠到逻辑起点。
    input.move_cursor_left(false, false);
    // 折叠位置保持为合法字素簇边界。
    assert_eq!(input.cursor_char, 2);
    // 折叠后选择被清除。
    assert_eq!(input.selection.get(), None);
    // 再次左移一次越过整个组合音标。
    input.move_cursor_left(false, false);
    // 光标回到文本起点。
    assert_eq!(input.cursor_char, 0);
}

/// 流式按词移动必须逐位置保持旧字符向量算法的结果。
#[test]
fn ctrl_word_movement_streams_without_changing_boundaries() {
    // 覆盖空文本、连续空格、多字节字符与复杂字素簇。
    let samples = ["", "  alpha  beta ", "甲 乙🙂 丙", "a\u{0301} 👩🏽‍💻 क्ष"];
    for text in samples {
        let chars = text.chars().collect::<Vec<_>>();
        let map = TextIndexMap::new(text);
        for cursor in 0..=chars.len() + 2 {
            // 精确复现优化前 Ctrl+Left 的空格分词算法。
            let mut expected_left = cursor.min(chars.len()).saturating_sub(1);
            while expected_left > 0 && chars[expected_left] == ' ' {
                expected_left -= 1;
            }
            while expected_left > 0 && chars[expected_left - 1] != ' ' {
                expected_left -= 1;
            }
            expected_left = map
                .normalize_char(CharIndex(expected_left), BoundaryBias::Backward)
                .0;

            let mut input = Input::new("").with_value(text);
            input.cursor_char = cursor;
            input.move_cursor_left(true, false);
            assert_eq!(
                input.cursor_char, expected_left,
                "文本 {text:?} 的位置 {cursor} 向左按词结果不一致"
            );

            // 精确复现优化前 Ctrl+Right 的空格分词算法。
            let mut expected_right = cursor.min(chars.len());
            while expected_right < chars.len() && chars[expected_right] == ' ' {
                expected_right += 1;
            }
            while expected_right < chars.len() && chars[expected_right] != ' ' {
                expected_right += 1;
            }
            expected_right = map
                .normalize_char(CharIndex(expected_right), BoundaryBias::Forward)
                .0;

            input.cursor_char = cursor;
            input.move_cursor_right(true, false);
            assert_eq!(
                input.cursor_char, expected_right,
                "文本 {text:?} 的位置 {cursor} 向右按词结果不一致"
            );
        }
    }
}

/// Backspace 与 Delete 必须删除完整扩展字素簇。
#[test]
// 使用连续复杂序列验证前删和后删不会留下残片。
fn backward_and_forward_delete_remove_whole_graphemes() {
    // 构造组合音标、emoji 与 Indic 连写。
    let mut input = Input::new("").with_value("a\u{0301}👩🏽‍💻क्ष");
    // 把光标放在 Indic 连写之后。
    input.cursor_char = 9;
    // 删除前一个完整字素簇。
    assert!(input.delete_previous_grapheme());
    // Indic 连写三个标量必须整体消失。
    assert_eq!(input.current_value(), "a\u{0301}👩🏽‍💻");
    // 光标退到 Indic 连写原起点。
    assert_eq!(input.cursor_char, 6);
    // 把光标放在 emoji 之前。
    input.cursor_char = 2;
    // 删除后一个完整字素簇。
    assert!(input.delete_next_grapheme());
    // 四标量 emoji 必须整体消失且组合音标保持完整。
    assert_eq!(input.current_value(), "a\u{0301}");
    // 光标保持在删除范围起点。
    assert_eq!(input.cursor_char, 2);
}

/// 选择归一与最大长度不得制造半个扩展字素簇。
#[test]
// 验证旧标量位置输入会扩展或拒绝到安全字素簇边界。
fn selection_and_max_length_preserve_complete_graphemes() {
    // 构造包含组合音标和 emoji 的输入值。
    let input = Input::new("").with_value("a\u{0301}👩🏽‍💻");
    // 模拟旧调用方传入两个字素簇内部位置。
    input.set_selection_range(1, 4);
    // 选择必须向外覆盖两个完整字素簇。
    assert_eq!(input.selection.get(), Some((0, 6)));
    // 提取结果不得遗漏组合标记或 ZWJ 成员。
    assert_eq!(input.slice_range(1, 4), "a\u{0301}👩🏽‍💻");
    // 构造只能容纳三个标量的空输入。
    let mut limited = Input::new("").max_length(3);
    // 四标量 emoji 不能被截成不完整序列插入。
    assert!(!limited.insert_text_at_cursor("👩🏽‍💻"));
    // 拒绝后值保持为空。
    assert_eq!(limited.current_value(), "");
}

/// 批量插入必须一次提交过滤后的文本并保持字符光标语义。
#[test]
fn batch_insert_filters_controls_and_normalizes_textarea_newlines() {
    // 单行输入过滤控制字符，但完整保留多字节字符并按标量推进光标。
    let mut input = Input::new("").with_value("首尾");
    input.cursor_char = 1;
    assert!(input.insert_text_at_cursor("中\n🙂"));
    assert_eq!(input.current_value(), "首中🙂尾");
    assert_eq!(input.cursor_char, 3);

    // 多行输入统一 CRLF/CR，并原地过滤其他控制字符。
    let mut textarea = Input::textarea();
    assert!(textarea.insert_text_at_cursor("甲\r\n乙\r丙\t丁"));
    assert_eq!(textarea.current_value(), "甲\n乙\n丙丁");
    assert_eq!(textarea.cursor_char, 6);

    // 替换选择后再计算长度预算，批量插入不得按删除前长度错误拒绝。
    let mut replacement = Input::new("").with_value("abcd").max_length(4);
    replacement.set_selection_range(1, 3);
    replacement.cursor_char = 3;
    assert!(replacement.insert_text_at_cursor("中🙂"));
    assert_eq!(replacement.current_value(), "a中🙂d");
    assert_eq!(replacement.cursor_char, 3);
}

/// 流式行列定位必须保留空行、Unicode 字符和越界收敛语义。
#[test]
fn cursor_line_col_streams_without_changing_positions() {
    // 空文本仍有首个空逻辑行。
    let mut input = Input::textarea();
    input.cursor_char = 0;
    assert_eq!(input.cursor_line_col(), (0, 0));

    // 连续换行会产生中间空行，列使用 Unicode 标量下标而非字节偏移。
    input.value = "甲a\n\n👩z".to_string();
    input.cursor_char = 4;
    assert_eq!(input.cursor_line_col(), (2, 0));
    input.cursor_char = 5;
    assert_eq!(input.cursor_line_col(), (2, 1));

    // 旧状态中的越界光标继续收敛到末行末尾。
    input.cursor_char = usize::MAX;
    assert_eq!(input.cursor_line_col(), (2, 2));
}

// 验证字节级逻辑行计数保持 split 的空文本与尾随空行语义。
#[test]
fn logical_line_count_preserves_empty_and_trailing_lines() {
    assert_eq!(logical_line_count(""), 1);
    assert_eq!(logical_line_count("甲\n\n尾\n"), 4);
}

// 验证上下移动流式定位相邻行并保留空行与 Unicode 字符位置。
#[test]
fn vertical_cursor_movement_streams_adjacent_line_offsets() {
    let mut input = Input::textarea().with_value("甲🙂\n\nab");
    // 从末行第一列开始，字符位置五位于 a 之后。
    input.cursor_char = 5;
    input.sel_anchor.set(5);

    input.move_cursor_up();
    assert_eq!(input.cursor_char, 3);
    input.move_cursor_up();
    assert_eq!(input.cursor_char, 0);
    input.move_cursor_down();
    assert_eq!(input.cursor_char, 3);
    input.move_cursor_down();
    assert_eq!(input.cursor_char, 4);
}

// 验证多行坐标命中流式计算全文行起点并收敛到末行。
#[test]
fn textarea_hit_test_streams_line_offsets_without_glyphs() {
    let input = Input::textarea().with_value("甲🙂\n\nab");
    let top = input.visual.layout.textarea_top_padding;
    let line_height = input.visual.typography.line_height;

    // 第三逻辑行从全文字符位置四开始。
    assert_eq!(input.char_at_xy(20.0, top + line_height * 2.0 + 1.0), 4);
    // 超出文本的纵坐标继续收敛到末行起点。
    assert_eq!(input.char_at_xy(20.0, top + line_height * 20.0), 4);
    // 非有限纵坐标也只扫描现有逻辑行，不按饱和后的巨大行号循环。
    assert_eq!(input.char_at_xy(20.0, f32::INFINITY), 4);
    // 顶部 padding 命中首行起点。
    assert_eq!(input.char_at_xy(20.0, top - 1.0), 0);
}
