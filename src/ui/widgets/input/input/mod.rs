//! Input widget — 单行/多行文本输入框
//!
//! 支持单行 Input 和多行 Textarea 两种模式。
//! textarea mode: Enter emits a submit semantic event, Shift+Enter inserts a newline.
//! 支持文字选择、粘贴、键盘导航、前缀/后缀图标等。

use std::borrow::Cow;
use std::cell::{Cell, RefCell};

use crate::core::{Constraints, Rect, Size};
use crate::draw::resources::font::text_backend::estimate_text_metrics;
use crate::widget;
// 保存单行真实 shaping 字形，以便方向感知命中。
use crate::draw::resources::font::text_backend::PositionedGlyph;
// 引入可停靠字素簇边界与显式字符索引。
use crate::draw::resources::font::text_index::{BoundaryBias, CharIndex, TextIndexCursor};
use crate::platform::windowing::ControlSize;
use crate::ui::reactive::state::State;
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::clipboard;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{
    EventResult, KeyCode, KeyMod, MouseButton, SemanticEvent, SystemEvent, WidgetId, WidgetTree,
};
use crate::ui::{SnapshotFields, SnapshotSource};

/// 返回指定控件尺寸对应的输入框标准高度。
pub fn input_height(size: ControlSize) -> f32 {
    INPUT_VISUAL_REF.layout.control_height(size)
}

/// 输入框即时状态。
///
/// 与 ValidateStatus/StepStatus/BadgeStatus/UploadStatus 共享「组件状态」命名模式，
/// 但各自语义与变体独立（本枚举仅输入反馈三态，无校验专用态），勿强行合并。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputStatus {
    /// 表示输入内容有效或操作成功。
    Success,
    /// 表示输入内容需要用户注意。
    Warning,
    /// 表示输入内容无效或操作失败。
    Error,
}

fn logical_line_count(text: &str) -> usize {
    // UTF-8 中换行标量保持单字节，直接计数避免构造切片集合或解码其他字符。
    text.as_bytes()
        .iter()
        .filter(|&&byte| byte == b'\n')
        .count()
        + 1
}

/// 以 Unicode 字符索引线性推进逻辑行，避免渲染时反复扫描所有前置行。
struct LogicalLineCursor<'a> {
    lines: std::str::Split<'a, char>,
    next_start: usize,
}

impl<'a> LogicalLineCursor<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            lines: text.split('\n'),
            next_start: 0,
        }
    }

    /// 返回仍存在的当前行、全文起点与全文终点。
    fn next_existing_line(&mut self) -> Option<(&'a str, usize, usize)> {
        let line = self.lines.next()?;
        let start = self.next_start;
        let end = start + line.chars().count();
        self.next_start = end + 1;
        Some((line, start, end))
    }

    /// 返回当前行；耗尽后继续按空行推进以匹配额外显示行。
    fn next_line(&mut self) -> (&'a str, usize, usize) {
        if let Some(line) = self.next_existing_line() {
            return line;
        }
        let start = self.next_start;
        self.next_start += 1;
        ("", start, start)
    }

    fn skip_lines(&mut self, count: usize) {
        for _ in 0..count {
            let _ = self.next_line();
        }
    }

    /// 返回目标行；目标越界时收敛到最后一个真实逻辑行。
    fn line_at_or_last(&mut self, target: usize) -> (usize, &'a str, usize, usize) {
        // split 对任意字符串至少产生一行。
        let (line, start, end) = self.next_existing_line().unwrap_or(("", 0, 0));
        let mut last = (0, line, start, end);
        for index in 1..=target {
            let Some((line, start, end)) = self.next_existing_line() else {
                break;
            };
            last = (index, line, start, end);
        }
        last
    }
}

fn normalize_newlines(text: &str) -> Cow<'_, str> {
    if !text.contains('\r') {
        return Cow::Borrowed(text);
    }
    Cow::Owned(text.replace("\r\n", "\n").replace('\r', "\n"))
}

