// 引入当前线型和统一样式契约。
use super::{super::Style, BorderStyle};
// 引入边框颜色与盒模型宽度所需值。
use crate::core::EdgeInsets;
// 引入可直接构造的绘制颜色。
use crate::draw::Color;
// 引入统一样式颜色值。
use crate::ui::theme::style::ColorValue;

// 显式 solid 必须能够覆盖既有 dashed。
#[test]
fn explicit_solid_overrides_inherited_border_style() {
    // 构造既有虚线样式。
    let base = Style::default().with_border_style(BorderStyle::Dashed);
    // 构造显式实线覆盖。
    let overlay = Style::default().with_border_style(BorderStyle::Solid);
    // 统一合并入口必须保留显式默认值。
    let merged = base.apply(overlay);
    // 有效线型应变为实线而不是继续虚线。
    assert_eq!(merged.effective_border_style(), BorderStyle::Solid);
}

// none 只禁止绘制，不删除盒模型边框宽度。
#[test]
fn none_disables_paint_without_removing_border_width() {
    // 从默认样式开始设置完整可绘制边框。
    let mut style = Style::default();
    // 设置确定的自定义边框颜色。
    style.border_color = Some(ColorValue::Custom(Color::black()));
    // 保留参与盒模型的两像素宽度。
    style.border_width = EdgeInsets::uniform(2.0);
    // 显式选择不绘制线型。
    style.border_style = Some(BorderStyle::None);
    // 绘制 Gate 必须关闭。
    assert!(!style.has_border());
    // 盒模型宽度必须保持不变。
    assert_eq!(style.border_width, EdgeInsets::uniform(2.0));
}
