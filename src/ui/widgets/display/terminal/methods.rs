use crate::core::{Rect, Size};
use crate::ui::virtualization::virtual_scroll::VirtualListScroll;
use crate::ui::{EventResult, SnapshotFields};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::{TERMINAL_VISUAL_REF, Terminal, TerminalLine};

// 提交历史保留上限：超过后丢弃最旧命令，防止长期会话无界增长。
const HISTORY_LIMIT: usize = 100;

impl Terminal {
    // 终端是滚动视口：内在尺寸为固定默认尺寸，不随输出行数增长。
    pub(crate) fn intrinsic_size(&self) -> Size {
        Size::new(
            self.view_style
                .width
                .unwrap_or(self.visual.geometry.default_width),
            self.view_style
                .height
                .unwrap_or(self.visual.geometry.default_height),
        )
    }

    // 公共布局声明通过同一内核测量，显式零 Flex 权重同样生效。
    pub(crate) fn apply_view_layout_style(
        &mut self,
        style: &crate::ui::theme::style::Style,
        flex_grow: Option<f32>,
        flex_shrink: Option<f32>,
    ) {
        self.view_style = crate::ui::theme::style::Style {
            width: style.width,
            height: style.height,
            flex_grow: style.flex_grow,
            flex_shrink: style.flex_shrink,
            margin: style.margin,
            align_self: style.align_self,
            ..Default::default()
        };
        if let Some(value) = flex_grow {
            self.view_style.flex_grow = value;
        }
        if let Some(value) = flex_shrink {
            self.view_style.flex_shrink = value;
        }
    }

    // 语义快照不保存颜色；视觉配置比较必须直接读取完整文本段。
    pub(crate) fn reconcile_config_changed(&self, next: &Self) -> bool {
        self.lines != next.lines
            || self.prompt != next.prompt
            || self.visual != next.visual
            || self.view_style != next.view_style
    }

    pub(crate) fn reconcile_layout_changed(&self, next: &Self) -> bool {
        self.view_style != next.view_style || self.visual.geometry != next.visual.geometry
    }

    pub(crate) fn local_frame(&self) -> Rect {
        self.last_frame
            .get()
            .map(|frame| Self::normalized_frame(Rect::new(0.0, 0.0, frame.w, frame.h)))
            .unwrap_or_else(|| {
                Rect::new(
                    0.0,
                    0.0,
                    self.visual.geometry.default_width,
                    self.visual.geometry.default_height,
                )
            })
    }

    pub(crate) fn normalized_frame(frame: Rect) -> Rect {
        Rect::new(
            frame.x,
            frame.y,
            if frame.w.is_finite() { frame.w.max(0.0) } else { 0.0 },
            if frame.h.is_finite() { frame.h.max(0.0) } else { 0.0 },
        )
    }

    // 底部提示符输入行矩形：占据内容区最后一行。
    pub(crate) fn input_row_rect(&self, frame: Rect) -> Rect {
        let geometry = self.visual.geometry;
        Rect::new(
            frame.x,
            frame.y + frame.h - geometry.padding_y - geometry.row_height,
            frame.w,
            geometry.row_height,
        )
    }

    // 由给定帧高换算输出区视口高度。
    pub(crate) fn output_viewport_height_from(&self, frame_height: f32) -> f32 {
        let geometry = self.visual.geometry;
        (frame_height - geometry.padding_y * 2.0 - geometry.row_height).max(0.0)
    }

    pub(crate) fn output_viewport_height(&self) -> f32 {
        self.output_viewport_height_from(self.local_frame().h)
    }

    // 跟随底部时绘制与滚动上报都使用内容末端偏移，无需等布局 mutate。
    pub(crate) fn effective_scroll_offset(&self) -> f32 {
        if self.follow_bottom {
            self.body_scroll.max_scroll_offset(
                self.lines.len(),
                self.visual.geometry.row_height,
                self.output_viewport_height(),
            )
        } else {
            self.body_scroll.scroll_offset()
        }
    }

    pub(crate) fn push_scroll_delta(&self, dx: f32, dy: f32) {
        if dx.abs() <= 0.01 && dy.abs() <= 0.01 {
            return;
        }
        let current = self.scroll_delta_strip.get();
        self.scroll_delta_strip
            .set((current.0 + dx, current.1 + dy));
    }

    // ── 命令行编辑 ──

    pub(crate) fn insert_text(&mut self, text: &str) {
        let byte = crate::ui::widgets::input::byte_index_for_char(&self.input, self.cursor);
        self.input.insert_str(byte, text);
        self.cursor += text.chars().count();
    }

