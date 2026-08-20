//! RHI 预乘颜色值对象的共享契约测试。

// 复用父模块唯一颜色值类型。
use super::RhiColor;

// 验证 straight 输入只在共享层预乘一次。
#[test]
fn straight_color_becomes_one_valid_premultiplied_value() {
    // 构造可精确表示的半透明 straight-alpha 颜色。
    let color = RhiColor::from_straight_rgba([1.0, 0.5, 0.25, 0.5]);
    // 三个颜色通道必须分别乘以二分之一 alpha。
    assert_eq!(color.components(), [0.5, 0.25, 0.125, 0.5]);
    // 转换结果必须满足 render target 的预乘不变量。
    assert!(color.is_valid());
}

// 验证共享边界拒绝伪装成预乘值的 straight 载荷。
#[test]
fn invalid_premultiplied_payload_is_rejected() {
    // 红色大于 alpha，不能作为预乘目标颜色。
    let color = RhiColor::from_premultiplied_rgba([0.75, 0.0, 0.0, 0.5]);
    // 两套 Adapter 都必须在原生 API 调用前拒绝同一个值。
    assert!(!color.is_valid());
}
