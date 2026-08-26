// 引入本文件的纯 lowering helper 与通用 RHI shape 类型。
use super::{RhiOp, RhiShapeRect, shape_rhi_op};

// 创建不依赖 native context 的有限 shape fixture。
fn shape_fixture() -> RhiShapeRect {
    // 返回有效的圆角矩形常量。
    RhiShapeRect {
        // 使用有限正几何覆盖 shape ABI。
        x: 1.0,
        // 保持 y 坐标有限。
        y: 2.0,
        // 保持宽度为正。
        w: 8.0,
        // 保持高度为正。
        h: 6.0,
        // 使用 premultiplied 红色常量。
        rgba: [1.0, 0.0, 0.0, 1.0],
        // 使用统一的有限圆角。
        radius: [2.0; 4],
        // 填充 shape 不需要描边宽度。
        half_stroke: 0.0,
        // fixture 不需要裁剪。
        scissor: None,
    }
}

// Additive 必须选择独立 pipeline，并受事实能力门禁保护。
#[test]
fn additive_shape_selection_requires_explicit_capability() {
    // 能力存在时必须生成 AdditiveShape。
    let supported = shape_rhi_op(shape_fixture(), true, true);
    // 不得把 Additive 偷换成普通 Shape。
    assert!(matches!(supported, Some(RhiOp::AdditiveShape(_))));
    // 能力缺失时必须原子回退。
    assert!(shape_rhi_op(shape_fixture(), true, false).is_none());
    // 普通 SrcOver shape 不依赖可选 Additive 能力。
    let normal = shape_rhi_op(shape_fixture(), false, false);
    // 普通路径仍选择 Shape。
    assert!(matches!(normal, Some(RhiOp::Shape(_))));
}

// 带半描边宽度的 shape 必须沿用同一 Additive pipeline 与能力门禁。
#[test]
fn additive_stroke_shape_selection_requires_explicit_capability() {
    // 从合法 shape fixture 构造一个真实描边载荷。
    let mut stroke = shape_fixture();
    // 非零半宽让测试明确覆盖描边 shader 分支。
    stroke.half_stroke = 0.5;
    // 能力存在时描边必须生成 AdditiveShape。
    let supported = shape_rhi_op(stroke, true, true);
    // 不得因为存在描边宽度而退回普通 Shape。
    assert!(matches!(supported, Some(RhiOp::AdditiveShape(_))));
    // 重新构造载荷验证缺少能力时的原子回退。
    let mut unsupported = shape_fixture();
    // 保持与支持分支相同的描边宽度。
    unsupported.half_stroke = 0.5;
    // adapter 未声明能力时不得生成任何 RHI operation。
    assert!(shape_rhi_op(unsupported, true, false).is_none());
}
