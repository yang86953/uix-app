//! 为普通文本与富文本布局提供共享的段落级 UAX #9 分析。

// 引入字符范围以描述视觉行对应的逻辑源区间。
use std::ops::Range;
// 引入 UAX #9 的完整段落分析与 L2 重排实现。
use unicode_bidi::BidiInfo;

/// 描述一个段落在完整源文本中的逻辑字符范围与基准级别。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BidiParagraph {
    // 保存段落的逻辑字符范围。
    pub char_range: Range<usize>,
    // 保存段落基准嵌入级别。
    pub base_level: u8,
}

/// 描述一组逻辑对象经过 UAX #9 L1/L2 后的视觉顺序。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BidiLineOrder {
    // 以视觉顺序保存输入逻辑对象的下标。
    pub visual_to_logical: Vec<usize>,
    // 按输入逻辑对象顺序保存应用 L1 后的嵌入级别。
    pub levels: Vec<u8>,
}

/// 保存可跨字体与样式分段复用的段落级 UAX #9 结果。
#[derive(Debug, Clone)]
pub(crate) struct BidiAnalysis {
    // 保存原始完整源文本以便视觉行应用 UAX #9 L1。
    text: String,
    // 保存每个逻辑字符起点及文本尾部的 UTF-8 字节偏移。
    char_bytes: Vec<usize>,
    // 保存每个逻辑字符的段落级解析嵌入级别。
    levels: Vec<u8>,
    // 保存全部段落的逻辑范围与基准级别。
    paragraphs: Vec<BidiParagraph>,
}

// 为共享双向分析实现构造、查询与视觉行重排。
impl BidiAnalysis {
    /// 对完整源文本执行段落级 UAX #9 分析。
    pub(crate) fn new(text: &str) -> Self {
        // 使用首个强字符自动确定每个段落的基准方向。
        let info = BidiInfo::new(text, None);
        // 收集每个 Unicode 标量的 UTF-8 字节起点。
        let mut char_bytes = text
            // 遍历稳定的字符边界。
            .char_indices()
            // 只保存字节起点。
            .map(|(byte_index, _)| byte_index)
            // 收集为可随机访问的边界表。
            .collect::<Vec<_>>();
        // 追加文本尾部作为排他字符终点。
        char_bytes.push(text.len());
        // 将 unicode-bidi 的逐字节级别收敛为逐字符级别。
        let levels = char_bytes
            // 尾部边界不对应真实字符。
            .iter()
            // 丢弃最后一个文本尾部哨兵。
            .take(char_bytes.len().saturating_sub(1))
            // 在每个字符首字节读取已解析级别。
            .map(|byte_index| info.levels[*byte_index].number())
            // 保存为不泄漏第三方类型的数值级别。
            .collect::<Vec<_>>();
        // 将段落字节范围转换为统一的逻辑字符范围。
        let paragraphs = info
            // 遍历完整 UAX #9 段落结果。
            .paragraphs
            // 借用结果以保留后续字段读取。
            .iter()
            // 构造公开给布局层的稳定段落描述。
            .map(|paragraph| BidiParagraph {
                // 将段落字节起止边界转换为字符边界。
                char_range: Self::byte_range_to_char_range(&char_bytes, paragraph.range.clone()),
                // 保存自动解析出的段落基准级别。
                base_level: paragraph.level.number(),
            })
            // 收集全部段落。
            .collect::<Vec<_>>();
        // 返回可在一次布局期间共享的分析结果。
        Self {
            // 自有完整文本避免跨调用生命周期耦合。
            text: text.to_owned(),
            // 保存字符到字节边界表。
            char_bytes,
            // 保存逐字符解析级别。
            levels,
            // 保存段落范围。
            paragraphs,
        }
    }

    /// 返回完整源文本中的逻辑字符数量。
    pub(crate) fn char_count(&self) -> usize {
        // 逐字符级别与源字符一一对应。
        self.levels.len()
    }

    /// 返回全部段落及其自动解析的基准级别。
    pub(crate) fn paragraphs(&self) -> &[BidiParagraph] {
        // 只读借用稳定段落表。
        &self.paragraphs
    }

