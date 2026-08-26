//! 保存 FontService 的 UAX #9 字体段切分与视觉 cluster 重排。

// 引入共享段落级 UAX #9 分析。
use crate::draw::resources::font::bidi::{BidiAnalysis, BidiLineOrderLine};
// 引入统一定位字形类型。
use crate::draw::resources::font::text_backend::PositionedGlyph;
// 引入字体句柄。
use crate::draw::FontHandle;
// 引入扩展字素簇边界，避免方向切分拆开组合文本。
use unicode_segmentation::UnicodeSegmentation;

// 引入原始字体回退段。
use super::font_layout::FontSegment;

/// 描述同时具有单一字体与单一 UAX #9 嵌入方向的逻辑段。
pub(super) struct BidiFontSegment {
    // 保存 UTF-8 字节起点。
    pub(super) byte_start: usize,
    // 保存 UTF-8 排他字节终点。
    pub(super) byte_end: usize,
    // 保存当前段使用的字体句柄。
    pub(super) font: FontHandle,
    // 保存强制换行字符数量。
    pub(super) line_break_chars: usize,
    // 保存段落级解析嵌入级别。
    pub(super) bidi_level: u8,
}

/// 将布局字形与 UAX #14 查询所需的代表源字符绑定。
#[derive(Clone, Copy)]
pub(super) struct LineGlyph {
    // 保存统一定位字形。
    pub(super) glyph: PositionedGlyph,
    // 保存 cluster 起点处的代表源字符。
    pub(super) source_char: char,
}

/// 在扩展字素簇边界上把字体回退段继续切分为单向 shaping 段。
pub(super) fn split_font_segments(
    // 借用逻辑顺序字体回退段。
    segments: &[FontSegment],
    // 借用完整源文本。
    text: &str,
    // 借用完整段落分析。
    bidi: &BidiAnalysis,
) -> Vec<BidiFontSegment> {
    // 预留与原字体段数量接近的结果容量。
    let mut directional_segments = Vec::with_capacity(segments.len());
    // 按逻辑顺序遍历字体回退段。
    for segment in segments {
        // 强制换行不参与 shaping，但必须保留逻辑字符偏移。
        if segment.line_break_chars > 0 {
            // 原样登记强制换行段。
            directional_segments.push(BidiFontSegment {
                // 保留换行字节起点。
                byte_start: segment.byte_start,
                // 保留换行字节终点。
                byte_end: segment.byte_end,
                // 保留主字体句柄占位。
                font: segment.font,
                // 保留 CR、LF 或 CRLF 字符数量。
                line_break_chars: segment.line_break_chars,
                // 强制换行不形成可见方向 run。
                bidi_level: 0,
            });
            // 继续处理下一个普通字体段。
            continue;
        }
        // 空字体段无需形成 shaping run。
        if segment.byte_start >= segment.byte_end {
            // 跳过异常空区间。
            continue;
        }
        // 借用当前字体段的源文本。
        let segment_text = &text[segment.byte_start..segment.byte_end];
        // 计算字体段起点的全局逻辑字符索引。
        let global_char_start = text[..segment.byte_start].chars().count();
        // 当前单向 run 从字体段局部字节零开始。
        let mut run_byte_start = 0usize;
        // 当前字素簇相对于字体段的逻辑字符起点。
        let mut local_char_start = 0usize;
        // 首个字符级别作为首个单向 run 的级别。
        let mut run_level = bidi.level_at(global_char_start);
        // 只在完整扩展字素簇边界观察方向变化。
        for (grapheme_byte, grapheme) in segment_text.grapheme_indices(true) {
            // 查询当前字素簇首字符的段落级解析级别。
            let grapheme_level = bidi.level_at(global_char_start + local_char_start);
            // 级别变化时在当前完整字素簇之前结束旧 run。
            if grapheme_byte > run_byte_start && grapheme_level != run_level {
                // 登记旧单向字体段。
                directional_segments.push(BidiFontSegment {
                    // 将局部字节起点提升到完整文本。
                    byte_start: segment.byte_start + run_byte_start,
                    // 当前字素簇起点就是旧段排他终点。
                    byte_end: segment.byte_start + grapheme_byte,
                    // 保留字体回退选择。
                    font: segment.font,
                    // 普通 shaping 段不含强制换行。
                    line_break_chars: 0,
                    // 保存旧段统一嵌入级别。
                    bidi_level: run_level,
                });
                // 新 run 从当前字素簇起点开始。
                run_byte_start = grapheme_byte;
                // 新 run 使用当前字素簇解析级别。
                run_level = grapheme_level;
            }
            // 推进当前字素簇消费的逻辑字符数量。
            local_char_start += grapheme.chars().count();
        }
        // 登记字体段尾部剩余的最后一个单向 run。
        directional_segments.push(BidiFontSegment {
            // 将局部 run 起点提升到完整文本。
            byte_start: segment.byte_start + run_byte_start,
            // 继承字体段排他终点。
            byte_end: segment.byte_end,
            // 保留字体回退选择。
            font: segment.font,
            // 普通 shaping 段不含强制换行。
            line_break_chars: 0,
            // 保存尾部 run 的统一嵌入级别。
            bidi_level: run_level,
        });
    }
    // 返回不拆字素簇的单向字体段。
    directional_segments
}

