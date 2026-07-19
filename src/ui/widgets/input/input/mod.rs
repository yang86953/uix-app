//! Input widget — 单行/多行文本输入框
//!
//! 支持单行 Input 和多行 Textarea 两种模式。
//! textarea mode: Enter emits a submit semantic event, Shift+Enter inserts a newline.
//! 支持文字选择、粘贴、键盘导航、前缀/后缀图标等。

use std::borrow::Cow;
use std::cell::{Cell, RefCell};

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::font::text_backend::estimate_text_metrics;
use crate::draw::painting::PaintContext;
use crate::native::traits::input::ControlSize;
use crate::ui::clipboard;
use crate::ui::state::State;
use crate::ui::{
    ComponentId, EventResult, KeyCode, KeyMod, MouseButton, SemanticEvent, SystemEvent, WidgetTree,
};
use crate::ui::{SnapshotFields, SnapshotSource};

pub fn input_height(size: ControlSize) -> f32 {
    match size {
        ControlSize::Small => 24.0,
        ControlSize::Medium => 32.0,
        ControlSize::Large => 40.0,
    }
}

pub(crate) const PAD: f32 = 12.0;
pub(crate) const FONT_SIZE: f32 = 14.0;
const LINE_HEIGHT: f32 = 22.0;
const ADDON_FONT_SIZE: f32 = 13.0;
const ADDON_HORIZONTAL_PADDING: f32 = 16.0;

fn logical_lines(text: &str) -> Vec<&str> {
    text.split('\n').collect()
}

fn normalize_newlines(text: &str) -> Cow<'_, str> {
    if !text.contains('\r') {
        return Cow::Borrowed(text);
    }
    Cow::Owned(text.replace("\r\n", "\n").replace('\r', "\n"))
}

fn addon_width(text: &str) -> f32 {
    if text.is_empty() {
        0.0
    } else {
        estimate_text_metrics(text, f32::INFINITY, ADDON_FONT_SIZE).max_line_width
            + ADDON_HORIZONTAL_PADDING
    }
}