    pub(crate) fn delete_char_before_cursor(&mut self) {
        let byte_end = crate::ui::widgets::input::byte_index_for_char(&self.input, self.cursor);
        let byte_start =
            crate::ui::widgets::input::byte_index_for_char(&self.input, self.cursor - 1);
        self.input.replace_range(byte_start..byte_end, "");
        self.cursor -= 1;
    }

    pub(crate) fn delete_char_at_cursor(&mut self) {
        let byte_start = crate::ui::widgets::input::byte_index_for_char(&self.input, self.cursor);
        let byte_end =
            crate::ui::widgets::input::byte_index_for_char(&self.input, self.cursor + 1);
        self.input.replace_range(byte_start..byte_end, "");
    }

    // 提交当前输入：清空草稿、记入历史、通知回调并登记 submit 语义事件。
    pub(crate) fn submit_current(&mut self) -> EventResult {
        // 空命令不提交：终端行没有可执行内容，也不产生回车事件。
        if self.input.is_empty() {
            return EventResult::NotHandled;
        }
        let command = std::mem::take(&mut self.input);
        self.cursor = 0;
        self.history_index = None;
        self.history.push(command.clone());
        let excess = self.history.len().saturating_sub(HISTORY_LIMIT);
        if excess > 0 {
            self.history.drain(0..excess);
        }
        if let Some(callback) = self.command_callback.as_ref() {
            callback(&command);
        }
        self.pending_submit.borrow_mut().replace(command);
        EventResult::Handled
    }

    // 历史漫游：Up 逐条回溯更旧命令，Down 回到更新命令直至清空草稿。
    pub(crate) fn recall_history(&mut self, older: bool) -> EventResult {
        if self.history.is_empty() {
            return EventResult::NotHandled;
        }
        let last = self.history.len() - 1;
        let target = if older {
            match self.history_index {
                // 已停留在最旧命令时保持现值并消费按键。
                Some(0) => return EventResult::Handled,
                Some(index) => index - 1,
                None => last,
            }
        } else {
            match self.history_index {
                None => return EventResult::NotHandled,
                Some(index) if index >= last => {
                    self.history_index = None;
                    self.input.clear();
                    self.cursor = 0;
                    return EventResult::Handled;
                }
                Some(index) => index + 1,
            }
        };
        self.history_index = Some(target);
        self.input.clone_from(&self.history[target]);
        self.cursor = self.input.chars().count();
        EventResult::Handled
    }

    // ── 公开配置 ──

    /// 创建空输出、默认 `$ ` 提示符且跟随底部的终端组件。
    pub fn new() -> Self {
        Self {
            lines: Vec::new(),
            prompt: "$ ".to_string(),
            input: String::new(),
            cursor: 0,
            history: Vec::new(),
            history_index: None,
            command_callback: None,
            focused: false,
            follow_bottom: true,
            body_scroll: VirtualListScroll::new(),
            scroll_delta_strip: Cell::new((0.0, 0.0)),
            layout_requested: Cell::new(false),
            last_frame: Cell::new(None),
            pending_submit: RefCell::new(None),
            view_style: crate::ui::theme::style::Style::default(),
            visual: TERMINAL_VISUAL_REF,
        }
    }

    /// 设置显示在输入行前方的提示符文本。
    pub fn prompt(mut self, prompt: impl Into<String>) -> Self {
        self.prompt = prompt.into();
        self
    }

    /// 用声明顺序的输出行替换整个回看缓冲。
    pub fn lines(mut self, lines: Vec<TerminalLine>) -> Self {
        self.lines = lines;
        self
    }

    /// 注册命令提交回调；空命令不会触发。
    pub fn on_command<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) + 'static,
    {
        self.command_callback = Some(Rc::new(callback));
        self
    }

    /// 运行时替换提示符文本。
    pub fn set_prompt(&mut self, prompt: impl Into<String>) {
        self.prompt = prompt.into();
    }

    /// 运行时用声明顺序的输出行替换整个回看缓冲。
    pub fn set_lines(&mut self, lines: Vec<TerminalLine>) {
        self.lines = lines;
    }

    /// 返回输入行当前草稿文本。
    pub fn input(&self) -> &str {
        &self.input
    }

    /// 返回按提交顺序保存的历史命令。
    pub fn history(&self) -> &[String] {
        &self.history
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.visual = next.visual;
        self.lines = next.lines;
        self.prompt = next.prompt;
        self.command_callback = next.command_callback;
        self.view_style = next.view_style;
        // 输入草稿、光标、历史与滚动跟随状态属于组件运行态，跨帧保留。
        self.body_scroll.clamp_to_content(
            self.lines.len(),
            self.visual.geometry.row_height,
            self.output_viewport_height(),
        );
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Terminal {
            prompt: self.prompt.clone(),
            lines: self.lines.iter().map(TerminalLine::plain).collect(),
            input: self.input.clone(),
            history: self.history.clone(),
        }
    }
}
