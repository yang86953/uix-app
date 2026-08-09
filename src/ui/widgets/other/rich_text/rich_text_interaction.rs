//! RichText 的 frame 命中、链接导航和文本选择内部逻辑。

use super::{LayoutLine, RichText, RichTextPointerAction, RichTextSegment};
use crate::core::{Point, Rect};

impl RichText {
    pub(super) fn local_frame(&self) -> Rect {
        self.last_frame
            .get()
            .map(|frame| Self::normalized_frame(Rect::new(0.0, 0.0, frame.w, frame.h)))
            .unwrap_or_else(Rect::zero)
    }

    pub(super) fn normalized_frame(frame: Rect) -> Rect {
        Rect::new(
            frame.x,
            frame.y,
            if frame.w.is_finite() {
                frame.w.max(0.0)
            } else {
                0.0
            },
            if frame.h.is_finite() {
                frame.h.max(0.0)
            } else {
                0.0
            },
        )
    }

    pub(super) fn pointer_action_at(&self, pos: Point) -> Option<RichTextPointerAction> {
        if !self.local_frame().contains(pos) {
            return None;
        }
        if let Some(segment_idx) = self
            .code_regions
            .borrow()
            .iter()
            .find(|region| region.rect.contains(pos))
            .map(|region| region.segment_idx)
        {
            return Some(RichTextPointerAction::CopyCode(segment_idx));
        }
        self.link_at_pos(pos)
            .map(|(segment_idx, _)| RichTextPointerAction::Link(segment_idx))
    }

    pub(super) fn commit_pointer_action(&mut self, action: RichTextPointerAction) {
        match action {
            RichTextPointerAction::Link(segment_idx) => {
                let Some(RichTextSegment::Link { url, .. }) = self.segments.get(segment_idx) else {
                    return;
                };
                if !url.trim().is_empty() {
                    self.pending_submit.replace(Some(url.clone()));
                }
            }
            RichTextPointerAction::CopyCode(segment_idx) => {
                let Some(RichTextSegment::Code { content }) = self.segments.get(segment_idx) else {
                    return;
                };
                if let Ok(mut pending_copy) = self.pending_copy.lock() {
                    *pending_copy = Some(content.clone());
                }
            }
        }
    }

    pub(super) fn char_at_pos(&self, pos: Point, lines: &[LayoutLine]) -> usize {
        for line in lines {
            if pos.y < line.y || pos.y >= line.y + line.height {
                continue;
            }
            // 按实际 x 排序字符；RTL run 内数组仍保留逻辑文本顺序供 shaping。
            let mut visual_glyphs = line.glyphs.iter().collect::<Vec<_>>();
            // 使用有限布局坐标形成稳定视觉顺序。
            visual_glyphs.sort_by(|left, right| left.x.total_cmp(&right.x));
            // 按视觉顺序判断每个字符中点。
            for glyph in &visual_glyphs {
                if pos.x < glyph.x + glyph.width * 0.5 {
                    // RTL 视觉左半区对应逻辑排他终点，LTR 对应逻辑起点。
                    return if glyph.bidi_level % 2 == 1 {
                        // RTL 字符左缘返回逻辑后一边界。
                        glyph.global_char_idx + 1
                    // LTR 字符左缘返回逻辑起点。
                    } else {
                        // LTR 字符左缘返回逻辑起点。
                        glyph.global_char_idx
                    };
                }
            }
            // 行右侧命中使用最后一个视觉字符的方向解析逻辑边界。
            if let Some(last) = visual_glyphs.last() {
                // RTL 右缘对应逻辑起点，LTR 右缘对应逻辑排他终点。
                return if last.bidi_level % 2 == 1 {
                    // RTL 字符右缘返回逻辑起点。
                    last.global_char_idx
                // LTR 字符右缘返回逻辑排他终点。
                } else {
                    // LTR 字符右缘返回逻辑排他终点。
                    last.global_char_idx + 1
                };
            }
        }
        if lines.is_empty() {
            return 0;
        }
        let mut best_line = 0;
        let mut best_dist = f32::MAX;
        for (index, line) in lines.iter().enumerate() {
            let closest_y = if pos.y < line.y {
                line.y
            } else {
                line.y + line.height
            };
            let distance = (pos.y - closest_y).abs();
            if distance < best_dist {
                best_dist = distance;
                best_line = index;
            }
        }
        // 最近视觉行仍按实际 x 排序后解析方向感知边界。
        let mut visual_glyphs = lines[best_line].glyphs.iter().collect::<Vec<_>>();
        // 使用有限布局坐标形成稳定视觉顺序。
        visual_glyphs.sort_by(|left, right| left.x.total_cmp(&right.x));
        // 查找指针左侧最近的视觉字符。
        visual_glyphs
            // 遍历视觉顺序字符。
            .iter()
            // 从右向左查找不晚于指针的字符。
            .rev()
            // 使用字符中点判断主光标侧。
            .find(|glyph| pos.x >= glyph.x + glyph.width * 0.5)
            // 按当前字符方向返回视觉右侧边界。
            .map(|glyph| {
                // RTL 视觉右侧对应逻辑起点。
                if glyph.bidi_level % 2 == 1 {
                    // 返回 RTL 逻辑起点。
                    glyph.global_char_idx
                // LTR 视觉右侧对应逻辑排他终点。
                } else {
                    // 返回 LTR 逻辑排他终点。
                    glyph.global_char_idx + 1
                }
            })
            // 指针位于整行左侧时按首字符方向返回视觉左边界。
            .unwrap_or_else(|| {
                // 查询首个视觉字符。
                visual_glyphs.first().map_or(0, |glyph| {
                    // RTL 视觉左侧对应逻辑排他终点。
                    if glyph.bidi_level % 2 == 1 {
                        // 返回 RTL 逻辑排他终点。
                        glyph.global_char_idx + 1
                    // LTR 视觉左侧对应逻辑起点。
                    } else {
                        // 返回 LTR 逻辑起点。
                        glyph.global_char_idx
                    }
                })
            })
    }