component! {
    pub struct Input {
        value: String,
        value_binding: Option<State<String>>,
        pub(crate) placeholder: String,
        input_size: ControlSize,
        disabled: bool,
        pub(crate) focused: bool,
        hovered: bool,
        pub(crate) composition: String,
        pub(crate) caret_rect: Cell<Rect>,
        /// 当前光标所在的字符索引（全文本平展）
        pub(crate) cursor_char: usize,
        /// 水平滚动偏移（单行模式）
        scroll_offset_x: Cell<f32>,
        /// 垂直滚动行偏移（多行模式）
        scroll_line: Cell<usize>,
        glyph_xs: RefCell<Vec<f32>>,
        /// 多行模式每行 glyph x 位置（行索引 → glyph x 数组）
        line_glyph_xs: RefCell<Vec<Vec<f32>>>,
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
        /// 用户输入允许的最大 Unicode 字符数；`None` 表示不限制。
        max_length: Option<usize>,
        pending_change: RefCell<Option<String>>,
        pending_submit: RefCell<Option<String>>,
        /// 清除按钮区域（用于命中检测）。
        pub(crate) clear_icon_rect: Cell<Rect>,
        /// 密码眼睛图标区域（用于命中检测）
        pub(crate) pwd_icon_rect: Cell<Rect>,
    }

    tab_index => (&self) -> i32 { 1 }

    accepts_text_input => (&self) -> bool { !self.disabled }

    text_input_cursor_rect => (&self) -> Rect { self.caret_rect.get() }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }


    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if self.disabled { return EventResult::NotHandled; }
        self.sync_bound_value();
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                mods,
            } => {
                if self.clearable
                    && !self.value.is_empty()
                    && self.clear_icon_rect.get().contains(*pos)
                {
                    self.replace_value(String::new());
                    self.publish_change();
                    return EventResult::Handled;
                }
                // 密码眼睛图标命中
                if self.password && self.pwd_icon_rect.get().contains(*pos) {
                    self.password_visible = !self.password_visible;
                    return EventResult::Handled;
                }
                let ci = if self.textarea {
                    self.char_at_xy(pos.x - PAD, pos.y)
                } else {
                    let text_x = pos.x - PAD + self.scroll_offset_x.get();
                    self.char_at_x(text_x)
                }
                .min(self.value.chars().count());
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
                }
                .min(self.value.chars().count());
                self.cursor_char = ci;
                let anchor = self.sel_anchor.get();
                self.set_selection_range(anchor, ci);
                EventResult::Handled
            }
            SystemEvent::PointerUp {
                button: MouseButton::Left,
                ..
            } => {
                self.sel_dragging.set(false);
                if let Some((s, e)) = self.selection.get() {
                    if s == e { self.selection.set(None); }
                }
                EventResult::Handled
            }
            SystemEvent::PointerEnter => { self.hovered = true; EventResult::Handled }
            SystemEvent::PointerLeave => { self.hovered = false; EventResult::Handled }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.composition.clear();
                self.selection.set(None);
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
                        self.write_bound_value();
                        EventResult::Handled
                    }
                    // textarea: Shift+Enter 换行, Enter 提交
                    KeyCode::Enter if self.textarea && shift => {
                        if self.insert_text_at_cursor("\n") {
                            EventResult::Handled
                        } else {
                            EventResult::NotHandled
                        }
                    }
                    KeyCode::Enter if self.textarea => {
                        self.pending_submit.replace(Some(self.value.clone()));
                        // 提交后清空值（聊天场景的通用行为）
                        self.value.clear();
                        self.cursor_char = 0;
                        self.scroll_line.set(0);
                        self.selection.set(None);
                        self.write_bound_value();
                        EventResult::Handled
                    }
                    // 单行: Enter 提交
                    KeyCode::Enter => {
                        self.pending_submit.replace(Some(self.value.clone()));
                        self.value.clear();
                        self.cursor_char = 0;
                        self.write_bound_value();
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
                        if self.password && !self.password_visible {
                            // 密码隐藏态禁止复制明文
                            EventResult::Handled
                        } else if let Some((s, e)) = self.selection.get() {
                            clipboard::copy_to_clipboard(&self.slice_range(s, e));
                            EventResult::Handled
                        } else {
                            clipboard::copy_to_clipboard(&self.value);
                            EventResult::Handled
                        }
                    }
                    KeyCode::X if ctrl => {
                        if self.password && !self.password_visible {
                            EventResult::Handled
                        } else if let Some((s, e)) = self.selection.get() {
                            clipboard::copy_to_clipboard(&self.slice_range(s, e));
                            self.delete_selection();
                            self.publish_change();
                            EventResult::Handled
                        } else {
                            EventResult::Handled
                        }
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
                        self.publish_change();
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
                        self.publish_change();
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
                self.composition.clear();
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
            SystemEvent::ImeCompositionStart => {
                self.composition.clear();
                EventResult::Handled
            }
            SystemEvent::ImeCompositionUpdate { text } => {
                self.composition.clone_from(text);
                EventResult::Handled
            }
            SystemEvent::ImeCompositionEnd { .. } => {
                self.composition.clear();
                EventResult::Handled
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
        self.capture_bound_value_dependency();
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
            let line_count = logical_lines(&self.value).len().max(self.textarea_rows);
            let h = (line_count as f32 * LINE_HEIGHT + 16.0).max(48.0);
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
                input_height(self.input_size),
            )
        }
    }

    pub fn new(placeholder: impl Into<String>) -> Self {
        let config = crate::ui::config::use_config();
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
    pub fn search_input() -> Self {
        let mut input = Self::new("");
        input.search = true;
        input
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
    fn replace_value(&mut self, value: String) {
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
    fn sync_bound_value(&mut self) {
        let Some(state) = self.value_binding.as_ref() else {
            return;
        };
        let value = state.get();
        if value != self.value {
            self.replace_value(value);
        }
    }
    fn capture_bound_value_dependency(&self) {
        if let Some(state) = self.value_binding.as_ref() {
            let _ = state.get();
        }
    }
    fn write_bound_value(&self) {
        if let Some(state) = self.value_binding.as_ref() {
            if state.get() != self.value {
                state.set(self.value.clone());
            }
        }
    }
    fn publish_change(&self) {
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
    pub fn search(mut self, v: bool) -> Self {
        self.search = v;
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

    fn display_value_with_composition(&self) -> Cow<'_, str> {
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

    fn visual_text_before_cursor(&self) -> Cow<'_, str> {
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
        let lines = logical_lines(&self.value);
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
    fn cursor_line_col(&self) -> (usize, usize) {
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

    fn insert_text_at_cursor(&mut self, text: &str) -> bool {
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
    fn char_at_xy(&self, x: f32, y: f32) -> usize {
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
            max_length: self.max_length,
        }
    }
}
