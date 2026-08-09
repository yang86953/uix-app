//! 文本编辑与排版共享的 Unicode 索引和扩展字素簇边界模型。

// 引入标准扩展字素簇分割能力。
use unicode_segmentation::UnicodeSegmentation;

/// UTF-8 字节偏移；不得与字符下标直接混用。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ByteIndex(pub usize);

/// Unicode 标量下标，对应 `str::chars()` 的序号。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CharIndex(pub usize);

/// 扩展字素簇下标，对应 `UnicodeSegmentation::graphemes()` 的序号。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct GraphemeIndex(pub usize);

/// shaping 字形簇覆盖的逻辑字符区间；它不等同于扩展字素簇。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShapingCluster {
    /// 保存簇的逻辑字符起点。
    pub start: CharIndex,
    /// 保存簇的逻辑字符排他终点。
    pub end: CharIndex,
}

/// 指定内部字符位置归一到哪一侧的扩展字素簇边界。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoundaryBias {
    /// 选择不晚于输入位置的边界。
    Backward,
    /// 选择不早于输入位置的边界。
    Forward,
    /// 选择字符距离最近的边界，距离相同时选择前进方向。
    Nearest,
}

/// 缓存一段 UTF-8 文本的字符、字节与扩展字素簇边界转换表。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextIndexMap {
    /// 每个字符边界对应的 UTF-8 字节偏移，包含文本末尾。
    char_byte_boundaries: Vec<usize>,
    /// 每个扩展字素簇边界对应的字符下标，包含文本末尾。
    grapheme_char_boundaries: Vec<usize>,
}

impl TextIndexMap {
    /// 为完整源文本建立不可混淆的索引转换表。
    pub fn new(text: &str) -> Self {
        // 收集每个 Unicode 标量的 UTF-8 起始偏移。
        let mut char_byte_boundaries = text
            .char_indices()
            .map(|(byte, _)| byte)
            .collect::<Vec<_>>();
        // 把文本末尾登记为最后一个合法字符边界。
        char_byte_boundaries.push(text.len());
        // 为每个扩展字素簇记录其字符起点。
        let mut grapheme_char_boundaries = text
            // 按标准扩展字素簇遍历源文本。
            .grapheme_indices(true)
            // 把字节起点精确转换为字符下标。
            .map(|(byte, _)| char_byte_boundaries.partition_point(|candidate| *candidate < byte))
            // 固化边界表以供重复查询。
            .collect::<Vec<_>>();
        // 空文本和非空文本都显式保留起始边界。
        if grapheme_char_boundaries.first() != Some(&0) {
            // 插入逻辑文本起点。
            grapheme_char_boundaries.insert(0, 0);
        }
        // 读取源文本字符数量。
        let char_len = char_byte_boundaries.len().saturating_sub(1);
        // 把文本末尾登记为最后一个合法字素簇边界。
        if grapheme_char_boundaries.last() != Some(&char_len) {
            // 追加逻辑文本终点。
            grapheme_char_boundaries.push(char_len);
        }
        // 返回两套边界表组成的索引模型。
        Self {
            // 保存字符到字节的转换表。
            char_byte_boundaries,
            // 保存字素簇到字符的转换表。
            grapheme_char_boundaries,
        }
    }

    /// 返回 UTF-8 字节长度。
    pub fn byte_len(&self) -> ByteIndex {
        // 最后一个字符边界就是文本字节长度。
        ByteIndex(*self.char_byte_boundaries.last().unwrap_or(&0))
    }

    /// 返回 Unicode 标量数量。
    pub fn char_len(&self) -> CharIndex {
        // 边界数量比字符数量多一。
        CharIndex(self.char_byte_boundaries.len().saturating_sub(1))
    }

    /// 返回扩展字素簇数量。
    pub fn grapheme_len(&self) -> GraphemeIndex {
        // 边界数量比字素簇数量多一。
        GraphemeIndex(self.grapheme_char_boundaries.len().saturating_sub(1))
    }

