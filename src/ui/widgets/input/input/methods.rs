use super::*;

impl Input {
    pub(super) fn intrinsic_size(&self) -> Size {
        if self.textarea {
            let line_count = logical_lines(&self.value).len().max(self.textarea_rows);
            let h =
                (line_count as f32 * LINE_HEIGHT + 16.0).max(48.0) + self.status_message_height();
            Size::new(80.0, h)
        } else {
            let prefix_w = if self.prefix.is_empty() { 0.0 } else { 20.0 };
            let suffix_w = if self.suffix.is_empty() { 0.0 } else { 20.0 };
            let clear_w = if self.clearable { 20.0 } else { 0.0 };
            let password_w = if self.password { 24.0 } else { 0.0 };
            let search_w = if self.search { 24.0 } else { 0.0 };
            Size::new(
                80.0 + addon_width(&self.addon_before)
                    + addon_width(&self.addon_after)
                    + prefix_w
                    + suffix_w
                    + clear_w
                    + password_w
                    + search_w,
                input_height(self.input_size) + self.status_message_height(),
            )
        }
    }

    pub fn new(placeholder: impl Into<String>) -> Self {
        let config = crate::ui::component::config::use_config();
        let input_overrides = config.overrides.input;
        Self {
            value: String::new(),
            value_binding: None,
            placeholder: placeholder.into(),
            input_size: config.size,
            disabled: config.disabled,
            focused: false,
            hovered: false,
            composition: String::new(),
            caret_rect: Cell::new(Rect::zero()),
            cursor_char: 0,
            scroll_offset_x: Cell::new(0.0),
            scroll_line: Cell::new(0),
            glyph_xs: RefCell::new(Vec::new()),
            line_glyph_xs: RefCell::new(Vec::new()),
            selection: Cell::new(None),
            sel_anchor: Cell::new(0),
            sel_dragging: Cell::new(false),
            prefix: input_overrides.prefix.unwrap_or_default(),
            suffix: input_overrides.suffix.unwrap_or_default(),
            addon_before: String::new(),
            addon_after: String::new(),
            password: false,
            password_visible: false,
            clearable: false,
            search: false,
            status: None,
            status_message: String::new(),
            textarea: false,
            textarea_rows: 3,
            max_length: None,
            pending_change: RefCell::new(None),
            pending_submit: RefCell::new(None),
            clear_icon_rect: Cell::new(Rect::zero()),
            pwd_icon_rect: Cell::new(Rect::zero()),
        }
    }
    /// 创建多行文本输入框。
    pub fn textarea() -> Self {
        let mut input = Self::new("");
        input.textarea = true;
        input
    }
    /// 创建密码输入框。
    pub fn password() -> Self {
        let mut input = Self::new("");
        input.password = true;
        input
    }
    /// 创建搜索输入框（带搜索图标，Enter 触发搜索）。
    pub fn search() -> Self {
        let mut input = Self::new("");
        input.search = true;
        input
    }
    /// 兼容早期预览名称；新代码使用 [`Input::search`].
    #[deprecated(note = "use Input::search()")]
    pub fn search_input() -> Self {
        Self::search()
    }
    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value_binding = None;
        self.replace_value(value.into());
        self
    }
    /// 将输入框绑定到外部 `State<String>`；输入与外部更新保持双向同步。
    pub fn value(mut self, state: &State<String>) -> Self {
        self.value_binding = Some(state.clone());
        self.replace_value(state.get());
        self
    }
    pub fn size(mut self, s: ControlSize) -> Self {
        self.input_size = s;
        self
    }
    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }
    pub fn placeholder(mut self, text: impl Into<String>) -> Self {
        self.placeholder = text.into();
        self
    }
    /// 返回组件当前缓存值；controlled 用法应以绑定的 `State` 为真值来源。
    pub fn current_value(&self) -> &str {
        &self.value
    }
    pub fn set_value(&mut self, v: impl Into<String>) {
        self.replace_value(v.into());
        self.write_bound_value();
    }
    pub(super) fn replace_value(&mut self, value: String) {
        self.value = if self.textarea {
            normalize_newlines(&value).into_owned()
        } else {
            value
        };
        self.composition.clear();
        self.cursor_char = self.value.chars().count();
        self.scroll_offset_x.set(0.0);
        self.scroll_line.set(0);
        self.selection.set(None);
    }
    pub(super) fn sync_bound_value(&mut self) {
        let Some(state) = self.value_binding.as_ref() else {
            return;
        };
        let value = state.get();
        if value != self.value {
            self.replace_value(value);
        }
    }
    pub(super) fn capture_bound_value_dependency(&self) {
        if let Some(state) = self.value_binding.as_ref() {
            let _ = state.get();
        }
    }
    pub(super) fn write_bound_value(&self) {
        if let Some(state) = self.value_binding.as_ref() {
            if state.get() != self.value {
                state.set(self.value.clone());
            }
        }
    }
    pub(super) fn publish_change(&self) {
        self.write_bound_value();
        self.pending_change.replace(Some(self.value.clone()));
    }
    /// 设置输入框的聚焦状态（供 tree.build 后恢复焦点用）
    pub fn set_focused(&mut self, v: bool) {
        self.focused = v;
    }
    pub(crate) fn sync_from(&mut self, next: Self) {
        // 未绑定时保留 reconcile 前的文本、光标与焦点；controlled 模式仅在外部值
        // 真正变化时替换文本，避免无关重建打断编辑位置。
        let controlled_value = next.value_binding.as_ref().map(|_| next.value.clone());
        let textarea_changed = self.textarea != next.textarea;
        self.textarea = next.textarea;
        self.value_binding = next.value_binding;
        if let Some(value) = controlled_value {
            if value != self.value {
                self.replace_value(value);
            }
        } else if textarea_changed && self.textarea && self.value.contains('\r') {
            let normalized = normalize_newlines(&self.value).into_owned();
            self.replace_value(normalized);
        }
        self.placeholder = next.placeholder;
        self.input_size = next.input_size;
        self.disabled = next.disabled;
        self.prefix = next.prefix;
        self.suffix = next.suffix;
        self.addon_before = next.addon_before;
        self.addon_after = next.addon_after;
        let remained_password = self.password && next.password;
        self.password = next.password;
        if !remained_password {
            self.password_visible = false;
        }
        self.clearable = next.clearable;
        self.search = next.search;
        self.status = next.status;
        self.status_message = next.status_message;
        self.textarea_rows = next.textarea_rows;
        self.max_length = next.max_length;
    }
    pub(crate) fn controlled_value_changed(&self, next: &Self) -> bool {
        next.value_binding.is_some() && self.value != next.value
    }
    pub fn prefix(mut self, s: &str) -> Self {
        self.prefix = s.to_string();
        self
    }
    pub fn suffix(mut self, s: &str) -> Self {
        self.suffix = s.to_string();
        self
    }
    pub fn addon_before(mut self, s: &str) -> Self {
        self.addon_before = s.to_string();
        self
    }
    pub fn addon_after(mut self, s: &str) -> Self {
        self.addon_after = s.to_string();
        self
    }
    pub fn clearable(mut self, v: bool) -> Self {
        self.clearable = v;
        self
    }
    pub fn search_enabled(mut self, v: bool) -> Self {
        self.search = v;
        self
    }
    pub fn status(mut self, status: InputStatus) -> Self {
        self.status = Some(status);
        self
    }
    pub fn message(mut self, message: impl Into<String>) -> Self {
        self.status_message = message.into();
        self
    }
    pub fn clear_status(mut self) -> Self {
        self.status = None;
        self.status_message.clear();
        self
    }
    pub fn rows(mut self, rows: usize) -> Self {
        self.textarea = true;
        self.textarea_rows = rows.max(1);
        if self.value.contains('\r') {
            let normalized = normalize_newlines(&self.value).into_owned();
            self.replace_value(normalized);
        }
        self
    }
    pub fn max_length(mut self, max_length: usize) -> Self {
        self.max_length = Some(max_length);
        self
    }

    fn value_with_composition(&self) -> Cow<'_, str> {
        if self.composition.is_empty() {
            return Cow::Borrowed(&self.value);
        }
        let byte_pos = self
            .value
            .char_indices()
            .nth(self.cursor_char)
            .map(|(index, _)| index)
            .unwrap_or(self.value.len());
        let mut value = String::with_capacity(self.value.len() + self.composition.len());
        value.push_str(&self.value[..byte_pos]);
        value.push_str(&self.composition);
        value.push_str(&self.value[byte_pos..]);
        Cow::Owned(value)
    }

    fn status_message_height(&self) -> f32 {
        if self.status_message.is_empty() {
            0.0
        } else {
            STATUS_MESSAGE_HEIGHT
        }
    }

    pub(super) fn display_value_with_composition(&self) -> Cow<'_, str> {
        if !self.password || self.password_visible {
            return self.value_with_composition();
        }

        let byte_pos = self
            .value
            .char_indices()
            .nth(self.cursor_char)
            .map(|(index, _)| index)
            .unwrap_or(self.value.len());
        let masked = |text: &str| {
            text.chars()
                .map(|ch| if ch == '\n' { '\n' } else { '\u{2022}' })
                .collect::<String>()
        };
        let mut value = String::with_capacity(self.value.len() + self.composition.len());
        value.push_str(&masked(&self.value[..byte_pos]));
        value.push_str(&self.composition);
        value.push_str(&masked(&self.value[byte_pos..]));
        Cow::Owned(value)
    }

    pub(super) fn visual_text_before_cursor(&self) -> Cow<'_, str> {
        let byte_pos = self
            .value
            .char_indices()
            .nth(self.cursor_char)
            .map(|(index, _)| index)
            .unwrap_or(self.value.len());
        let before = &self.value[..byte_pos];
        if !self.password || self.password_visible {
            Cow::Borrowed(before)
        } else {
            Cow::Owned(
                before
                    .chars()
                    .map(|ch| if ch == '\n' { '\n' } else { '\u{2022}' })
                    .collect(),
            )
        }
    }
    // ── 内部：光标移动 ──

    pub(super) fn move_cursor_left(&mut self, ctrl: bool) {
        self.selection.set(None);
        if ctrl {
            // 跳到前一个单词
            let chars: Vec<char> = self.value.chars().collect();
            let mut pos = self.cursor_char.min(chars.len());
            pos = pos.saturating_sub(1);
            while pos > 0 && chars[pos] == ' ' {
                pos -= 1;
            }
            while pos > 0 && chars[pos - 1] != ' ' {
                pos -= 1;
            }
            self.cursor_char = pos;
        } else {
            if self.cursor_char > 0 {
                self.cursor_char -= 1;
            }
        }
        self.sel_anchor.set(self.cursor_char);
    }

    pub(super) fn move_cursor_right(&mut self, ctrl: bool) {
        self.selection.set(None);
        let len = self.value.chars().count();
        if ctrl {
            let chars: Vec<char> = self.value.chars().collect();
            let mut pos = self.cursor_char.min(chars.len());
            while pos < len && chars[pos] == ' ' {
                pos += 1;
            }
            while pos < len && chars[pos] != ' ' {
                pos += 1;
            }
            self.cursor_char = pos;
        } else {
            if self.cursor_char < len {
                self.cursor_char += 1;
            }
        }
        self.sel_anchor.set(self.cursor_char);
    }

    pub(super) fn move_cursor_up(&mut self) {
        let (line, col) = self.cursor_line_col();
        if line == 0 {
            return;
        }
        let lines = logical_lines(&self.value);
        let prev_line = lines[line - 1];
        let col = col.min(prev_line.chars().count());
        // 计算光标位置：之前所有行的字符数 + 换行符数 + col
        let prev_chars: usize = lines[..line - 1].iter().map(|s| s.chars().count()).sum();
        self.cursor_char = prev_chars + (line - 1) + col; // + (line-1) for newlines
        self.sel_anchor.set(self.cursor_char);
        self.selection.set(None);
    }

    pub(super) fn move_cursor_down(&mut self) {
        let (line, col) = self.cursor_line_col();
        let lines = logical_lines(&self.value);
        if line + 1 >= lines.len() {
            return;
        }
        let next_line = lines[line + 1];
        let col = col.min(next_line.chars().count());
        let prev_chars: usize = lines[..line + 1].iter().map(|s| s.chars().count()).sum();
        self.cursor_char = prev_chars + (line + 1) + col; // + (line+1) for newlines
        self.sel_anchor.set(self.cursor_char);
        self.selection.set(None);
    }

    /// 返回 (行号, 列号) 对应 cursor_char 的位置
    pub(super) fn cursor_line_col(&self) -> (usize, usize) {
        let lines = logical_lines(&self.value);
        let mut remaining = self.cursor_char;
        for (i, line) in lines.iter().enumerate() {
            let line_len = line.chars().count();
            // 每个换行符消耗 1 个字符位置（'\n'）
            if remaining <= line_len {
                return (i, remaining);
            }
            remaining -= line_len + 1; // +1 for the newline
        }
        (
            lines.len().saturating_sub(1),
            lines.last().map(|l| l.chars().count()).unwrap_or(0),
        )
    }

    fn insert_at_cursor(&mut self, ch: char) {
        let byte_pos = self
            .value
            .char_indices()
            .nth(self.cursor_char)
            .map(|(i, _)| i)
            .unwrap_or(self.value.len());
        self.value.insert(byte_pos, ch);
        self.cursor_char += 1;
    }

    pub(super) fn insert_text_at_cursor(&mut self, text: &str) -> bool {
        let normalized = if self.textarea {
            normalize_newlines(text)
        } else {
            Cow::Borrowed(text)
        };
        let mut chars: Vec<char> = if self.textarea {
            normalized
                .chars()
                .filter(|&c| c >= ' ' || c == '\n' || c == '\r')
                .collect()
        } else {
            normalized.chars().filter(|c| !c.is_control()).collect()
        };
        if chars.is_empty() {
            return false;
        }
        let replaced_selection = self.selection.get().is_some();
        if replaced_selection {
            self.delete_selection();
        }
        if let Some(max_length) = self.max_length {
            let available = max_length.saturating_sub(self.value.chars().count());
            chars.truncate(available);
        }
        if chars.is_empty() {
            if replaced_selection {
                self.sel_anchor.set(self.cursor_char);
                self.publish_change();
                return true;
            }
            return false;
        }
        for ch in chars {
            self.insert_at_cursor(ch);
        }
        self.sel_anchor.set(self.cursor_char);
        self.publish_change();
        true
    }

    pub(super) fn char_at_x(&self, text_x: f32) -> usize {
        let xs = self.glyph_xs.borrow();
        if xs.is_empty() {
            return 0;
        }
        for (i, &gx) in xs.iter().enumerate() {
            if text_x < gx {
                return i;
            }
        }
        xs.len()
    }

    /// 多行模式下根据 (x, y) 找字符索引
    pub(super) fn char_at_xy(&self, x: f32, y: f32) -> usize {
        let lines = logical_lines(&self.value);
        // y < 6.0 时（点击顶部 padding 区）映射到第 0 行，防止负数转 usize panic
        if y < 6.0 {
            return self.x_to_char_on_line(0, &lines, x);
        }
        let line_idx = ((y - 6.0) / LINE_HEIGHT) as usize + self.scroll_line.get();
        let line_idx = line_idx.min(lines.len().saturating_sub(1));
        self.x_to_char_on_line(line_idx, &lines, x)
    }

    /// 根据 x 坐标在该行内找字符索引
    fn x_to_char_on_line(&self, line_idx: usize, lines: &[&str], x: f32) -> usize {
        let line_glyph_xs = self.line_glyph_xs.borrow();
        let prev: usize = lines[..line_idx].iter().map(|s| s.chars().count()).sum();
        let newlines_before = line_idx; // each '\n' adds 1 char position
        let line_offset = prev + newlines_before;
        if let Some(xs) = line_glyph_xs.get(line_idx) {
            if xs.is_empty() {
                return line_offset;
            }
            for (i, &gx) in xs.iter().enumerate() {
                if x < gx {
                    // hit before this char's left edge → previous character
                    return if i == 0 {
                        line_offset
                    } else {
                        line_offset + i - 1
                    };
                }
            }
            line_offset + xs.len() - 1
        } else {
            // fallback: 无 glyph 数据时放到行首
            line_offset
        }
    }

    pub(crate) fn set_selection_range(&self, a: usize, b: usize) {
        if a == b {
            self.selection.set(None);
        } else {
            self.selection.set(Some((a.min(b), a.max(b))));
        }
    }
    pub(super) fn slice_range(&self, start_char: usize, end_char: usize) -> String {
        let chars: Vec<char> = self.value.chars().collect();
        let e = end_char.min(chars.len());
        let s = start_char.min(e);
        chars[s..e].iter().collect()
    }
    pub(super) fn delete_selection(&mut self) {
        if let Some((s, e)) = self.selection.get() {
            let chars: Vec<char> = self.value.chars().collect();
            let es = e.min(chars.len());
            let ss = s.min(es);
            let byte_s = chars[..ss].iter().map(|c| c.len_utf8()).sum::<usize>();
            let byte_e = byte_s + chars[ss..es].iter().map(|c| c.len_utf8()).sum::<usize>();
            self.value.replace_range(byte_s..byte_e, "");
            self.cursor_char = ss;
            self.selection.set(None);
        }
    }
}

