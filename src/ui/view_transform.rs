use crate::core::{Point, Rect};
use crate::draw::Transform;

/// Visual-only transform metadata attached to a View/Widget node.
///
/// Layout continues to use the untransformed frame. Scale is centered on that
/// frame and offset is applied after scaling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ViewTransform {
    pub(crate) offset: Point,
    pub(crate) scale: f32,
}

impl ViewTransform {
    pub(crate) fn matrix(self, frame: Rect) -> Transform {
        if self == Self::default() {
            return Transform::identity();
        }
        let center_x = frame.x + frame.w * 0.5;
        let center_y = frame.y + frame.h * 0.5;
        Transform::translate(self.offset.x, self.offset.y)
            .concat(Transform::translate(center_x, center_y))
            .concat(Transform::scale(self.scale, self.scale))
            .concat(Transform::translate(-center_x, -center_y))
    }
}

impl Default for ViewTransform {
    fn default() -> Self {
        Self {
            offset: Point::new(0.0, 0.0),
            scale: 1.0,
        }
    }
}
