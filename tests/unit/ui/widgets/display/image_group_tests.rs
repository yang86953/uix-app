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
    // 同目录 UIX 必须真实提供固有尺寸、双条带、导航控制与覆盖层级。
    assert_eq!(
        kernel.visual_contract_for_test(),
        (320.0, 220.0, 56.0, 72.0, 30.0, 1100)
    );
}

// 验证无分配缩略图迭代器保持原居中窗口与等距几何。
#[test]
fn thumbnail_layout_preserves_centered_visible_range() {
    let group = ImageGroup::new();
    // 320×56 条带可容纳六个 44px 缩略图，当前第 4 项应位于窗口中央附近。
    let thumbnails = group
        .thumbnail_layout(Rect::new(0.0, 100.0, 320.0, 56.0), 10, 4, 44.0)
        .iter()
        .collect::<Vec<_>>();

    assert_eq!(thumbnails.len(), 6);
    assert_eq!(thumbnails[0], (1, Rect::new(13.0, 106.0, 44.0, 44.0)));
    assert_eq!(thumbnails[5], (6, Rect::new(263.0, 106.0, 44.0, 44.0)));
    // 无图片或无可用条带时必须保持空迭代，不生成伪命中区域。
    assert_eq!(
        group
            .thumbnail_layout(Rect::zero(), 0, 0, 44.0)
            .iter()
            .count(),
        0
    );
}

// 验证 UIX 视觉表由全部 ImageGroup 实例共享，不按实例复制大配置。
#[test]
fn image_group_instances_share_uix_visual_table() {
    let first = View::build(ImageGroup::new());
    let second = View::build(ImageGroup::new());
    let first = first.widget.as_any().downcast_ref::<ImageGroup>().unwrap();
    let second = second.widget.as_any().downcast_ref::<ImageGroup>().unwrap();
    assert!(first.shares_visual_with_for_test(second));
}

// 验证稳定预览帧复用计数字符串的同一缓冲区。
#[test]
fn preview_counter_cache_does_not_reallocate_when_selection_is_unchanged() {
    let group = ImageGroup::new();
    group.refresh_counter_text(1, 12);
    let first = group.counter_cache.borrow();
    let pointer = first.text.as_ptr();
    let capacity = first.text.capacity();
    assert_eq!(first.text, "2 / 12");
    drop(first);

    group.refresh_counter_text(1, 12);
    let second = group.counter_cache.borrow();
    assert_eq!(second.text.as_ptr(), pointer);
    assert_eq!(second.text.capacity(), capacity);
}