fn addon_width(text: &str, visual: &InputVisual) -> f32 {
    if text.is_empty() {
        0.0
    } else {
        estimate_text_metrics(text, f32::INFINITY, visual.typography.addon_font_size).max_line_width
            + visual.layout.addon_horizontal_padding
    }
}

widget! {
    /// 拥有文本编辑、选区、校验展示与可选双向绑定的输入组件。
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
        /// 单行模式的视觉字形簇，命中时不得按字形数组下标冒充字符下标。
        glyphs: RefCell<Vec<PositionedGlyph>>,
        /// 多行模式每行 glyph x 位置（行索引 → glyph x 数组）
        /// 多行模式逐行保存真实 shaping 字形簇，供方向感知命中使用。
        line_glyphs: RefCell<Vec<Vec<PositionedGlyph>>>,
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
        // 同目录 UIX 生成的唯一静态视觉表。
        #[snapshot(skip)]
        visual: &'static InputVisual,
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
                    self.char_at_xy(pos.x - self.visual.layout.horizontal_padding, pos.y)
                } else {
                    let text_x = pos.x - self.visual.layout.horizontal_padding
                        + self.scroll_offset_x.get();
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
                    self.char_at_xy(pos.x - self.visual.layout.horizontal_padding, pos.y)
                } else {
                    let text_x = pos.x - self.visual.layout.horizontal_padding
                        + self.scroll_offset_x.get();
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
                        // 无选择时删除前一个完整扩展字素簇。
                        else if !self.delete_previous_grapheme() { return EventResult::NotHandled; }
                        self.publish_change();
                        EventResult::Handled
                    }
                    KeyCode::Delete => {
                        if self.selection.get().is_some() { self.delete_selection(); }
                        // 无选择时删除后一个完整扩展字素簇。
                        else if !self.delete_next_grapheme() { return EventResult::NotHandled; }
                        self.publish_change();
                        EventResult::Handled
                    }
                    // 左移同时传递 Shift 扩展选择语义。
                    KeyCode::Left => { self.move_cursor_left(ctrl, shift); EventResult::Handled }
                    // 右移同时传递 Shift 扩展选择语义。
                    KeyCode::Right => { self.move_cursor_right(ctrl, shift); EventResult::Handled }
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

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
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
            self.visual.chrome.status_message_height.min(frame.h.max(0.0))
        };
        let control_height = if self.textarea {
            (frame.h - message_height).max(0.0)
        } else {
            input_height(self.input_size).min((frame.h - message_height).max(0.0))
        };
        let control_frame = Rect::new(frame.x, frame.y, frame.w, control_height);
        // 单行、多行与状态消息共享同帧一次 UIX 主题解析。
        let visual = self.visual.resolve(ctx.tokens());
        if self.textarea {
            self.render_textarea(control_frame, ctx, visual);
        } else {
            self.render_singleline(control_frame, ctx, visual);
        }
        self.render_status_message(frame, control_height, ctx, visual);
    }
}

mod ext;
mod input_render;
mod methods;
// 声明 Input 的 UIX 静态视觉与主题解析模块。
mod presentation;
// 仅在单元测试中编译输入控件字素簇交互回归。
#[cfg(test)]
// 使用独立文件避免继续膨胀核心控件模块。
#[path = "../../../../../tests/unit/ui/widgets/input/input/grapheme_tests.rs"]
// 注册输入控件字素簇测试模块。
mod grapheme_tests;

pub use self::ext::*;
use presentation::*;

// 把 Input Rust 文本编辑内核与 UIX 静态视觉组合为单一组件节点。
fn build_input_view(mut kernel: Input, visual: &'static InputVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

// 为 UIX 根提供稳定的 Rust 内核绑定名称。
fn build_input_uix_root(kernel: Input) -> ViewNode {
    crate::uix!("src/ui/widgets/input/input/input.uix")
}

impl View for Input {
    fn build(self) -> ViewNode {
        build_input_uix_root(self)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 公共方法
// ════════════════════════════════════════════════════════════════════════════
