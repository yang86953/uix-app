//! `ui/widgets/display/rich_text/typography.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;
use crate::ui::theme::style::ColorValue;

#[test]
fn layout_only_style_preserves_authored_rich_text_color() {
    let mut rich = RichText::new().color(Color::from_rgb(255, 0, 0));
    rich.apply_view_layout_style(&Style::default(), None, None);
    assert_eq!(rich.view_text_color(), None);
    let mut explicit = Style::default();
    explicit.color = ColorValue::Custom(Color::from_rgb(0, 0, 255));
    rich.apply_view_layout_style(&explicit, None, None);
    assert_eq!(rich.view_text_color(), Some(explicit.color));
    let mut themed = RichText::new();
    themed.apply_view_layout_style(&Style::default(), None, None);
    assert_eq!(themed.view_text_color(), Some(Style::default().color));
}
