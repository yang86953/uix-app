// 引入被测的纯几何函数。
use super::spread_shadow_geometry;
// 引入矩形与圆角值类型。
use crate::{core::Rect, draw::Radius};
// 引入公开阴影定义以验证兼容构造器。
use crate::ui::theme::style::BoxShadowDef;

// 验证旧构造器保持零 spread，并允许显式替换。
#[test]
fn box_shadow_definition_preserves_legacy_constructor() {
    // 构造旧四参数阴影定义。
    let legacy = BoxShadowDef::new(crate::draw::Color::BLACK, 4.0, 1.0, 2.0);
    // 旧构造器必须保持零 spread。
    assert_eq!(legacy.spread, 0.0);
    // 显式 builder 只替换 spread。
    let expanded = legacy.with_spread(3.0);
    // spread 必须保存调用方值。
    assert_eq!(expanded.spread, 3.0);
    // 其他旧字段必须完整保留。
    assert_eq!(
        // 提取旧字段形成稳定比较元组。
        (
            expanded.color,
            expanded.blur,
            expanded.offset_x,
            expanded.offset_y
        ),
        // 与旧定义逐项比较。
        (legacy.color, legacy.blur, legacy.offset_x, legacy.offset_y)
    );
}

// 验证正 spread 同步外扩矩形和圆角。
#[test]
fn positive_spread_expands_shadow_geometry() {
    // 对 10x20 基准盒应用 2 像素 spread。
    let geometry = spread_shadow_geometry(
        // 使用非零原点覆盖左上角移动。
        Rect::new(5.0, 7.0, 10.0, 20.0),
        // 使用统一圆角验证轮廓同步扩张。
        Some(Radius::uniform(3.0)),
        // 向外扩张两个逻辑像素。
        2.0,
    )
    // 正 spread 不会产生空几何。
    .expect("正 spread 应保留阴影几何");
    // 矩形四边必须各外扩两个像素。
    assert_eq!(geometry.0, Rect::new(3.0, 5.0, 14.0, 24.0));
    // 圆角必须同步增加两个像素。
    assert_eq!(geometry.1, Some(Radius::uniform(5.0)));
}

// 验证负 spread 内缩并在耗尽尺寸时不绘制。
#[test]
fn negative_spread_contracts_or_removes_shadow_geometry() {
    // 先验证仍保留正面积的内缩。
    let contracted = spread_shadow_geometry(
        // 使用方形基准盒。
        Rect::new(0.0, 0.0, 10.0, 10.0),
        // 使用小圆角验证下限钳制。
        Some(Radius::uniform(1.0)),
        // 每边向内收缩两个像素。
        -2.0,
    )
    // 仍有面积时必须返回几何。
    .expect("有限负 spread 应保留剩余几何");
    // 矩形必须从四边各内缩两个像素。
    assert_eq!(contracted.0, Rect::new(2.0, 2.0, 6.0, 6.0));
    // 圆角不能降到负值。
    assert_eq!(contracted.1, Some(Radius::zero()));
    // 尺寸被完全耗尽时不得提交零面积阴影。
    assert!(spread_shadow_geometry(Rect::new(0.0, 0.0, 4.0, 4.0), None, -2.0).is_none());
}

// 验证零值与非有限 Rust 输入都保持原几何。
#[test]
fn zero_and_non_finite_spread_keep_original_geometry() {
    // 固定一份原始矩形。
    let rect = Rect::new(1.0, 2.0, 3.0, 4.0);
    // 固定一份原始圆角。
    let radius = Some(Radius::uniform(2.0));
    // 零 spread 必须保持输入不变。
    assert_eq!(
        spread_shadow_geometry(rect, radius, 0.0),
        Some((rect, radius))
    );
    // Rust 直接传入非有限值时按零 spread 安全处理。
    assert_eq!(
        spread_shadow_geometry(rect, radius, f32::INFINITY),
        Some((rect, radius))
    );
}
