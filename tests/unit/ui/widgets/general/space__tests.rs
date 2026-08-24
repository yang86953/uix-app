// 导入当前模块的 Space 与布局类型。
use super::*;
// 导入调用布局 trait 所需的公开接口。
use crate::ui::WidgetLayout;

// 验证 Space 把子项 margin 传入共享 Flex 并计入缓存。
#[test]
fn child_margins_affect_positions_and_cached_content_size() {
    // 构造默认水平且无固定尺寸的 Space。
    let space = Space::new();
    // 构造二十乘十的自然尺寸子项。
    let mut child = LayoutChild::new(WidgetId::new(1), Size::new(20.0, 10.0));
    // 四侧 margin 使自然外尺寸达到三十乘二十。
    child.margin = crate::core::EdgeInsets::new(2.0, 3.0, 8.0, 7.0);
    // 空树足以满足无树读取的布局入口。
    let tree = WidgetTree::new();
    // 在零尺寸 bootstrap frame 中执行组件布局。
    let positions = space.layout_children(Rect::zero(), &[child], &tree);
    // 子项应从左上 margin 后开始并保留自然尺寸。
    assert_eq!(positions[0].1, Rect::new(2.0, 3.0, 20.0, 10.0));
    // 缓存必须记录包含右下 margin 的完整外尺寸。
    assert_eq!(space.cached_content_size.get(), Size::new(30.0, 20.0));
}

// 验证间距档位与直接构造、View 构建都读取同一 UIX 静态视觉。
#[test]
fn view_build_and_gap_sizes_use_uix_visual() {
    assert_eq!(SpaceSize::Small.value(), 8.0);
    assert_eq!(SpaceSize::Middle.value(), 16.0);
    assert_eq!(SpaceSize::Large.value(), 24.0);
    let space = Space::new();
    assert!(std::ptr::eq(space.visual, SPACE_VISUAL_REF));
    let node = crate::ui::view::View::build(space);
    let space = node
        .widget
        .as_any()
        .downcast_ref::<Space>()
        .expect("UIX 根必须保留 Space Rust 内核");
    assert!(std::ptr::eq(space.visual, SPACE_VISUAL_REF));
}
