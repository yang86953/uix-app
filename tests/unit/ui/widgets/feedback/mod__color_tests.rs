// 引入被测 alpha 衰减入口。
use super::fade_token_color;
// 引入测试颜色值。
use crate::draw::Color;

// 动画必须缩放而不是覆盖主题 token 的基础 alpha。
#[test]
fn fade_token_color_scales_existing_alpha() {
    // 构造携带基础透明度的主题遮罩色。
    let mask = Color::from_rgba(1, 2, 3, 160);
    // 半程动画必须得到基础 alpha 的一半。
    assert_eq!(fade_token_color(mask, 0.5), Color::from_rgba(1, 2, 3, 80));
    // 超出范围的动画值必须限制为完整 token。
    assert_eq!(fade_token_color(mask, 2.0), mask);
}
