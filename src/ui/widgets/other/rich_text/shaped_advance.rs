//! 将文本后端返回的 shaping cluster 几何映射为富文本字符 advance。

// 读取携带源字符区间的定位字形。
use crate::draw::resources::font::text_backend::PositionedGlyph;

// 根据后端字形的源字符索引读取真实 advance，避免 glyph 槽位缺失时错配宽度。
pub(super) fn measured_advance_for_char(
    // 接收视觉顺序的后端定位字形。
    glyphs: &[PositionedGlyph],
    // 接收待查询的源字符索引。
    char_index: usize,
    // 接收缺字或异常几何时的估算宽度。
    fallback: f32,
    // 返回目标源字符应消费的水平 advance。
) -> f32 {
    // 查找覆盖目标字符的完整 shaping cluster，而不是假设一字符一字形。
    let mut cluster = glyphs
        // 遍历后端返回的视觉顺序字形。
        .iter()
        // 选择源区间覆盖目标字符的字形。
        .filter(|glyph| glyph.char_index <= char_index && char_index < glyph.char_end);
    // 第一个匹配字形确定 cluster 的源区间与视觉锚点。
    let Some(glyph) = cluster.next() else {
        // 缺字时回退到估算宽度，保持布局可以继续收敛。
        return fallback;
    };
    // cluster 内后续源字符不重复消费 advance，避免 ligature 总宽度膨胀。
    if char_index > glyph.char_index {
        // 零 advance 让后续字符与 cluster 起点共享几何。
        return 0.0;
    }
    // 聚合 cluster 内全部字形的视觉左右边界。
    let (cluster_left, cluster_right) = std::iter::once(glyph)
        // 拼接剩余同 cluster 字形。
        .chain(cluster)
        // 从无界哨兵开始折叠可见范围。
        .fold((f32::INFINITY, f32::NEG_INFINITY), |(left, right), item| {
            // 同时更新最小 x 与最大 advance 右缘。
            (left.min(item.x), right.max(item.x + item.width.max(0.0)))
        });
    // 相邻逻辑 cluster 的视觉起点差包含 kerning 修正。
    let positioned_advance = glyphs
        // 遍历全部字形寻找下一个逻辑 cluster。
        .iter()
        // 下一个 cluster 从当前排他终点开始。
        .find(|next_glyph| next_glyph.char_index == glyph.char_end)
        // 读取相邻视觉起点差。
        .map(|next_glyph| next_glyph.x - cluster_left)
        // 只接受有限非负间距，避免异常字体坐标污染布局。
        .filter(|advance| advance.is_finite() && *advance >= 0.0);
    // 没有可用相邻起点时使用完整 cluster 的视觉 advance。
    positioned_advance
        // 只接受有限且有序的 cluster 几何。
        .or_else(|| {
            // 将完整 cluster 右缘差作为总 advance。
            (cluster_left.is_finite() && cluster_right.is_finite() && cluster_right >= cluster_left)
                // 返回可用的非负宽度。
                .then_some(cluster_right - cluster_left)
        })
        // 所有真实几何都异常时回退估算宽度。
        .unwrap_or(fallback)
}
