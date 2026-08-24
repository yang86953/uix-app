// 引入 Button 与 UIX 生成的唯一视觉引用。
use super::{BUTTON_VISUAL_REF, Button};
use crate::core::{Constraints, EdgeInsets};
use crate::ui::theme::style::{Style, StyleSet, TypographyToken};
use crate::ui::widget_runtime::traits::WidgetLayout;

// 验证直接构造与 View 构建共享同目录 UIX 的静态视觉地址。
#[test]
fn view_build_uses_uix_visual_without_a_rust_default_copy() {
    let button = Button::new("UIX Button");
    assert!(std::ptr::eq(button.visual, BUTTON_VISUAL_REF));
    let node = crate::ui::view::View::build(button);
    let button = node
        .widget
        .as_any()
        .downcast_ref::<Button>()
        .expect("UIX 根必须保留 Button Rust 内核");
    assert!(std::ptr::eq(button.visual, BUTTON_VISUAL_REF));
}

// 验证默认按钮实例复用同一缓存样式集，避免每次构建重复分配。
#[test]
fn default_buttons_share_the_cached_uix_selected_style_set() {
    let first = Button::new("一");
    let second = Button::new("二");
    assert!(std::sync::Arc::ptr_eq(&first.style_set, &second.style_set));
}

// 验证借用式尺寸解析保持基础样式和固定覆盖的优先级。
#[test]
fn intrinsic_size_resolves_only_sizing_fields_with_style_precedence() {
    let base = Style {
        padding: EdgeInsets::uniform(5.0),
        height: Some(44.0),
        font_size: TypographyToken::Custom(20.0),
        ..Style::default()
    };
    let text = "WWWWWWWWWW";
    let base_button = Button::new(text).style_set(StyleSet::new(base.clone()));
    let base_size = WidgetLayout::measure(&base_button, Constraints::unconstrained());
    // 十个 W 各占字号 0.7 倍，加基础样式左右各五像素内边距。
    assert!((base_size.w - 150.0).abs() < 0.0001);
    assert_eq!(base_size.h, 44.0);

    let fixed = Style {
        padding: EdgeInsets::uniform(12.0),
        font_size: TypographyToken::Custom(10.0),
        ..Style::default()
    };
    let overridden = Button::new(text)
        .style_set(StyleSet::new(base))
        .style(fixed);
    let overridden_size = WidgetLayout::measure(&overridden, Constraints::unconstrained());
    // 固定覆盖将字号降为十像素并把左右内边距提高到二十四像素。
    assert!((overridden_size.w - 94.0).abs() < 0.0001);
    // 未覆盖高度继续继承基础样式。
    assert_eq!(overridden_size.h, 44.0);
}
