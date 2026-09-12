//! 提供基于 Unicode UAX #14 的共享断行边界。

// 引入标准断行机会与字符行断属性查询。
use unicode_linebreak::{BreakClass, BreakOpportunity, break_property, linebreaks};

// 从文本中切出下一个 CRLF、CR 或 LF 强制换行前的逻辑行。
#[cfg_attr(not(test), allow(dead_code))]
pub fn split_once_mandatory(text: &str) -> (&str, Option<&str>) {
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
pub struct LineBreakMap {
    // 每个字符边界保存 UAX #14 的强制或允许机会。
    opportunities: Vec<Option<BreakOpportunity>>,
    // 只为超长字母数字词保存受控紧急断行边界。
    emergency: Vec<bool>,
}

// 提供共享断行查询。
impl LineBreakMap {
    // 从完整 UTF-8 文本构建字符索引口径的断行表。
    pub fn new(text: &str) -> Self {
        // 为每个字符边界预留一个断行机会槽位，并保留文本末尾边界。
        let char_count = text.chars().count();
        let mut opportunities = vec![None; char_count + 1];
        // 以单调游标流式消费 UTF-8 边界，避免短命的字节边界 Vec。
        let mut byte_boundaries = text
            .char_indices()
            .map(|(byte_index, _)| byte_index)
            .chain(std::iter::once(text.len()));
        // 当前边界对应的字符索引从文本起点开始。
        let mut boundary = byte_boundaries.next();
        let mut char_index = 0usize;
        // 从首个字节边界开始匹配 UAX 输出。
        // UAX 迭代器返回断行后继字符的 UTF-8 字节索引。
        for (byte_index, opportunity) in linebreaks(text) {
            // 推进到不早于当前 UAX 字节边界的位置。
            while boundary.is_some_and(|candidate| candidate < byte_index) {
                // 单调推进保证整体构建为线性复杂度。
                boundary = byte_boundaries.next();
                char_index += 1;
            }
            // 只接受真实 Unicode 标量边界。
            // 防御性比较避免异常索引污染字符表。
            if boundary == Some(byte_index) && char_index < opportunities.len() {
                // 记录 UAX #14 给出的强制或允许机会。
                opportunities[char_index] = Some(opportunity);
            }
        }
        // 默认所有边界都不允许偏离 UAX 的紧急断行。
        let mut emergency = vec![false; opportunities.len()];
        // 流式检查相邻 Unicode 标量，避免为推导边界复制字符数组。
        let mut previous = None;
        for (boundary, current) in text.chars().enumerate() {
            // 文本首字符之前没有可供比较的内部边界。
            let Some(previous_char) = previous else {
                previous = Some(current);
                continue;
            };
            // 已有标准 UAX 机会时不需要额外紧急标记。
            if opportunities[boundary].is_none()
                // 前一标量必须是字母或数字。
                && previous_char.is_alphanumeric()
                // 后一标量也必须是字母或数字。
                && current.is_alphanumeric()
            {
                // 允许超长单词在此边界进行受控兜底断行。
                emergency[boundary] = true;
            }
            // 保存当前标量供下一个内部边界比较。
            previous = Some(current);
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
    pub fn allows_at(&self, char_index: usize) -> bool {
        // 强制与允许机会都可以作为宽度折行的候选边界。
        self.opportunities
            // 读取目标字符边界。
            .get(char_index)
            // 只要存在 UAX 机会就允许断行。
            .is_some_and(Option::is_some)
    }

    // 查询超长字母数字词是否可在目标边界紧急断行。
    pub fn emergency_allows_at(&self, char_index: usize) -> bool {
        // 越界边界默认不允许。
        self.emergency.get(char_index).copied().unwrap_or(false)
    }

    // 判断字符是否是自动换行时可折叠的空白。
    pub fn collapsible_whitespace(ch: char) -> bool {
        // NBSP/NNBSP 的 NonBreakingGlue 属性必须保留不可断语义。
        ch == '\u{200b}'
            // 普通 Unicode 空白可以在自动行首折叠。
            || (ch.is_whitespace()
                // 非断空格即使属于 whitespace 也不能折叠。
                && break_property(ch as u32) != BreakClass::NonBreakingGlue)
    }
}

// cfg(test) 完整辅助实现位于 tests-src，仅测试构建编译。
#[cfg(test)]
#[path = "../../../../tests-src/draw/resources/font/line_break_tests.rs"]
mod line_break_tests;