//! RichText 的 frame 命中、链接导航和文本选择内部逻辑。

// 为不分配的视觉次序比较提供全序结果。
use std::cmp::Ordering;

use super::{LayoutGlyph, LayoutLine, RichText, RichTextPointerAction, RichTextSegment};
// 复用富文本逻辑段借用与完整源文本投影。
use super::layout_metrics::segment_source_text;
// 跨样式段交互直接借用逻辑正文，不建立临时完整字符串。
use super::segmented_text::SegmentedTextCursor;
use crate::core::{Point, Rect};
// 引入共享扩展字素簇边界模型。
use crate::draw::resources::font::text_index::{BoundaryBias, CharIndex, TextIndexCursor};

// 按有限 x 坐标与原始逻辑次序复现稳定视觉排序。
fn compare_visual_order(left: &(usize, &LayoutGlyph), right: &(usize, &LayoutGlyph)) -> Ordering {
    left.1
        .x
        .total_cmp(&right.1.x)
        // 相同 x 时保留原字形数组的稳定次序。
        .then_with(|| left.0.cmp(&right.0))
}

// 返回光标中点右侧最靠左的视觉字形。
fn first_visual_glyph_after(glyphs: &[LayoutGlyph], x: f32) -> Option<&LayoutGlyph> {
    glyphs
        .iter()
        .enumerate()
        .filter(|(_, glyph)| x < glyph.x + glyph.width * 0.5)
        .min_by(compare_visual_order)
        .map(|(_, glyph)| glyph)
}

// 返回光标中点左侧最靠右的视觉字形。
fn last_visual_glyph_before(glyphs: &[LayoutGlyph], x: f32) -> Option<&LayoutGlyph> {
    glyphs
        .iter()
        .enumerate()
        .filter(|(_, glyph)| x >= glyph.x + glyph.width * 0.5)
        .max_by(compare_visual_order)
        .map(|(_, glyph)| glyph)
}

// 返回视觉次序最靠左的字形。
fn first_visual_glyph(glyphs: &[LayoutGlyph]) -> Option<&LayoutGlyph> {
    glyphs
        .iter()
        .enumerate()
        .min_by(compare_visual_order)
        .map(|(_, glyph)| glyph)
}

// 返回视觉次序最靠右的字形。
fn last_visual_glyph(glyphs: &[LayoutGlyph]) -> Option<&LayoutGlyph> {
    glyphs
        .iter()
        .enumerate()
        .max_by(compare_visual_order)
        .map(|(_, glyph)| glyph)
}

impl RichText {
    /// 设置普通文字是否允许选择；关闭时立即清除当前选择生命周期。
    pub fn selectable(mut self, value: bool) -> Self {
        // 保存调用方声明的选择能力。
        self.selectable = value;
        // 关闭能力时不得保留旧选区。
        if !value {
            self.selection.set(None);
        }
        // 关闭能力时必须重置旧选择锚点。
        if !value {
            self.sel_anchor.set(0);
        }
        // 关闭能力时不得保留拖拽会话。
        if !value {
            self.sel_dragging.set(false);
        }
        // 返回完成配置的组件。
        self
    }

    // 接收 WidgetTree 已结合祖先约束解析出的最终选择策略。
    pub(crate) fn set_user_select_policy(&mut self, value: crate::ui::UserSelect) {
        // 保存新策略供事件与跨节点参与资格共同读取。
        self.user_select_policy = value;
        // 禁止策略必须终止当前普通选区和拖选生命周期。
        if !self.selection_enabled() {
            // 清除旧选区。
            self.selection.set(None);
            // 把锚点恢复到初始位置。
            self.sel_anchor.set(0);
            // 结束旧拖选会话。
            self.sel_dragging.set(false);
        }
    }

    // 判断结构策略与 RichText 显式构建器组合后的最终选择能力。
    pub(crate) fn selection_enabled(&self) -> bool {
        // auto 保留 selectable 构建器，其余值由公开策略决定。
        self.user_select_policy.allows_text(self.selectable)
    }

    /// 返回当前实例是否参加跨节点文字选择协调。
    pub(crate) fn participates_in_cross_text_selection(&self) -> bool {
        // 使用结构策略与显式构建器组合后的最终能力。
        self.selection_enabled()
    }

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
        self.link_segment_at_pos(pos)
            .map(RichTextPointerAction::Link)
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

