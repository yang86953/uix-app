use crate::core::{Point, Rect};
use crate::draw::Transform;
// 引入 UI System 根拥有的公开变换原点值。
use crate::ui::TransformOrigin;

/// Visual-only transform metadata attached to a View/Widget node.
///
/// Layout continues to use the untransformed frame. Scale is centered on that
/// frame and offset is applied after scaling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ViewTransform {
    pub(crate) offset: Point,
    pub(crate) scale: f32,
    // 保存由声明层提供、围绕节点中心应用的任意二维仿射变换。
    pub(crate) affine: Transform,
    // 保存布局帧已知后才解析的二维变换原点。
    pub(crate) origin: TransformOrigin,
}

impl ViewTransform {
    pub(crate) fn combined(self, overlay: Self) -> Self {
        Self {
            offset: Point::new(
                self.offset.x + overlay.offset.x,
                self.offset.y + overlay.offset.y,
            ),
            scale: self.scale * overlay.scale,
            // 基础声明变换先于覆盖层变换组合，动画覆盖层通常保持单位矩阵。
            affine: self.affine.concat(overlay.affine),
            // 过渡覆盖层不取得声明原点所有权，始终保留基础节点原点。
            origin: self.origin,
        }
    }

    pub(crate) fn matrix(self, frame: Rect) -> Transform {
        if self == Self::default() {
            return Transform::identity();
        }
        if self.scale == 1.0 {
            // 没有额外仿射内容时保留纯平移快速路径。
            if self.affine.is_identity() {
                // 返回与历史实现完全一致的平移矩阵。
                return Transform::translate(self.offset.x, self.offset.y);
            }
        }
        // 在布局帧确定后解析比例或像素原点。
        let origin = self.origin.resolve(frame);
        // 平移仍在全部中心变换之后应用，保持现有 offset 语义。
        Transform::translate(self.offset.x, self.offset.y)
            // 把节点中心移动到局部原点。
            .concat(Transform::translate(origin.x, origin.y))
            // 声明式仿射矩阵先作用于节点局部几何。
            .concat(self.affine)
            // 既有等比缩放继续围绕节点中心应用。
            .concat(Transform::scale(self.scale, self.scale))
            // 恢复节点原有坐标空间。
            .concat(Transform::translate(-origin.x, -origin.y))
    }
}

impl Default for ViewTransform {
    fn default() -> Self {
        Self {
            offset: Point::new(0.0, 0.0),
            scale: 1.0,
            // 默认不改变节点局部几何。
            affine: Transform::identity(),
            // 默认围绕布局帧中心应用变换。
            origin: TransformOrigin::default(),
        }
    }
}
