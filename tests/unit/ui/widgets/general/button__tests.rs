// 引入 Button 与 UIX 生成的唯一视觉引用。
use super::{BUTTON_VISUAL_REF, Button};

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
