//! Input widget — Ant Design style text input with placeholder, focus, and states.

use std::cell::Cell;

use crate::define_widget;
use crate::graphics::{Color, GraphicsEngine, Radius};
use crate::base::{Rect, Size};
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, KeyCode, WidgetEvent, WidgetTree};

/// Input size matching Ant Design.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputSize {
    Small, Middle, Large,
}

impl InputSize {
    pub fn height(&self) -> f32 {
        match self { Self::Small => 24.0, Self::Middle => 32.0, Self::Large => 40.0 }
    }
}

define_widget! {
    /// Text input widget.
    pub struct Input {
        value: String,
        placeholder: String,
        input_size: InputSize,
        disabled: bool,
        focused: bool,
        hovered: bool,
        /// 光标位置（字符索引，不是字节偏移）。
        cursor_char: usize,
        /// 水平滚动偏移（像素），文字超出输入框时平滑左移。
        /// 水平滚动偏移（像素），文字超出输入框时平滑左移。使用 Cell 以在 &self 渲染中修改。
        scroll_offset_x: Cell<f32>,
    }

    preferred_size => (&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        let h = self.input_size.height();
        // 最小宽度 80px，实际宽度由 flex 布局分配
        Size::new(80.0, h)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if self.disabled { return EventResult::NotHandled; }
        match event {
            WidgetEvent::MouseDown { .. } => { self.focused = true; EventResult::Handled }
            WidgetEvent::HoverEnter => { self.hovered = true; EventResult::Handled }
            WidgetEvent::HoverLeave => { self.hovered = false; EventResult::Handled }
            WidgetEvent::FocusOut => { self.focused = false; EventResult::Handled }
            WidgetEvent::KeyDown { key, .. } => {
                match key {
                    KeyCode::Backspace => {
                        if self.cursor_char > 0 {
                            let byte_pos = self.value.char_indices()
                                .nth(self.cursor_char - 1)
                                .map(|(i, _)| i)
                                .unwrap_or(0);
                            self.value.remove(byte_pos);
                            self.cursor_char -= 1;
                            EventResult::Handled
                        } else {
                            EventResult::NotHandled
                        }
                    }
                    KeyCode::Left => {
                        if self.cursor_char > 0 {
                            self.cursor_char -= 1;
                        }
                        EventResult::Handled
                    }
                    KeyCode::Right => {
                        let len = self.value.chars().count();
                        if self.cursor_char < len {
                            self.cursor_char += 1;
                        }
                        EventResult::Handled
                    }
                    KeyCode::Home => {
                        self.cursor_char = 0;
                        EventResult::Handled
                    }
                    KeyCode::End => {
                        self.cursor_char = self.value.chars().count();
                        EventResult::Handled
                    }
                    KeyCode::Enter => EventResult::Handled,
                    _ => EventResult::NotHandled,
                }
            }
            WidgetEvent::KeyPress { text } => {
                // 只接受可打印字符：Windows WM_CHAR 会把退格(0x08)、回车(0x0D)等
                // 控制字符也作为 KeyPress 发送，直接拼入 value 会破坏输入状态。
                if text.chars().any(|c| c.is_control()) {
                    return EventResult::NotHandled;
                }
                // 在光标位置插入字符（而非追加到末尾）
                for ch in text.chars() {
                    let byte_pos = self.value.char_indices()
                        .nth(self.cursor_char)
                        .map(|(i, _)| i)
                        .unwrap_or(self.value.len());
                    self.value.insert(byte_pos, ch);
                    self.cursor_char += 1;
                }
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let h = self.input_size.height();
        let input_frame = Rect::new(frame.x, frame.y, frame.w, h.min(frame.h));

        let fill_tertiary = ctx.tokens().color_fill_tertiary();
        let border_color = ctx.tokens().color_border();
        let text_quaternary = ctx.tokens().color_text_quaternary();
        let primary = ctx.tokens().color_primary();
        let primary_hover = ctx.tokens().color_primary_hover();
        let text_color_token = ctx.tokens().color_text();
        let text_tertiary = ctx.tokens().color_text_tertiary();
        let border_radius_sm = ctx.tokens().border_radius_sm();

        let (bg, border, text_color) = if self.disabled {
            (fill_tertiary, border_color, text_quaternary)
        } else if self.focused {
            (Color::white(), primary, text_color_token)
        } else if self.hovered {
            (Color::white(), primary_hover, text_color_token)
        } else {
            (Color::white(), border_color, text_color_token)
        };

        let radius = Some(Radius::uniform(border_radius_sm));
        ctx.fill_rect(input_frame, bg, radius);
        ctx.stroke_rect(input_frame, border, if self.focused { 2.0 } else { 1.0 }, radius);

        let display_text = if self.value.is_empty() { &self.placeholder } else { &self.value };
        let text_color = if self.value.is_empty() && !self.focused { text_tertiary } else { text_color };

        let pad = 12.0;
        // 限制文本绘制区域在输入框范围内（文字溢出时自动裁剪）
        let text_area = Rect::new(input_frame.x + pad, input_frame.y, input_frame.w - pad * 2.0, input_frame.h);
        if text_area.w > 0.0 {
            ctx.engine().push_clip_rect(text_area);
        }
        if text_area.w > 0.0 {
            ctx.engine().push_clip_rect(text_area);
        }

        // 计算光标前的文本宽度（用于滚动定位和光标绘制）
        let cursor_byte_pos = self.value.char_indices()
            .nth(self.cursor_char)
            .map(|(i, _)| i)
            .unwrap_or(self.value.len());
        let text_before = &self.value[..cursor_byte_pos];
        let text_before_w = if !text_before.is_empty() {
            ctx.measure_text(text_before, 14.0).w
        } else { 0.0 };

        // 自适应滚动：保持光标在可视区域内
        let text_area_w = input_frame.w - pad * 2.0;
        let right_margin = 10.0;
        let left_margin = 0.0;
        let mut scroll_off = self.scroll_offset_x.get();
        if text_before_w - scroll_off > text_area_w - right_margin {
            // 光标超出右边界 → 右滚
            scroll_off = text_before_w - text_area_w + right_margin;
        }
        if text_before_w - scroll_off < left_margin {
            // 光标超出左边界 → 左滚
            scroll_off = text_before_w;
        }
        // 滚动不超出文本总宽度
        let total_text_w = if !self.value.is_empty() {
            ctx.measure_text(&self.value, 14.0).w
        } else { 0.0 };
        scroll_off = scroll_off.min(total_text_w - 1.0).max(0.0);
        self.scroll_offset_x.set(scroll_off);

        // 绘制文本（应用滚动偏移），垂直使用 font metrics 居中
        let scroll_off = self.scroll_offset_x.get();
        if !display_text.is_empty() {
            let draw_x = input_frame.x + pad - scroll_off;
            let text_rect = Rect::new(input_frame.x + pad, input_frame.y, text_area_w, input_frame.h);
            let draw_y = ctx.visual_center_y(text_rect, 14.0);
            ctx.draw_text(display_text,
                crate::base::Point::new(draw_x, draw_y),
                text_color, 14.0);
        }

        if text_area.w > 0.0 {
            ctx.engine().pop_clip_rect();
        }

        if self.focused {
            // 光标位置（应用滚动偏移）
            let cursor_x = input_frame.x + pad + text_before_w - scroll_off;
            ctx.fill_rect(Rect::new(cursor_x, input_frame.y + 4.0, 1.5, input_frame.h - 8.0), primary, None);
        }
    }
}

impl Input {
    pub fn new(placeholder: impl Into<String>) -> Self {
        Self {
            value: String::new(),
            placeholder: placeholder.into(),
            input_size: InputSize::Middle,
            disabled: false,
            focused: false,
            hovered: false,
            cursor_char: 0,
            scroll_offset_x: Cell::new(0.0),
        }
    }
    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = value.into();
        self.cursor_char = self.value.chars().count();
        self.scroll_offset_x.set(0.0);
        self
    }
    pub fn size(mut self, s: InputSize) -> Self { self.input_size = s; self }
    pub fn disabled(mut self, v: bool) -> Self { self.disabled = v; self }
    pub fn value(&self) -> &str { &self.value }
    pub fn set_value(&mut self, v: impl Into<String>) {
        self.value = v.into();
        self.cursor_char = self.value.chars().count();
        self.scroll_offset_x.set(0.0);
    }
}
