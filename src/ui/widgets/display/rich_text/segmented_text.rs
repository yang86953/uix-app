//! 在不拼接正文的前提下，为富文本段提供统一 Unicode 字素簇边界。

// 使用 unicode-segmentation 面向 rope/chunk 的标准游标接口。
use unicode_segmentation::{GraphemeCursor, GraphemeIncomplete};

// 借用各段的逻辑正文，不复制样式与资源元数据。
use super::RichTextSegment;
use super::layout_metrics::segment_source_text;
// 复用共享文本索引的显式字符下标与边界偏向契约。
use crate::draw::resources::font::text_index::{BoundaryBias, CharIndex};

// 保存一个非空逻辑文本块及其在虚拟连续 UTF-8 字符串中的起点。
#[derive(Clone, Copy)]
struct TextChunk<'a> {
    // 当前块的全局 UTF-8 字节起点。
    start: usize,
    // 当前块直接借用某个富文本段或静态换行。
    text: &'a str,
}

impl TextChunk<'_> {
    // 返回当前块的全局排他字节终点。
    fn end(self) -> usize {
        self.start + self.text.len()
    }
}

/// 借用富文本段并执行一次或少量跨段 Unicode 边界查询。
#[derive(Clone, Copy)]
pub(super) struct SegmentedTextCursor<'a> {
    // 保留公开段列表的只读借用。
    segments: &'a [RichTextSegment],
    // 缓存虚拟连续正文的 UTF-8 总字节数。
    byte_len: usize,
    // 缓存虚拟连续正文的 Unicode 标量总数。
    char_len: usize,
}

impl<'a> SegmentedTextCursor<'a> {
    /// 创建不拥有正文、不建立边界表的分段文本游标。
    pub(super) fn new(segments: &'a [RichTextSegment]) -> Self {
        // 同一次扫描累计字节与字符长度，避免后续重复求全文长度。
        let (byte_len, char_len) = segments
            .iter()
            .filter_map(segment_source_text)
            .fold((0usize, 0usize), |(bytes, chars), text| {
                (bytes + text.len(), chars + text.chars().count())
            });
        // 只保存段借用与两个标量长度。
        Self {
            segments,
            byte_len,
            char_len,
        }
    }

    /// 按指定偏向把任意字符位置归一为跨段扩展字素簇边界。
    pub(super) fn normalize_char(self, index: CharIndex, bias: BoundaryBias) -> CharIndex {
        // 先把越界字符位置收敛到虚拟正文末尾并转换为字节边界。
        let target_char = index.0.min(self.char_len);
        let target_byte = self.char_to_byte(target_char);
        // 精确边界无需向任一侧移动。
        if self.is_grapheme_boundary(target_byte) {
            return CharIndex(target_char);
        }
        // 内部位置分别寻找前后完整字素簇边界。
        let backward = self.byte_to_char(self.previous_grapheme_boundary(target_byte));
        let forward = self.byte_to_char(self.next_grapheme_boundary(target_byte));
        // 使用与连续 TextIndexCursor 相同的字符距离与平局前进规则。
        CharIndex(match bias {
            BoundaryBias::Backward => backward,
            BoundaryBias::Forward => forward,
            BoundaryBias::Nearest if target_char - backward < forward - target_char => backward,
            BoundaryBias::Nearest => forward,
        })
    }

    /// 把无方向字符区间向外扩展到完整跨段扩展字素簇边界。
    pub(super) fn normalize_selection(self, a: CharIndex, b: CharIndex) -> (CharIndex, CharIndex) {
        // 消除方向并把两个端点收敛到虚拟正文字符长度。
        let start_char = a.0.min(b.0).min(self.char_len);
        let end_char = a.0.max(b.0).min(self.char_len);
        // 转换到 GraphemeCursor 使用的全局 UTF-8 字节空间。
        let start_byte = self.char_to_byte(start_char);
        let end_byte = self.char_to_byte(end_char);
        // 内部起点向后扩展，精确边界保持不动。
        let start = if self.is_grapheme_boundary(start_byte) {
            start_char
        } else {
            self.byte_to_char(self.previous_grapheme_boundary(start_byte))
        };
        // 内部终点向前扩展，精确边界保持不动。
        let end = if self.is_grapheme_boundary(end_byte) {
            end_char
        } else {
            self.byte_to_char(self.next_grapheme_boundary(end_byte))
        };
        // 返回与共享连续文本索引相同的字符下标契约。
        (CharIndex(start), CharIndex(end))
    }

