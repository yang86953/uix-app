// 引入当前模块公开值和 Style。
use super::{super::Style, LineHeight};

// 验证倍率与像素值按各自单位解析。
#[test]
fn line_height_resolves_factor_and_pixels() {
    // 创建一倍半行高。
    let factor = LineHeight::factor(1.5).expect("正倍率必须有效");
    // 创建固定二十四像素行高。
    let pixels = LineHeight::pixels(24.0).expect("正像素必须有效");
    // 倍率必须乘以最终字号。
    assert_eq!(factor.resolve(16.0), 24.0);
    // 固定像素不随字号变化。
    assert_eq!(pixels.resolve(16.0), 24.0);
}

// 验证非法值拒绝且显式行高覆盖继承值。
#[test]
fn line_height_rejects_invalid_and_overrides_inherited_value() {
    // 零值不能进入行高契约。
    assert!(LineHeight::factor(0.0).is_none());
    // 非有限像素不能进入行高契约。
    assert!(LineHeight::pixels(f32::INFINITY).is_none());
    // 基础样式使用倍率。
    let base = Style::default()
        // 设置一倍半继承值。
        .with_line_height(LineHeight::factor(1.5).expect("倍率有效"));
    // 差异样式使用固定像素。
    let overlay = Style::default()
        // 设置二十像素覆盖值。
        .with_line_height(LineHeight::pixels(20.0).expect("像素有效"));
    // 显式差异值必须覆盖继承值。
    assert_eq!(base.apply(overlay).resolve_line_height(10.0), Some(20.0));
}
