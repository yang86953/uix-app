//! Reusable Unicode text editing, independent of controls, visuals and windows.
use crate::draw::resources::font::text_index::{BoundaryBias, CharIndex, TextIndexCursor};
use std::{borrow::Cow, cell::Cell};
#[derive(Default, Debug, Clone)]
pub struct TextEditState {
    value: String,
    cursor: usize,
    selection: Cell<Option<(usize, usize)>>,
    anchor: Cell<usize>,
}
impl TextEditState {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            ..Self::default()
        }
    }
    pub fn value(&self) -> &str {
        &self.value
    }
    pub fn cursor(&self) -> usize {
        self.cursor
    }
    pub fn selection(&self) -> Option<(usize, usize)> {
        self.selection.get()
    }
    pub fn edit(&mut self, multiline: bool, max_length: Option<usize>) -> TextEditor<'_> {
        TextEditor::new(
            &mut self.value,
            &mut self.cursor,
            &self.selection,
            &self.anchor,
            multiline,
            max_length,
        )
    }
    pub fn normalized_selection(text: &str, a: usize, b: usize) -> Option<(usize, usize)> {
        if a == b {
            return None;
        }
        let (start, end) =
            TextIndexCursor::new(text).normalize_selection(CharIndex(a), CharIndex(b));
        (start != end).then_some((start.0, end.0))
    }
    pub fn slice(text: &str, a: usize, b: usize) -> String {
        let index = TextIndexCursor::new(text);
        let (a, b) = index.normalize_selection(CharIndex(a), CharIndex(b));
        let (a, b) = index.char_range_to_bytes(a, b);
        text[a.0..b.0].to_owned()
    }
}
pub struct TextEditor<'a> {
    value: &'a mut String,
    cursor_char: &'a mut usize,
    selection: &'a Cell<Option<(usize, usize)>>,
    sel_anchor: &'a Cell<usize>,
    textarea: bool,
    max_length: Option<usize>,
}
impl<'a> TextEditor<'a> {
    pub fn new(
        value: &'a mut String,
        cursor: &'a mut usize,
        selection: &'a Cell<Option<(usize, usize)>>,
        anchor: &'a Cell<usize>,
        multiline: bool,
        max_length: Option<usize>,
    ) -> Self {
        *cursor = TextIndexCursor::new(value)
            .normalize_char(CharIndex(*cursor), BoundaryBias::Nearest)
            .0;
        if let Some((a, b)) = selection.get() {
            selection.set(TextEditState::normalized_selection(value, a, b));
        }
        Self {
            value,
            cursor_char: cursor,
            selection,
            sel_anchor: anchor,
            textarea: multiline,
            max_length,
        }
    }
    pub fn set_cursor(&mut self, index: usize) {
        *self.cursor_char = TextIndexCursor::new(self.value)
            .normalize_char(CharIndex(index), BoundaryBias::Nearest)
            .0;
        self.sel_anchor.set(*self.cursor_char);
        self.selection.set(None);
    }
    pub fn move_cursor_left(&mut self, ctrl: bool, extend: bool) {
        // 普通左移优先把现有选择折叠到逻辑起点。
        if !extend && self.selection.get().is_some() {
            // 读取已归一选择起点。
            let start = self.selection.get().map(|(start, _)| start).unwrap_or(0);
            // 把光标折叠到选择起点。
            (*self.cursor_char) = start;
            // 清除折叠后的选择。
            self.selection.set(None);
            // 更新后续选择锚点。
            self.sel_anchor.set(start);
            // 折叠已经完成本次移动。
            return;
        }
        if ctrl {
            // 单次正向扫描记录光标前最后一个空格分词起点。
            let mut pos = 0usize;
            let mut in_word = false;
            for (index, ch) in self.value.chars().enumerate() {
                // 光标及其后的字符不参与向左按词定位。
                if index >= (*self.cursor_char) {
                    break;
                }
                if ch == ' ' {
                    // 空格结束当前词，下一个非空格字符会建立新起点。
                    in_word = false;
                } else if !in_word {
                    // 保存最近一个词的逻辑字符起点。
                    pos = index;
                    in_word = true;
                }
            }
            // 按词结果向后收敛，禁止停在字素簇内部。
            (*self.cursor_char) = TextIndexCursor::new(&self.value)
                // 归一原始字符位置。
                .normalize_char(CharIndex(pos), BoundaryBias::Backward)
                // 保存兼容字符下标。
                .0;
        } else {
            // 普通左移一次越过整个扩展字素簇。
            (*self.cursor_char) = TextIndexCursor::new(&self.value)
                // 从当前字符位置查找前一边界。
                .previous_grapheme_boundary(CharIndex((*self.cursor_char)))
                // 保存兼容字符下标。
                .0;
        }
        // Shift 移动保留锚点并扩展选择。
        if extend {
            // 更新为完整字素簇选择范围。
            self.set_selection_range(self.sel_anchor.get(), (*self.cursor_char));
        } else {
            // 普通移动清除旧选择。
            self.selection.set(None);
            // 普通移动重置选择锚点。
            self.sel_anchor.set((*self.cursor_char));
        }
    }
    pub fn move_cursor_right(&mut self, ctrl: bool, extend: bool) {
        // 普通右移优先把现有选择折叠到逻辑终点。
        if !extend && self.selection.get().is_some() {
            // 读取已归一选择终点。
            let end = self.selection.get().map(|(_, end)| end).unwrap_or(0);
            // 把光标折叠到选择终点。
            (*self.cursor_char) = end;
            // 清除折叠后的选择。
            self.selection.set(None);
            // 更新后续选择锚点。
            self.sel_anchor.set(end);
            // 折叠已经完成本次移动。
            return;
        }
        if ctrl {
            // 单次扫描跳过光标前缀，再越过空格和紧随其后的完整单词。
            let mut pos = 0usize;
            let mut in_word = false;
            for ch in self.value.chars() {
                if pos < (*self.cursor_char) {
                    // 先把逻辑位置限制到现有文本末尾。
                    pos += 1;
                    continue;
                }
                if ch == ' ' {
                    if in_word {
                        // 已越过一个词，停在它的排他终点。
                        break;
                    }
                    // 前导空格属于本次按词移动范围。
                    pos += 1;
                } else {
                    // 越过当前词的每个非空格字符。
                    in_word = true;
                    pos += 1;
                }
            }
            // 按词结果向前收敛，禁止停在字素簇内部。
            (*self.cursor_char) = TextIndexCursor::new(&self.value)
                // 归一原始字符位置。
                .normalize_char(CharIndex(pos), BoundaryBias::Forward)
                // 保存兼容字符下标。
                .0;
        } else {
            // 普通右移一次越过整个扩展字素簇。
            (*self.cursor_char) = TextIndexCursor::new(&self.value)
                // 从当前字符位置查找后一边界。
                .next_grapheme_boundary(CharIndex((*self.cursor_char)))
                // 保存兼容字符下标。
                .0;
        }
        // Shift 移动保留锚点并扩展选择。
        if extend {
            // 更新为完整字素簇选择范围。
            self.set_selection_range(self.sel_anchor.get(), (*self.cursor_char));
        } else {
            // 普通移动清除旧选择。
            self.selection.set(None);
            // 普通移动重置选择锚点。
            self.sel_anchor.set((*self.cursor_char));
        }
    }
    pub fn insert_text_at_cursor(&mut self, text: &str) -> bool {
        // 多行输入先统一平台换行；没有回车时继续借用事件载荷。
        let normalized = if self.textarea {
            normalize_newlines(text)
        } else {
            Cow::Borrowed(text)
        };
        // 统一声明当前输入模式允许进入值状态的字符。
        let accepts = |ch: char| {
            if self.textarea {
                ch >= ' ' || ch == '\n' || ch == '\r'
            } else {
                !ch.is_control()
            }
        };
        // 常见的完整合法事件保持借用；仅在确实需要过滤时构造紧凑副本。
        let (insertion, insertion_chars) = match normalized {
            Cow::Borrowed(value) => {
                // 校验时同时计数字符，合法常见路径无需稍后再次扫描事件载荷。
                let mut char_count = 0usize;
                let valid = value.chars().all(|ch| {
                    let accepted = accepts(ch);
                    char_count += usize::from(accepted);
                    accepted
                });
                if valid {
                    (Cow::Borrowed(value), char_count)
                } else {
                    // 过滤后 UTF-8 字节数不会增长，按源长度一次预留即可避免渐进扩容。
                    let mut filtered = String::with_capacity(value.len());
                    char_count = 0;
                    for ch in value.chars().filter(|ch| accepts(*ch)) {
                        filtered.push(ch);
                        char_count += 1;
                    }
                    (Cow::Owned(filtered), char_count)
                }
            }
            Cow::Owned(mut value) => {
                // 换行规范化已经取得所有权时直接原地过滤，避免第二份字符串。
                value.retain(accepts);
                let char_count = value.chars().count();
                (Cow::Owned(value), char_count)
            }
        };
        if insertion.is_empty() {
            return false;
        }
        let replaced_selection = self.selection.get().is_some();
        if replaced_selection {
            self.delete_selection();
        }
        // 借用最终待插入片段，并记录它消费的逻辑字符数量。
        let insertion = insertion.as_ref();
        let (insertion, inserted_chars) = if let Some(max_length) = self.max_length {
            // 计算还能接收的 Unicode 标量数量。
            let available = max_length.saturating_sub(self.value.chars().count());
            // 最大长度截断只能发生在不晚于预算的完整字素簇边界。
            let safe_len = TextIndexCursor::new(insertion)
                // 从字符预算向后收敛。
                .normalize_char(CharIndex(available), BoundaryBias::Backward)
                // 提取可安全保留的字符数量。
                .0;
            // 一次字符到字节转换得到可直接交给 String 的完整前缀。
            let safe_byte_len = TextIndexCursor::new(insertion)
                .char_to_byte(CharIndex(safe_len))
                .0;
            (&insertion[..safe_byte_len], safe_len)
        } else {
            // 无长度限制直接复用校验或过滤阶段得到的标量数量。
            (insertion, insertion_chars)
        };
        if insertion.is_empty() {
            if replaced_selection {
                self.sel_anchor.set((*self.cursor_char));
                return true;
            }
            return false;
        }
        // 只定位一次插入点，随后让 String 一次搬移尾部并完成容量增长。
        let byte_pos = TextIndexCursor::new(&self.value)
            .char_to_byte(CharIndex((*self.cursor_char)))
            .0;
        self.value.insert_str(byte_pos, insertion);
        // 光标按逻辑字符而不是 UTF-8 字节推进。
        (*self.cursor_char) += inserted_chars;
        self.sel_anchor.set((*self.cursor_char));
        true
    }
    pub fn delete_selection(&mut self) {
        if let Some((s, e)) = self.selection.get() {
            // 借用删除前文本并流式归一选择与字节端点。
            let index_cursor = TextIndexCursor::new(&self.value);
            // 防御性地把选择扩展到完整字素簇边界。
            let (start, end) = index_cursor.normalize_selection(CharIndex(s), CharIndex(e));
            // 单次字符遍历转换两个合法字节端点。
            let (byte_start, byte_end) = index_cursor.char_range_to_bytes(start, end);
            // 删除完整字素簇范围。
            self.value.replace_range(byte_start.0..byte_end.0, "");
            // 光标停在删除范围原起点。
            (*self.cursor_char) = start.0;
            self.selection.set(None);
        }
    }
    pub fn delete_previous_grapheme(&mut self) -> bool {
        // 借用删除前文本，避免建立临时字素簇与字节向量。
        let index_cursor = TextIndexCursor::new(&self.value);
        // 把光标收敛到真实可停靠位置。
        let cursor = index_cursor.normalize_char(
            // 包装当前字符下标。
            CharIndex((*self.cursor_char)),
            // 旧状态使用最近边界修复。
            BoundaryBias::Nearest,
        );
        // 查找完整前一字素簇起点。
        let previous = index_cursor.previous_grapheme_boundary(cursor);
        // 文本起点没有可删除的前一字素簇。
        if previous == cursor {
            // 报告未发生删除。
            return false;
        }
        // 起点转换为 UTF-8 字节偏移。
        let (byte_start, byte_end) = index_cursor.char_range_to_bytes(previous, cursor);
        // 删除完整前一字素簇。
        self.value.replace_range(byte_start.0..byte_end.0, "");
        // 光标退到删除范围起点。
        (*self.cursor_char) = previous.0;
        // 后续 Shift 选择从新光标开始。
        self.sel_anchor.set(previous.0);
        // 报告删除成功。
        true
    }
    pub fn delete_next_grapheme(&mut self) -> bool {
        // 借用删除前文本，避免建立临时字素簇与字节向量。
        let index_cursor = TextIndexCursor::new(&self.value);
        // 把光标收敛到真实可停靠位置。
        let cursor = index_cursor.normalize_char(
            // 包装当前字符下标。
            CharIndex((*self.cursor_char)),
            // 旧状态使用最近边界修复。
            BoundaryBias::Nearest,
        );
        // 查找完整后一字素簇终点。
        let next = index_cursor.next_grapheme_boundary(cursor);
        // 文本末尾没有可删除的后一字素簇。
        if next == cursor {
            // 报告未发生删除。
            return false;
        }
        // 起点转换为 UTF-8 字节偏移。
        let (byte_start, byte_end) = index_cursor.char_range_to_bytes(cursor, next);
        // 删除完整后一字素簇。
        self.value.replace_range(byte_start.0..byte_end.0, "");
        // 光标保持在删除范围起点。
        (*self.cursor_char) = cursor.0;
        // 后续 Shift 选择从当前光标开始。
        self.sel_anchor.set(cursor.0);
        // 报告删除成功。
        true
    }
    pub fn set_selection_range(&self, a: usize, b: usize) {
        // 同一逻辑位置始终表示空选择，不因旧位置非法而扩展文本。
        if a == b {
            // 清除空选择。
            self.selection.set(None);
            // 无需构造范围。
            return;
        }
        // 把无方向选择向外扩展到完整字素簇边界。
        let (start, end) = TextIndexCursor::new(&self.value)
            // 归一显式字符索引范围。
            .normalize_selection(CharIndex(a), CharIndex(b));
        // 空范围不保留选择状态。
        if start == end {
            self.selection.set(None);
        } else {
            // 保存合法字符边界组成的选择范围。
            self.selection.set(Some((start.0, end.0)));
        }
    }
    pub fn slice_range(&self, start_char: usize, end_char: usize) -> String {
        // 借用文本并流式归一选择与字节端点。
        let index_cursor = TextIndexCursor::new(&self.value);
        // 防御性地把调用方范围扩展到完整字素簇边界。
        let (start, end) = index_cursor.normalize_selection(
            // 包装字符起点。
            CharIndex(start_char),
            // 包装字符终点。
            CharIndex(end_char),
        );
        // 把合法字符起点转换为字节偏移。
        let (byte_start, byte_end) = index_cursor.char_range_to_bytes(start, end);
        // 返回完整 UTF-8 字素簇片段。
        self.value[byte_start.0..byte_end.0].to_owned()
    }
}
fn normalize_newlines(text: &str) -> Cow<'_, str> {
    if text.contains('\r') {
        Cow::Owned(text.replace("\r\n", "\n").replace('\r', "\n"))
    } else {
        Cow::Borrowed(text)
    }
}
