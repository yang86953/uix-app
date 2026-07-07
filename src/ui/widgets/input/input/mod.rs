//! Input widget — 单行/多行文本输入框
//!
//! 支持单行 Input 和多行 Textarea 两种模式。
//! textarea mode: Enter emits a submit semantic event, Shift+Enter inserts a newline.
//! 支持文字选择、粘贴、键盘导航、前缀/后缀图标等。

use std::cell::{Cell, RefCell};

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::native::traits::input::ControlSize;
use crate::ui::clipboard;
use crate::ui::{
    ComponentId, EventResult, KeyCode, KeyMod, SemanticEvent, SystemEvent, WidgetTree,
};
use crate::ui::{SnapshotFields, SnapshotSource};

pub fn input_height(size: ControlSize) -> f32 {
    match size {
        ControlSize::Small => 24.0,
        ControlSize::Medium => 32.0,
        ControlSize::Large => 40.0,
    }
}

const PAD: f32 = 12.0;
pub(crate) const FONT_SIZE: f32 = 14.0;
const LINE_HEIGHT: f32 = 22.0;

component! {
    pub struct Input {
        value: String,
        placeholder: String,
        input_size: ControlSize,
        disabled: bool,
        focused: bool,
        hovered: bool,
        /// 当前光标所在的字符索引（全文本平展）
        cursor_char: usize,
        /// 水平滚动偏移（单行模式）
        scroll_offset_x: Cell<f32>,
        /// 垂直滚动行偏移（多行模式）
        scroll_line: Cell<usize>,
        glyph_xs: RefCell<Vec<f32>>,
        selection: Cell<Option<(usize, usize)>>,
        sel_anchor: Cell<usize>,
        sel_dragging: Cell<bool>,
        // 扩展字段
        prefix: String,
        suffix: String,
        addon_before: String,
        addon_after: String,
        password: bool,
        password_visible: bool,
        clearable: bool,
        search: bool,
        /// 多行模式
        textarea: bool,
        /// 默认显示行数
        textarea_rows: usize,
        pending_change: RefCell<Option<String>>,
        pending_submit: RefCell<Option<String>>,
    }

    tab_index => (&self) -> i32 { 1 }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if self.disabled { return EventResult::NotHandled; }
        match event {
            SystemEvent::PointerDown { pos, mods, .. } => {
                self.focused = true;
                let ci = if self.textarea {
                    self.char_at_xy(pos.x - PAD, pos.y)
                } else {
                    let text_x = pos.x - PAD + self.scroll_offset_x.get();
                    self.char_at_x(text_x)
                };
                self.cursor_char = ci;
                if mods.contains(KeyMod::SHIFT) {
                    let anchor = self.sel_anchor.get();
                    self.set_selection_range(anchor, ci);
                } else {
                    self.selection.set(None);
                    self.sel_anchor.set(ci);
                }
                self.sel_dragging.set(true);
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if !self.sel_dragging.get() { return EventResult::NotHandled; }
                let ci = if self.textarea {
                    self.char_at_xy(pos.x - PAD, pos.y)
                } else {
                    let text_x = pos.x - PAD + self.scroll_offset_x.get();
                    self.char_at_x(text_x)
                };
                self.cursor_char = ci;
                let anchor = self.sel_anchor.get();
                self.set_selection_range(anchor, ci);
                EventResult::Handled
            }
            SystemEvent::PointerUp { .. } => {
                self.sel_dragging.set(false);
                if let Some((s, e)) = self.selection.get() {
                    if s == e { self.selection.set(None); }
                }
                EventResult::Handled
            }
            SystemEvent::PointerEnter => { self.hovered = true; EventResult::Handled }
            SystemEvent::PointerLeave => { self.hovered = false; EventResult::Handled }
            SystemEvent::FocusOut => {
                self.focused = false; self.selection.set(None);
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, mods } => {
                let ctrl = mods.contains(KeyMod::CTRL);
                let shift = mods.contains(KeyMod::SHIFT);
                match key {
                    KeyCode::Enter if self.search => {
                        self.pending_submit.replace(Some(self.value.clone()));
                        self.value.clear();
                        self.cursor_char = 0;
                        EventResult::Handled
                    }
                    // textarea: Shift+Enter 换行, Enter 提交
                    KeyCode::Enter if self.textarea && shift => {
                        self.insert_at_cursor('\n');
                        self.pending_change.replace(Some(self.value.clone()));
                        EventResult::Handled
                    }
                    KeyCode::Enter if self.textarea => {
                        self.pending_submit.replace(Some(self.value.clone()));
                        // 提交后清空值（聊天场景的通用行为）
                        self.value.clear();
                        self.cursor_char = 0;
                        self.scroll_line.set(0);
                        self.selection.set(None);
                        EventResult::Handled
                    }
                    // 单行: Enter 提交
                    KeyCode::Enter => {
                        self.pending_submit.replace(Some(self.value.clone()));
                        self.value.clear();
                        self.cursor_char = 0;
                        EventResult::Handled
                    }
                    KeyCode::A if ctrl => {
                        let len = self.value.chars().count();
                        self.sel_anchor.set(0);
                        self.set_selection_range(0, len);
                        self.cursor_char = len;
                        EventResult::Handled
                    }
                    KeyCode::C if ctrl => {
                        if let Some((s, e)) = self.selection.get() {
                            clipboard::copy_to_clipboard(&self.slice_range(s, e));
                        } else { clipboard::copy_to_clipboard(&self.value); }
                        EventResult::Handled
                    }
                    KeyCode::X if ctrl => {
                        if let Some((s, e)) = self.selection.get() {
                            clipboard::copy_to_clipboard(&self.slice_range(s, e));
                            self.delete_selection();
                        }
                        EventResult::Handled
                    }
                    KeyCode::Backspace => {
                        if self.selection.get().is_some() { self.delete_selection(); }
                        else if self.cursor_char > 0 {
                            let chars: Vec<char> = self.value.chars().collect();
                            let len = chars.len();
                            if self.cursor_char > len { self.cursor_char = len; }
                            if self.cursor_char == 0 { return EventResult::NotHandled; }
                            let byte_start: usize = chars[..self.cursor_char - 1].iter().map(|c| c.len_utf8()).sum();
                            let byte_end = byte_start + chars[self.cursor_char - 1].len_utf8();
                            self.value.replace_range(byte_start..byte_end, "");
                            self.cursor_char -= 1;
                        } else { return EventResult::NotHandled; }
                        self.pending_change.replace(Some(self.value.clone()));
                        EventResult::Handled
                    }
                    KeyCode::Delete => {
                        if self.selection.get().is_some() { self.delete_selection(); }
                        else {
                            let chars: Vec<char> = self.value.chars().collect();
                            let len = chars.len();
                            if self.cursor_char > len { self.cursor_char = len; }
                            if self.cursor_char < len {
                                let byte_start: usize = chars[..self.cursor_char].iter().map(|c| c.len_utf8()).sum();
                                let byte_end = byte_start + chars[self.cursor_char].len_utf8();
                                self.value.replace_range(byte_start..byte_end, "");
                            } else { return EventResult::NotHandled; }
                        }
                        self.pending_change.replace(Some(self.value.clone()));
                        EventResult::Handled
                    }
                    KeyCode::Left => { self.move_cursor_left(ctrl); EventResult::Handled }
                    KeyCode::Right => { self.move_cursor_right(ctrl); EventResult::Handled }
                    KeyCode::Up if self.textarea => { self.move_cursor_up(); EventResult::Handled }
                    KeyCode::Down if self.textarea => { self.move_cursor_down(); EventResult::Handled }
                    KeyCode::Home => {
                        if !shift { self.selection.set(None); }
                        else { self.set_selection_range(self.sel_anchor.get(), 0); }
                        self.cursor_char = 0;
                        if !shift { self.sel_anchor.set(0); }
                        EventResult::Handled
                    }
                    KeyCode::End => {
                        let len = self.value.chars().count();
                        if !shift { self.selection.set(None); }
                        else { self.set_selection_range(self.sel_anchor.get(), len); }
                        self.cursor_char = len;
                        if !shift { self.sel_anchor.set(len); }
                        EventResult::Handled
                    }
                    KeyCode::V if ctrl => {
                        if let Some(text) = clipboard::read_text_from_clipboard() {
                            if self.insert_text_at_cursor(&text) {
                                EventResult::Handled
                            } else {
                                EventResult::NotHandled
                            }
                        } else {
                            EventResult::NotHandled
                        }
                    }
                    _ => EventResult::NotHandled,
                }
            }
            SystemEvent::TextInput { text } => {
                if self.insert_text_at_cursor(text) {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::Paste { text } => {
                if self.insert_text_at_cursor(text) {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        if let Some(value) = self.pending_submit.borrow_mut().take() {
            return Some(SemanticEvent::submit(id, value));
        }
        self.pending_change
            .borrow_mut()
            .take()
            .map(|value| SemanticEvent::change(id, value))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        if self.textarea {
            self.render_textarea(frame, ctx);
        } else {
            self.render_singleline(frame, ctx);
        }
    }
}

mod input_render;

// ════════════════════════════════════════════════════════════════════════════
// 公共方法
// ════════════════════════════════════════════════════════════════════════════

impl Input {
    fn intrinsic_size(&self) -> Size {
        if self.textarea {
            let line_count = self.value.lines().count().max(self.textarea_rows);
            let h = (line_count as f32 * LINE_HEIGHT + 16.0).max(48.0);
            Size::new(80.0, h)
        } else {
            Size::new(80.0, input_height(self.input_size))
        }
    }

    pub fn new(placeholder: impl Into<String>) -> Self {
        Self {
            value: String::new(),
            placeholder: placeholder.into(),
            input_size: ControlSize::Medium,
            disabled: false,
            focused: false,
            hovered: false,
            cursor_char: 0,
            scroll_offset_x: Cell::new(0.0),
            scroll_line: Cell::new(0),
            glyph_xs: RefCell::new(Vec::new()),
            selection: Cell::new(None),
            sel_anchor: Cell::new(0),
            sel_dragging: Cell::new(false),
            prefix: String::new(),
            suffix: String::new(),
            addon_before: String::new(),
            addon_after: String::new(),
            password: false,
            password_visible: false,
            clearable: false,
            search: false,
            textarea: false,
            textarea_rows: 3,
            pending_change: RefCell::new(None),
            pending_submit: RefCell::new(None),
        }
    }
    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self.cursor_char = self.value.chars().count();
        self.scroll_offset_x.set(0.0);
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
    pub fn value(&self) -> &str {
        &self.value
    }
    pub fn set_value(&mut self, v: impl Into<String>) {
        self.value = v.into();
        self.cursor_char = self.value.chars().count();
        self.scroll_offset_x.set(0.0);
        self.scroll_line.set(0);
        self.selection.set(None);
    }
    /// 设置输入框的聚焦状态（供 tree.build 后恢复焦点用）
    pub fn set_focused(&mut self, v: bool) {
        self.focused = v;
    }
    pub(crate) fn sync_from(&mut self, next: Self) {
        if self.value != next.value {
            self.set_value(next.value);
        }
        self.placeholder = next.placeholder;
        self.input_size = next.input_size;
        self.disabled = next.disabled;
        self.prefix = next.prefix;
        self.suffix = next.suffix;
        self.addon_before = next.addon_before;
        self.addon_after = next.addon_after;
        self.password = next.password;
        self.password_visible = next.password_visible;
        self.clearable = next.clearable;
        self.search = next.search;
        self.textarea = next.textarea;
        self.textarea_rows = next.textarea_rows;
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
    pub fn password(mut self, v: bool) -> Self {
        self.password = v;
        self
    }
    pub fn clearable(mut self, v: bool) -> Self {
        self.clearable = v;
        self
    }
    pub fn search(mut self, v: bool) -> Self {
        self.search = v;
        self
    }
    pub fn textarea(mut self, v: bool) -> Self {
        self.textarea = v;
        self.textarea_rows = if v { 3 } else { 0 };
        self
    }
    pub fn textarea_rows(mut self, n: usize) -> Self {
        self.textarea_rows = n;
        self
    }
    // ── 内部：光标移动 ──

    fn move_cursor_left(&mut self, ctrl: bool) {
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

    fn move_cursor_right(&mut self, ctrl: bool) {
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

    fn move_cursor_up(&mut self) {
        let (line, col) = self.cursor_line_col();
        if line == 0 {
            return;
        }
        let lines: Vec<&str> = self.value.lines().collect();
        let prev_line = lines[line - 1];
        let col = col.min(prev_line.chars().count());
        // 计算光标位置：之前所有行的字符数 + 换行符数 + col
        let prev_chars: usize = lines[..line - 1].iter().map(|s| s.chars().count()).sum();
        self.cursor_char = prev_chars + (line - 1) + col; // + (line-1) for newlines
        self.sel_anchor.set(self.cursor_char);
        self.selection.set(None);
    }

    fn move_cursor_down(&mut self) {
        let (line, col) = self.cursor_line_col();
        let lines: Vec<&str> = self.value.lines().collect();
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
    fn cursor_line_col(&self) -> (usize, usize) {
        let lines: Vec<&str> = self.value.lines().collect();
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

    fn insert_text_at_cursor(&mut self, text: &str) -> bool {
        let chars: Vec<char> = if self.textarea {
            text.chars()
                .filter(|&c| c >= ' ' || c == '\n' || c == '\r')
                .collect()
        } else {
            text.chars().filter(|c| !c.is_control()).collect()
        };
        if chars.is_empty() {
            return false;
        }
        if self.selection.get().is_some() {
            self.delete_selection();
        }
        for ch in chars {
            self.insert_at_cursor(ch);
        }
        self.sel_anchor.set(self.cursor_char);
        self.pending_change.replace(Some(self.value.clone()));
        true
    }

    fn char_at_x(&self, text_x: f32) -> usize {
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
    fn char_at_xy(&self, _x: f32, y: f32) -> usize {
        let lines: Vec<&str> = self.value.lines().collect();
        // y < 6.0 时（点击顶部 padding 区）映射到第 0 行，防止负数转 usize panic
        if y < 6.0 {
            return 0;
        }
        let line_idx = ((y - 6.0) / LINE_HEIGHT) as usize + self.scroll_line.get();
        let line_idx = line_idx.min(lines.len().saturating_sub(1));
        let prev: usize = lines[..line_idx].iter().map(|s| s.chars().count()).sum();
        prev + line_idx // + newlines before this line
    }

    fn set_selection_range(&self, a: usize, b: usize) {
        if a == b {
            self.selection.set(None);
        } else {
            self.selection.set(Some((a.min(b), a.max(b))));
        }
    }
    fn slice_range(&self, start_char: usize, end_char: usize) -> String {
        let chars: Vec<char> = self.value.chars().collect();
        let e = end_char.min(chars.len());
        let s = start_char.min(e);
        chars[s..e].iter().collect()
    }
    fn delete_selection(&mut self) {
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

impl SnapshotSource for Input {
    fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Input {
            placeholder: self.placeholder.clone(),
            input_size: self.input_size,
            disabled: self.disabled,
            prefix: self.prefix.clone(),
            suffix: self.suffix.clone(),
            addon_before: self.addon_before.clone(),
            addon_after: self.addon_after.clone(),
            password: self.password,
            password_visible: self.password_visible,
            clearable: self.clearable,
            search: self.search,
            textarea: self.textarea,
            textarea_rows: self.textarea_rows,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::test_harness::FakeClipboard;
    use crate::native::traits::input::IClipboard;
    use crate::ui::traits::EventHandler;
    use crate::ui::traits::WidgetLayout;

    fn install_clipboard(clipboard: &mut FakeClipboard) {
        let c: &mut dyn IClipboard = clipboard;
        let wide: *mut dyn IClipboard = c;
        let parts: (usize, usize) = unsafe { std::mem::transmute(wide) };
        clipboard::set_clipboard_parts(parts.0, parts.1);
    }

    fn clear_clipboard() {
        clipboard::set_clipboard_parts(0, 0);
    }

    #[test]
    fn ctrl_v_pastes_from_injected_clipboard() {
        let mut clipboard = FakeClipboard::new();
        clipboard.set_text("clip");
        install_clipboard(&mut clipboard);

        let mut input = Input::new("").with_value("ab");
        input.cursor_char = 1;
        let result = input.on_event(&SystemEvent::KeyDown {
            key: KeyCode::V,
            mods: KeyMod::CTRL,
        });

        assert_eq!(result, EventResult::Handled);
        assert_eq!(input.value(), "aclipb");
        clear_clipboard();
    }

    #[test]
    fn paste_event_replaces_selection() {
        let mut input = Input::new("").with_value("abcd");
        input.set_selection_range(1, 3);
        input.cursor_char = 3;

        let result = input.on_event(&SystemEvent::Paste {
            text: "XY".to_string(),
        });

        assert_eq!(result, EventResult::Handled);
        assert_eq!(input.value(), "aXYd");
    }

    #[test]
    fn measure_clamps_input_size() {
        let measured = Input::new("Search").measure(Constraints::loose(Size::new(60.0, 20.0)));

        assert_eq!(measured, Size::new(60.0, 20.0));
    }
}
