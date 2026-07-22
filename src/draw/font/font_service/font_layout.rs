//! 文本布局：按字体分段、逐段布局、行拼接与对齐。

use std::collections::HashMap;

use crate::draw::font::text_backend::{self as tb, TextLayout, TextLayoutOptions};
use crate::draw::{FontHandle, HAlign, VAlign};

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
        let mut segments: Vec<FontSegment> = Vec::new();
        let mut resolved_fonts: HashMap<char, FontHandle> = HashMap::new();
        let mut seg_start = 0usize;
        let mut seg_font = *font;

        let mut chars = text.char_indices().peekable();
        while let Some((byte_i, ch)) = chars.next() {
            if matches!(ch, '\n' | '\r') {
                if byte_i > seg_start {
                    segments.push(FontSegment {
                        byte_start: seg_start,
                        byte_end: byte_i,
                        font: seg_font,
                        line_break_chars: 0,
                    });
                }
                let mut byte_end = byte_i + ch.len_utf8();
                let mut line_break_chars = 1;
                if ch == '\r' && chars.peek().is_some_and(|(_, next)| *next == '\n') {
                    if let Some((next_i, next)) = chars.next() {
                        byte_end = next_i + next.len_utf8();
                        line_break_chars = 2;
                    }
                }
                segments.push(FontSegment {
                    byte_start: byte_i,
                    byte_end,
                    font: *font,
                    line_break_chars,
                });
                seg_start = byte_end;
                seg_font = *font;
                continue;
            }

            let use_font = *resolved_fonts
                .entry(ch)
                .or_insert_with(|| self.find_font_for_char(font, ch).unwrap_or(*font));

            if use_font.0 != seg_font.0 {
                if byte_i > seg_start {
                    segments.push(FontSegment {
                        byte_start: seg_start,
                        byte_end: byte_i,
                        font: seg_font,
                        line_break_chars: 0,
                    });
                }
                seg_start = byte_i;
                seg_font = use_font;
            }
        }

        if seg_start < text.len() {
            segments.push(FontSegment {
                byte_start: seg_start,
                byte_end: text.len(),
                font: seg_font,
                line_break_chars: 0,
            });
        }

        segments
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
            let mut source_chars = seg_text.chars().enumerate().peekable();

            // 后端 layout 的 char_index 是段内相对值。这里按字形推进，保证
            // 同一字体形成的长段也能在 max_width 内折行，而不是只能在字体段之间换行。
            for mut g in seg_layout.glyphs {
                while source_chars
                    .peek()
                    .is_some_and(|(index, _)| *index < g.char_index)
                {
                    source_chars.next();
                }
                let source_char = source_chars
                    .peek()
                    .filter(|(index, _)| *index == g.char_index)
                    .map_or('\0', |(_, ch)| *ch);
                let global_char_index = char_idx + g.char_index;
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
                if do_wrap {
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
                                    .map_or(next_char, |glyph| glyph.glyph.char_index + 1);
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
                                line.char_end = previous_char_index + 1;
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
                }
                g.x = target_x;
                g.y += cy + baseline_offset;
                g.char_index = global_char_index;
                g.font = seg.font;
                cx = cx.max(g.x + g.width);
                line.char_end = global_char_index + 1;
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
