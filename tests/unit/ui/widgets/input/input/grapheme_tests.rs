//! Input 字素簇光标、选择、删除与长度限制回归测试。

// 引入被测 Input 及其私有交互辅助。
use super::{Input, LogicalLineCursor};

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
