//! 保存 RichText 的段落级 UAX #9 视觉 run 定位。

// 引入共享段落级双向分析。
use crate::draw::resources::font::bidi::BidiAnalysis;

// 引入富文本布局字形与视觉行。
use super::{LayoutGlyph, LayoutLine};

// 描述一个保持样式与逻辑文本顺序的单向富文本 run。
struct RichBidiRun {
    // 保存 run 内按逻辑顺序排列的字符字形。
    glyphs: Vec<LayoutGlyph>,
    // 保存当前视觉行应用 L1 后的统一嵌入级别。
    level: u8,
}

/// 对已完成 UAX #14 折行的富文本行应用 UAX #9 L1/L2 与视觉定位。
pub(super) fn reorder_lines(lines: &mut [LayoutLine], bidi: &BidiAnalysis) -> f32 {
    // 保存全部视觉行中的最大实际宽度。
    let mut max_width = 0.0f32;
    // 同一富文本布局周期内复用完整文本的 unicode-bidi 分析。
    let order_context = bidi.line_order_context();
    // 每一行必须独立应用 UAX #9 L1。
    for line in lines {
        // 空行不包含可重排对象。
        if line.glyphs.is_empty() {
            // 保留空行并继续下一行。
            continue;
        }
        // 先按逻辑顺序取得每个字符对象的源索引。
        let logical_indices = line
            // 借用当前行逻辑字形。
            .glyphs
            // 遍历字形引用。
            .iter()
            // 提取完整源字符索引。
            .map(|glyph| glyph.global_char_idx)
            // 收集供共享分析应用 L1。
            .collect::<Vec<_>>();
        // 当前行逻辑起点取最小源字符索引。
        let line_start = logical_indices.iter().copied().min().unwrap_or(0);
        // 当前行逻辑排他终点取最大源索引后一位。
        let line_end = logical_indices
            // 遍历源字符索引。
            .iter()
            // 复制数值供聚合。
            .copied()
            // 取得最大源索引。
            .max()
            // 转换为排他终点。
            .map_or(line_start, |index| index + 1);
        // 当前行只执行一次 L1，字符对象与样式 run 共享结果。
        let line_order = order_context.line(line_start..line_end);
        // 先取得逐字符行级嵌入级别，以便跨样式安全切 run。
        let character_order = line_order.order(&logical_indices);
        // 取出当前行逻辑字形所有权。
        let logical_glyphs = std::mem::take(&mut line.glyphs);
        // 保存按逻辑顺序形成的样式与方向 run。
        let mut runs: Vec<RichBidiRun> = Vec::new();
        // 逐字符绑定行级嵌入级别。
        for (logical_index, mut glyph) in logical_glyphs.into_iter().enumerate() {
            // 缺少行级结果时保守使用段落级解析级别。
            let level = character_order
                // 查询当前逻辑字符对象级别。
                .levels
                // 借用对应级别。
                .get(logical_index)
                // 复制数值。
                .copied()
                // 使用段落级别安全兜底。
                .unwrap_or_else(|| bidi.level_at(glyph.global_char_idx));
            // 保存同一数据供交互几何复用。
            glyph.bidi_level = level;
            // 只有样式段和行级方向都相同才可合并为一个绘制 run。
            let continues_run = runs.last().is_some_and(|run| {
                // 末尾 run 必须有字符才能比较样式段。
                run.glyphs.last().is_some_and(|previous| {
                    // 样式段与嵌入级别必须同时一致。
                    previous.segment_idx == glyph.segment_idx && run.level == level
                })
            });
            // 相同 run 追加逻辑字符。
            if continues_run {
                // 最后一个 run 已由条件保证存在。
                let Some(run) = runs.last_mut() else {
                    // 条件与集合状态不一致时立即暴露内部不变量破坏。
                    unreachable!("continuing bidi run requires a previous run");
                };
                // 保留 run 内逻辑文本顺序并追加当前字符。
                run.glyphs.push(glyph);
            // 样式或方向变化时开始新 run。
            } else {
                // 登记新的逻辑 run。
                runs.push(RichBidiRun {
                    // 首字符作为 run 初始内容。
                    glyphs: vec![glyph],
                    // 保存统一行级嵌入级别。
                    level,
                });
            }
        }
        // 提取每个逻辑 run 的首字符索引。
        let run_indices = runs
            // 保持逻辑 run 顺序。
            .iter()
            // 读取 run 首字符源索引。
            .filter_map(|run| run.glyphs.first().map(|glyph| glyph.global_char_idx))
            // 收集供 UAX #9 L2 重排。
            .collect::<Vec<_>>();
        // 使用同一行范围取得视觉 run 顺序。
        let run_order = line_order.order(&run_indices);
        // 转为可按视觉映射转移所有权的 run 槽位。
        let mut available = runs.into_iter().map(Some).collect::<Vec<_>>();
        // 从视觉行左缘开始定位 run。
        let mut cursor_x = 0.0f32;
        // 保存最终视觉 run 顺序，run 内仍保留逻辑文本顺序。
        let mut visual_glyphs = Vec::new();
        // 按 L2 视觉顺序消费 run。
        for logical_run_index in run_order.visual_to_logical {
            // 取得对应 run 的唯一所有权。
            let Some(mut run) = available
                // 查询逻辑 run 槽位。
                .get_mut(logical_run_index)
                // 从槽位取出 run。
                .and_then(Option::take)
            else {
                // 异常映射跳过不存在 run。
                continue;
            };
            // run 宽度使用全部非负字符 advance 之和。
            let run_width = run
                // 遍历逻辑字符。
                .glyphs
                // 借用字符字形。
                .iter()
                // 提取非负 advance。
                .map(|glyph| glyph.width.max(0.0))
                // 汇总 run 宽度。
                .sum::<f32>();
            // 偶数级别按逻辑顺序从左向右定位。
            if run.level % 2 == 0 {
                // 使用局部画笔定位每个 LTR 字符。
                let mut run_x = cursor_x;
                // 遍历 run 内逻辑字符。
                for glyph in &mut run.glyphs {
                    // 当前字符从局部画笔开始。
                    glyph.x = run_x;
                    // 推进当前字符 advance。
                    run_x += glyph.width.max(0.0);
                }
            // 奇数级别保持逻辑文本顺序，但从视觉右缘反向分配字符位置。
            } else {
                // RTL run 的首个逻辑字符位于视觉右缘。
                let mut run_right = cursor_x + run_width;
                // 按逻辑字符顺序从右向左定位。
                for glyph in &mut run.glyphs {
                    // 先回退当前字符 advance。
                    run_right -= glyph.width.max(0.0);
                    // 保存当前字符视觉左缘。
                    glyph.x = run_right;
                }
            }
            // 推进到下一个视觉 run 左缘。
            cursor_x += run_width;
            // 保留 run 内逻辑文本顺序供 OpenType shaping 使用。
            visual_glyphs.extend(run.glyphs);
        }
        // 返回视觉 run 顺序与方向感知字符位置。
        line.glyphs = visual_glyphs;
        // 更新全部行最大宽度。
        max_width = max_width.max(cursor_x);
    }
    // 返回同一视觉定位数据派生的最大宽度。
    max_width
}