    // 先保留视觉布局给出的原始 shaping cluster 字符边界。
    fn raw_char_at_pos(&self, pos: Point, lines: &[LayoutLine]) -> usize {
        let pos = self.content_point(pos);
        for line in lines {
            if pos.y < line.y || pos.y >= line.y + line.height {
                continue;
            }
            // 直接寻找光标右侧最靠左的视觉字符，避免为每次指针移动复制和排序引用。
            if let Some(glyph) = first_visual_glyph_after(&line.glyphs, pos.x) {
                // RTL 视觉左半区对应逻辑排他终点，LTR 对应逻辑起点。
                return if glyph.bidi_level % 2 == 1 {
                    // RTL 字符左缘返回逻辑后一边界。
                    glyph.global_char_idx + glyph.source_char_len
                // LTR 字符左缘返回逻辑起点。
                } else {
                    // LTR 字符左缘返回逻辑起点。
                    glyph.global_char_idx
                };
            }
            // 行右侧命中使用最靠右视觉字符的方向解析逻辑边界。
            if let Some(last) = last_visual_glyph(&line.glyphs) {
                // RTL 右缘对应逻辑起点，LTR 右缘对应逻辑排他终点。
                return if last.bidi_level % 2 == 1 {
                    // RTL 字符右缘返回逻辑起点。
                    last.global_char_idx
                // LTR 字符右缘返回逻辑排他终点。
                } else {
                    // LTR 字符右缘返回逻辑排他终点。
                    last.global_char_idx + last.source_char_len
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
        // 最近视觉行直接查找指针左侧最靠右的字符，不建立临时排序表。
        last_visual_glyph_before(&lines[best_line].glyphs, pos.x)
            // 按当前字符方向返回视觉右侧边界。
            .map(|glyph| {
                // RTL 视觉右侧对应逻辑起点。
                if glyph.bidi_level % 2 == 1 {
                    // 返回 RTL 逻辑起点。
                    glyph.global_char_idx
                // LTR 视觉右侧对应逻辑排他终点。
                } else {
                    // 返回 LTR 逻辑排他终点。
                    glyph.global_char_idx + glyph.source_char_len
                }
            })
            // 指针位于整行左侧时按首字符方向返回视觉左边界。
            .unwrap_or_else(|| {
                // 查询最靠左的视觉字符。
                first_visual_glyph(&lines[best_line].glyphs).map_or(0, |glyph| {
                    // RTL 视觉左侧对应逻辑排他终点。
                    if glyph.bidi_level % 2 == 1 {
                        // 返回 RTL 逻辑排他终点。
                        glyph.global_char_idx + glyph.source_char_len
                    // LTR 视觉左侧对应逻辑起点。
                    } else {
                        // 返回 LTR 逻辑起点。
                        glyph.global_char_idx
                    }
                })
            })
    }

    // 把富文本视觉命中统一约束到扩展字素簇边界。
    pub(super) fn char_at_pos(&self, pos: Point, lines: &[LayoutLine]) -> usize {
        // 查询现有双向视觉几何给出的原始字符位置。
        let raw_index = self.raw_char_at_pos(pos, lines);
        // 直接借用全部逻辑段，命中采用最近合法跨段字素簇边界。
        SegmentedTextCursor::new(&self.segments)
            // 归一显式字符位置。
            .normalize_char(CharIndex(raw_index), BoundaryBias::Nearest)
            // 返回兼容字符下标。
            .0
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

    fn link_segment_at_pos(&self, pos: Point) -> Option<usize> {
        let pos = self.content_point(pos);
        let lines = self.layout_lines.borrow();
        for line in lines.iter() {
            if pos.y < line.y || pos.y >= line.y + line.height {
                continue;
            }
            for glyph in &line.glyphs {
                if glyph.is_link && pos.x >= glyph.x && pos.x < glyph.x + glyph.width {
                    if matches!(
                        self.segments.get(glyph.segment_idx),
                        Some(RichTextSegment::Link { url, .. }) if !url.trim().is_empty()
                    ) {
                        return Some(glyph.segment_idx);
                    }
                }
            }
        }
        None
    }

    pub(super) fn set_selection_range(&self, a: usize, b: usize) {
        // 关闭能力时任何内部协调入口都不得建立选区。
        if !self.selection_enabled() {
            // 防御性清除可能残留的旧选区。
            self.selection.set(None);
            // 结束当前范围更新。
            return;
        }
        // 同一逻辑位置始终表示空选择，不因旧位置非法而扩展文本。
        if a == b {
            // 清除空选择。
            self.selection.set(None);
            // 无需构造范围。
            return;
        }
        // 借用分段正文，把无方向选择向外扩展到完整跨段字素簇边界。
        let (start, end) = SegmentedTextCursor::new(&self.segments)
            // 归一显式字符范围。
            .normalize_selection(CharIndex(a), CharIndex(b));
        // 图片能力开启时，任何与图片 alt 相交的范围扩展到完整原子跨度。
        #[cfg(feature = "image-codecs")]
        let (start, end) = self.expand_inline_image_selection(start.0, end.0);
        // 图片能力关闭时直接使用字素簇归一结果。
        #[cfg(not(feature = "image-codecs"))]
        let (start, end) = (start.0, end.0);
        // 空范围不保留选择。
        if start == end {
            self.selection.set(None);
        } else {
            // 保存合法字符边界组成的选择范围。
            self.selection.set(Some((start, end)));
        }
    }

    // 把与任一图片 alt 相交的选择扩展到完整原子逻辑跨度。
    #[cfg(feature = "image-codecs")]
    fn expand_inline_image_selection(&self, mut start: usize, mut end: usize) -> (usize, usize) {
        // 保存当前段在完整逻辑源中的字符起点。
        let mut offset = 0usize;
        // 按公开段顺序扫描全部逻辑范围。
        for segment in &self.segments {
            // 计算当前段逻辑字符数量。
            let length = match segment {
                // 文本、代码与链接使用可见正文长度。
                RichTextSegment::Text { content, .. }
                | RichTextSegment::Code { content }
                | RichTextSegment::Link { content, .. } => content.chars().count(),
                // 图片使用完整 alt 长度。
                RichTextSegment::Image { alt, .. } => alt.chars().count(),
                // 主题分隔线不占逻辑字符。
                RichTextSegment::ThematicBreak => 0,
                // 显式换行占一个逻辑字符。
                RichTextSegment::NewLine => 1,
            };
            // 图片原子只在非空 alt 范围与选择相交时扩展。
            if matches!(segment, RichTextSegment::Image { .. })
                // 空 alt 没有可扩展逻辑范围。
                && length > 0
                // 选择终点必须晚于图片起点。
                && end > offset
                // 选择起点必须早于图片终点。
                && start < offset + length
            {
                // 向前扩展到图片 alt 起点。
                start = start.min(offset);
                // 向后扩展到图片 alt 排他终点。
                end = end.max(offset + length);
            }
            // 推进到下一段逻辑起点。
            offset += length;
        }
        // 返回保持方向归一后的完整原子范围。
        (start, end)
    }

    pub(super) fn extract_text_range(&self, start: usize, end: usize) -> String {
        // selection 字段只由 set_selection_range 写入，边界已按完整跨段字素簇归一。
        let (start, end) = if start <= end {
            // 正向范围直接沿用。
            (start, end)
        } else {
            // 反向范围交换端点后再提取。
            (end, start)
        };
        // 先计算结果的精确 UTF-8 容量，使复制只申请一次堆内存。
        let output_bytes = self.selected_source_slices(start, end, |slice| slice.len());
        // 空选择不会触发分配，非空选择按精确容量一次申请。
        let mut result = String::with_capacity(output_bytes);
        // 第二次扫描直接复制连续 UTF-8 切片，避免逐字符 extend 扩容。
        self.selected_source_slices(start, end, |slice| {
            result.push_str(slice);
            // 本轮只执行副作用，返回零避免参与容量累计。
            0
        });
        // 返回只包含可见逻辑正文的拥有型复制结果。
        result
    }

    // 遍历选择覆盖的连续段内 UTF-8 切片，并累计调用方返回值。
    fn selected_source_slices(
        &self,
        start: usize,
        end: usize,
        mut visit: impl FnMut(&str) -> usize,
    ) -> usize {
        // 保存当前段在完整逻辑源中的字符起点。
        let mut offset = 0usize;
        // 聚合容量查询结果；写入阶段保持为零。
        let mut total = 0usize;
        // 按公开段顺序扫描所有可见逻辑正文。
        for text in self.segments.iter().filter_map(segment_source_text) {
            // 当前段字符数用于把全局选择转换为局部字符范围。
            let segment_length = text.chars().count();
            // 计算当前段排他逻辑终点。
            let segment_end = offset + segment_length;
            // 只处理与非空选择真正相交的段。
            if segment_end > start && offset < end {
                // 把全局起点钳制并转换为段内字符位置。
                let local_start = start.saturating_sub(offset);
                // 把全局终点钳制并转换为段内字符位置。
                let local_end = end.min(segment_end) - offset;
                // 单遍转换两个字符端点，直接取得合法 UTF-8 切片。
                let (start_byte, end_byte) = TextIndexCursor::new(text)
                    .char_range_to_bytes(CharIndex(local_start), CharIndex(local_end));
                // 交付当前段被选择的连续字节范围。
                total += visit(&text[start_byte.0..end_byte.0]);
            }
            // 推进到下一逻辑段。
            offset = segment_end;
        }
        // 返回调用方累计值。
        total
    }
}
