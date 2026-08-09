//! 文本布局：按字体分段、逐段布局、行拼接与对齐。

use crate::draw::resources::font::text_backend::{self as tb, TextLayout, TextLayoutOptions};
use crate::draw::{FontHandle, HAlign, VAlign};
// 使用扩展字素簇边界选择回退字体，避免拆开组合文本。
use unicode_segmentation::UnicodeSegmentation;

use super::FontService;

/// 布局辅助：按字体分割的文本段（追踪字节偏移）。
struct FontSegment {
    byte_start: usize,
    byte_end: usize,
    font: FontHandle,
    line_break_chars: usize,
}

impl FontService {
    /// 第 1 遍：将文本按字体分割为段，追踪字节偏移。
    fn segment_text(&self, font: &FontHandle, text: &str) -> Vec<FontSegment> {
        // 预留按字体连续区间形成的段列表。
        let mut segments: Vec<FontSegment> = Vec::new();
        // 当前连续字体段从文本起点开始。
        let mut seg_start = 0usize;
        // 初始字体使用调用方主字体。
        let mut seg_font = *font;
        // 逐个扩展字素簇遍历，保证字体回退不会切开组合序列。
        for (byte_i, grapheme) in text.grapheme_indices(true) {
            // CRLF 作为一个字素簇和一个强制换行段处理。
            if matches!(grapheme, "\n" | "\r" | "\r\n") {
                // 强制换行前先提交已有可见文本段。
                if byte_i > seg_start {
                    // 保存换行前连续字体区间。
                    segments.push(FontSegment {
                        // 保存区间 UTF-8 起点。
                        byte_start: seg_start,
                        // 保存区间 UTF-8 排他终点。
                        byte_end: byte_i,
                        // 保存整个区间使用的字体。
                        font: seg_font,
                        // 普通文本段不包含强制换行字符。
                        line_break_chars: 0,
                        // 结束普通字体段构造。
                    });
                    // 结束换行前段提交。
                }
                // 字素簇字节长度天然覆盖 CRLF 的两个标量。
                let byte_end = byte_i + grapheme.len();
                // chars() 数量与全局逻辑索引口径一致。
                let line_break_chars = grapheme.chars().count();
                // 单独登记强制换行段。
                segments.push(FontSegment {
                    // 保存换行簇 UTF-8 起点。
                    byte_start: byte_i,
                    // 保存换行簇 UTF-8 排他终点。
                    byte_end,
                    // 强制换行沿用主字体句柄但不会参与 shaping。
                    font: *font,
                    // 保存 CR、LF 或 CRLF 的逻辑字符数量。
                    line_break_chars,
                    // 结束强制换行段构造。
                });
                // 下一普通段从换行簇之后开始。
                seg_start = byte_end;
                // 换行后重新从主字体选择。
                seg_font = *font;
                // 跳过当前换行簇的字体覆盖检查。
                continue;
                // 结束强制换行分支。
            }
            // 从主字体和回退链中选择能完整覆盖整个字素簇的第一个字体。
            let use_font = std::iter::once(*font)
                // 主字体之后按已登记优先级尝试回退字体。
                .chain(self.fallback_handles.iter().copied())
                // 选择有效且覆盖簇内全部非空白标量的字体。
                .find(|candidate| {
                    // 无效句柄不能参与回退选择。
                    self.text_backend.is_valid(candidate)
                        // 空白控制仍由主布局语义处理，不要求字体提供轮廓。
                        && grapheme.chars().all(|ch| {
                            // 空白无需可见字形，其他标量必须由同一字体覆盖。
                            ch.is_whitespace() || self.text_backend.has_glyph(candidate, ch)
                        })
                })
                // 没有字体完整覆盖时保留主字体，绝不拆分当前字素簇。
                .unwrap_or(*font);
            // 字体变化时结束前一个连续字体段。
            if use_font.0 != seg_font.0 {
                // 只有非空区间才需要登记。
                if byte_i > seg_start {
                    // 保存字体切换前的完整字素簇区间。
                    segments.push(FontSegment {
                        // 保存前一段 UTF-8 起点。
                        byte_start: seg_start,
                        // 当前字素簇起点就是前一段排他终点。
                        byte_end: byte_i,
                        // 保存前一段统一字体。
                        font: seg_font,
                        // 普通字体段没有强制换行字符。
                        line_break_chars: 0,
                        // 结束字体段构造。
                    });
                    // 结束非空字体段提交。
                }
                // 新段从当前完整字素簇开始。
                seg_start = byte_i;
                // 新段采用完整覆盖当前簇的字体。
                seg_font = use_font;
                // 结束字体切换处理。
            }
            // 结束扩展字素簇遍历。
        }
        // 文本尾部仍有普通内容时提交最后一段。
        if seg_start < text.len() {
            // 保存最终连续字体区间。
            segments.push(FontSegment {
                // 保存最终段 UTF-8 起点。
                byte_start: seg_start,
                // 文本长度就是最终段排他终点。
                byte_end: text.len(),
                // 保存最终段统一字体。
                font: seg_font,
                // 最终普通段没有强制换行字符。
                line_break_chars: 0,
                // 结束最终字体段构造。
            });
            // 结束最终段提交。
        }
        // 返回不会拆开扩展字素簇的字体段列表。
        segments
        // 结束字体分段。
    }

