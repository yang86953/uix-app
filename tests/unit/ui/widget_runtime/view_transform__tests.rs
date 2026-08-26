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
