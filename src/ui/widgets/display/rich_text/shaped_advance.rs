//! 将文本后端返回的 shaping cluster 几何映射为富文本字符 advance。

// 复用富文本的估算字符宽度作为缺字兜底。
use super::layout_metrics::char_width;
use super::presentation::RichTextMetricsVisual;
// 读取统一字体服务以执行无换行的真实 shaping。
use crate::draw::resources::font::font_service::FontService;
// 读取携带源字符区间的定位字形。
use crate::draw::resources::font::text_backend::{PositionedGlyph, TextLayoutOptions};
// 读取字体句柄与布局对齐枚举。
use crate::draw::{FontHandle, HAlign, VAlign};

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

// 获取文本中每个字符的真实 advance 宽度。
pub(super) fn real_char_advances(
    // 使用统一字体服务执行 shaping。
    font_service: &FontService,
    // 使用调用方选定的字体句柄。
    font: &FontHandle,
    // 接收待测量的完整文本段。
    text: &str,
    // 接收当前文本段字号。
    fs: f32,
    // 接收 UIX 声明的缺字估算比例。
    metrics: RichTextMetricsVisual,
    // 返回与源字符索引一一对应的 advance。
) -> Vec<f32> {
    // 空文本无需调用字体后端。
    if text.is_empty() {
        // 返回稳定空宽度数组。
        return Vec::new();
    }
    // 禁用换行以获得当前文本段的连续 shaping 几何。
    let options = TextLayoutOptions {
        // 使用当前文本段字号。
        font_size: fs,
        // 连续度量不施加水平宽度约束。
        max_width: f32::MAX,
        // 高度由字体自身决定。
        max_height: 0.0,
        // 使用字体服务默认行高。
        line_height: 0.0,
        // 禁止本次度量自行折行。
        word_wrap: false,
        // 从左侧原点读取原始字形位置。
        h_align: HAlign::Left,
        // 从顶部原点读取原始行位置。
        v_align: VAlign::Top,
        // 结束布局选项构造。
    };
    // 执行真实字体 shaping 与多字体回退。
    let layout = font_service.layout_text(font, text, &options);
    // 固化源字符以构建逐字符 advance 数组。
    let chars = text.chars().collect::<Vec<_>>();
    // 为每个源字符预留一个宽度槽位。
    let mut advances = Vec::with_capacity(chars.len());
    // 按源字符索引读取 shaping cluster 几何。
    for (char_index, ch) in chars.iter().enumerate() {
        // 当前字符没有可用字形时使用估算宽度保持布局可收敛。
        let fallback = char_width(fs, *ch, metrics);
        // cluster 起点消费完整 advance，后继源字符保持零宽。
        advances.push(measured_advance_for_char(
            // 传入连续 shaping 的字形列表。
            &layout.glyphs,
            // 传入当前源字符索引。
            char_index,
            // 传入缺字估算宽度。
            fallback,
        ));
    }
    // 返回逐字符真实 advance。
    advances
}