    /// 第 2 遍：逐段布局 → 按行拼接。
    #[allow(
        clippy::too_many_arguments,
        reason = "the parameters are immutable state for one font layout pass"
    )]
    fn layout_segments(
        &self,
        segments: &[FontSegment],
        text: &str,
        seg_opts: &TextLayoutOptions,
        fs: f32,
        line_h: f32,
        primary_ascent: f32,
        do_wrap: bool,
        max_w: f32,
        h_align: HAlign,
    ) -> (Vec<tb::PositionedGlyph>, Vec<tb::LineInfo>) {
        #[derive(Clone, Copy)]
        struct LineGlyph {
            glyph: tb::PositionedGlyph,
            source_char: char,
        }

        #[derive(Default)]
        struct LineAccum {
            glyphs: Vec<LineGlyph>,
            y: f32,
            height: f32,
            width: f32,
            char_start: usize,
            char_end: usize,
        }

        let line_width = |line: &LineAccum| {
            line.glyphs.iter().fold(0.0f32, |width, glyph| {
                width.max(glyph.glyph.x + glyph.glyph.width)
            })
        };
        let last_soft_break = |line: &LineAccum| {
            let first_content = line.glyphs.iter().position(|glyph| {
                !tb::collapsible_wrap_whitespace(glyph.source_char) || glyph.glyph.width > 0.0
            })?;
            line.glyphs
                .iter()
                .enumerate()
                .skip(first_content)
                .rposition(|(_, glyph)| tb::soft_wrap_opportunity_after(glyph.source_char))
                .map(|index| first_content + index + 1)
        };
        let line_has_content = |line: &LineAccum| {
            line.glyphs.iter().any(|glyph| {
                !tb::collapsible_wrap_whitespace(glyph.source_char) || glyph.glyph.width > 0.0
            })
        };
        let collapse_trailing_wrap_whitespace = |line: &mut LineAccum| {
            let trailing_start = line
                .glyphs
                .iter()
                .rposition(|glyph| !tb::collapsible_wrap_whitespace(glyph.source_char))
                .map_or(0, |index| index + 1);
            if let Some(x) = line.glyphs.get(trailing_start).map(|glyph| glyph.glyph.x) {
                for glyph in &mut line.glyphs[trailing_start..] {
                    glyph.glyph.x = x;
                    glyph.glyph.width = 0.0;
                }
            }
        };

        let flush_line = |lines: &mut Vec<LineAccum>,
                          line: &mut LineAccum,
                          width: f32,
                          y: f32,
                          height: f32,
                          char_end: usize| {
            line.width = width.max(line.width);
            line.height = height;
            line.y = y;
            line.char_end = char_end;
            lines.push(std::mem::take(line));
        };

        let mut lines: Vec<LineAccum> = Vec::new();
        let mut line = LineAccum::default();
        let mut cx = 0.0f32;
        let mut cy = 0.0f32;
        let mut char_idx = 0usize;
        let mut collapse_auto_line_start_whitespace = false;

        for seg in segments {
            if seg.byte_start >= seg.byte_end {
                continue;
            }

            if seg.line_break_chars > 0 {
                if line.glyphs.is_empty() {
                    line.char_start = char_idx;
                }
                flush_line(&mut lines, &mut line, cx, cy, line_h, char_idx);
                cx = 0.0;
                cy += line_h;
                char_idx += seg.line_break_chars;
                line.char_start = char_idx;
                collapse_auto_line_start_whitespace = false;
                continue;
            }

            let seg_metrics = self.text_backend.horizontal_line_metrics(&seg.font, fs);
            let seg_ascent = seg_metrics.map(|m| m.ascent).unwrap_or(fs * 0.8);

            // 用 &str 切片代替 String 分配
            let seg_text = &text[seg.byte_start..seg.byte_end];
            let seg_layout = self.text_backend.layout_text(&seg.font, seg_text, seg_opts);
            let seg_char_count = seg_text.chars().count();
            let baseline_offset = primary_ascent - seg_ascent;
            let mut chunk_source_x = 0.0f32;
            let mut chunk_target_x = cx;
            // 随机访问源字符，兼容 RTL shaping 返回的递减 cluster 顺序。
            let source_chars = seg_text.chars().collect::<Vec<_>>();

            // 后端 layout 的 char_index 是段内相对值。这里按字形推进，保证
            // 同一字体形成的长段也能在 max_width 内折行，而不是只能在字体段之间换行。
            for mut g in seg_layout.glyphs {
                // 按 cluster 逻辑起点读取代表字符，不依赖字形视觉顺序。
                let source_char = source_chars.get(g.char_index).copied().unwrap_or('\0');
                // 将段内 cluster 起点转换为全文逻辑字符起点。
                let global_char_index = char_idx + g.char_index;
                // 将段内 cluster 排他终点转换为全文逻辑字符终点。
                let global_char_end = char_idx + g.char_end;
                // 同一 cluster 已有字形进入当前行时，后续字形不得单独换行。
                let cluster_already_on_line = line
                    // 查看当前视觉行的最后一个已提交字形。
                    .glyphs
                    // 相同逻辑起点表示仍处于同一个连续 shaping cluster。
                    .last()
                    // 比较全局 cluster 起点。
                    .is_some_and(|glyph| glyph.glyph.char_index == global_char_index);
                let source_x = g.x;
                let source_advance = g.width;
                let mut target_x = chunk_target_x + (g.x - chunk_source_x);
                if do_wrap
                    && collapse_auto_line_start_whitespace
                    && tb::collapsible_wrap_whitespace(source_char)
                {
                    g.width = 0.0;
                    target_x = 0.0;
                    chunk_source_x = source_x + source_advance;
                    chunk_target_x = 0.0;
                }
                // 只允许在 cluster 的第一个输出字形前决定换行。
                if do_wrap && !cluster_already_on_line {
                    loop {
                        if !line_has_content(&line) || target_x + g.width <= max_w {
                            break;
                        }

                        if let Some(break_index) = last_soft_break(&line) {
                            if break_index < line.glyphs.len() {
                                let mut tail = line.glyphs.split_off(break_index);
                                let tail_origin = tail[0].glyph.x;
                                let next_char = tail[0].glyph.char_index;
                                collapse_trailing_wrap_whitespace(&mut line);
                                let completed_width = line_width(&line);
                                flush_line(
                                    &mut lines,
                                    &mut line,
                                    completed_width,
                                    cy,
                                    line_h,
                                    next_char,
                                );
                                cy += line_h;
                                for glyph in &mut tail {
                                    glyph.glyph.x -= tail_origin;
                                    glyph.glyph.y += line_h;
                                }
                                line.char_start = next_char;
                                line.char_end = tail
                                    .last()
                                    // 保留 shaping cluster 的排他源终点。
                                    .map_or(next_char, |glyph| glyph.glyph.char_end);
                                line.glyphs = tail;
                                cx = line_width(&line);
                                chunk_target_x -= tail_origin;
                                target_x -= tail_origin;
                                collapse_auto_line_start_whitespace = false;
                                continue;
                            }

                            collapse_trailing_wrap_whitespace(&mut line);
                            let completed_width = line_width(&line);
                            flush_line(
                                &mut lines,
                                &mut line,
                                completed_width,
                                cy,
                                line_h,
                                global_char_index,
                            );
                            cx = 0.0;
                            cy += line_h;
                            chunk_source_x = g.x;
                            chunk_target_x = 0.0;
                            target_x = 0.0;
                            collapse_auto_line_start_whitespace = true;
                            if tb::collapsible_wrap_whitespace(source_char) {
                                g.width = 0.0;
                                chunk_source_x = source_x + source_advance;
                            }
                            continue;
                        }

                        let move_previous = line.glyphs.len() > 1
                            && (tb::prohibited_at_line_start(source_char)
                                || line.glyphs.last().is_some_and(|glyph| {
                                    tb::prohibited_at_line_end(glyph.source_char)
                                }));
                        if move_previous {
                            if let Some(mut previous) = line.glyphs.pop() {
                                let previous_origin = previous.glyph.x;
                                let previous_char_index = previous.glyph.char_index;
                                let completed_width = line_width(&line);
                                flush_line(
                                    &mut lines,
                                    &mut line,
                                    completed_width,
                                    cy,
                                    line_h,
                                    previous_char_index,
                                );
                                cy += line_h;
                                previous.glyph.x = 0.0;
                                previous.glyph.y += line_h;
                                line.char_start = previous_char_index;
                                // 移动整个字形 cluster 时保留其排他源终点。
                                line.char_end = previous.glyph.char_end;
                                line.glyphs.push(previous);
                                cx = line_width(&line);
                                chunk_target_x -= previous_origin;
                                target_x -= previous_origin;
                                collapse_auto_line_start_whitespace = false;
                                continue;
                            }
                        }

                        if tb::prohibited_at_line_start(source_char) {
                            break;
                        }

                        let completed_width = line_width(&line);
                        flush_line(
                            &mut lines,
                            &mut line,
                            completed_width,
                            cy,
                            line_h,
                            global_char_index,
                        );
                        cx = 0.0;
                        cy += line_h;
                        chunk_source_x = g.x;
                        chunk_target_x = 0.0;
                        target_x = 0.0;
                        collapse_auto_line_start_whitespace = true;
                        if tb::collapsible_wrap_whitespace(source_char) {
                            g.width = 0.0;
                            chunk_source_x = source_x + source_advance;
                        }
                    }
                }

                if !tb::collapsible_wrap_whitespace(source_char) {
                    collapse_auto_line_start_whitespace = false;
                }

                if line.glyphs.is_empty() {
                    line.char_start = global_char_index;
                // RTL 视觉顺序可能先提交更大的逻辑索引。
                } else {
                    // 始终保存当前行最小逻辑 cluster 起点。
                    line.char_start = line.char_start.min(global_char_index);
                }
                g.x = target_x;
                g.y += cy + baseline_offset;
                g.char_index = global_char_index;
                // 同步提升 cluster 排他终点到全文索引口径。
                g.char_end = global_char_end;
                g.font = seg.font;
                cx = cx.max(g.x + g.width);
                // RTL 或多字形 cluster 都以最大排他逻辑终点描述本行。
                line.char_end = line.char_end.max(global_char_end);
                line.glyphs.push(LineGlyph {
                    glyph: g,
                    source_char,
                });
            }
            char_idx += seg_char_count;
        }

        if line.glyphs.is_empty() {
            line.char_start = char_idx;
        }
        flush_line(&mut lines, &mut line, cx, cy, line_h, char_idx);

        // 水平对齐：以 max_width（如果有限）或最大行宽度为容器
        let container_w = if max_w.is_finite() && max_w > 0.0 {
            max_w
        } else {
            lines.iter().fold(0.0f32, |m, l| m.max(l.width))
        };

        let mut all_glyphs = Vec::new();
        let mut line_infos = Vec::new();
        for l in &lines {
            let gs = all_glyphs.len();
            let align_off = match h_align {
                HAlign::Left => 0.0,
                HAlign::Center => (container_w - l.width) * 0.5,
                HAlign::Right => (container_w - l.width).max(0.0),
            };
            for g in &l.glyphs {
                let mut shifted = g.glyph;
                shifted.x += align_off;
                all_glyphs.push(shifted);
            }
            line_infos.push(tb::LineInfo {
                y: l.y,
                height: l.height,
                width: l.width,
                start_char: l.char_start,
                end_char: l.char_end,
                glyph_start: gs,
                glyph_count: l.glyphs.len(),
            });
        }

        (all_glyphs, line_infos)
    }

    /// 布局文本，返回定位后的字形（含自动多字体回退和水平对齐）。
    ///
    /// - 逐行内分段：连续相同字体的字符组成一段，每段调用后端 layout_text。
    /// - 段间 x 坐标累积：同一行内后一段的 glyph.x += 前一段总宽度。
    /// - 垂直基线对齐：不同字体段共享同一行基线，用段 ascent 修正 y 偏移。
    /// - 换行：CRLF / CR / LF 强制换行；word_wrap 优先词边界并以字形级折行为兜底。
    /// - LineInfo：按实际行构建，每行包含正确的起止字符偏移、glyph 索引、宽度和高度。
    /// - 水平对齐：根据 opts.h_align（Left/Center/Right）在容器宽度内对齐各行。
    pub fn layout_text(
        &self,
        font: &FontHandle,
        text: &str,
        opts: &TextLayoutOptions,
    ) -> TextLayout {
        if !self.text_backend.is_valid(font) || text.is_empty() {
            return TextLayout {
                glyphs: vec![],
                lines: vec![],
                width: 0.0,
                height: 0.0,
            };
        }

        let fs = tb::bounded_font_size(opts.font_size);
        let line_h = if opts.line_height.is_finite() && opts.line_height > 0.0 {
            opts.line_height
        } else {
            fs * 1.5
        };
        let has_max_w = opts.max_width.is_finite() && opts.max_width > 0.0;
        let do_wrap = has_max_w && opts.word_wrap;
        let max_w = if has_max_w { opts.max_width } else { f32::MAX };

        let primary_metrics = self.text_backend.horizontal_line_metrics(font, fs);
        let primary_ascent = primary_metrics.map(|m| m.ascent).unwrap_or(fs * 0.8);

        let seg_opts = TextLayoutOptions {
            max_width: f32::MAX,
            max_height: 0.0,
            font_size: fs,
            line_height: line_h,
            v_align: VAlign::Top,
            ..opts.clone()
        };

        let segments = self.segment_text(font, text);
        let (mut all_glyphs, mut line_infos) = self.layout_segments(
            &segments,
            text,
            &seg_opts,
            fs,
            line_h,
            primary_ascent,
            do_wrap,
            max_w,
            opts.h_align,
        );

        let natural_height = line_infos.last().map_or(0.0, |l| l.y + l.height);
        let container_height = if opts.max_height.is_finite() && opts.max_height > natural_height {
            opts.max_height
        } else {
            natural_height
        };
        let vertical_offset = match opts.v_align {
            VAlign::Top | VAlign::Baseline => 0.0,
            VAlign::Middle => (container_height - natural_height) * 0.5,
            VAlign::Bottom => container_height - natural_height,
        };
        if vertical_offset > 0.0 {
            for glyph in &mut all_glyphs {
                glyph.y += vertical_offset;
            }
            for line in &mut line_infos {
                line.y += vertical_offset;
            }
        }
        let max_line_width = line_infos.iter().fold(0.0f32, |m, l| m.max(l.width));

        TextLayout {
            glyphs: all_glyphs,
            lines: line_infos,
            width: max_line_width,
            height: container_height,
        }
    }
}

