//! 文本布局：按字体分段、逐段布局、行拼接与对齐。

use crate::draw::font::text_backend::{self as tb, TextLayout, TextLayoutOptions};
use crate::draw::{FontHandle, HAlign};

use super::FontService;

/// 布局辅助：按字体分割的文本段（追踪字节偏移）。
struct FontSegment {
    byte_start: usize,
    byte_end: usize,
    font: FontHandle,
}

impl FontService {
    /// 第 1 遍：将文本按字体分割为段，追踪字节偏移。
    fn segment_text(&self, font: &FontHandle, text: &str) -> Vec<FontSegment> {
        let mut segments: Vec<FontSegment> = Vec::new();
        let mut seg_start = 0usize;
        let mut seg_font = *font;

        for (byte_i, ch) in text.char_indices() {
            if ch == '\n' {
                if byte_i > seg_start {
                    segments.push(FontSegment {
                        byte_start: seg_start,
                        byte_end: byte_i,
                        font: seg_font,
                    });
                }
                segments.push(FontSegment {
                    byte_start: byte_i,
                    byte_end: byte_i + ch.len_utf8(),
                    font: *font,
                });
                seg_start = byte_i + ch.len_utf8();
                seg_font = *font;
                continue;
            }

            let best = self.find_font_for_char(font, ch);
            let use_font = best.unwrap_or(*font);

            if use_font.0 != seg_font.0 {
                if byte_i > seg_start {
                    segments.push(FontSegment {
                        byte_start: seg_start,
                        byte_end: byte_i,
                        font: seg_font,
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
            });
        }

        segments
    }

    /// 第 2 遍：逐段布局 → 按行拼接。
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
        #[derive(Default)]
        struct LineAccum {
            glyphs: Vec<tb::PositionedGlyph>,
            y: f32,
            height: f32,
            width: f32,
            char_start: usize,
            char_end: usize,
        }

        let mut lines: Vec<LineAccum> = Vec::new();
        let mut line = LineAccum::default();
        let mut cx = 0.0f32;
        let mut cy = 0.0f32;
        let mut char_idx = 0usize;

        for seg in segments {
            if seg.byte_start >= seg.byte_end {
                continue;
            }

            if text.as_bytes()[seg.byte_start] == b'\n' {
                if !line.glyphs.is_empty() || lines.is_empty() {
                    line.width = cx.max(line.width);
                    line.height = line_h;
                    line.y = cy;
                    line.char_end = char_idx;
                    lines.push(line);
                }
                line = LineAccum::default();
                cx = 0.0;
                cy += line_h;
                char_idx += 1;
                continue;
            }

            let seg_metrics = self.text_backend.horizontal_line_metrics(&seg.font, fs);
            let seg_ascent = seg_metrics.map(|m| m.ascent).unwrap_or(fs * 0.8);

            // 用 &str 切片代替 String 分配
            let seg_text = &text[seg.byte_start..seg.byte_end];
            let seg_layout = self.text_backend.layout_text(&seg.font, seg_text, seg_opts);
            let seg_width = seg_layout.width;
            let seg_char_count = seg_text.chars().count();

            if do_wrap && cx > 0.0 && cx + seg_width > max_w {
                line.width = cx.max(line.width);
                line.height = line_h;
                line.y = cy;
                line.char_end = char_idx;
                lines.push(line);
                line = LineAccum::default();
                cx = 0.0;
                cy += line_h;
            }

            if line.glyphs.is_empty() {
                line.char_start = char_idx;
            }

            let baseline_offset = primary_ascent - seg_ascent;

            // 后端 layout 的 char_index 是段内相对值，合并时映射到全文下标。
            for mut g in seg_layout.glyphs {
                g.x += cx;
                g.y += cy + baseline_offset;
                g.char_index += char_idx;
                g.font = seg.font;
                line.glyphs.push(g);
            }

            cx += seg_width;
            char_idx += seg_char_count;
        }

        if !line.glyphs.is_empty() || lines.is_empty() {
            line.width = cx.max(line.width);
            line.height = line_h;
            line.y = cy;
            line.char_end = char_idx;
            lines.push(line);
        }

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
                let mut shifted = *g;
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
    /// - 换行：`\n` 强制换行；word_wrap 开启时按 max_width 自动换行（段级精度）。
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

        let fs = opts.font_size.max(1.0);
        let line_h = if opts.line_height > 0.0 {
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
            ..opts.clone()
        };

        let segments = self.segment_text(font, text);
        let (all_glyphs, line_infos) = self.layout_segments(
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

        let total_height = line_infos.last().map_or(0.0, |l| l.y + l.height);
        let max_line_width = line_infos.iter().fold(0.0f32, |m, l| m.max(l.width));

        TextLayout {
            glyphs: all_glyphs,
            lines: line_infos,
            width: max_line_width,
            height: total_height,
        }
    }
}
