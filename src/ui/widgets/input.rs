//! Input widget — Ant Design style text input with placeholder, focus, and states.

use crate::graphics::{Color, GraphicsEngine, Point, Rect, Size};
use crate::graphics::Radius;
use crate::ui::render_context::RenderContext;
use crate::ui::theme::DesignTokens;
use crate::ui::widget::{EventResult, Widget, WidgetEvent, WidgetTree};

/// Input size matching Ant Design.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputSize {
    Small,
    Middle,
    Large,
}

impl InputSize {
    pub fn height(&self) -> f32 {
        match self {
            Self::Small => 24.0,
            Self::Middle => 32.0,
            Self::Large => 40.0,
        }
    }
}

/// Text input widget.
pub struct Input {
    value: String,
    placeholder: String,
    input_size: InputSize,
    disabled: bool,
    focused: bool,
    hovered: bool,
}

impl Input {
    pub fn new(placeholder: &str) -> Self {
        Self {
            value: String::new(),
            placeholder: placeholder.to_string(),
            input_size: InputSize::Middle,
            disabled: false,
            focused: false,
            hovered: false,
        }
    }

    pub fn with_value(mut self, value: &str) -> Self {
        self.value = value.to_string();
        self
    }
    pub fn size(mut self, s: InputSize) -> Self {
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
    }
}

impl Widget for Input {
    fn preferred_size(&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        let h = self.input_size.height();
        let text_w = self
            .value
            .len()
            .max(self.placeholder.len()) as f32
            * 7.0;
        let w = text_w + 24.0;
        Size::new(w.max(80.0), h)
    }

    fn on_event(&mut self, event: &WidgetEvent) -> EventResult {
        if self.disabled {
            return EventResult::NotHandled;
        }
        match event {
            WidgetEvent::MouseDown { .. } => {
                self.focused = true;
                EventResult::Handled
            }
            WidgetEvent::HoverEnter => {
                self.hovered = true;
                EventResult::Handled
            }
            WidgetEvent::HoverLeave => {
                self.hovered = false;
                EventResult::Handled
            }
            WidgetEvent::FocusOut => {
                self.focused = false;
                EventResult::Handled
            }
            WidgetEvent::KeyDown { key } => {
                use crate::ui::widget::KeyCode;
                match key {
                    KeyCode::Backspace => {
                        self.value.pop();
                        EventResult::Handled
                    }
                    KeyCode::Enter => {
                        // Submit handled elsewhere; just consume
                        EventResult::Handled
                    }
                    _ => EventResult::NotHandled,
                }
            }
            _ => EventResult::NotHandled,
        }
    }

    fn render(&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let tokens = DesignTokens::antd_light();
        let h = self.input_size.height();
        let font_size = 14.0;

        let input_frame = Rect::new(frame.x, frame.y, frame.w, h.min(frame.h));

        // Determine colors based on state
        let (bg, border, text_color) = if self.disabled {
            (
                tokens.color_fill_tertiary,
                tokens.color_border,
                tokens.color_text_quaternary,
            )
        } else if self.focused {
            (
                Color::white(),
                tokens.color_primary,
                tokens.color_text,
            )
        } else if self.hovered {
            (
                Color::white(),
                tokens.color_primary_hover,
                tokens.color_text,
            )
        } else {
            (
                Color::white(),
                tokens.color_border,
                tokens.color_text,
            )
        };

        let radius = Some(Radius::uniform(tokens.border_radius_sm));

        // Background fill
        ctx.fill_rect(input_frame, bg, radius);

        // Border
        let bw = if self.focused { 2.0 } else { 1.0 };
        ctx.stroke_rect(input_frame, border, bw, radius);

        // Text content
        let display_text = if self.value.is_empty() {
            &self.placeholder
        } else {
            &self.value
        };
        let text_color = if self.value.is_empty() && !self.focused {
            tokens.color_text_tertiary
        } else {
            text_color
        };

        let pad = 12.0;
        if !display_text.is_empty() {
            ctx.draw_text(
                display_text,
                Point::new(input_frame.x + pad, input_frame.y + (input_frame.h - font_size) * 0.5),
                text_color,
                font_size,
            );
        }

        // Focus indicator: cursor line
        if self.focused {
            let cursor_x = input_frame.x
                + pad
                + (if self.value.is_empty() {
                    0.0
                } else {
                    self.value.len() as f32 * 7.0
                });
            ctx.fill_rect(
                Rect::new(cursor_x, input_frame.y + 4.0, 1.5, input_frame.h - 8.0),
                tokens.color_primary,
                None,
            );
        }
    }
}