    pub(super) fn link_count(&self) -> usize {
        self.segments
            .iter()
            .filter(|segment| {
                matches!(segment, RichTextSegment::Link { url, .. } if !url.trim().is_empty())
            })
            .count()
    }

    pub(super) fn link_at_ordinal(&self, ordinal: usize) -> Option<(&str, &str)> {
        self.segments
            .iter()
            .filter_map(|segment| match segment {
                RichTextSegment::Link { content, url } if !url.trim().is_empty() => {
                    Some((content.as_str(), url.as_str()))
                }
                _ => None,
            })
            .nth(ordinal)
    }

    pub(super) fn link_segment_at_ordinal(&self, ordinal: usize) -> Option<usize> {
        self.segments
            .iter()
            .enumerate()
            .filter_map(|(index, segment)| match segment {
                RichTextSegment::Link { url, .. } if !url.trim().is_empty() => Some(index),
                _ => None,
            })
            .nth(ordinal)
    }

    pub(super) fn link_ordinal(&self, segment_idx: usize) -> Option<usize> {
        self.segments
            .iter()
            .enumerate()
            .filter(|(_, segment)| {
                matches!(segment, RichTextSegment::Link { url, .. } if !url.trim().is_empty())
            })
            .position(|(index, _)| index == segment_idx)
    }

    fn link_at_pos(&self, pos: Point) -> Option<(usize, String)> {
        let lines = self.layout_lines.borrow();
        for line in lines.iter() {
            if pos.y < line.y || pos.y >= line.y + line.height {
                continue;
            }
            for glyph in &line.glyphs {
                if glyph.is_link && pos.x >= glyph.x && pos.x < glyph.x + glyph.width {
                    if let Some(url) = glyph
                        .link_url
                        .as_deref()
                        .filter(|url| !url.trim().is_empty())
                    {
                        return Some((glyph.segment_idx, url.to_string()));
                    }
                }
            }
        }
        None
    }

    pub(super) fn set_selection_range(&self, a: usize, b: usize) {
        if a == b {
            self.selection.set(None);
        } else {
            self.selection.set(Some((a.min(b), a.max(b))));
        }
    }

    pub(super) fn extract_text_range(&self, start: usize, end: usize) -> String {
        let mut result = String::new();
        let mut offset = 0;
        for segment in &self.segments {
            let segment_length = match segment {
                RichTextSegment::NewLine => 1,
                RichTextSegment::Text { content, .. }
                | RichTextSegment::Code { content }
                | RichTextSegment::Link { content, .. } => content.chars().count(),
            };
            let segment_start = offset;
            let segment_end = offset + segment_length;
            if segment_end > start && segment_start < end {
                let local_start = start.saturating_sub(segment_start);
                let local_end = end.min(segment_end) - segment_start;
                match segment {
                    RichTextSegment::NewLine => result.push('\n'),
                    RichTextSegment::Text { content, .. }
                    | RichTextSegment::Code { content }
                    | RichTextSegment::Link { content, .. } => {
                        result.extend(
                            content
                                .chars()
                                .skip(local_start)
                                .take(local_end - local_start),
                        );
                    }
                }
            }
            offset = segment_end;
        }
        result
    }
}
