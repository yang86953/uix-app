//! Skeleton loading placeholder.

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::ui::SnapshotFields;
use crate::ui::WidgetTree;

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

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let color = ctx.tokens().color_fill_tertiary();

        match self.shape {
            SkeletonShape::Rect => {
                ctx.fill_rect(frame, color, Some(crate::draw::Radius::uniform(4.0)));
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
                let line_h = self.h * 0.35;
                let gap = self.h * 0.15;
                ctx.fill_rect(
                    Rect::new(frame.x, frame.y, frame.w * 0.8, line_h),
                    color,
                    Some(crate::draw::Radius::uniform(2.0)),
                );
                ctx.fill_rect(
                    Rect::new(frame.x, frame.y + line_h + gap, frame.w * 0.5, line_h),
                    color,
                    Some(crate::draw::Radius::uniform(2.0)),
                );
            }
        }
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
        self.w = w;
        self.h = h;
        self
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(self.w, self.h)
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Skeleton {
            shape: self.shape,
            width: self.w,
            height: self.h,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::traits::WidgetLayout;

    #[test]
    fn measure_clamps_skeleton_size() {
        let measured = Skeleton::new()
            .size(120.0, 40.0)
            .measure(Constraints::loose(Size::new(80.0, 24.0)));

        assert_eq!(measured, Size::new(80.0, 24.0));
    }
}
