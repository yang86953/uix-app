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
    // 保存由声明层提供、围绕节点中心应用的任意二维仿射变换。
    pub(crate) affine: Transform,
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
        // 计算布局帧中心，作为当前 transform 的默认原点。
        let center_x = frame.x + frame.w * 0.5;
        // 独立计算垂直中心，避免假设方形节点。
        let center_y = frame.y + frame.h * 0.5;
        // 平移仍在全部中心变换之后应用，保持现有 offset 语义。
        Transform::translate(self.offset.x, self.offset.y)
            // 把节点中心移动到局部原点。
            .concat(Transform::translate(center_x, center_y))
            // 声明式仿射矩阵先作用于节点局部几何。
            .concat(self.affine)
            // 既有等比缩放继续围绕节点中心应用。
            .concat(Transform::scale(self.scale, self.scale))
            // 恢复节点原有坐标空间。
            .concat(Transform::translate(-center_x, -center_y))
    }
}

impl Default for ViewTransform {
    fn default() -> Self {
        Self {
            offset: Point::new(0.0, 0.0),
            scale: 1.0,
            // 默认不改变节点局部几何。
            affine: Transform::identity(),
        }
    }
}

// 仅在本模块验证视觉矩阵的组合与逆命中契约。
#[cfg(test)]
mod tests {
    // 引入待测内部变换结构。
    use super::*;

    // 验证仿射矩阵与既有平移缩放共享同一可逆运行时矩阵。
    #[test]
    fn preserves_affine_visual_and_inverse_hit_mapping() {
        // 构造围绕节点中心旋转并倾斜的声明矩阵。
        let affine = Transform::rotate(std::f32::consts::FRAC_PI_2)
            // 叠加单轴倾斜以覆盖非正交仿射分量。
            .concat(Transform::skew(0.25, 0.0));
        // 组合声明变换与现有平移缩放字段。
        let visual = ViewTransform {
            // 平移应在中心变换之后应用。
            offset: Point::new(5.0, -3.0),
            // 缩放继续围绕节点中心应用。
            scale: 1.5,
            // 保存任意仿射内容。
            affine,
        };
        // 使用非原点、非方形帧验证中心计算。
        let frame = Rect::new(10.0, 20.0, 8.0, 6.0);
        // 取得绘制、包围盒和命中共同消费的最终矩阵。
        let matrix = visual.matrix(frame);
        // 选择不位于中心的局部点以观察全部分量。
        let original = Point::new(16.0, 24.0);
        // 把局部点变换到屏幕空间。
        let transformed = matrix.transform_point(original);
        // 使用同一矩阵的逆变换模拟命中坐标还原。
        let restored = matrix
            // 当前矩阵必须保持可逆。
            .inverse()
            // 不可逆时测试立即失败。
            .expect("有限旋转、倾斜与正缩放应保持可逆")
            // 还原原始局部坐标。
            .transform_point(transformed);
        // 浮点组合后横坐标应在小误差内恢复。
        assert!((restored.x - original.x).abs() < 0.0001);
        // 浮点组合后纵坐标应在小误差内恢复。
        assert!((restored.y - original.y).abs() < 0.0001);
        // 节点中心只受最终 offset 平移，不受围绕中心的线性变换影响。
        let center = Point::new(14.0, 23.0);
        // 计算中心的屏幕位置。
        let transformed_center = matrix.transform_point(center);
        // 横坐标应只增加水平 offset。
        assert!((transformed_center.x - 19.0).abs() < 0.0001);
        // 纵坐标应只增加垂直 offset。
        assert!((transformed_center.y - 20.0).abs() < 0.0001);
    }
}
