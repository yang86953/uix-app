//! OpenType shaping 到 UIX 定位字形的内部适配层。

// 引入字体句柄，保持 shaping 结果可直接进入既有光栅缓存。
use crate::draw::FontHandle;
// 引入后端公开布局数据，不向上层暴露 rustybuzz 类型。
use crate::draw::resources::font::text_backend::{
    // 引入行信息类型。
    LineInfo,
    // 引入定位字形类型。
    PositionedGlyph,
    // 引入 UAX #9 已解析的 shaping 方向。
    TextDirection,
    // 引入文本布局聚合类型。
    TextLayout,
    // 引入本次 shaping 的约束类型。
    TextLayoutOptions,
    // 结束布局类型导入。
};

// 将 UTF-8 字节 cluster 转换为源文本 Unicode 标量区间。
fn cluster_char_range(
    // 原始文本用于验证 UTF-8 边界并计算字符下标。
    text: &str,
    // 排序去重后的 cluster 字节起点。
    cluster_starts: &[usize],
    // 当前字形所属的 cluster 字节起点。
    cluster: usize,
    // 返回排他的字符区间，异常字体数据返回空值。
) -> Option<(usize, usize)> {
    // cluster 必须落在合法 UTF-8 字符边界上。
    if cluster > text.len() || !text.is_char_boundary(cluster) {
        // 拒绝传播异常字体返回的无效源索引。
        return None;
        // 结束无效 cluster 分支。
    }
    // 查找严格大于当前起点的下一个 cluster。
    let next_index = cluster_starts.partition_point(|start| *start <= cluster);
    // 文本尾部作为最后一个 cluster 的排他终点。
    let end_byte = cluster_starts
        .get(next_index)
        .copied()
        .unwrap_or(text.len());
    // 下一个起点也必须是合法边界且不能倒退。
    if end_byte < cluster || !text.is_char_boundary(end_byte) {
        // 拒绝形成逆序或半个 UTF-8 标量的区间。
        return None;
        // 结束无效终点分支。
    }
    // 将字节起点转换为 chars() 序号。
    let char_start = text[..cluster].chars().count();
    // 将字节终点转换为 chars() 排他序号。
    let char_end = text[..end_byte].chars().count();
    // 有内容的 glyph cluster 至少覆盖一个源字符。
    let char_end = char_end.max(char_start.saturating_add(1).min(text.chars().count()));
    // 返回稳定的源字符区间。
    Some((char_start, char_end))
    // 结束 cluster 区间转换。
}

// 将当前行的字形范围登记为统一 LineInfo。
fn push_line(
    // 接收已完成的行列表。
    lines: &mut Vec<LineInfo>,
    // 当前行在全局字形数组中的起点。
    glyph_start: usize,
    // 当前行结束后的全局字形数量。
    glyph_end: usize,
    // 当前行顶部坐标。
    y: f32,
    // 当前行统一高度。
    line_height: f32,
    // 当前行实际 advance 宽度。
    width: f32,
    // 当前行最小逻辑字符起点。
    char_start: usize,
    // 当前行最大逻辑字符终点。
    char_end: usize,
    // 行登记没有额外返回值。
) {
    // 只登记非空字形范围，空文本由调用方单独处理。
    if glyph_end > glyph_start {
        // 写入与字形范围一一对应的行信息。
        lines.push(LineInfo {
            // 保存行顶部。
            y,
            // 保存行盒高度。
            height: line_height,
            // 保存有限非负宽度。
            width: width.max(0.0),
            // 保存最小逻辑源索引。
            start_char: char_start,
            // 保存最大排他逻辑源索引。
            end_char: char_end,
            // 保存全局字形起点。
            glyph_start,
            // 保存本行字形数量。
            glyph_count: glyph_end - glyph_start,
            // 结束行信息构造。
        });
        // 结束非空行登记。
    }
    // 结束行登记辅助函数。
}

// 对单一字体段执行 OpenType shaping，并保留源文本 cluster。
#[allow(
    // 参数来自一个完整字体布局调用，拆散会破坏边界语义。
    clippy::too_many_arguments,
    // 记录保留完整调用上下文的原因。
    reason = "one shaping pass needs font bytes, metrics, handle, text, and constraints"