/// 将后端视觉输出恢复为逻辑 cluster 顺序，供 UAX #14 决定视觉行边界。
pub(super) fn logical_cluster_order(glyphs: Vec<PositionedGlyph>) -> Vec<PositionedGlyph> {
    // 稳定按逻辑字符起点排序，保留相同起点字形的原有视觉顺序。
    let mut glyphs = glyphs;
    glyphs.sort_by_key(|glyph| glyph.char_index);
    // 逐个完整源区间扫描 cluster，避免为每个 cluster 创建子 Vec。
    let mut cluster_start = 0usize;
    // 从逻辑行首重新建立连续水平坐标。
    let mut cursor_x = 0.0f32;
    while cluster_start < glyphs.len() {
        // 完整起止区间共同决定不可拆分的 shaping cluster。
        let cluster_char_index = glyphs[cluster_start].char_index;
        let cluster_char_end = glyphs[cluster_start].char_end;
        // 找到当前完整源区间的排他位置。
        let mut cluster_end = cluster_start + 1;
        while cluster_end < glyphs.len()
            && glyphs[cluster_end].char_index == cluster_char_index
            && glyphs[cluster_end].char_end == cluster_char_end
        {
            cluster_end += 1;
        }
        // 聚合 cluster 原始几何，保留 GPOS 相对偏移。
        let origin = glyphs[cluster_start..cluster_end]
            .iter()
            .map(|glyph| glyph.x)
            .fold(f32::INFINITY, f32::min);
        let advance = glyphs[cluster_start..cluster_end]
            .iter()
            .map(|glyph| glyph.width.max(0.0))
            .sum::<f32>();
        // 原地平移，直接复用后端返回的字形缓冲区。
        for glyph in &mut glyphs[cluster_start..cluster_end] {
            glyph.x = cursor_x + (glyph.x - origin);
        }
        // 推进到下一个逻辑 cluster。
        cursor_x += advance;
        cluster_start = cluster_end;
    }
    // 返回已按逻辑 cluster 排序并重新定位的扁平字形数组。
    glyphs
}

