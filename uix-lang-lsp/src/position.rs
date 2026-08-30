//! UTF-8 字节偏移与 LSP UTF-16 行列位置之间的确定换算 Component。

// 保存一份源码的行起点索引，让同一请求内的多次换算只扫描一次源码。
pub(crate) struct LineIndex {
    // 保存每行首字节的偏移，首元素恒为 0。
    line_starts: Vec<usize>,
    // 保存源码总字节数，用于越界钳制。
    source_len: usize,
}

impl LineIndex {
    // 为完整源码建立行起点索引。
    pub(crate) fn new(source: &str) -> Self {
        // 行首从偏移 0 开始。
        let mut line_starts = vec![0usize];
        // 扫描全部字节，遇到换行即登记下一行起点。
        for (index, byte) in source.bytes().enumerate() {
            // 换行字节的下一字节是新行首。
            if byte == b'\n' {
                line_starts.push(index + 1);
            }
        }
        Self {
            line_starts,
            source_len: source.len(),
        }
    }

    // 把 LSP 行列位置换算为字节偏移；越界行列一律钳制到最近合法位置。
    pub(crate) fn offset(&self, source: &str, line: u64, character: u64) -> usize {
        // 行号超出末行时钳制到最后一行。
        let line_index = (line as usize).min(self.line_starts.len() - 1);
        // 取行起点字节偏移。
        let line_start = self.line_starts[line_index];
        // 行尾是下一行起点之前；换行符本身不属于行内容。
        let line_end = self
            .line_starts
            .get(line_index + 1)
            .map(|start| start - 1)
            .unwrap_or(self.source_len);
        // 行内按 UTF-16 码元计数推进到目标列。
        let mut units = 0u64;
        let mut offset = line_start;
        for step in source[line_start..line_end].chars() {
            // 已达目标列时停在当前字符起点。
            if units >= character {
                break;
            }
            units += u64::try_from(step.len_utf16()).unwrap_or(u64::MAX);
            offset += step.len_utf8();
        }
        // 结果必然落在字符边界上。
        offset
    }

    // 把字节偏移换算为 LSP 行列位置；越界与非法边界一律钳制。
    pub(crate) fn position(&self, source: &str, offset: usize) -> (u64, u64) {
        // 先把偏移钳制回源码范围与字符边界。
        let mut offset = offset.min(self.source_len);
        while offset > 0 && !source.is_char_boundary(offset) {
            offset -= 1;
        }
        // 找出偏移所在行：最后一个起点不大于偏移的行。
        let line_index = self
            .line_starts
            .partition_point(|&start| start <= offset)
            .saturating_sub(1);
        // 取行起点字节偏移。
        let line_start = self.line_starts[line_index];
        // 列使用 UTF-16 码元计数，与客户端编码保持一致。
        let character = source[line_start..offset].encode_utf16().count();
        ((line_index as u64), (character as u64))
    }
}

#[cfg(test)]
mod tests {
    use super::LineIndex;

    #[test]
    fn offsets_round_trip_through_utf16_positions() {
        // 一基与多字节混排：中文字符占 1 个 UTF-16 码元。
        let source = "甲x\n<Text 值=\"x\" />";
        let index = LineIndex::new(source);
        let start = source.find("值").expect("fixture 必须包含中文字符");
        let position = index.position(source, start);
        assert_eq!(position, (1, 6));
        assert_eq!(index.offset(source, position.0, position.1), start);
    }

    #[test]
    fn astral_characters_count_as_two_utf16_units() {
        // U+1D54F 是增补平面字符，占 2 个 UTF-16 码元。
        let source = "𝕏\n𝕏𝕏 end";
        let index = LineIndex::new(source);
        let start = source.find(" end").expect("fixture 必须包含行尾标记");
        assert_eq!(index.position(source, start), (1, 4));
        assert_eq!(index.offset(source, 1, 4), start);
        // 同一行上第 6 个码元落在字母 n 上。
        assert_eq!(index.offset(source, 1, 6), start + 2);
    }

    #[test]
    fn out_of_range_positions_are_clamped() {
        let source = "<App />";
        let index = LineIndex::new(source);
        // 行号越界钳制到末行；列号 0 停在行首。
        assert_eq!(index.offset(source, 9, 0), 0);
        // 列号越界钳制到行尾（同时是源码末尾）。
        assert_eq!(index.offset(source, 0, 99), source.len());
        // 偏移越界时钳制到末尾行列。
        assert_eq!(
            index.position(source, 99),
            (0, u64::try_from(source.chars().count()).unwrap_or(0))
        );
    }

    #[test]
    fn offsets_inside_multibyte_characters_clamp_to_boundaries() {
        let source = "中文";
        let index = LineIndex::new(source);
        // 第 1 字节位于多字节字符内部，必须向前吸附到行首。
        assert_eq!(index.position(source, 1), (0, 0));
    }
}
