// 引入被测图片组组件。
use super::ImageGroup;
// 引入公开 View 构建入口。
use crate::ui::view::View;

// 验证 UIX 声明壳保持原图片组单叶节点与默认状态。
#[test]
fn uix_root_preserves_image_group_kernel_and_defaults() {
    // 通过公开 View 契约构建默认画廊。
    let node = View::build(ImageGroup::new());
    // UIX 声明不得增加包装或展示子节点。
    assert!(node.children.is_empty());
    // 根动态类型必须继续是拥有画廊索引和预览生命周期的 ImageGroup。
    let kernel = node
        .widget
        .as_any()
        .downcast_ref::<ImageGroup>()
        .expect("UIX 根必须保留 ImageGroup 内核");
    // 默认集合、索引和预览开关必须无损进入同一内核。
    assert!(kernel.images.is_empty());
    assert_eq!(kernel.current_index(), 0);
    assert!(!kernel.is_preview_open());
}