/// 按当前视觉行的 UAX #9 L1/L2 结果重排并定位 cluster。
pub(super) fn reorder_line(
    // 接收当前行逻辑顺序字形。
    glyphs: &mut Vec<LineGlyph>,
    // 复用当前布局周期已经准备好的视觉行级双向结果。
    bidi: &BidiLineOrderLine<'_>,
) -> f32 {
    // 将相邻同源区间字形组合为不可拆 cluster。
    let mut clusters: Vec<Option<Vec<LineGlyph>>> = Vec::new();
    // 当前输入已经按逻辑 cluster 顺序排列。
    for glyph in std::mem::take(glyphs) {
        // 判断当前字形是否延续最后一个 cluster。
        let same_cluster = clusters
            .last()
            .and_then(|cluster| cluster.as_ref())
            .is_some_and(|cluster| {
                // 比较完整逻辑源区间。
                cluster.first().is_some_and(|first| {
                    // 起止边界相同才属于同一 cluster。
                    first.glyph.char_index == glyph.glyph.char_index
                        && first.glyph.char_end == glyph.glyph.char_end
                })
            });
        // 相同 cluster 保留内部视觉字形顺序。
        if same_cluster {
            // 最后一个 cluster 已由 same_cluster 条件保证存在；缺失说明内部状态被破坏。
            let Some(Some(last)) = clusters.last_mut() else {
                // 附带当前 cluster 数与源区间，便于定位异常输入。
                panic!(
                    "same cluster requires a previous line glyph group (clusters={}, char_range={}..{})",
                    clusters.len(),
                    glyph.glyph.char_index,
                    glyph.glyph.char_end
                );
            };
            // 追加当前字形。
            last.push(glyph);
        // 新源区间开始新的 cluster。
        } else {
            // 使用当前字形创建新组。
            clusters.push(Some(vec![glyph]));
        }
    }
    // 提取每个逻辑 cluster 的源字符起点。
    let logical_indices = clusters
        // 保持逻辑 cluster 顺序。
        .iter()
        // 读取每个 cluster 首字形的逻辑起点。
        .filter_map(|cluster| {
            cluster
                .as_ref()
                .and_then(|cluster| cluster.first())
                .map(|glyph| glyph.glyph.char_index)
        })
        // 收集对象索引供共享 UAX #9 分析重排。
        .collect::<Vec<_>>();
    // 从同一行级数据取得视觉顺序与对象级别。
    let order = bidi.order(&logical_indices);
    // 预留全部行字形数量，避免视觉顺序消费时扩容。
    let glyph_count = clusters
        .iter()
        .map(|cluster| cluster.as_ref().map_or(0, Vec::len))
        .sum();
    // 保存最终视觉字形数组。
    let mut visual_glyphs = Vec::with_capacity(glyph_count);
    // 按视觉 cluster 顺序重新建立单调 x 坐标。
    let mut cursor_x = 0.0f32;
    // 按 UAX #9 L2 视觉对象顺序消费 cluster。
    for logical_index in order.visual_to_logical {
        // 读取当前逻辑对象的行级嵌入级别。
        let level = order.levels.get(logical_index).copied().unwrap_or(0);
        // 取得对应 cluster 的唯一所有权。
        let Some(mut cluster) = clusters
            // 查找逻辑对象槽位。
            .get_mut(logical_index)
            // 从槽位取出 cluster。
            .and_then(Option::take)
        else {
            // 异常映射跳过不存在对象，避免重复字形。
            continue;
        };
        // 同一 cluster 的全部字形共享行级方向。
        for glyph in &mut cluster {
            // 保存奇偶级别供选择、命中与光标几何复用。
            glyph.glyph.bidi_level = level;
        }
        // 取得 cluster 原始最小水平坐标。
        let origin = cluster
            // 遍历 cluster 内字形。
            .iter()
            // 提取水平坐标。
            .map(|glyph| glyph.glyph.x)
            // 聚合有限最小值。
            .fold(f32::INFINITY, f32::min);
        // cluster advance 使用全部字形非负宽度之和。
        let advance = cluster
            // 遍历 cluster 内字形。
            .iter()
            // 提取非负布局 advance。
            .map(|glyph| glyph.glyph.width.max(0.0))
            // 汇总 cluster advance。
            .sum::<f32>();
        // 平移 cluster 内字形到当前视觉画笔。
        for glyph in &mut cluster {
            // 保留 cluster 内 GPOS 相对偏移。
            glyph.glyph.x = cursor_x + (glyph.glyph.x - origin);
        }
        // 追加完整 cluster 字形。
        visual_glyphs.extend(cluster);
        // 推进到下一个视觉 cluster。
        cursor_x += advance;
    }
    // 返回视觉字形所有权给调用方。
    *glyphs = visual_glyphs;
    // 返回同一视觉数据计算出的行宽。
    cursor_x
}
