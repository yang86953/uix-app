// 引入被测断行表。
use super::{LineBreakMap, split_once_mandatory};

// 将 UTF-8 文本中的字节边界换算为字符边界。
fn char_boundary(text: &str, byte_boundary: usize) -> usize {
    // 统计目标字节边界之前的 Unicode 标量数量。
    text[..byte_boundary].chars().count()
}

// 验证 CJK 与标点使用默认 UAX #14 禁则。
#[test]
fn cjk_and_punctuation_follow_uax14_boundaries() {
    // CJK 字符之间应存在允许断行边界。
    let cjk = LineBreakMap::new("天地");
    // 第一个汉字之后必须可以断行。
    assert!(cjk.allows_at(1));
    // 逗号前不能断行，避免标点出现在行首。
    let punctuation = LineBreakMap::new("天，地");
    // 汉字与逗号之间不得产生标准断行机会。
    assert!(!punctuation.allows_at(1));
    // 逗号之后可以恢复 CJK 断行机会。
    assert!(punctuation.allows_at(2));
}

// 验证非断空格、组合序列与 emoji 序列内部均无断点。
#[test]
fn non_breaking_sequences_stay_atomic() {
    // NBSP 必须把两侧正文保持在同一不可断序列中。
    let nbsp = LineBreakMap::new("a\u{00a0}b");
    // NBSP 前不得断行。
    assert!(!nbsp.allows_at(1));
    // NBSP 后也不得断行。
    assert!(!nbsp.allows_at(2));
    // 组合音标必须依附前一基字。
    let combining = LineBreakMap::new("a\u{0301}b");
    // 基字与组合符之间不得断行。
    assert!(!combining.allows_at(1));
    // ZWJ emoji 家庭序列必须保持内部原子性。
    let emoji_text = "👩‍👩‍👧‍👦x";
    // 构建完整 emoji 序列断行表。
    let emoji = LineBreakMap::new(emoji_text);
    // 找到末尾 ASCII 字符的字节起点。
    let x_byte = emoji_text.find('x').expect("测试文本必须包含 x");
    // emoji 序列内部每个字符边界均不得断行。
    for boundary in 1..char_boundary(emoji_text, x_byte) {
        // 检查当前内部边界没有 UAX 机会。
        assert!(!emoji.allows_at(boundary));
    }
}

// 验证 CRLF 强制边界与超长单词紧急断行边界。
#[test]
fn mandatory_crlf_and_long_word_emergency_are_distinct() {
    // CRLF 应被视为一个强制换行序列。
    let crlf = LineBreakMap::new("a\r\nb");
    // CR 与 LF 之间不能拆开。
    assert!(!crlf.mandatory_at(2));
    // 完整 CRLF 之后必须强制换行。
    assert!(crlf.mandatory_at(3));
    // 普通长单词内部没有标准 UAX 机会。
    let word = LineBreakMap::new("abcd");
    // 字母之间不属于标准允许断行。
    assert!(!word.allows_at(2));
    // 仅紧急兜底允许长单词在字母边界折行。
    assert!(word.emergency_allows_at(2));
    // NBSP 序列不能被紧急兜底绕过。
    let nbsp = LineBreakMap::new("a\u{00a0}b");
    // NBSP 后仍不得紧急断行。
    assert!(!nbsp.emergency_allows_at(2));
    // 富文本辅助切分同样必须整体消费 CRLF。
    assert_eq!(split_once_mandatory("a\r\nb"), ("a", Some("b")));
    // 单独 CR 也必须产生强制换行。
    assert_eq!(split_once_mandatory("a\rb"), ("a", Some("b")));
}