    /// 返回指定逻辑字符的段落级嵌入级别。
    pub(crate) fn level_at(&self, char_index: usize) -> u8 {
        // 越界查询使用最接近段落或默认 LTR 的稳定级别。
        self.levels
            // 首选精确字符级别。
            .get(char_index)
            // 文本尾部沿用最后一个字符级别。
            .copied()
            // 空文本或异常越界收敛为 LTR 零级。
            .unwrap_or_else(|| self.levels.last().copied().unwrap_or(0))
    }

    /// 按逻辑字符索引输入对象，返回应用 L1/L2 后的视觉顺序与级别。
    pub(crate) fn line_order(
        // 借用共享段落分析。
        &self,
        // 当前视觉行覆盖的逻辑字符范围。
        line_range: Range<usize>,
        // 每个待排对象对应的逻辑字符起点，必须按逻辑顺序输入。
        logical_char_indices: &[usize],
    ) -> BidiLineOrder {
        // 空对象无需执行第三方算法。
        if logical_char_indices.is_empty() {
            // 返回稳定空映射。
            return BidiLineOrder {
                // 空输入没有视觉对象。
                visual_to_logical: Vec::new(),
                // 空输入没有嵌入级别。
                levels: Vec::new(),
            };
        }
        // 将调用方范围限制在真实字符边界内。
        let bounded_start = line_range.start.min(self.char_count());
        // 排他终点不得早于起点。
        let bounded_end = line_range.end.min(self.char_count()).max(bounded_start);
        // 查找完整覆盖当前视觉行的原始段落。
        let paragraph_index = self.paragraphs.iter().position(|paragraph| {
            // 行起点与终点必须落在同一段落范围内。
            bounded_start >= paragraph.char_range.start && bounded_end <= paragraph.char_range.end
        });
        // 缺少合法段落时使用段落级解析结果保持安全退化。
        let Some(paragraph_index) = paragraph_index else {
            // 返回不改变逻辑顺序的稳定结果。
            return self.identity_order(logical_char_indices);
        };
        // 为当前视觉行重新应用 UAX #9 L1，避免复用段落尾部空白级别。
        let info = BidiInfo::new(&self.text, None);
        // 使用与自有段落表相同的顺序取得第三方段落对象。
        let Some(paragraph) = info.paragraphs.get(paragraph_index) else {
            // 第三方结果不一致时保守退化为逻辑顺序。
            return self.identity_order(logical_char_indices);
        };
        // 将逻辑字符范围转换为 unicode-bidi 所需的 UTF-8 字节范围。
        let byte_range = self.char_bytes[bounded_start]..self.char_bytes[bounded_end];
        // 应用 UAX #9 L1 得到适用于当前视觉行的逐字节级别。
        let line_levels = info.reordered_levels(paragraph, byte_range);
        // 为每个布局对象读取其逻辑字符对应的行级别。
        let object_levels = logical_char_indices
            // 保持输入逻辑对象顺序。
            .iter()
            // 将对象逻辑字符起点映射为行级别。
            .map(|char_index| {
                // 防止异常对象索引越过文本尾部。
                let bounded_index = (*char_index).min(self.char_count().saturating_sub(1));
                // 从对象字符首字节读取 L1 后的级别。
                line_levels[self.char_bytes[bounded_index]]
            })
            // 收集第三方级别供 L2 重排。
            .collect::<Vec<_>>();
        // 使用 UAX #9 L2 生成视觉对象到逻辑对象的索引映射。
        let visual_to_logical = BidiInfo::reorder_visual(&object_levels);
        // 将第三方级别转换为稳定数值表示。
        let levels = object_levels
            // 消费临时级别数组。
            .into_iter()
            // 只保留嵌入级别数值。
            .map(|level| level.number())
            // 收集为布局层可复用数组。
            .collect::<Vec<_>>();
        // 返回同一份行级数据派生的顺序与方向。
        BidiLineOrder {
            // 保存视觉到逻辑对象映射。
            visual_to_logical,
            // 保存逻辑对象的行级嵌入级别。
            levels,
        }
    }

    // 将字节范围转换为 chars() 序的排他字符范围。
    fn byte_range_to_char_range(char_bytes: &[usize], byte_range: Range<usize>) -> Range<usize> {
        // 字节起点之前的全部字符边界数量就是字符起点。
        let char_start = char_bytes.partition_point(|byte| *byte < byte_range.start);
        // 字节终点之前的全部字符边界数量包含文本尾部哨兵，需要限制。
        let char_end = char_bytes
            // 找出严格早于字节终点的字符起点数量。
            .partition_point(|byte| *byte < byte_range.end)
            // 排他终点不能超过真实字符数量。
            .min(char_bytes.len().saturating_sub(1));
        // 返回稳定逻辑字符范围。
        char_start..char_end
    }

