// 引入被测头像及其私有声明配置。
use super::Avatar;
// 引入公开 View 构建入口。
use crate::ui::view::View;

// 验证 UIX 声明壳保持原头像单叶节点及资源配置。
#[test]
fn uix_root_preserves_avatar_kernel() {
    // 构建带方形与自定义尺寸配置的头像。
    let node = View::build(Avatar::new("AL").size(40.0).square(true));
    // UIX 声明不得增加包装或展示子节点。
    assert!(node.children.is_empty());
    // 运行时动态类型必须继续是拥有资源缓存与绘制机制的 Avatar。
    let kernel = node
        .widget
        .as_any()
        .downcast_ref::<Avatar>()
        .expect("UIX 根必须保留 Avatar 内核");
    // 构建前后共享 UIX 生成的唯一静态地址，实例不复制完整视觉表。
    assert!(std::ptr::eq(kernel.visual, super::AVATAR_VISUAL_REF));
    assert!(
        std::mem::size_of::<super::AvatarVisual>()
            > std::mem::size_of::<&'static super::AvatarVisual>()
    );
    // 作者配置必须无损进入同一内核。
    assert_eq!(kernel.text, "AL");
    assert_eq!(kernel.size, 40.0);
    assert!(kernel.square);
    // 显式尺寸必须优先于 UIX 默认值，静态视觉则由 UIX 融合。
    assert!(kernel.size_authored);
    assert_eq!(kernel.visual.default_extent, 32.0);
    assert_eq!(kernel.visual.text_scale, 0.45);
    assert_eq!(kernel.visual.square_text_fit, 0.78);
    assert_eq!(kernel.visual.circle_text_fit, 0.68);
}