    /// 把字符边界转换为 UTF-8 字节边界，越界输入收敛到文本末尾。
    pub fn char_to_byte(&self, index: CharIndex) -> ByteIndex {
        // 限制字符下标并读取对应字节偏移。
        ByteIndex(self.char_byte_boundaries[index.0.min(self.char_len().0)])
    }

    /// 把 UTF-8 字节位置向后收敛到不晚于它的字符边界。
    pub fn byte_to_char_floor(&self, index: ByteIndex) -> CharIndex {
        // 先限制字节位置，避免越过源文本。
        let byte = index.0.min(self.byte_len().0);
        // 统计严格早于该位置的字符边界，并让精确边界保留自身。
        let insertion = self
            .char_byte_boundaries
            .partition_point(|candidate| *candidate <= byte);
        // 前一个边界就是向后收敛结果。
        CharIndex(insertion.saturating_sub(1))
    }

    /// 把 UTF-8 字节位置向前收敛到不早于它的字符边界。
    pub fn byte_to_char_ceil(&self, index: ByteIndex) -> CharIndex {
        // 先限制字节位置，避免越过源文本。
        let byte = index.0.min(self.byte_len().0);
        // 第一个不小于该位置的字符边界就是向前结果。
        CharIndex(
            self.char_byte_boundaries
                .partition_point(|candidate| *candidate < byte),
        )
    }

    /// 把字素簇下标转换为其字符起点，越界输入收敛到文本末尾。
    pub fn grapheme_to_char(&self, index: GraphemeIndex) -> CharIndex {
        // 限制字素簇下标并读取对应字符边界。
        CharIndex(self.grapheme_char_boundaries[index.0.min(self.grapheme_len().0)])
    }

    /// 判断字符下标是否为扩展字素簇边界。
    pub fn is_grapheme_boundary(&self, index: CharIndex) -> bool {
        // 只接受边界表中的真实字符位置。
        self.grapheme_char_boundaries
            .binary_search(&index.0)
            .is_ok()
    }

    /// 按指定偏向把任意字符位置归一为扩展字素簇边界。
    pub fn normalize_char(&self, index: CharIndex, bias: BoundaryBias) -> CharIndex {
        // 先把输入限制到有效字符区间。
        let index = index.0.min(self.char_len().0);
        // 找到第一个不小于输入位置的字素簇边界。
        let forward_slot = self
            // 查询有序字素簇边界表。
            .grapheme_char_boundaries
            // 跳过所有严格早于输入位置的边界。
            .partition_point(|candidate| *candidate < index);
        // 读取向前边界并在末尾兜底。
        let forward = self.grapheme_char_boundaries[forward_slot.min(self.grapheme_len().0)];
        // 精确命中边界时无需继续选择偏向。
        if forward == index {
            // 保留合法字素簇边界。
            return CharIndex(index);
        }
        // 读取严格早于输入位置的向后边界。
        let backward = self.grapheme_char_boundaries[forward_slot.saturating_sub(1)];
        // 根据调用方语义选择合法边界。
        CharIndex(match bias {
            // 向后选择当前字素簇起点。
            BoundaryBias::Backward => backward,
            // 向前选择当前字素簇终点。
            BoundaryBias::Forward => forward,
            // 距离相同时偏向前进，避免点击后半区仍回退。
            BoundaryBias::Nearest if index - backward < forward - index => backward,
            // 其余最近边界情况选择前方。
            BoundaryBias::Nearest => forward,
        })
    }

    /// 返回严格早于当前位置的扩展字素簇边界。
    pub fn previous_grapheme_boundary(&self, index: CharIndex) -> CharIndex {
        // 限制输入并定位第一个不小于它的边界。
        let slot = self
            // 查询有序字素簇边界表。
            .grapheme_char_boundaries
            // 严格保留早于当前位置的边界。
            .partition_point(|candidate| *candidate < index.0.min(self.char_len().0));
        // 前一个边界就是向左移动目标。
        CharIndex(self.grapheme_char_boundaries[slot.saturating_sub(1)])
    }

