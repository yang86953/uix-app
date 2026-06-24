//! Divider widget — Ant Design style horizontal/vertical divider with optional text.

use uix_graphics::{Color};
use uix_core::{Point, Rect, Size};
use crate::render_context::RenderContext;
use crate::define_widget;
use crate::widget::WidgetTree;

/// Divider orientation for text placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DividerOrientation {
    Left,
    Center,
    Right,
}

/// Divider direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DividerDirection {
    Horizontal,
    Vertical,
}

define_widget! {
    /// Divider widget with optional label.
    pub struct Divider {
        text: Option<String>,
        orientation: DividerOrientation,
        direction: DividerDirection,
        pub color: Option<Color>,
        text_size: f32,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        match self.direction {
            DividerDirection::Horizontal => {
                if self.text.is_some() { Size::new(0.0, 24.0) } else { Size::new(0.0, 1.0) }
            }
            DividerDirection::Vertical => Size::new(1.0, 0.0),
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let line_color = self.color.unwrap_or(ctx.tokens().color_border_secondary());
        let text_secondary = ctx.tokens().color_text_secondary();

        match self.direction {
            DividerDirection::Horizontal => {
                let center_y = frame.y + frame.h * 0.5;

                if let Some(ref text) = self.text {
                    let text_w = text.len() as f32 * 7.5 + 8.0;
                    let text_h = self.text_size;

                    let text_x = match self.orientation {
                        DividerOrientation::Left => frame.x + 32.0,
                        DividerOrientation::Center => frame.x + (frame.w - text_w) * 0.5,
                        DividerOrientation::Right => frame.x + frame.w - text_w - 32.0,
                    };
                    let text_y = ctx.visual_center_y(frame, self.text_size);

                    let left_end = text_x - 8.0;
                    if left_end > frame.x {
                        ctx.fill_rect(Rect::new(frame.x, center_y, left_end - frame.x, 1.0), line_color, None);
                    }

                    ctx.draw_text(text, Point::new(text_x, text_y), text_secondary, self.text_size);

                    let right_start = text_x + text_w;
                    if right_start < frame.x + frame.w {
                        ctx.fill_rect(Rect::new(right_start, center_y, frame.x + frame.w - right_start, 1.0), line_color, None);
                    }
                } else {
                    // 无文本时直接填满整个 frame，避免 center 偏移导致的半像素不可见问题
                    ctx.fill_rect(Rect::new(frame.x, frame.y, frame.w, frame.h), line_color, None);
                }
            }
            DividerDirection::Vertical => {
                // 垂直分割线同样直接填满 frame
                ctx.fill_rect(Rect::new(frame.x, frame.y, frame.w, frame.h), line_color, None);
            }
        }
    }
}

impl Default for Divider {
    fn default() -> Self {
        Self::new()
    }
}

impl Divider {
    pub fn new() -> Self {
        Self {
            text: None,
            orientation: DividerOrientation::Center,
            direction: DividerDirection::Horizontal,
            color: None,
            text_size: 14.0,
        }
    }

    pub fn with_text(mut self, t: &str) -> Self {
        self.text = Some(t.to_string());
        self
    }
    pub fn orientation(mut self, o: DividerOrientation) -> Self {
        self.orientation = o;
        self
    }
    pub fn vertical(mut self) -> Self {
        self.direction = DividerDirection::Vertical;
        self
    }
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }
}
