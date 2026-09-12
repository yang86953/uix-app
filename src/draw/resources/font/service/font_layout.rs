//! 文本布局：按字体分段、逐段布局、行拼接与对齐。

use crate::draw::resources::font::text_backend::{self as tb, TextLayout, TextLayoutOptions};
// 引入共享段落级 UAX #9 分析。
use crate::draw::resources::font::bidi::BidiAnalysis;
// 引入显式 shaping 方向。
use crate::draw::resources::font::text_backend::TextDirection;
use crate::draw::{FontHandle, HAlign, VAlign};
// 使用扩展字素簇边界选择回退字体，避免拆开组合文本。
use unicode_segmentation::UnicodeSegmentation;
// 帧诊断：统计每秒文本布局（shaping）调用次数，供性能摘要读取。
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

// 引入字体模块共享的 UAX #14 断行边界。
use super::super::line_break::LineBreakMap;
// 引入字体段方向切分与视觉 cluster 重排辅助。
use super::FontService;
use super::bidi_layout::{
    BidiFontSegment, LineGlyph, logical_cluster_order, reorder_line, split_font_segments,
};
// 引入视觉行两端对齐的中性几何算法。
use super::text_justify::justify_line;

// 帧诊断：实际执行 shaping 的布局计数器（缓存命中不计），每秒摘要读取后清零。
static TEXT_LAYOUT_CALLS: AtomicU64 = AtomicU64::new(0);

// 空布局跨服务共享，避免空 Label、Input 占位与测量路径反复申请 Arc 控制块。
static EMPTY_TEXT_LAYOUT: OnceLock<Arc<TextLayout>> = OnceLock::new();

// 帧诊断：读取并清零实际 shaping 计数。
pub(crate) fn take_text_layout_calls() -> u64 {
    // 原子交换取出当前计数并复位。
    TEXT_LAYOUT_CALLS.swap(0, Ordering::Relaxed)
}

