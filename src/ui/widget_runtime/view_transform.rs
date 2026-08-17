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
            // 默认原点保持历史中心语义。
            origin: TransformOrigin::default(),
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

    // 验证比例与像素轴值只在布局帧确定后解析。
    #[test]
    fn resolves_fraction_and_pixel_transform_origin() {
        // 构造围绕自定义原点的二倍缩放。
        let visual = ViewTransform {
            // 本测试不叠加最终平移。
            offset: Point::new(0.0, 0.0),
            // 使用二倍缩放观察原点不动点。
            scale: 2.0,
            // 不叠加额外旋转或倾斜。
            affine: Transform::identity(),
            // 水平轴使用固定像素，垂直轴使用帧底部比例。
            origin: TransformOrigin::new(
                // 从布局帧左侧偏移两个像素。
                crate::ui::TransformOriginValue::Pixels(2.0),
                // 使用布局帧底部。
                crate::ui::TransformOriginValue::Fraction(1.0),
            ),
        };
        // 使用非原点、非方形帧证明轴解析包含帧起点与尺寸。
        let frame = Rect::new(10.0, 20.0, 8.0, 6.0);
        // 自定义原点应解析到横坐标十二、纵坐标二十六。
        let origin = Point::new(12.0, 26.0);
        // 围绕原点缩放时原点自身必须保持不动。
        let transformed = visual.matrix(frame).transform_point(origin);
        // 横坐标保持解析后的固定像素原点。
        assert!((transformed.x - origin.x).abs() < 0.0001);
        // 纵坐标保持解析后的比例原点。
        assert!((transformed.y - origin.y).abs() < 0.0001);
    }
}
