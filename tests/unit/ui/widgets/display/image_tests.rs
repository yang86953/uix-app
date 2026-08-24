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
}
