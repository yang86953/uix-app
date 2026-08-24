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

/// 无需建立索引表，把一次性字符位置归一到扩展字素簇边界。
pub(crate) fn normalize_char_in_text(
    text: &str,
    index: CharIndex,
    bias: BoundaryBias,
) -> CharIndex {
    // 保存目标位置；越界输入在遍历结束后统一收敛到文本末尾。
    let index = index.0;
    // 当前字素簇的字符起点。
    let mut backward = 0usize;
    // 流式寻找首个包住目标位置的完整字素簇。
    for grapheme in text.graphemes(true) {
        // 当前字素簇的排他字符终点。
        let forward = backward + grapheme.chars().count();
        // 精确边界无需应用偏向。
        if index == backward {
            return CharIndex(backward);
        }
        // 内部位置按既有 TextIndexMap 规则选择两侧边界。
        if index < forward {
            return CharIndex(match bias {
                BoundaryBias::Backward => backward,
                BoundaryBias::Forward => forward,
                BoundaryBias::Nearest if index - backward < forward - index => backward,
                BoundaryBias::Nearest => forward,
            });
        }
        // 推进到下一字素簇。
        backward = forward;
    }
    // 文本末尾和全部越界位置都收敛到最后边界。
    CharIndex(backward)
}

/// 无需建立索引表，把一次性选择区间向外归一到完整扩展字素簇边界。
pub(crate) fn normalize_selection_in_text(
    text: &str,
    a: CharIndex,
    b: CharIndex,
) -> (CharIndex, CharIndex) {
    // 先消除选择方向，越界值在遍历结束后统一收敛。
    let mut start = a.0.min(b.0);
    // 保存逻辑排他终点。
    let mut end = a.0.max(b.0);
    // 逐个累计扩展字素簇包含的 Unicode 标量数量。
    let mut boundary = 0usize;
    // 流式遍历避免为一次绘制建立两套边界向量。
    for grapheme in text.graphemes(true) {
        // 下一个位置是当前字素簇的排他字符终点。
        let next = boundary + grapheme.chars().count();
        // 内部起点必须向后扩展到当前字素簇起点。
        if boundary < start && start < next {
            start = boundary;
        }
        // 内部终点必须向前扩展到当前字素簇终点。
        if boundary < end && end < next {
            end = next;
        }
        // 推进到下一个字素簇。
        boundary = next;
    }
    // 越界调用方输入保持与 TextIndexMap 相同的文本末尾收敛语义。
    (CharIndex(start.min(boundary)), CharIndex(end.min(boundary)))
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
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../tests/unit/draw/resources/font/text_index__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
