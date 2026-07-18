//! Skeleton loading placeholder.

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::ui::core::widget::WidgetTree;
use crate::ui::SnapshotFields;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SkeletonShape {
    Rect,
    Circle,
    Text,
}

component! {
    /// A static loading placeholder.
    pub struct Skeleton {
        shape: SkeletonShape,
        w: f32,
        h: f32,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::compositor::PicturePolicy {
        crate::draw::compositor::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let frame = Self::normalized_frame(frame);
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let color = ctx.tokens().color_fill_tertiary();
        ctx.push_clip(frame);

        match self.shape {
            SkeletonShape::Rect => {
                let radius = 4.0_f32.min(frame.w.min(frame.h) * 0.5);
                ctx.fill_rect(
                    frame,
                    color,
                    Some(crate::draw::Radius::uniform(radius)),
                );
            }
            SkeletonShape::Circle => {
                ctx.fill_circle(
                    frame.x + frame.w * 0.5,
                    frame.y + frame.h * 0.5,
                    frame.w.min(frame.h) * 0.5,
                    color,
                );
            }
            SkeletonShape::Text => {
                let line_h = frame.h * 0.35;
                let gap = frame.h * 0.15;
                let content_height = line_h * 2.0 + gap;
                let y = frame.y + (frame.h - content_height) * 0.5;
                ctx.fill_rect(
                    Rect::new(frame.x, y, frame.w * 0.8, line_h),
                    color,
                    Some(crate::draw::Radius::uniform(
                        2.0_f32.min(line_h * 0.5),
                    )),
                );
                ctx.fill_rect(
                    Rect::new(frame.x, y + line_h + gap, frame.w * 0.5, line_h),
                    color,
                    Some(crate::draw::Radius::uniform(
                        2.0_f32.min(line_h * 0.5),
                    )),
                );
            }
        }
        ctx.pop_clip();
    }
}

impl Default for Skeleton {
    fn default() -> Self {
        Self::new()
    }
}

impl Skeleton {
    pub fn new() -> Self {
        Self {
            shape: SkeletonShape::Rect,
            w: 200.0,
            h: 16.0,
        }
    }

    pub fn shape(mut self, s: SkeletonShape) -> Self {
        self.shape = s;
        self
    }

    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.w = Self::normalized_dimension(w);
        self.h = Self::normalized_dimension(h);
        self
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(self.w, self.h)
    }

    fn normalized_dimension(value: f32) -> f32 {
        if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        }
    }

    fn normalized_frame(frame: Rect) -> Rect {
        Rect::new(
            frame.x,
            frame.y,
            Self::normalized_dimension(frame.w),
            Self::normalized_dimension(frame.h),
        )
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Skeleton {
            shape: self.shape,
            width: self.w,
            height: self.h,
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.shape = next.shape;
        self.w = Self::normalized_dimension(next.w);
        self.h = Self::normalized_dimension(next.h);
    }
}