/// 布局辅助：按字体分割的文本段（追踪字节偏移）。
pub(super) struct FontSegment {
    pub(super) byte_start: usize,
    pub(super) byte_end: usize,
    pub(super) font: FontHandle,
    pub(super) line_break_chars: usize,
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
        segments: &[BidiFontSegment],
        text: &str,
        // 复用完整源文本生成的 UAX #14 字符边界。
        breaks: &LineBreakMap,
        seg_opts: &TextLayoutOptions,
        fs: f32,
        line_h: f32,
        primary_ascent: f32,
        do_wrap: bool,
        max_w: f32,
        h_align: HAlign,
        // 复用完整源文本生成的段落级 UAX #9 分析。
        bidi: &BidiAnalysis,
    ) -> (Vec<tb::PositionedGlyph>, Vec<tb::LineInfo>) {
        #[derive(Default)]
        struct LineAccum {
            glyphs: Vec<LineGlyph>,
            y: f32,
            height: f32,
            width: f32,
            char_start: usize,
            char_end: usize,
            // 首个非可折叠空白字形下标；None 表示当前行还没有实际内容。
            first_content: Option<usize>,
            // 增量维护的最近合法断点（排他字形下标），避免逐字形反向重扫整行。
            last_break: Option<usize>,
        }

        // 判断字形是否算作行内容，与既有 has_content 判定保持一致。
        let glyph_is_content = |glyph: &LineGlyph| {
            !LineBreakMap::collapsible_whitespace(glyph.source_char) || glyph.glyph.width > 0.0
        };
        let line_width = |line: &LineAccum| {
            line.glyphs.iter().fold(0.0f32, |width, glyph| {
                width.max(glyph.glyph.x + glyph.glyph.width)
            })
        };
        let last_soft_break = |line: &LineAccum| {
            // 全空白行没有任何可断内容。
            line.first_content?;
            // 增量记录的断点已经覆盖行首内容之后的全部历史字形。
            let mut best = line.last_break;
            // 行尾 cluster 是否完整由调用点保证：进入换行判定时待放置字形
            // 必然开启新 cluster（cluster_already_on_line 已排除同簇续排）。
            if let Some(last) = line.glyphs.last()
                && breaks.allows_at(last.glyph.char_end)
            {
                // 行尾本身也是候选断点，取两者较晚者等价于原 rposition 结果。
                best = best.max(Some(line.glyphs.len()));
            }
            // 返回最近合法断点，O(1) 读取不再反向扫描整行。
            best
        };
        let line_has_content = |line: &LineAccum| line.first_content.is_some();
        let collapse_trailing_wrap_whitespace = |line: &mut LineAccum| {
            let trailing_start = line
                .glyphs
                .iter()
                .rposition(|glyph| !LineBreakMap::collapsible_whitespace(glyph.source_char))
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
            let seg_ascent = seg_metrics
                .map(|m| m.baseline_in_line_box(line_h))
                .unwrap_or(fs * 0.8 + (line_h - fs) * 0.5);

            // 用 &str 切片代替 String 分配
            let seg_text = &text[seg.byte_start..seg.byte_end];
            // 使用段落级已解析方向执行单向 run shaping。
            let seg_layout = self.text_backend.layout_text_directional(
                // 保留字体回退选择。
                &seg.font,
                // 传入当前单向 run 的逻辑文本。
                seg_text,
                // 禁止后端自行换行，由 FontService 统一应用 UAX #14。
                seg_opts,
                // 嵌入级别奇偶决定 shaping 方向。
                if seg.bidi_level % 2 == 0 {
                    // 偶数级别使用 LTR shaping。
                    TextDirection::LeftToRight
                // 奇数级别使用 RTL shaping。
                } else {
                    // 奇数级别使用 RTL shaping。
                    TextDirection::RightToLeft
                },
            );
            let seg_char_count = seg_text.chars().count();
            let baseline_offset = primary_ascent - seg_ascent;
            let mut chunk_source_x = 0.0f32;
            let mut chunk_target_x = cx;
            // 逻辑 cluster 已按源索引排序，使用单调游标读取代表字符，避免临时 Vec。
            let mut source_chars = seg_text.chars();
            let mut source_cursor = 0usize;
            let mut source_char_index = None;
            let mut source_char = '\0';

            // 后端 layout 的 char_index 是段内相对值。这里按字形推进，保证
            // 同一字体形成的长段也能在 max_width 内折行，而不是只能在字体段之间换行。
            // UAX #14 必须先按逻辑 cluster 顺序决定行边界。
            for mut g in logical_cluster_order(seg_layout.glyphs) {
                // 按 cluster 逻辑起点读取代表字符，不依赖字形视觉顺序。
                if source_char_index != Some(g.char_index) {
                    if g.char_index < source_cursor {
                        // 防御异常倒退输入，保留原有随机读取的收敛行为。
                        source_char = seg_text.chars().nth(g.char_index).unwrap_or('\0');
                    } else {
                        while source_cursor <= g.char_index {
                            source_char = source_chars.next().unwrap_or('\0');
                            source_cursor += 1;
                        }
                    }
                    source_char_index = Some(g.char_index);
                }
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
                    && LineBreakMap::collapsible_whitespace(source_char)
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
                                // 尾段状态重建：分裂位置就是原行最近断点，尾段内部
                                // 不存在更晚的合法断点，只需重扫首个内容位置。
                                line.first_content = line.glyphs.iter().position(glyph_is_content);
                                line.last_break = None;
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
                            if LineBreakMap::collapsible_whitespace(source_char) {
                                g.width = 0.0;
                                chunk_source_x = source_x + source_advance;
                            }
                            continue;
                        }

                        // 没有标准机会时只允许超长字母数字词在 cluster 边界紧急折行。
                        let emergency_break = line.glyphs.last().is_some_and(|previous| {
                            // 前一 cluster 必须恰好结束于当前 cluster 的逻辑起点。
                            previous.glyph.char_end == global_char_index
                                // 共享断行表必须显式允许此紧急边界。
                                && breaks.emergency_allows_at(global_char_index)
                        });
                        // 标点、NBSP、emoji 与组合序列等不合法边界宁可溢出也不能拆行。
                        if !emergency_break {
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
                        if LineBreakMap::collapsible_whitespace(source_char) {
                            g.width = 0.0;
                            chunk_source_x = source_x + source_advance;
                        }
                    }
                }

                if !LineBreakMap::collapsible_whitespace(source_char) {
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
                // 增量登记首个行内容，供 has_content 与断点范围判定复用。
                if line.first_content.is_none()
                    && (!LineBreakMap::collapsible_whitespace(source_char) || g.width > 0.0)
                {
                    line.first_content = Some(line.glyphs.len());
                }
                // 前一字形在本字形落地后即确认 cluster 是否完整；完整且其
                // 排他源终点存在 UAX 机会时，当前位置成为候选断点。
                if let (Some(first_content), Some(previous)) =
                    (line.first_content, line.glyphs.last())
                    && line.glyphs.len() > first_content
                    && previous.glyph.char_index != global_char_index
                    && breaks.allows_at(previous.glyph.char_end)
                {
                    line.last_break = line.last_break.max(Some(line.glyphs.len()));
                }
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

        // 同一布局周期内复用完整文本的 unicode-bidi 分析。
        let order_context = bidi.line_order_context();
        // 每个已确定逻辑边界的视觉行统一应用 UAX #9 L1/L2。
        for line in &mut lines {
            // 当前行只执行一次 L1，并把结果交给 cluster 重排。
            let line_order = order_context.line(line.char_start..line.char_end);
            // 从同一视觉数据重新定位字形并取得实际行宽。
            line.width = reorder_line(
                // 传入当前行逻辑 cluster 字形。
                &mut line.glyphs,
                // 传入当前行已准备好的双向结果。
                &line_order,
            );
        }

        // 水平对齐：以 max_width（如果有限）或最大行宽度为容器
        let container_w = if max_w.is_finite() && max_w > 0.0 {
            max_w
        } else {
            lines.iter().fold(0.0f32, |m, l| m.max(l.width))
        };
        // 按逻辑行终点单调推进字符游标，避免两端对齐判断复制整段文本。
        let mut text_chars = text.chars();
        let mut text_char_cursor = 0usize;
        let mut next_text_char = text_chars.next();

        let mut all_glyphs = Vec::new();
        let mut line_infos = Vec::new();
        // 逐行应用对齐并转移最终中性字形。
        for l in &mut lines {
            // 两端对齐只扩展自动换行形成的段落非末行。
            if h_align == HAlign::Justify {
                // 段尾或显式换行字符之前的视觉行保持自然宽度。
                if l.char_end < text_char_cursor {
                    // 防御异常行序，重置为源文本起点后再单调推进。
                    text_chars = text.chars();
                    text_char_cursor = 0;
                    next_text_char = text_chars.next();
                }
                while text_char_cursor < l.char_end {
                    text_char_cursor += 1;
                    next_text_char = text_chars.next();
                }
                // 文本结束或强制换行都结束当前段落。
                let paragraph_final = next_text_char.is_none_or(|ch| matches!(ch, '\r' | '\n'));
                // 同步更新空白 advance、后续字形坐标与行宽。
                l.width = justify_line(&mut l.glyphs, l.width, container_w, paragraph_final);
            }
            let gs = all_glyphs.len();
            let align_off = match h_align {
                HAlign::Left => 0.0,
                HAlign::Center => (container_w - l.width) * 0.5,
                HAlign::Right => (container_w - l.width).max(0.0),
                // 两端对齐已经在视觉字形几何中填满容器。
                HAlign::Justify => 0.0,
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
    /// - 换行：CRLF / CR / LF 强制换行；word_wrap 使用 UAX #14，并仅为超长字母数字词保留 cluster 级紧急折行。
    /// - LineInfo：按实际行构建，每行包含正确的起止字符偏移、glyph 索引、宽度和高度。
    /// - 水平对齐：根据 opts.h_align 在容器宽度内完成左、右、居中或段落两端对齐。
    pub fn layout_text(
        &self,
        font: &FontHandle,
        text: &str,
        opts: &TextLayoutOptions,
    ) -> TextLayout {
        if !self.text_backend.is_valid(font) || text.is_empty() {
            return Self::empty_text_layout();
        }
        // 兼容既有值返回接口；内部绘制热路径使用共享入口避免深拷贝。
        (*self.layout_text_cached(font, text, opts)).clone()
    }

    /// 布局文本并共享缓存所有权，命中时只增加引用计数。
    pub fn layout_text_shared(
        &self,
        font: &FontHandle,
        text: &str,
        opts: &TextLayoutOptions,
    ) -> Arc<TextLayout> {
        if !self.text_backend.is_valid(font) || text.is_empty() {
            return Arc::clone(
                EMPTY_TEXT_LAYOUT.get_or_init(|| Arc::new(Self::empty_text_layout())),
            );
        }
        self.layout_text_cached(font, text, opts)
    }

    fn empty_text_layout() -> TextLayout {
        TextLayout {
            glyphs: Vec::new(),
            lines: Vec::new(),
            width: 0.0,
            height: 0.0,
        }
    }

    fn layout_text_cached(
        &self,
        font: &FontHandle,
        text: &str,
        opts: &TextLayoutOptions,
    ) -> Arc<TextLayout> {
        let fs = tb::bounded_font_size(opts.font_size);
        let line_h = if opts.line_height.is_finite() && opts.line_height > 0.0 {
            opts.line_height
        } else {
            tb::normal_line_height(fs)
        };
        let has_max_w = opts.max_width.is_finite() && opts.max_width > 0.0;
        let do_wrap = has_max_w && opts.word_wrap;
        // 左对齐且不换行时宽度约束既不参与断行也不产生偏移，规整为无界值以复用测量结果。
        let width_affects_layout = do_wrap || opts.h_align != HAlign::Left;
        let max_w = if has_max_w && width_affects_layout {
            opts.max_width
        } else {
            f32::MAX
        };

        // 用实际布局语义规整 key，让等价的无界参数共享同一份跨帧结果。
        let cache_opts = TextLayoutOptions {
            max_width: max_w,
            max_height: if opts.max_height.is_finite() && opts.max_height > 0.0 {
                opts.max_height
            } else {
                0.0
            },
            line_height: line_h,
            word_wrap: do_wrap,
            h_align: opts.h_align,
            v_align: opts.v_align,
            font_size: fs,
        };
        if let Some(layout) = self.layout_cache.get(font, text, &cache_opts) {
            return layout;
        }
        // 帧诊断：只统计实际执行的 shaping；缓存命中复用既有字形，
        // 不产生布局工作，计入会把纯透明度等重绘动画误报为逐帧布局。
        TEXT_LAYOUT_CALLS.fetch_add(1, Ordering::Relaxed);

        let primary_metrics = self.text_backend.horizontal_line_metrics(font, fs);
        let primary_ascent = primary_metrics
            .map(|m| m.baseline_in_line_box(line_h))
            .unwrap_or(fs * 0.8 + (line_h - fs) * 0.5);

        let seg_opts = TextLayoutOptions {
            max_width: f32::MAX,
            max_height: 0.0,
            font_size: fs,
            line_height: line_h,
            v_align: VAlign::Top,
            ..opts.clone()
        };

        // 先按字体回退选择形成不拆扩展字素簇的逻辑段。
        let font_segments = self.segment_text(font, text);
        // 对完整源文本只执行一次段落级 UAX #9 分析。
        let bidi = BidiAnalysis::new(text);
        // 在字素簇边界继续切分为单一字体且单一方向的 shaping run。
        let segments = split_font_segments(&font_segments, text, &bidi);
        // 以完整源文本构建一次 UAX #14 边界，跨字体段保持一致。
        let breaks = LineBreakMap::new(text);
        let (mut all_glyphs, mut line_infos) = self.layout_segments(
            &segments,
            text,
            // 将同一断行表传给全部字体段。
            &breaks,
            &seg_opts,
            fs,
            line_h,
            primary_ascent,
            do_wrap,
            max_w,
            opts.h_align,
            // 同一分析结果同时驱动 shaping 方向与视觉行重排。
            &bidi,
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

        let layout = Arc::new(TextLayout {
            glyphs: all_glyphs,
            lines: line_infos,
            width: max_line_width,
            height: container_height,
        });
        self.layout_cache.insert(font, text, &cache_opts, layout)
    }
}

#[cfg(test)]
#[path = "../../../../../tests-src/draw/resources/font/service/font_layout_tests.rs"]
mod tests;

