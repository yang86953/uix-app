//! Input widget — 单行/多行文本输入框
//!
//! 支持单行 Input 和多行 Textarea 两种模式。
//! textarea mode: Enter emits a submit semantic event, Shift+Enter inserts a newline.
//! 支持文字选择、粘贴、键盘导航、前缀/后缀图标等。

use std::borrow::Cow;
use std::cell::{Cell, RefCell};

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::resources::font::text_backend::estimate_text_metrics;
use crate::native::windowing::input::ControlSize;
use crate::ui::component::clipboard;
use crate::ui::component::paint_context::PaintContext;
use crate::ui::reactive::state::State;
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
const STATUS_MESSAGE_HEIGHT: f32 = 18.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputStatus {
    Success,
    Warning,
    Error,
}

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
        status: Option<InputStatus>,
        status_message: String,
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
        let message_height = if self.status_message.is_empty() {
            0.0
        } else {
            STATUS_MESSAGE_HEIGHT.min(frame.h.max(0.0))
        };
        let control_height = if self.textarea {
            (frame.h - message_height).max(0.0)
        } else {
            input_height(self.input_size).min((frame.h - message_height).max(0.0))
        };
        let control_frame = Rect::new(frame.x, frame.y, frame.w, control_height);
        if self.textarea {
            self.render_textarea(control_frame, ctx);
        } else {
            self.render_singleline(control_frame, ctx);
        }
        self.render_status_message(frame, control_height, ctx);
    }
}

mod input_render;
mod ext;
mod methods;

pub use self::ext::*;


// ════════════════════════════════════════════════════════════════════════════
// 公共方法
// ════════════════════════════════════════════════════════════════════════════

