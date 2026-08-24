// 引入 Affix 与 UIX 生成的唯一视觉引用。
use super::{AFFIX_VISUAL_REF, Affix};

// 验证直接构造与 View 构建共享同目录 UIX 的静态视觉地址。
#[test]
fn view_build_uses_uix_visual_without_a_rust_default_copy() {
    let affix = Affix::default();
    assert!(std::ptr::eq(affix.visual, AFFIX_VISUAL_REF));
    let node = crate::ui::view::View::build(affix);
    let affix = node
        .widget
        .as_any()
        .downcast_ref::<Affix>()
        .expect("UIX 根必须保留 Affix Rust 内核");
    assert!(std::ptr::eq(affix.visual, AFFIX_VISUAL_REF));
}

// 验证吸顶状态边界读取 UIX 声明阈值。
#[test]
fn sticky_activation_uses_uix_threshold() {
    let mut affix = Affix::default();
    affix.set_child_bounds(10.0, 20.0);
    assert!(!affix.update_scroll(10.0));
    assert!(affix.update_scroll(10.02));
    assert!(affix.is_affixed());
}
