//! Input widget — Ant Design style text input with placeholder, focus, and states.

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
            WidgetEvent::KeyDown { key } => {
                match key {
                    KeyCode::Backspace => { self.value.pop(); EventResult::Handled }
                    KeyCode::Enter => EventResult::Handled,
                    _ => EventResult::NotHandled,
                }
            }
            WidgetEvent::KeyPress { text } => {
                self.value.push_str(text);
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
        if !display_text.is_empty() {
            ctx.text_center(display_text,
                Rect::new(input_frame.x + pad, input_frame.y, 0.0, input_frame.h),
                text_color, 14.0);
        }
        if text_area.w > 0.0 {
            ctx.engine().pop_clip_rect();
        }
        if self.focused {
            // 使用实际渲染文本宽度定位光标
            let text_w = if !self.value.is_empty() {
                ctx.measure_text(&self.value, 14.0).w
            } else { 0.0 };
            // 光标不超出输入框可见区域
            let cursor_x = (input_frame.x + pad + text_w).min(input_frame.x + input_frame.w - pad - 1.5);
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
            disabled: false, focused: false, hovered: false,
        }
    }
    pub fn with_value(mut self, value: impl Into<String>) -> Self { self.value = value.into(); self }
    pub fn size(mut self, s: InputSize) -> Self { self.input_size = s; self }
    pub fn disabled(mut self, v: bool) -> Self { self.disabled = v; self }
    pub fn value(&self) -> &str { &self.value }
    pub fn set_value(&mut self, v: impl Into<String>) { self.value = v.into(); }
}
