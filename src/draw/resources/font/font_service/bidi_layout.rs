//! 保存 FontService 的 UAX #9 字体段切分与视觉 cluster 重排。

// 引入共享段落级 UAX #9 分析。
use crate::draw::resources::font::bidi::BidiAnalysis;
// 引入统一定位字形类型。
use crate::draw::resources::font::text_backend::PositionedGlyph;
// 引入字体句柄。
use crate::draw::FontHandle;
// 引入字符范围以描述视觉行逻辑源区间。
use std::ops::Range;
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
    // 相邻同源区间字形共同形成一个不可拆 shaping cluster。
    let mut clusters: Vec<Vec<PositionedGlyph>> = Vec::new();
    // 按后端输出顺序扫描视觉字形。
    for glyph in glyphs {
        // 查询最后一个 cluster 是否具有相同逻辑源区间。
        let same_cluster = clusters.last().is_some_and(|cluster| {
            // 首字形存在时比较完整源区间。
            cluster.first().is_some_and(|first| {
                // 起点与排他终点都相同才属于同一 cluster。
                first.char_index == glyph.char_index && first.char_end == glyph.char_end
            })
        });
        // 相同 cluster 追加到现有组。
        if same_cluster {
            // 最后一个 cluster 已由条件保证存在。
            clusters
                // 取得最后一个 cluster 的可变引用。
                .last_mut()
                // 仅在异常空数组时跳过。
                .expect("same cluster requires a previous glyph group")
                // 保留 cluster 内后端视觉字形顺序。
                .push(glyph);
        // 新源区间开始新的 cluster。
        } else {
            // 使用当前字形创建 cluster。
            clusters.push(vec![glyph]);
        }
    }
    // UAX #14 必须按逻辑源顺序观察 cluster 边界。
    clusters.sort_by_key(|cluster| {
        // 空 cluster 只可能来自异常内部状态并排序到尾部。
        cluster
            // 读取 cluster 首字形。
            .first()
            // 使用逻辑字符起点作为稳定排序键。
            .map(|glyph| glyph.char_index)
            // 空组使用最大值。
            .unwrap_or(usize::MAX)
    });
    // 重新建立单调逻辑 cluster 坐标供既有换行算法消费。
    position_clusters(clusters, None)
}

/// 按当前视觉行的 UAX #9 L1/L2 结果重排并定位 cluster。
pub(super) fn reorder_line(
    // 接收当前行逻辑顺序字形。
    glyphs: &mut Vec<LineGlyph>,
    // 指定视觉行覆盖的逻辑源范围。
    line_range: Range<usize>,
    // 借用完整段落分析。
    bidi: &BidiAnalysis,
) -> f32 {
    // 将相邻同源区间字形组合为不可拆 cluster。
    let mut clusters: Vec<Vec<LineGlyph>> = Vec::new();
    // 当前输入已经按逻辑 cluster 顺序排列。
    for glyph in std::mem::take(glyphs) {
        // 判断当前字形是否延续最后一个 cluster。
        let same_cluster = clusters.last().is_some_and(|cluster| {
            // 比较完整逻辑源区间。
            cluster.first().is_some_and(|first| {
                // 起止边界相同才属于同一 cluster。
                first.glyph.char_index == glyph.glyph.char_index
                    && first.glyph.char_end == glyph.glyph.char_end
            })
        });
        // 相同 cluster 保留内部视觉字形顺序。
        if same_cluster {
            // 最后一个 cluster 已由条件保证存在。
            clusters
                // 取得最后一个 cluster。
                .last_mut()
                // 防止异常内部状态静默丢字形。
                .expect("same cluster requires a previous line glyph group")
                // 追加当前字形。
                .push(glyph);
        // 新源区间开始新的 cluster。
        } else {
            // 使用当前字形创建新组。
            clusters.push(vec![glyph]);
        }
    }
    // 提取每个逻辑 cluster 的源字符起点。
    let logical_indices = clusters
        // 保持逻辑 cluster 顺序。
        .iter()
        // 读取每个 cluster 首字形的逻辑起点。
        .filter_map(|cluster| cluster.first().map(|glyph| glyph.glyph.char_index))
        // 收集对象索引供共享 UAX #9 分析重排。
        .collect::<Vec<_>>();
    // 从同一行级数据取得视觉顺序与对象级别。
    let order = bidi.line_order(line_range, &logical_indices);
    // 转为可按视觉映射转移所有权的可选 cluster。
    let mut available = clusters.into_iter().map(Some).collect::<Vec<_>>();
    // 准备最终视觉顺序 cluster。
    let mut visual_clusters = Vec::with_capacity(available.len());
    // 按 UAX #9 L2 视觉对象顺序消费 cluster。
    for logical_index in order.visual_to_logical {
        // 读取当前逻辑对象的行级嵌入级别。
        let level = order.levels.get(logical_index).copied().unwrap_or(0);
        // 取得对应 cluster 的唯一所有权。
        let Some(mut cluster) = available
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
        // 按视觉顺序登记当前 cluster。
        visual_clusters.push(cluster);
    }
    // 按视觉 cluster 顺序重新建立单调 x 坐标。
    let mut cursor_x = 0.0f32;
    // 预留全部行字形数量。
    let glyph_count = visual_clusters.iter().map(Vec::len).sum();
    // 保存最终视觉字形数组。
    let mut visual_glyphs = Vec::with_capacity(glyph_count);
    // 逐个视觉 cluster 定位。
    for mut cluster in visual_clusters {
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

// 将定位字形 cluster 按指定顺序重新建立连续 x 坐标。
fn position_clusters(
    // 接收已排序 cluster。
    clusters: Vec<Vec<PositionedGlyph>>,
    // 可选逐 cluster 行级嵌入级别。
    levels: Option<&[u8]>,
) -> Vec<PositionedGlyph> {
    // 预留最终字形数量。
    let glyph_count = clusters.iter().map(Vec::len).sum();
    // 保存重新定位后的字形。
    let mut positioned = Vec::with_capacity(glyph_count);
    // 从视觉或逻辑行首开始定位。
    let mut cursor_x = 0.0f32;
    // 按调用方已确定的 cluster 顺序遍历。
    for (cluster_index, mut cluster) in clusters.into_iter().enumerate() {
        // 取得 cluster 原始最小水平坐标。
        let origin = cluster
            // 遍历 cluster 字形。
            .iter()
            // 提取水平坐标。
            .map(|glyph| glyph.x)
            // 聚合最小值。
            .fold(f32::INFINITY, f32::min);
        // cluster advance 使用全部非负字形宽度之和。
        let advance = cluster
            // 遍历 cluster 字形。
            .iter()
            // 提取非负 advance。
            .map(|glyph| glyph.width.max(0.0))
            // 汇总 cluster advance。
            .sum::<f32>();
        // 取得可选行级嵌入级别。
        let level = levels
            // 借用逐 cluster 级别。
            .and_then(|values| values.get(cluster_index))
            // 复制数值级别。
            .copied();
        // 平移 cluster 内全部字形。
        for glyph in &mut cluster {
            // 保留 cluster 内 GPOS 相对位置。
            glyph.x = cursor_x + (glyph.x - origin);
            // 调用方提供级别时同步回填几何方向。
            if let Some(level) = level {
                // 保存行级嵌入级别。
                glyph.bidi_level = level;
            }
        }
        // 追加完整 cluster。
        positioned.extend(cluster);
        // 推进逻辑或视觉画笔。
        cursor_x += advance;
    }
    // 返回连续定位字形。
    positioned
}