// 为字体回退与 cluster 原子换行保留纯内存回归测试。
#[cfg(test)]
mod tests {
    // 引入被测试的字体服务实现。
    use super::FontService;
    // 引入错误类型以实现测试后端加载契约。
    use crate::core::Error;
    // 引入统一文本后端 trait。
    use crate::draw::TextBackend;
    // 引入字体句柄与对齐枚举。
    use crate::draw::{FontHandle, HAlign, VAlign};
    // 引入测试后端所需的布局与光栅类型。
    use crate::draw::resources::font::text_backend::{
        // 引入空光栅返回类型。
        GlyphRaster,
        // 引入行信息类型。
        LineInfo,
        // 引入水平行度量类型。
        LineMetrics,
        // 引入定位字形类型。
        PositionedGlyph,
        // 引入布局结果类型。
        TextLayout,
        // 引入布局选项类型。
        TextLayoutOptions,
        // 结束测试类型导入。
    };

    // 使用可控覆盖矩阵模拟主字体与回退字体。
    #[derive(Debug)]
    struct CoverageBackend;

    // 实现最小文本后端以隔离字体文件差异。
    impl TextBackend for CoverageBackend {
        // 测试不依赖真实字体数据，始终返回主句柄。
        fn load_font(&mut self, _data: &[u8]) -> Result<FontHandle, Error> {
            // 返回稳定主字体句柄。
            Ok(FontHandle::new(0))
            // 结束测试加载实现。
        }
        // 测试后端没有需要释放的外部资源。
        fn unload_font(&mut self, _handle: &FontHandle) {}
        // 句柄零和一分别代表主字体与回退字体。
        fn is_valid(&self, handle: &FontHandle) -> bool {
            // 只接受测试矩阵中的两个句柄。
            matches!(handle.0, 0 | 1)
            // 结束句柄有效性判断。
        }
        // 主字体只覆盖基字，回退字体同时覆盖组合符。
        fn has_glyph(&self, font: &FontHandle, ch: char) -> bool {
            // 按句柄返回可控字符覆盖。
            match font.0 {
                // 主字体故意缺少组合音标。
                0 => ch == 'a',
                // 回退字体完整覆盖扩展字素簇。
                1 => matches!(ch, 'a' | '\u{0301}'),
                // 其他句柄均无字符覆盖。
                _ => false,
                // 结束覆盖矩阵匹配。
            }
            // 结束字形覆盖判断。
        }
        // 返回两个字形组成的同一 shaping cluster。
        fn layout_text(
            // 测试后端不依赖自身状态。
            &self,
            // 保留调用方选择的回退字体句柄。
            font: &FontHandle,
            // 测试文本仅用于计算逻辑终点。
            text: &str,
            // 复用调用方字号与行高。
            opts: &TextLayoutOptions,
            // 返回可被 FontService 二次拼接的段布局。
        ) -> TextLayout {
            // 当前测试文本固定包含两个 Unicode 标量。
            let char_end = text.chars().count();
            // 构造同一 cluster 的基字与组合符字形。
            let glyphs = vec![
                // 第一个字形消费主要 advance。
                PositionedGlyph {
                    // 基字从段起点开始。
                    x: 0.0,
                    // 使用稳定测试基线。
                    y: 8.0,
                    // 基字宽度小于测试最大行宽。
                    width: 6.0,
                    // 使用调用方字号作为行盒高度。
                    height: opts.font_size,
                    // 使用稳定测试字形编号。
                    glyph_id: 1,
                    // cluster 从第一个字符开始。
                    char_index: 0,
                    // cluster 覆盖基字与组合符。
                    char_end,
                    // 保留回退字体句柄。
                    font: *font,
                    // 结束基字字形构造。
                },
                // 第二个字形仍属于同一 cluster。
                PositionedGlyph {
                    // 组合符从基字 advance 后开始，用于触发潜在拆行。
                    x: 6.0,
                    // 使用同一测试基线。
                    y: 8.0,
                    // 第二字形使完整 cluster 超过测试最大行宽。
                    width: 6.0,
                    // 使用调用方字号作为行盒高度。
                    height: opts.font_size,
                    // 使用另一个稳定测试字形编号。
                    glyph_id: 2,
                    // 两个输出字形共享 cluster 起点。
                    char_index: 0,
                    // 两个输出字形共享 cluster 终点。
                    char_end,
                    // 保留回退字体句柄。
                    font: *font,
                    // 结束组合符字形构造。
                },
                // 结束测试字形数组。
            ];
            // 返回单段原始布局，最终行信息由 FontService 重建。
            TextLayout {
                // 返回两个 cluster 字形。
                glyphs,
                // 返回占位单行信息。
                lines: vec![LineInfo {
                    // 原始段行位于顶部。
                    y: 0.0,
                    // 使用调用方行高。
                    height: opts.line_height,
                    // 完整 cluster advance 为十二像素。
                    width: 12.0,
                    // 源区间从零开始。
                    start_char: 0,
                    // 源区间覆盖完整文本。
                    end_char: char_end,
                    // 字形范围从零开始。
                    glyph_start: 0,
                    // 字形范围包含两个字形。
                    glyph_count: 2,
                    // 结束占位行构造。
                }],
                // 返回完整 cluster 宽度。
                width: 12.0,
                // 返回稳定字体高度。
                height: opts.font_size,
                // 结束测试布局构造。
            }
            // 结束测试布局实现。
        }
        // 测试不执行光栅化，返回稳定空光栅。
        fn rasterize_glyph(
            // 测试后端不依赖自身状态。
            &self,
            // 测试不读取字体句柄。
            _font: &FontHandle,
            // 测试不读取字形编号。
            _glyph_id: u32,
            // 测试不读取像素字号。
            _pixel_size: f32,
            // 返回空光栅。
        ) -> GlyphRaster {
            // 使用内部稳定空值构造。
            GlyphRaster::empty()
            // 结束测试光栅实现。
        }
        // 返回稳定水平度量以驱动 FontService 行盒。
        fn horizontal_line_metrics(
            // 测试后端不依赖自身状态。
            &self,
            // 测试不区分字体句柄。
            _font: &FontHandle,
            // 使用调用方像素字号。
            pixel_size: f32,
            // 返回可用行度量。
        ) -> Option<LineMetrics> {
            // 构造八成 ascent 与两成 descent。
            Some(LineMetrics {
                // ascent 使用字号八成。
                ascent: pixel_size * 0.8,
                // descent 使用字号两成。
                descent: pixel_size * 0.2,
                // 换行步长使用完整字号。
                new_line_size: pixel_size,
                // 结束测试行度量构造。
            })
            // 结束测试行度量实现。
        }
        // 结束测试后端实现。
    }