    /// 返回严格晚于当前位置的扩展字素簇边界。
    pub fn next_grapheme_boundary(&self, index: CharIndex) -> CharIndex {
        // 限制输入并定位第一个严格更大的边界。
        let slot = self
            // 查询有序字素簇边界表。
            .grapheme_char_boundaries
            // 跳过全部不晚于当前位置的边界。
            .partition_point(|candidate| *candidate <= index.0.min(self.char_len().0));
        // 在文本末尾收敛到最后一个边界。
        CharIndex(self.grapheme_char_boundaries[slot.min(self.grapheme_len().0)])
    }

    /// 把选择区间向外扩展到完整扩展字素簇边界。
    pub fn normalize_selection(&self, a: CharIndex, b: CharIndex) -> (CharIndex, CharIndex) {
        // 先确定无方向选择的逻辑起点。
        let start = CharIndex(a.0.min(b.0));
        // 再确定无方向选择的逻辑终点。
        let end = CharIndex(a.0.max(b.0));
        // 起点向后、终点向前，保证不会截断字素簇。
        (
            // 归一选择起点。
            self.normalize_char(start, BoundaryBias::Backward),
            // 归一选择终点。
            self.normalize_char(end, BoundaryBias::Forward),
        )
    }

    /// 把 shaping cluster 显式扩展到它接触的完整扩展字素簇区间。
    pub fn normalize_shaping_cluster(&self, cluster: ShapingCluster) -> ShapingCluster {
        // 复用选择区间的向外归一规则。
        let (start, end) = self.normalize_selection(cluster.start, cluster.end);
        // 返回仍保持 shaping cluster 身份的合法区间。
        ShapingCluster { start, end }
    }
}

// 仅在单元测试中编译索引模型契约测试。
#[cfg(test)]
// 把测试隔离在当前模块内部。
mod tests {
    // 引入全部被测索引类型。
    use super::*;

    /// 收集便于断言的全部字符边界。
    fn grapheme_boundaries(text: &str) -> Vec<usize> {
        // 建立源文本索引模型。
        let map = TextIndexMap::new(text);
        // 遍历从起点到文本末尾的全部字符位置。
        (0..=map.char_len().0)
            // 只保留合法扩展字素簇边界。
            .filter(|index| map.is_grapheme_boundary(CharIndex(*index)))
            // 固化为测试向量。
            .collect()
    }

    /// 组合音标必须与基础字母形成单一可编辑单元。
    #[test]
    // 验证 combining mark 不会产生内部光标位置。
    fn combining_mark_has_no_internal_cursor_boundary() {
        // 基础拉丁字母与组合锐音包含两个标量。
        let text = "a\u{0301}";
        // 只能在完整字素簇的两端停靠。
        assert_eq!(grapheme_boundaries(text), vec![0, 2]);
        // 建立索引模型以验证移动行为。
        let map = TextIndexMap::new(text);
        // 向右移动必须一次越过整个组合序列。
        assert_eq!(map.next_grapheme_boundary(CharIndex(0)), CharIndex(2));
        // 向左移动必须一次返回整个组合序列之前。
        assert_eq!(map.previous_grapheme_boundary(CharIndex(2)), CharIndex(0));
    }

