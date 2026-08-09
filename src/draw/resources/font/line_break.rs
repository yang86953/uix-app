//! 提供基于 Unicode UAX #14 的共享断行边界。

// 引入标准断行机会与字符行断属性查询。
use unicode_linebreak::{break_property, linebreaks, BreakClass, BreakOpportunity};

// 从文本中切出下一个 CRLF、CR 或 LF 强制换行前的逻辑行。
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn split_once_mandatory(text: &str) -> (&str, Option<&str>) {
    // 查找首个强制换行标量的 UTF-8 字节起点。
    let Some((byte_index, separator)) = text
        // 枚举全部 Unicode 标量字节起点。
        .char_indices()
        // 只接受 CR 或 LF。
        .find(|(_, ch)| matches!(ch, '\r' | '\n'))
    else {
        // 没有换行时完整文本就是最后一行。
        return (text, None);
    };
    // CRLF 作为一个不可拆分的强制换行序列消费。
    let separator_len = if separator == '\r' && text[byte_index..].starts_with("\r\n") {
        // CRLF 占两个 ASCII 字节。
        2
    // 单独 CR 或 LF 只占一个 ASCII 字节。
    } else {
        // 消费单个换行字节。
        1
    };
    // 返回换行前逻辑行和完整分隔符后的剩余文本。
    (
        // 当前逻辑行不包含换行标量。
        &text[..byte_index],
        // 即使剩余为空也保留 Some 以表达尾随空行。
        Some(&text[byte_index + separator_len..]),
    )
}

// 保存按 Unicode 标量边界索引的断行机会。
#[derive(Debug, Clone)]
pub(crate) struct LineBreakMap {
    // 每个字符边界保存 UAX #14 的强制或允许机会。
    opportunities: Vec<Option<BreakOpportunity>>,
    // 只为超长字母数字词保存受控紧急断行边界。
    emergency: Vec<bool>,
}

// 提供共享断行查询。
impl LineBreakMap {
    // 从完整 UTF-8 文本构建字符索引口径的断行表。
    pub(crate) fn new(text: &str) -> Self {
        // 保存每个 UTF-8 字节边界对应的 Unicode 标量索引。
        let byte_boundaries = text
            // 枚举每个 Unicode 标量的字节起点。
            .char_indices()
            // 将字节起点转换为字符边界对。
            .enumerate()
            // 调整元组顺序便于按字节推进。
            .map(|(char_index, (byte_index, _))| (byte_index, char_index))
            // 补入文本末尾的排他边界。
            .chain(std::iter::once((text.len(), text.chars().count())))
            // 固化边界以供线性游标查询。
            .collect::<Vec<_>>();
        // 为每个字符边界预留一个断行机会槽位。
        let mut opportunities = vec![None; byte_boundaries.len()];
        // 从首个字节边界开始匹配 UAX 输出。
        let mut boundary_cursor = 0usize;
        // UAX 迭代器返回断行后继字符的 UTF-8 字节索引。
        for (byte_index, opportunity) in linebreaks(text) {
            // 推进到不早于当前 UAX 字节边界的位置。
            while byte_boundaries
                // 读取当前候选字节边界。
                .get(boundary_cursor)
                // 只跳过严格更早的边界。
                .is_some_and(|(boundary, _)| *boundary < byte_index)
            {
                // 单调推进保证整体构建为线性复杂度。
                boundary_cursor += 1;
            }
            // 只接受真实 Unicode 标量边界。
            if let Some((boundary, char_index)) = byte_boundaries.get(boundary_cursor) {
                // 防御性比较避免异常索引污染字符表。
                if *boundary == byte_index {
                    // 记录 UAX #14 给出的强制或允许机会。
                    opportunities[*char_index] = Some(opportunity);
                }
            }
        }
        // 收集字符用于推导受控的超长词紧急断行边界。
        let chars = text.chars().collect::<Vec<_>>();
        // 默认所有边界都不允许偏离 UAX 的紧急断行。
        let mut emergency = vec![false; opportunities.len()];
        // 只检查两个相邻 Unicode 标量之间的内部边界。
        for boundary in 1..chars.len() {
            // 已有标准 UAX 机会时不需要额外紧急标记。
            if opportunities[boundary].is_none()
                // 前一标量必须是字母或数字。
                && chars[boundary - 1].is_alphanumeric()
                // 后一标量也必须是字母或数字。
                && chars[boundary].is_alphanumeric()
            {
                // 允许超长单词在此边界进行受控兜底断行。
                emergency[boundary] = true;
            }
        }
        // 返回字符索引口径的共享断行表。
        Self {
            // 保存标准断行机会。
            opportunities,
            // 保存紧急断行机会。
            emergency,
        }
    }

    // 查询边界是否允许标准自动断行。
    pub(crate) fn allows_at(&self, char_index: usize) -> bool {
        // 强制与允许机会都可以作为宽度折行的候选边界。
        self.opportunities
            // 读取目标字符边界。
            .get(char_index)
            // 只要存在 UAX 机会就允许断行。
            .is_some_and(Option::is_some)
    }

    // 查询边界是否是 UAX 强制断行。
    #[cfg(test)]
    pub(crate) fn mandatory_at(&self, char_index: usize) -> bool {
        // 精确匹配强制断行类型。
        matches!(
            // 读取目标字符边界并复制轻量枚举。
            self.opportunities.get(char_index).copied().flatten(),
            // 只接受 Mandatory。
            Some(BreakOpportunity::Mandatory)
        )
    }

    // 查询超长字母数字词是否可在目标边界紧急断行。
    pub(crate) fn emergency_allows_at(&self, char_index: usize) -> bool {
        // 越界边界默认不允许。
        self.emergency.get(char_index).copied().unwrap_or(false)
    }

    // 判断字符是否是自动换行时可折叠的空白。
    pub(crate) fn collapsible_whitespace(ch: char) -> bool {
        // NBSP/NNBSP 的 NonBreakingGlue 属性必须保留不可断语义。
        ch == '\u{200b}'
            // 普通 Unicode 空白可以在自动行首折叠。
            || (ch.is_whitespace()
                // 非断空格即使属于 whitespace 也不能折叠。
                && break_property(ch as u32) != BreakClass::NonBreakingGlue)
    }
}

// 覆盖 UAX #14 验收矩阵中的关键边界。
#[cfg(test)]
mod tests {
    // 引入被测断行表。
    use super::{split_once_mandatory, LineBreakMap};

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
}
