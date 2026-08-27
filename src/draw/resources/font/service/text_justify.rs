// 引入带源字符身份的视觉行字形。
use super::bidi_layout::LineGlyph;
// 引入共享断行模块定义的可折叠空白语义。
use super::super::line_break::LineBreakMap;

// 判断当前视觉字形是否结束一个可扩展空白 cluster。
fn is_gap_end(
    // 借用完整视觉行字形。
    glyphs: &[LineGlyph],
    // 接收当前字形下标。
    index: usize,
) -> bool {
    // 缺失当前字形不能形成空白间隔。
    let Some(current) = glyphs.get(index) else {
        // 返回不可扩展。
        return false;
    };
    // 当前字形必须属于仍有正 advance 的可折叠空白。
    if !LineBreakMap::collapsible_whitespace(current.source_char) || current.glyph.width <= 0.0 {
        // 排除正文、非断空白与已折叠的行尾空白。
        return false;
    }
    // 行尾空白和同一 shaping cluster 的中间字形不能重复形成间隔。
    glyphs.get(index + 1).is_some_and(|next| {
        // 下一视觉字形必须属于不同源 cluster。
        next.glyph.char_index != current.glyph.char_index
    })
}

// 将有限容器剩余宽度平均分配给视觉行的有效空白间隔。
pub(super) fn justify_line(
    // 借用已经完成双向重排的视觉字形。
    glyphs: &mut [LineGlyph],
    // 接收当前视觉行自然宽度。
    natural_width: f32,
    // 接收最终有限文本容器宽度。
    container_width: f32,
    // 指示当前行是否为段落末行或显式换行前一行。
    paragraph_final: bool,
) -> f32 {
    // 段落末行、溢出行与非有限容器保持自然排版。
    if paragraph_final || !container_width.is_finite() || container_width <= natural_width {
        // 返回未经扩展的自然宽度。
        return natural_width;
    }
    // 统计可扩展的独立空白 cluster。
    let gap_count = (0..glyphs.len())
        // 只保留视觉行中的有效间隔末端。
        .filter(|index| is_gap_end(glyphs, *index))
        // 得到间隔数量。
        .count();
    // 没有词间空白时不能伪造字距拉伸。
    if gap_count == 0 {
        // 返回自然宽度并允许长词保持既有策略。
        return natural_width;
    }
    // 计算每个间隔需要增加的稳定宽度。
    let extra_per_gap = (container_width - natural_width) / gap_count as f32;
    // 保存当前字形之前已经累计的平移量。
    let mut offset = 0.0;
    // 按视觉顺序同步更新位置与空白 advance。
    for index in 0..glyphs.len() {
        // 先把前序间隔产生的扩展应用到当前字形位置。
        glyphs[index].glyph.x += offset;
        // 当前字形结束有效间隔时取得额外 advance。
        if is_gap_end(glyphs, index) {
            // 扩展空白本身，使选择和命中覆盖完整间隔。
            glyphs[index].glyph.width += extra_per_gap;
            // 后续视觉字形整体向右平移。
            offset += extra_per_gap;
        }
    }
    // 有效空白已精确填满有限容器。
    container_width
}