    // 按声明顺序生成非空逻辑块及其虚拟连续字节起点。
    fn chunks(self) -> impl Iterator<Item = TextChunk<'a>> {
        // map 闭包内部保存下一个块的全局字节起点。
        let mut start = 0usize;
        self.segments
            .iter()
            .filter_map(segment_source_text)
            // 空段不满足 GraphemeCursor 的非空 chunk 前提，也不改变偏移。
            .filter(|text| !text.is_empty())
            .map(move |text| {
                let chunk = TextChunk { start, text };
                start += text.len();
                chunk
            })
    }

    // 返回包含给定位置后继字符的块；段边界选择右侧块。
    fn chunk_after(self, offset: usize) -> Option<TextChunk<'a>> {
        self.chunks()
            .find(|chunk| chunk.start <= offset && offset < chunk.end())
    }

    // 返回包含给定位置前驱字符的块；段边界选择左侧块。
    fn chunk_before(self, offset: usize) -> Option<TextChunk<'a>> {
        self.chunks()
            .find(|chunk| chunk.start < offset && offset <= chunk.end())
    }

    // 返回恰好结束于请求位置的前文块，必要时借用段内前缀。
    fn context_ending_at(self, offset: usize) -> Option<TextChunk<'a>> {
        let chunk = self.chunk_before(offset)?;
        // 请求位置由 unicode-segmentation 保证落在 UTF-8 字符边界。
        let local_end = offset - chunk.start;
        Some(TextChunk {
            start: chunk.start,
            text: &chunk.text[..local_end],
        })
    }

    // 把字符下标转换为虚拟连续 UTF-8 字节位置。
    fn char_to_byte(self, target: usize) -> usize {
        // 保存已经越过的全局字符数量。
        let mut char_offset = 0usize;
        // 逐块寻找包含目标字符边界的正文。
        for chunk in self.chunks() {
            let chunk_chars = chunk.text.chars().count();
            if target <= char_offset + chunk_chars {
                let local_char = target - char_offset;
                let local_byte = chunk
                    .text
                    .char_indices()
                    .nth(local_char)
                    .map(|(byte, _)| byte)
                    .unwrap_or(chunk.text.len());
                return chunk.start + local_byte;
            }
            char_offset += chunk_chars;
        }
        // 空文本与越界输入统一收敛到虚拟正文末尾。
        self.byte_len
    }

    // 把已知 UTF-8 字符边界转换为全局字符下标。
    fn byte_to_char(self, target: usize) -> usize {
        // 防御性地把字节位置收敛到虚拟正文长度。
        let target = target.min(self.byte_len);
        // 保存已经越过的全局字符数量。
        let mut char_offset = 0usize;
        // 逐块累计目标字节之前的 Unicode 标量。
        for chunk in self.chunks() {
            if target <= chunk.end() {
                return char_offset + chunk.text[..target - chunk.start].chars().count();
            }
            char_offset += chunk.text.chars().count();
        }
        // 空文本或末尾位置返回完整字符长度。
        self.char_len
    }

    // 查询给定 UTF-8 字节位置是否为完整扩展字素簇边界。
    fn is_grapheme_boundary(self, offset: usize) -> bool {
        // 文本两端永远是合法边界，无需向 GraphemeCursor 提供空块。
        if offset == 0 || offset == self.byte_len {
            return true;
        }
        let mut cursor = GraphemeCursor::new(offset, self.byte_len, true);
        // 最坏情况下每个段可能各触发一次上下文补充，设置有界防护。
        for _ in 0..self.cursor_step_limit() {
            let Some(chunk) = self.chunk_after(cursor.cur_cursor()) else {
                // 缺失后继块表示内部映射不完整，保守地禁止拆分。
                return false;
            };
            match cursor.is_boundary(chunk.text, chunk.start) {
                Ok(is_boundary) => return is_boundary,
                Err(GraphemeIncomplete::PreContext(requested)) => {
                    if !self.provide_context(&mut cursor, requested) {
                        return false;
                    }
                }
                // is_boundary 不应请求前后移动块或收到无效位置。
                Err(_) => return false,
            }
        }
        // 异常长上下文链保守视为同一字素簇，避免拆分用户可见字符。
        false
    }

    // 返回严格早于给定内部位置的扩展字素簇字节边界。
    fn previous_grapheme_boundary(self, offset: usize) -> usize {
        let mut cursor = GraphemeCursor::new(offset, self.byte_len, true);
        for _ in 0..self.cursor_step_limit() {
            let Some(chunk) = self.chunk_before(cursor.cur_cursor()) else {
                return 0;
            };
            match cursor.prev_boundary(chunk.text, chunk.start) {
                Ok(Some(boundary)) => return boundary,
                Ok(None) => return 0,
                Err(GraphemeIncomplete::PrevChunk) => {}
                Err(GraphemeIncomplete::PreContext(requested)) => {
                    if !self.provide_context(&mut cursor, requested) {
                        return 0;
                    }
                }
                // 其他结果表示内部 chunk 映射不满足库契约，向文本起点扩展。
                Err(_) => return 0,
            }
        }
        // 防护触发时向起点扩展，仍不产生破碎字素簇。
        0
    }

    // 返回严格晚于给定内部位置的扩展字素簇字节边界。
    fn next_grapheme_boundary(self, offset: usize) -> usize {
        let mut cursor = GraphemeCursor::new(offset, self.byte_len, true);
        for _ in 0..self.cursor_step_limit() {
            let Some(chunk) = self.chunk_after(cursor.cur_cursor()) else {
                return self.byte_len;
            };
            match cursor.next_boundary(chunk.text, chunk.start) {
                Ok(Some(boundary)) => return boundary,
                Ok(None) => return self.byte_len,
                Err(GraphemeIncomplete::NextChunk) => {}
                Err(GraphemeIncomplete::PreContext(requested)) => {
                    if !self.provide_context(&mut cursor, requested) {
                        return self.byte_len;
                    }
                }
                // 其他结果表示内部 chunk 映射不满足库契约，向文本末尾扩展。
                Err(_) => return self.byte_len,
            }
        }
        // 防护触发时向末尾扩展，仍不产生破碎字素簇。
        self.byte_len
    }

    // 向 GraphemeCursor 提供恰好结束于请求位置的借用前文。
    fn provide_context(self, cursor: &mut GraphemeCursor, requested: usize) -> bool {
        let Some(context) = self.context_ending_at(requested) else {
            return false;
        };
        cursor.provide_context(context.text, context.start);
        true
    }

    // 限制第三方游标异常状态下的重试次数，不影响正常跨段链长度。
    fn cursor_step_limit(self) -> usize {
        self.segments.len().saturating_mul(4).saturating_add(8)
    }
}