    // 构造使用可控覆盖后端的字体服务。
    fn service() -> FontService {
        // 从默认服务复用注册表与缓存初始化。
        let mut service = FontService::new();
        // 替换为不依赖系统字体的覆盖后端。
        service.text_backend = Box::new(CoverageBackend);
        // 显式登记主字体句柄以允许添加回退字体。
        service.loaded_font_handle = FontHandle::new(0);
        // 设置唯一回退字体句柄。
        service.set_fallback_chain(&[FontHandle::new(1)]);
        // 返回已配置服务。
        service
        // 结束测试服务构造。
    }

    // 验证扩展字素簇只能整体选择一个回退字体。
    #[test]
    fn fallback_does_not_split_extended_grapheme_cluster() {
        // 创建可控字体覆盖服务。
        let service = service();
        // 基字由主字体覆盖，组合符只由回退字体覆盖。
        let text = "a\u{0301}";
        // 执行真实字体分段逻辑。
        let segments = service.segment_text(&FontHandle::new(0), text);
        // 完整扩展字素簇只能形成一个字体段。
        assert_eq!(segments.len(), 1);
        // 整个 cluster 必须选择能完整覆盖它的回退字体。
        assert_eq!(segments[0].font, FontHandle::new(1));
        // 字体段必须覆盖完整 UTF-8 文本而不是只覆盖基字。
        assert_eq!(
            (segments[0].byte_start, segments[0].byte_end),
            (0, text.len())
        );
        // 结束字素簇回退测试。
    }

