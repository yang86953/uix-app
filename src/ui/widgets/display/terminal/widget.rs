use crate::core::{Constraints, Rect, Size};
use crate::ui::virtualization::virtual_scroll::{
    VirtualListScroll, virtual_list_index_range,
};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{
    EventResult, KeyCode, SemanticEvent, SystemEvent, WidgetId, WidgetTree,
};
use crate::widget;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::{ResolvedTerminalVisual, TerminalColor, TerminalLine, TerminalVisual};

widget! {
    /// 终端组件：滚动输出区、提示符输入行、命令历史与语义着色。
    pub struct Terminal {
        pub(crate) lines: Vec<TerminalLine>,
        pub(crate) prompt: String,
        pub(crate) input: String,
        pub(crate) cursor: usize,
        pub(crate) history: Vec<String>,
        pub(crate) history_index: Option<usize>,
        pub(crate) command_callback: Option<Rc<dyn Fn(&str)>>,
        pub(crate) focused: bool,
        pub(crate) follow_bottom: bool,
        pub(crate) body_scroll: VirtualListScroll,
        pub(crate) scroll_delta_strip: Cell<(f32, f32)>,
        pub(crate) layout_requested: Cell<bool>,
        pub(crate) last_frame: Cell<Option<Rect>>,
        pub(crate) pending_submit: RefCell<Option<String>>,
        // 全部实例共享 UIX 声明固化后的只读视觉配置。
        #[snapshot(skip)]
        pub(crate) visual: &'static TerminalVisual,
    }

    tab_index => (&self) -> i32 { 1 }

    // 输入行随时接收文本输入与输入法。
    accepts_text_input => (&self) -> bool { true }

    // 文本输入光标位置：提示符宽度加光标前输入字符的估算宽度。
    text_input_cursor_rect => (&self) -> Rect {
        let frame = self.local_frame();
        let chrome = self.visual.chrome;
        let row = self.input_row_rect(frame);
        // 光标前文本 = 提示符 + 光标前输入字符，按估算字宽换算。
        let before_cursor = self.input.chars().take(self.cursor).count();
        let width = chrome.cursor_base_width
            + (self.prompt.chars().count() + before_cursor) as f32 * chrome.cursor_char_width;
        let x = (frame.x + self.visual.geometry.padding_x + width)
            .min(frame.x + frame.w - self.visual.geometry.padding_x - chrome.cursor_width);
        Rect::new(
            x,
            row.y + (row.h - row.h * chrome.cursor_height_ratio) * 0.5,
            chrome.cursor_width,
            row.h * chrome.cursor_height_ratio,
        )
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    // 滚动增量（累积后返回并清零）。
    scroll_delta_for_dirty => (&self) -> Option<(f32, f32)> {
        let delta = self.scroll_delta_strip.get();
        if delta.0.abs() > 0.01 || delta.1.abs() > 0.01 {
            self.scroll_delta_strip.set((0.0, 0.0));
            Some(delta)
        } else {
            None
        }
    }

    // 视口滚动偏移：仅垂直方向；跟随底部时上报内容末端。
    viewport_scroll_offset => (&self) -> Option<(f32, f32)> {
        Some((0.0, self.effective_scroll_offset()))
    }

    // 取出布局请求标记（一次性）。
    take_layout_request => (&mut self) -> bool {
        self.layout_requested.replace(false)
    }

    // 事件入口：滚轮、焦点与命令行键盘编辑。
    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            // 滚轮：滚动输出区；离开底部即停止跟随，回到底部恢复跟随。
            SystemEvent::Wheel { pos, delta } => {
                let frame = self.local_frame();
                if !frame.contains(*pos) {
                    return EventResult::NotHandled;
                }
                let viewport_h = self.output_viewport_height();
                let row_height = self.visual.geometry.row_height;
                // 跟随底部时先吸附内容末端，保证向上滚动从最新输出开始。
                if self.follow_bottom {
                    let max = self.body_scroll.max_scroll_offset(self.lines.len(), row_height, viewport_h);
                    self.body_scroll.set_scroll_offset(max);
                }
                let applied = self.body_scroll.scroll_by_wheel(
                    delta.y,
                    self.lines.len(),
                    row_height,
                    viewport_h,
                );
                if applied.abs() <= 0.01 {
                    return EventResult::NotHandled;
                }
                let max = self.body_scroll.max_scroll_offset(self.lines.len(), row_height, viewport_h);
                self.follow_bottom = self.body_scroll.scroll_offset() >= max - 0.5;
                self.push_scroll_delta(0.0, applied);
                EventResult::Handled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                EventResult::Handled
            }
            // 键盘：命令行编辑、历史漫游与提交。
            SystemEvent::KeyDown { key, .. } => match key {
                KeyCode::Enter => self.submit_current(),
                KeyCode::Backspace => {
                    if self.cursor == 0 {
                        return EventResult::NotHandled;
                    }
                    self.delete_char_before_cursor();
                    EventResult::Handled
                }
                KeyCode::Delete => {
                    if self.cursor >= self.input.chars().count() {
                        return EventResult::NotHandled;
                    }
                    self.delete_char_at_cursor();
                    EventResult::Handled
                }
                KeyCode::Left => {
                    if self.cursor == 0 {
                        return EventResult::NotHandled;
                    }
                    self.cursor -= 1;
                    EventResult::Handled
                }
                KeyCode::Right => {
                    if self.cursor >= self.input.chars().count() {
                        return EventResult::NotHandled;
                    }
                    self.cursor += 1;
                    EventResult::Handled
                }
                KeyCode::Home => {
                    if self.cursor == 0 {
                        return EventResult::NotHandled;
                    }
                    self.cursor = 0;
                    EventResult::Handled
                }
                KeyCode::End => {
                    let end = self.input.chars().count();
                    if self.cursor == end {
                        return EventResult::NotHandled;
                    }
                    self.cursor = end;
                    EventResult::Handled
                }
                KeyCode::Up => self.recall_history(true),
                KeyCode::Down => self.recall_history(false),
                KeyCode::Escape => {
                    if self.input.is_empty() {
                        return EventResult::NotHandled;
                    }
                    self.input.clear();
                    self.cursor = 0;
                    self.history_index = None;
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },
            // 文本输入/粘贴：在光标处插入，拒绝控制字符。
            SystemEvent::TextInput { text } | SystemEvent::Paste { text }
                if !text.is_empty() && !text.chars().any(char::is_control) =>
            {
                self.insert_text(text);
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    // 语义事件：取出提交待发命令并上报 submit 事件。
    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_submit
            .borrow_mut()
            .take()
            .map(|command| SemanticEvent::submit(id, command))
    }

    // 渲染：容器底与边框、虚拟化输出行（逐段着色）与底部提示符输入行。
    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let frame = Self::normalized_frame(frame);
        if frame.w <= 0.0 || frame.h <= 0.0 {
            self.last_frame.set(Some(frame));
            return;
        }
        self.last_frame.set(Some(frame));
        let resolved = self.visual.resolve(ctx.tokens());
        let geometry = self.visual.geometry;
        let chrome = self.visual.chrome;

        // 容器背景与边框。
        ctx.fill_rect(frame, resolved.background, None);
        ctx.stroke_rect(frame, resolved.border, chrome.border_stroke, None);

        // 输出区：按有效滚动偏移仅绘制可见行；跟随底部时偏移即内容末端。
        let output_h = self.output_viewport_height_from(frame.h);
        let output_top = frame.y + geometry.padding_y;
        let scroll_offset = self.effective_scroll_offset();
        ctx.push_clip(Rect::new(frame.x, output_top, frame.w, output_h));
        let (start, end) = virtual_list_index_range(
            self.lines.len(),
            geometry.row_height,
            scroll_offset,
            output_h,
            self.body_scroll.overscan_count(),
        );
        for i in start..end {
            let line = &self.lines[i];
            let y = output_top + i as f32 * geometry.row_height - scroll_offset;
            if y + geometry.row_height < output_top || y > output_top + output_h {
                continue;
            }
            let mut pen_x = frame.x + geometry.padding_x;
            let row_right = frame.x + frame.w - geometry.padding_x;
            for span in line.spans() {
                if span.text.is_empty() || pen_x >= row_right {
                    break;
                }
                let color = resolved_span_color(&resolved, span.color);
                // 逐段在剩余宽度内绘制；超出右缘由裁剪收敛。
                ctx.draw_text_in_frame(
                    &span.text,
                    Rect::new(pen_x, y, (row_right - pen_x).max(0.0), geometry.row_height),
                    color,
                    chrome.font_size,
                );
                pen_x += ctx.measure_text(&span.text, chrome.font_size).w;
            }
        }
        ctx.pop_clip();

        // 底部提示符输入行：提示符 + 草稿文本 + 焦点光标。
        let row = self.input_row_rect(frame);
        let prompt_rect = Rect::new(frame.x + geometry.padding_x, row.y, frame.w, row.h);
        ctx.draw_text_in_frame(&self.prompt, prompt_rect, resolved.prompt, chrome.font_size);
        let prompt_w = ctx.measure_text(&self.prompt, chrome.font_size).w;
        let input_x = frame.x + geometry.padding_x + prompt_w + chrome.prompt_spacing;
        if !self.input.is_empty() {
            ctx.draw_text_in_frame(
                &self.input,
                Rect::new(input_x, row.y, (frame.x + frame.w - input_x).max(0.0), row.h),
                resolved.text,
                chrome.font_size,
            );
        }
        // 焦点可见时绘制实体光标：光标前文本测量宽度。
        if self.focused && tree.keyboard_focus_visible() {
            let before: String = self.input.chars().take(self.cursor).collect();
            let caret_x = (input_x + ctx.measure_text(&before, chrome.font_size).w)
                .min(frame.x + frame.w - geometry.padding_x - chrome.cursor_width);
            let caret_h = row.h * chrome.cursor_height_ratio;
            ctx.fill_rect(
                Rect::new(caret_x, row.y + (row.h - caret_h) * 0.5, chrome.cursor_width, caret_h),
                resolved.cursor,
                None,
            );
        }

        // 组件整体焦点框。
        if self.focused && tree.keyboard_focus_visible() {
            let inset = chrome.focus_inset.min(frame.w * 0.5).min(frame.h * 0.5);
            let focus = Rect::new(
                frame.x + inset,
                frame.y + inset,
                (frame.w - inset * 2.0).max(0.0),
                (frame.h - inset * 2.0).max(0.0),
            );
            if focus.w > 0.0 && focus.h > 0.0 {
                ctx.stroke_rect(focus, resolved.prompt, chrome.focus_stroke, None);
            }
        }
    }
}

// 把语义段颜色映射为当帧已解析的主题颜色。
fn resolved_span_color(resolved: &ResolvedTerminalVisual, color: TerminalColor) -> crate::draw::Color {
    match color {
        TerminalColor::Default => resolved.text,
        TerminalColor::Muted => resolved.muted,
        TerminalColor::Primary => resolved.primary,
        TerminalColor::Success => resolved.success,
        TerminalColor::Warning => resolved.warning,
        TerminalColor::Error => resolved.error,
        TerminalColor::Info => resolved.info,
    }
}
