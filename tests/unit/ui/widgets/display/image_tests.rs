// 引入被测图片组件。
use super::Image;
// 引入公开 View 构建入口。
use crate::ui::view::View;

// 验证 UIX 声明壳保持原图片单叶节点与公开配置。
#[test]
fn uix_root_preserves_image_kernel_and_configuration() {
    // 构建带资源、替代文本和非默认预览开关的图片。
    let node = View::build(
        Image::new(96.0, 64.0)
            .src("assets/hero.png")
            .alt("主图")
            .preview(false),
    );
    // UIX 声明不得增加包装或展示子节点。
    assert!(node.children.is_empty());
    // 根动态类型必须继续是拥有加载和预览生命周期的 Image。
    let kernel = node
        .widget
        .as_any()
        .downcast_ref::<Image>()
        .expect("UIX 根必须保留 Image 内核");
    // 固有尺寸、资源与交互开关必须无损进入同一内核。
    assert_eq!((kernel.width, kernel.height), (96.0, 64.0));
    assert_eq!(kernel.src, "assets/hero.png");
    assert_eq!(kernel.alt, "主图");
    assert!(!kernel.preview);
    // 同目录 UIX 必须真实提供占位、指示器、预览边距与覆盖层级。
    assert_eq!(
        kernel.visual_contract_for_test(),
        (6.0, 8.0, 20.0, 48.0, 1100)
    );
}

// 验证 UIX 视觉表由全部 Image 实例共享，不按实例复制大配置。
#[test]
fn image_instances_share_uix_visual_table() {
    let first = View::build(Image::new(32.0, 32.0));
    let second = View::build(Image::new(64.0, 48.0));
    let first = first.widget.as_any().downcast_ref::<Image>().unwrap();
    let second = second.widget.as_any().downcast_ref::<Image>().unwrap();
    assert!(first.shares_visual_with_for_test(second));
}
