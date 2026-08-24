// 引入被测时间线与节点构建器。
use super::{Timeline, TimelineItem};
// 引入最终绘制颜色值。
use crate::draw::Color;
// 引入公开 View 构建入口。
use crate::ui::view::View;

// 默认节点跟随主题，显式颜色保持最高优先级。
#[test]
fn timeline_item_resolves_theme_and_explicit_colors() {
    // 使用与默认主题无关的测试主色。
    let theme_primary = Color::from_rgb(1, 2, 3);
    // 未覆写节点必须使用调用方提供的当前主题主色。
    let themed = TimelineItem::new("主题节点");
    // 核对主题默认路径。
    assert_eq!(themed.resolved_dot_color(theme_primary), theme_primary);
    // 构造明确不同的显式节点色。
    let explicit = Color::from_rgb(4, 5, 6);
    // 使用公开构建器登记显式颜色。
    let customized = TimelineItem::new("自定义节点").color(explicit);
    // 核对显式覆写优先于主题主色。
    assert_eq!(customized.resolved_dot_color(theme_primary), explicit);
}

// 验证 UIX 声明根保持数据配置并注入真实视觉契约。
#[test]
fn uix_root_preserves_timeline_kernel_and_visual_contract() {
    let node = View::build(
        Timeline::new()
            .add(TimelineItem::new("发布"))
            .pending(true)
            .reverse(true),
    );
    assert!(node.children.is_empty());
    let kernel = node
        .widget
        .as_any()
        .downcast_ref::<Timeline>()
        .expect("UIX 根必须保留 Timeline 内核");
    assert_eq!(kernel.items.len(), 1);
    assert!(kernel.pending);
    assert!(kernel.reverse);
    assert_eq!(
        kernel.visual_contract_for_test(),
        (400.0, 60.0, 28.0, 5.0, 2.0)
    );
}

// 验证全部 Timeline 实例共享同一份 UIX 视觉表。
#[test]
fn timeline_instances_share_uix_visual_table() {
    let first = View::build(Timeline::new());
    let second = View::build(Timeline::new());
    let first = first.widget.as_any().downcast_ref::<Timeline>().unwrap();
    let second = second.widget.as_any().downcast_ref::<Timeline>().unwrap();
    assert!(first.shares_visual_with_for_test(second));
}