    // 验证超宽 shaping cluster 宁可整簇溢出也不能在内部拆行。
    #[test]
    fn wrapping_keeps_all_glyphs_of_cluster_on_one_line() {
        // 创建可控字体覆盖服务。
        let service = service();
        // 构造小于完整 cluster 宽度的换行约束。
        let options = TextLayoutOptions {
            // 八像素只能容纳 cluster 的首个六像素字形。
            max_width: 8.0,
            // 高度由文本自身决定。
            max_height: 0.0,
            // 使用稳定行高。
            line_height: 12.0,
            // 显式启用自动换行。
            word_wrap: true,
            // 使用左对齐观察原始行。
            h_align: HAlign::Left,
            // 使用顶部对齐观察原始行。
            v_align: VAlign::Top,
            // 使用稳定测试字号。
            font_size: 10.0,
            // 结束换行选项构造。
        };
        // 执行完整 FontService 字体分段与行拼接。
        let layout = service.layout_text(&FontHandle::new(0), "a\u{0301}", &options);
        // 两个同 cluster 字形必须保留在同一视觉行。
        assert_eq!(layout.lines.len(), 1);
        // 唯一行必须同时引用两个字形。
        assert_eq!(layout.lines[0].glyph_count, 2);
        // 超宽 cluster 可以整簇溢出，但宽度必须保持完整十二像素。
        assert_eq!(layout.lines[0].width, 12.0);
        // 两个字形必须保留相同的全局 cluster 源区间。
        assert!(layout
            // 遍历最终定位字形。
            .glyphs
            // 检查完整源区间。
            .iter()
            // 两个字形都覆盖两个源字符。
            .all(|glyph| glyph.char_index == 0 && glyph.char_end == 2));
        // 结束 cluster 原子换行测试。
    }
    // 结束字体布局测试模块。
}