    // 构造不改变输入逻辑顺序的安全退化结果。
    fn identity_order(&self, logical_char_indices: &[usize]) -> BidiLineOrder {
        // 输入位置本身就是视觉到逻辑映射。
        let visual_to_logical = (0..logical_char_indices.len()).collect::<Vec<_>>();
        // 仍保留段落级方向供 shaping 与几何使用。
        let levels = logical_char_indices
            // 遍历每个逻辑对象字符起点。
            .iter()
            // 查询稳定段落级别。
            .map(|char_index| self.level_at(*char_index))
            // 收集对象级别。
            .collect::<Vec<_>>();
        // 返回安全退化映射。
        BidiLineOrder {
            // 保存恒等视觉顺序。
            visual_to_logical,
            // 保存可用方向级别。
            levels,
        }
    }
}

// 使用纯内存字符串验证段落级 UAX #9 数据契约。
#[cfg(test)]
mod tests {
    // 引入被测共享分析类型。
    use super::BidiAnalysis;

    // 验证每个段落独立解析首个强字符形成基准级别。
    #[test]
    fn paragraph_base_levels_follow_first_strong_character() {
        // 同时构造 LTR 与 RTL 两个段落。
        let analysis = BidiAnalysis::new("abc\nאבג");
        // 两个显式段落必须分别登记。
        assert_eq!(analysis.paragraphs().len(), 2);
        // Latin 段落基准级别必须为偶数零级。
        assert_eq!(analysis.paragraphs()[0].base_level, 0);
        // Hebrew 段落基准级别必须为奇数一级。
        assert_eq!(analysis.paragraphs()[1].base_level, 1);
    }

    // 验证 LTR 段落中的 RTL 文本和数字按解析级别形成视觉顺序。
    #[test]
    fn mixed_text_preserves_number_order_and_reverses_rtl_letters() {
        // 构造包含 Latin、Hebrew、空格与欧洲数字的混排行。
        let text = "abc אבג 123";
        // 执行完整段落分析。
        let analysis = BidiAnalysis::new(text);
        // 每个字符作为一个逻辑布局对象输入。
        let logical_indices = (0..text.chars().count()).collect::<Vec<_>>();
        // 取得完整单行的视觉映射。
        let order = analysis.line_order(0..logical_indices.len(), &logical_indices);
        // 按视觉顺序重建仅用于断言的字符序列。
        let logical_chars = text.chars().collect::<Vec<_>>();
        // 应用视觉到逻辑映射。
        let visual_text = order
            // 遍历视觉对象位置。
            .visual_to_logical
            // 消费映射以便直接索引逻辑字符。
            .iter()
            // 读取对应逻辑字符。
            .map(|logical_index| logical_chars[*logical_index])
            // 收集为可读视觉字符串。
            .collect::<String>();
        // 数字内部保持 LTR，Hebrew 字母按视觉顺序反向。
        assert_eq!(visual_text, "abc 123 גבא");
        // 数字段落内解析为偶数级，而 Hebrew 文本保持奇数级。
        assert!(order.levels[8..11].iter().all(|level| level % 2 == 0));
    }

    // 验证视觉行尾空白通过 L1 重置到段落基准级别。
    #[test]
    fn trailing_neutral_uses_paragraph_level_for_line_reordering() {
        // RTL 段落末尾保留一个中性空格。
        let text = "אבג ";
        // 执行段落分析。
        let analysis = BidiAnalysis::new(text);
        // 每个字符作为独立逻辑对象。
        let logical_indices = (0..text.chars().count()).collect::<Vec<_>>();
        // 对完整视觉行执行 L1 与 L2。
        let order = analysis.line_order(0..logical_indices.len(), &logical_indices);
        // 行尾空格必须重置为 RTL 段落的一级。
        assert_eq!(order.levels[3], 1);
        // RTL 行中的逻辑尾随空格位于最左侧视觉位置。
        assert_eq!(order.visual_to_logical[0], 3);
    }
}