// 结束 lint 配置。
)]
pub(super) fn layout_text(
    // 完整字体文件字节，支持 TTF 以及集合字体。
    data: &[u8],
    // 集合字体中的字体面编号。
    face_index: u32,
    // 字形光栅化使用的稳定字体句柄。
    font: FontHandle,
    // 当前单字体段的原始文本。
    text: &str,
    // 像素字号、宽高与换行约束。
    opts: &TextLayoutOptions,
    // 与 ab_glyph 光栅化一致的字体设计单位到像素缩放。
    glyph_scale: f32,
    // ab_glyph 计算的像素 ascent。
    ascent: f32,
    // ab_glyph 计算的实际字体高度。
    font_height: f32,
    // 调用方解析后的行高。
    line_height: f32,
    // 可选的 UAX #9 已解析方向；空值保留独立后端自动推断。
    direction: Option<TextDirection>,
    // 解析或索引异常时返回空值以启用旧后端回退。
) -> Option<TextLayout> {
    // 控制换行与制表符继续交给已有兼容路径处理。
    if text.chars().any(|ch| matches!(ch, '\r' | '\n' | '\t')) {
        // 避免 rustybuzz 把布局控制符当成普通缺字。
        return None;
        // 结束控制字符回退分支。
    }
    // 从完整字体文件创建只借用数据的 OpenType 字体面。
    let face = rustybuzz::Face::from_slice(data, face_index)?;
    // 布局推进必须与实际字形光栅使用同一缩放，避免字体高度与 UPEM
    // 不相等时字形位置比可见轮廓更宽。
    if !glyph_scale.is_finite() || glyph_scale <= 0.0 {
        // 返回空值触发有界字号兼容路径。
        return None;
        // 结束无效缩放分支。
    }
    // 创建 UTF-8 shaping 输入缓冲区。
    let mut buffer = rustybuzz::UnicodeBuffer::new();
    // 保留 push_str 自动生成的 UTF-8 字节 cluster。
    buffer.push_str(text);
    // 由脚本首个强字符推断脚本、语言与方向。
    buffer.guess_segment_properties();
    // FontService 提供段落级解析方向时覆盖局部首强字符猜测。
    if let Some(direction) = direction {
        // 将内部稳定方向映射为 rustybuzz 缓冲区方向。
        buffer.set_direction(match direction {
            // 偶数嵌入级别使用从左向右 shaping。
            TextDirection::LeftToRight => rustybuzz::Direction::LeftToRight,
            // 奇数嵌入级别使用从右向左 shaping。
            TextDirection::RightToLeft => rustybuzz::Direction::RightToLeft,
        });
    }
    // 使用字体默认 OpenType 特性执行 GSUB 与 GPOS。
    let shaped = rustybuzz::shape(&face, &[], buffer);
    // 字形信息保存 glyph id 与源 cluster。
    let infos = shaped.glyph_infos();
    // 字形位置保存 advance 与二维 offset。
    let positions = shaped.glyph_positions();
    // 两个并行数组长度必须完全一致。
    if infos.len() != positions.len() {
        // 拒绝传播不完整的 shaping 输出。
        return None;
        // 结束输出一致性检查。
    }
    // 收集所有 cluster 字节起点，用于推导排他源区间。
    let mut cluster_starts = infos
        // 遍历每个输出字形。
        .iter()
        // 将 rustybuzz 的 u32 起点转为平台 usize。
        .map(|info| info.cluster as usize)
        // 收集成可排序数组。
        .collect::<Vec<_>>();
    // 逻辑排序不改变视觉字形顺序。
    cluster_starts.sort_unstable();
    // 合并 ligature 或重排产生的重复 cluster。
    cluster_starts.dedup();
    // 预留最终定位字形，避免 shaping 后重复扩容。
    let mut glyphs = Vec::with_capacity(infos.len());
    // 预留按宽度形成的行信息。
    let mut lines = Vec::new();
    // 当前视觉行的水平 advance。
    let mut cursor_x = 0.0_f32;
    // 当前视觉行的顶部坐标。
    let mut line_y = 0.0_f32;
    // 当前行在字形数组中的起点。
    let mut line_glyph_start = 0_usize;
    // 当前行最小逻辑源字符起点。
    let mut line_char_start = usize::MAX;
    // 当前行最大逻辑源字符终点。
    let mut line_char_end = 0_usize;
    // 记录所有行中的最大 advance。
    let mut max_line_width = 0.0_f32;
    // 只有显式启用且有限正宽度才执行后端 cluster 换行。
    let wrap = opts.word_wrap && opts.max_width.is_finite() && opts.max_width > 0.0;
    // 逐个连续 cluster 处理，保证 cluster 内部永不拆行。
    let mut group_start = 0_usize;
    // 遍历全部 shaping 输出。
    while group_start < infos.len() {
        // 当前连续组的源 cluster 标识。
        let cluster = infos[group_start].cluster;
        // 从下一个字形开始寻找连续组终点。
        let mut group_end = group_start + 1;
        // 相同 cluster 的多个字形必须作为原子组处理。
        while group_end < infos.len() && infos[group_end].cluster == cluster {
            // 扩展当前 cluster 组。
            group_end += 1;
            // 结束连续 cluster 扫描。
        }
        // 计算整个 cluster 的水平 advance，防止只看首字形后拆行。
        let group_advance = positions[group_start..group_end]
            // 遍历 cluster 内全部定位记录。
            .iter()
            // 水平排版统一消费 advance 的绝对像素量。
            .map(|position| (position.x_advance as f32 * glyph_scale).abs())
            // 汇总 cluster 总 advance。
            .sum::<f32>();
        // 当前行已有内容且完整 cluster 超宽时在 cluster 前换行。
        if wrap && cursor_x > 0.0 && cursor_x + group_advance > opts.max_width {
            // 登记上一行的字形和逻辑源范围。
            push_line(
                // 传入行集合。
                &mut lines,
                // 传入上一行字形起点。
                line_glyph_start,
                // 当前字形数量就是上一行终点。
                glyphs.len(),
                // 传入上一行顶部。
                line_y,
                // 传入统一行高。
                line_height,
                // 传入上一行实际宽度。
                cursor_x,
                // 空哨兵只会出现在无字形行，正常行保留最小值。
                line_char_start.min(line_char_end),
                // 传入最大排他逻辑终点。
                line_char_end,
                // 结束上一行登记参数。
            );
            // 更新全局最大行宽。
            max_line_width = max_line_width.max(cursor_x);
            // 新行水平起点归零。
            cursor_x = 0.0;
            // 新行下移一个完整行高。
            line_y += line_height;
            // 新行从当前字形数组尾部开始。
            line_glyph_start = glyphs.len();
            // 重置新行最小逻辑源索引。
            line_char_start = usize::MAX;
            // 重置新行最大逻辑源终点。
            line_char_end = 0;
            // 结束 cluster 前自动换行。
        }
        // cluster 内部使用独立画笔累加每个字形的 advance。
        let mut cluster_pen = 0.0_f32;
        // 逐个输出 cluster 内的真实字形。
        for index in group_start..group_end {
            // 读取当前字形的源字符排他区间。
            let (char_index, char_end) = cluster_char_range(
                // 传入原始文本。
                text,
                // 传入已排序 cluster 起点。
                &cluster_starts,
                // 传入当前字形 cluster。
                infos[index].cluster as usize,
                // 无效 cluster 触发整个 shaping 回退。
            )?;
            // 将当前字形水平 advance 缩放为非负像素。
            let advance = (positions[index].x_advance as f32 * glyph_scale).abs();
            // 将 GPOS 水平偏移缩放为像素。
            let offset_x = positions[index].x_offset as f32 * glyph_scale;
            // 字体坐标 y 向上，画布坐标 y 向下，因此反转 GPOS 垂直偏移。
            let offset_y = -(positions[index].y_offset as f32 * glyph_scale);
            // 写入可由既有渲染路径直接消费的定位字形。
            glyphs.push(PositionedGlyph {
                // 视觉顺序字形沿正向画笔排列并应用 GPOS 偏移。
                x: cursor_x + cluster_pen + offset_x,
                // 基线坐标叠加 GPOS 垂直偏移。
                y: line_y + ascent + offset_y,
                // width 继续表示布局 advance，不冒充轮廓包围盒。
                width: advance,
                // 字形行盒高度沿用 ab_glyph 度量。
                height: font_height,
                // rustybuzz 输出可直接作为 ab_glyph GlyphId 编号。
                glyph_id: infos[index].glyph_id,
                // 保存 cluster 的逻辑字符起点。
                char_index,
                // 保存 cluster 的排他逻辑字符终点。
                char_end,
                // 行级 UAX #9 重排由 FontService 统一回填。
                bidi_level: 0,
                // 保存加载该字形的字体句柄。
                font,
                // 结束定位字形构造。
            });
            // cluster 内画笔消费当前字形 advance。
            cluster_pen += advance;
            // 更新本行最小逻辑源起点，兼容 RTL 递减 cluster。
            line_char_start = line_char_start.min(char_index);
            // 更新本行最大排他逻辑源终点。
            line_char_end = line_char_end.max(char_end);
            // 结束 cluster 内字形生成。
        }
        // 当前行画笔一次性消费完整 cluster。
        cursor_x += group_advance;
        // 移动到下一个连续 cluster。
        group_start = group_end;
        // 结束 shaping 输出遍历。
    }
    // 登记最后一个非空视觉行。
    push_line(
        // 传入行集合。
        &mut lines,
        // 传入最后一行字形起点。
        line_glyph_start,
        // 全部字形数量即排他终点。
        glyphs.len(),
        // 传入最后一行顶部。
        line_y,
        // 传入统一行高。
        line_height,
        // 传入最后一行实际宽度。
        cursor_x,
        // 空哨兵在空文本时收敛为零。
        line_char_start.min(line_char_end),
        // 传入最后一行最大排他终点。
        line_char_end,
        // 结束最后一行登记参数。
    );
    // 合并最后一行宽度得到全局最大宽度。
    max_line_width = max_line_width.max(cursor_x);
    // 空文本仍返回一个零宽行盒，保持既有后端契约。
    if lines.is_empty() {
        // 写入不含字形的稳定空行信息。
        lines.push(LineInfo {
            // 空行位于顶部。
            y: 0.0,
            // 空行保留调用方行高。
            height: line_height,
            // 空行宽度为零。
            width: 0.0,
            // 空行逻辑起点为零。
            start_char: 0,
            // 空行逻辑终点为零。
            end_char: 0,
            // 空行字形起点为零。
            glyph_start: 0,
            // 空行不含字形。
            glyph_count: 0,
            // 结束空行构造。
        });
        // 结束空文本兼容分支。
    }
    // 实际文本高度使用最后一行顶部加字体行盒。
    let text_height = line_y + font_height;
    // 正 max_height 形成垂直对齐容器，否则使用文本自身高度。
    let max_height = if opts.max_height > 0.0 {
        // 使用调用方显式高度。
        opts.max_height
    // 没有显式高度时不引入额外空白。
    } else {
        // 使用实际文本高度。
        text_height
        // 结束高度容器选择。
    };
    // 根据垂直对齐规则计算统一偏移。
    let vertical_offset = if max_height > text_height {
        // 只在容器有正剩余空间时对齐。
        match opts.v_align {
            // 顶部和基线对齐不增加偏移。
            crate::draw::VAlign::Top | crate::draw::VAlign::Baseline => 0.0,
            // 居中对齐平分剩余高度。
            crate::draw::VAlign::Middle => (max_height - text_height) * 0.5,
            // 底部对齐消费全部前置剩余高度。
            crate::draw::VAlign::Bottom => max_height - text_height,
            // 结束垂直对齐匹配。
        }
    // 容器没有正剩余空间时不偏移。
    } else {
        // 保持顶部原点。
        0.0
        // 结束垂直偏移计算。
    };
    // 将统一垂直偏移应用到全部定位字形。
    for glyph in &mut glyphs {
        // 平移字形基线位置。
        glyph.y += vertical_offset;
        // 结束字形垂直平移。
    }
    // 将同一偏移应用到行盒，保持命中与绘制一致。
    for line in &mut lines {
        // 平移行顶部。
        line.y += vertical_offset;
        // 结束行盒垂直平移。
    }
    // 返回不泄漏第三方类型的统一文本布局。
    Some(TextLayout {
        // 返回全部视觉顺序定位字形。
        glyphs,
        // 返回与字形范围对应的行信息。
        lines,
        // 返回最大行 advance。
        width: max_line_width,
        // 返回容器或文本自身高度中的较大者。
        height: max_height.max(text_height),
        // 结束文本布局构造。
    })
    // 结束 OpenType shaping 入口。
}

// 为复杂脚本 shaping 保留不依赖窗口的自动化回归测试。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../../../tests/unit/draw/resources/font/text_backends/shaping__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
