//! Divider widget — Ant Design style horizontal/vertical divider with optional text.

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Color;
use crate::ui::core::paint_context::PaintContext;
use crate::ui::core::widget::WidgetTree;
use crate::ui::SnapshotFields;

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

component! {
    /// Divider widget with optional label.
    pub struct Divider {
        text: Option<String>,
        orientation: DividerOrientation,
        direction: DividerDirection,
        pub color: Option<Color>,
        text_size: f32,
        dashed: bool,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let line_color = self.color.unwrap_or(ctx.tokens().color_border_secondary());
        let text_secondary = ctx.tokens().color_text_secondary();

        let draw_line = |ctx: &mut PaintContext, x: f32, y: f32, w: f32, h: f32, color: Color| {
            if !self.dashed {
                ctx.fill_rect(Rect::new(x, y, w, h), color, None);
            } else {
                let seg_len: f32 = 6.0;
                let gap_len: f32 = 4.0;
                let mut dx = 0.0;
                let is_h = h <= w;
                while dx < (if is_h { w } else { h }) {
                    let seg = seg_len.min(if is_h { w - dx } else { h - dx });
                    if is_h {
                        ctx.fill_rect(Rect::new(x + dx, y, seg, h), color, None);
                    } else {
                        ctx.fill_rect(Rect::new(x, y + dx, w, seg), color, None);
                    }
                    dx += seg + gap_len;
                }
            }
        };

        match self.direction {
            DividerDirection::Horizontal => {
                let center_y = frame.y + frame.h * 0.5;

                if let Some(ref text) = self.text {
                    let text_w = text.len() as f32 * 7.5 + 8.0;
                    let _text_h = self.text_size;

                    let text_x = match self.orientation {
                        DividerOrientation::Left => frame.x + 32.0,
                        DividerOrientation::Center => frame.x + (frame.w - text_w) * 0.5,
                        DividerOrientation::Right => frame.x + frame.w - text_w - 32.0,
                    };
                    let text_y = ctx.visual_center_y(frame, self.text_size);

                    let left_end = text_x - 8.0;
                    if left_end > frame.x {
                        draw_line(ctx, frame.x, center_y, left_end - frame.x, 1.0, line_color);
                    }

                    ctx.draw_text(text, Point::new(text_x, text_y), text_secondary, self.text_size);

                    let right_start = text_x + text_w;
                    if right_start < frame.x + frame.w {
                        draw_line(ctx, right_start, center_y, frame.x + frame.w - right_start, 1.0, line_color);
                    }
                } else {
                    draw_line(ctx, frame.x, frame.y, frame.w, frame.h, line_color);
                }
            }
            DividerDirection::Vertical => {
                draw_line(ctx, frame.x, frame.y, frame.w, frame.h, line_color);
            }
        }
    }
}

impl Divider {
    pub(crate) fn sync_from(&mut self, next: Self) {
        self.text = next.text;
        self.orientation = next.orientation;
        self.direction = next.direction;
        self.color = next.color;
        self.text_size = next.text_size;
        self.dashed = next.dashed;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Divider {
            text: self.text.clone(),
            orientation: self.orientation,
            direction: self.direction,
            color: self.color,
            text_size: self.text_size,
            dashed: self.dashed,
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
            dashed: false,
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
    pub fn dashed(mut self) -> Self {
        self.dashed = true;
        self
    }

    fn intrinsic_size(&self) -> Size {
        match self.direction {
            DividerDirection::Horizontal => {
                if self.text.is_some() {
                    Size::new(0.0, 24.0)
                } else {
                    Size::new(0.0, 1.0)
                }
            }
            DividerDirection::Vertical => Size::new(1.0, 0.0),
        }
    }
}
