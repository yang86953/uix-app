//! Divider widget — Ant Design style horizontal/vertical divider with optional text.

use crate::graphics::{Color, Point, Rect, Size};
use crate::ui::render_context::RenderContext;
use crate::ui::theme::DesignTokens;
use crate::ui::widget::{Widget, WidgetTree};

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

/// Divider widget with optional label.
pub struct Divider {
    text: Option<String>,
    orientation: DividerOrientation,
    direction: DividerDirection,
    color: Option<Color>,
    text_size: f32,
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

impl Widget for Divider {
    fn preferred_size(&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        match self.direction {
            DividerDirection::Horizontal => {
                // Full width, 1px height (or more if text)
                if self.text.is_some() {
                    Size::new(0.0, 24.0)
                } else {
                    Size::new(0.0, 1.0)
                }
            }
            DividerDirection::Vertical => Size::new(1.0, 0.0),
        }
    }

    fn render(&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let tokens = DesignTokens::antd_light();
        let line_color = self.color.unwrap_or(tokens.color_border_secondary);

        match self.direction {
            DividerDirection::Horizontal => {
                let center_y = frame.y + frame.h * 0.5;

                if let Some(ref text) = self.text {
                    // Measure text approximately
                    let text_w = text.len() as f32 * 7.5 + 8.0; // +padding
                    let text_h = self.text_size;

                    // Determine text start x based on orientation
                    let text_x = match self.orientation {
                        DividerOrientation::Left => frame.x + 32.0,
                        DividerOrientation::Center => frame.x + (frame.w - text_w) * 0.5,
                        DividerOrientation::Right => frame.x + frame.w - text_w - 32.0,
                    };
                    let text_y = center_y - text_h * 0.5;

                    // Left line: from frame start to text
                    let left_end = text_x - 8.0;
                    if left_end > frame.x {
                        ctx.fill_rect(
                            Rect::new(frame.x, center_y, left_end - frame.x, 1.0),
                            line_color,
                            None,
                        );
                    }

                    // Draw text
                    ctx.draw_text(
                        text,
                        Point::new(text_x, text_y),
                        tokens.color_text_secondary,
                        self.text_size,
                    );

                    // Right line: from text end to frame end
                    let right_start = text_x + text_w;
                    if right_start < frame.x + frame.w {
                        ctx.fill_rect(
                            Rect::new(right_start, center_y, frame.x + frame.w - right_start, 1.0),
                            line_color,
                            None,
                        );
                    }
                } else {
                    // Plain line
                    ctx.fill_rect(Rect::new(frame.x, center_y, frame.w, 1.0), line_color, None);
                }
            }
            DividerDirection::Vertical => {
                let center_x = frame.x + frame.w * 0.5;
                ctx.fill_rect(Rect::new(center_x, frame.y, 1.0, frame.h), line_color, None);
            }
        }
    }
}