    /// 肤色修饰符与 ZWJ emoji 必须保持为单一可编辑单元。
    #[test]
    // 验证 emoji 序列不会暴露内部标量边界。
    fn emoji_zwj_and_skin_tone_have_no_internal_cursor_boundary() {
        // 女性、肤色、ZWJ 与电脑组成四标量 emoji。
        let text = "👩🏽‍💻";
        // 只能在整个 emoji 两端停靠。
        assert_eq!(grapheme_boundaries(text), vec![0, 4]);
        // 建立索引模型以验证内部命中归一。
        let map = TextIndexMap::new(text);
        // 靠近前半区的内部位置归一到起点。
        assert_eq!(
            map.normalize_char(CharIndex(1), BoundaryBias::Nearest),
            CharIndex(0)
        );
        // 靠近后半区的内部位置归一到终点。
        assert_eq!(
            map.normalize_char(CharIndex(3), BoundaryBias::Nearest),
            CharIndex(4)
        );
    }

    /// Indic 连写必须保持标准扩展字素簇边界。
    #[test]
    // 验证天城文辅音连写不会被标量索引拆开。
    fn indic_conjunct_has_no_internal_cursor_boundary() {
        // Ka、virama 与 Ssa 组成一个连写字素簇。
        let text = "क्ष";
        // 三个标量共享同一对编辑边界。
        assert_eq!(grapheme_boundaries(text), vec![0, 3]);
        // 建立索引模型以验证选择归一。
        let map = TextIndexMap::new(text);
        // 任意内部选择必须扩展为完整连写。
        assert_eq!(
            // 归一中间标量形成的选择。
            map.normalize_selection(CharIndex(1), CharIndex(2)),
            // 期望完整覆盖三标量连写。
            (CharIndex(0), CharIndex(3))
        );
    }

    /// 混合 RTL 文本仍使用逻辑字符索引表达稳定字素簇边界。
    #[test]
    // 验证希伯来组合音标与周边 LTR 文本之间的边界。
    fn mixed_rtl_text_preserves_logical_grapheme_boundaries() {
        // 拉丁前缀、希伯来组合序列与拉丁后缀形成混合方向文本。
        let text = "Aא\u{05B7}בZ";
        // 希伯来字母与元音点共享一个逻辑字素簇。
        assert_eq!(grapheme_boundaries(text), vec![0, 1, 3, 4, 5]);
        // 建立索引模型以验证 shaping cluster 区间转换。
        let map = TextIndexMap::new(text);
        // 模拟后端只覆盖组合序列内部标量的错误簇范围。
        let cluster = ShapingCluster {
            // cluster 从希伯来基字符后的内部位置开始。
            start: CharIndex(2),
            // cluster 在组合序列后结束。
            end: CharIndex(3),
        };
        // shaping cluster 必须扩展回完整希伯来字素簇。
        assert_eq!(
            // 归一后端簇范围。
            map.normalize_shaping_cluster(cluster),
            // 期望覆盖基字符与组合点。
            ShapingCluster {
                // 合法字素簇起点。
                start: CharIndex(1),
                // 合法字素簇终点。
                end: CharIndex(3),
            }
        );
    }

    /// UTF-8 字节、字符和字素簇转换必须保持各自单位。
    #[test]
    // 验证多字节字符内部的向前与向后字符边界转换。
    fn byte_char_and_grapheme_units_convert_explicitly() {
        // ASCII、二字节字符与 emoji 共同覆盖不同 UTF-8 宽度。
        let text = "Aé👩🏽‍💻";
        // 建立显式索引模型。
        let map = TextIndexMap::new(text);
        // 三个字素簇分别覆盖一、一和四个字符。
        assert_eq!(map.grapheme_len(), GraphemeIndex(3));
        // 第二个字符边界对应三个 UTF-8 字节。
        assert_eq!(map.char_to_byte(CharIndex(2)), ByteIndex(3));
        // 落在 é 内部的字节向后收敛到字符一。
        assert_eq!(map.byte_to_char_floor(ByteIndex(2)), CharIndex(1));
        // 落在 é 内部的字节向前收敛到字符二。
        assert_eq!(map.byte_to_char_ceil(ByteIndex(2)), CharIndex(2));
        // 第三个字素簇从逻辑字符二开始。
        assert_eq!(map.grapheme_to_char(GraphemeIndex(2)), CharIndex(2));
    }
}
