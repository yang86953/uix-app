// 引入被测图片组组件。
use super::ImageGroup;
// 引入缩略图范围测试所需矩形。
use crate::core::Rect;
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

// 验证无分配缩略图迭代器保持原居中窗口与等距几何。
#[test]
fn thumbnail_layout_preserves_centered_visible_range() {
    // 320×56 条带可容纳六个 44px 缩略图，当前第 4 项应位于窗口中央附近。
    let thumbnails = ImageGroup::thumbnail_layout(Rect::new(0.0, 100.0, 320.0, 56.0), 10, 4, 44.0)
        .iter()
        .collect::<Vec<_>>();

    assert_eq!(thumbnails.len(), 6);
    assert_eq!(thumbnails[0], (1, Rect::new(13.0, 106.0, 44.0, 44.0)));
    assert_eq!(thumbnails[5], (6, Rect::new(263.0, 106.0, 44.0, 44.0)));
    // 无图片或无可用条带时必须保持空迭代，不生成伪命中区域。
    assert_eq!(
        ImageGroup::thumbnail_layout(Rect::zero(), 0, 0, 44.0)
            .iter()
            .count(),
        0
    );
}
