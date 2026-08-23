// 引入矩形以固定滚动视口与期望可见边界。
use crate::core::Rect;
#[cfg(feature = "feedback")]
use crate::draw::Transform;
#[cfg(feature = "feedback")]
use crate::draw::scene::ScenePaint;
// 引入声明树到实际树的唯一适配边界。
use crate::ui::adapter::ViewAdapter;
// 引入实际组件 frame 的测试写入契约。
use crate::ui::widget_runtime::widget::WidgetCore;
// 引入滚动容器、固定尺寸叶节点与声明节点。
#[cfg(feature = "feedback")]
use crate::ui::{Modal, View};
use crate::ui::{ScrollView, Space, ViewNode};
// 引入纵向滚动模式。
use crate::platform::windowing::ScrollDirection;

// 滚动内容的视觉子树边界必须裁剪到真实可见视口。
#[test]
fn visual_subtree_bounds_clip_oversized_scroll_content() {
    // 用一百二十像素内容制造超过五十像素视口的稳定溢出。
    let root = ViewNode::new(
        ScrollView::new(ScrollDirection::Vertical)
            .size(100.0, 50.0)
            .show_scrollbar(false),
        vec![ViewNode::leaf(Space::new().width(100.0).height(120.0))],
    );
    // 通过真实适配器发布并取得滚动内容节点。
    let mut tree = ViewAdapter::build_nodes(root);
    let root_id = tree.root_id().expect("滚动根节点必须存在");
    let content_id = tree.get(root_id).expect("滚动根节点必须可读").children()[0];
    // 固定窗口等价客户区并执行正式布局。
    tree.get_mut(root_id)
        .expect("滚动根节点必须可写")
        .set_frame(Rect::new(0.0, 0.0, 100.0, 50.0));
    tree.layout();
    // 原始内容仍保留完整高度，证明测试确实覆盖了溢出路径。
    assert_eq!(
        tree.get(content_id).expect("滚动内容必须存在").frame(),
        Rect::new(0.0, 0.0, 100.0, 120.0)
    );
    // 失效边界只能覆盖屏幕上真实可见的五十像素视口。
    assert_eq!(
        tree.visual_subtree_bounds(content_id),
        Some(Rect::new(0.0, 0.0, 100.0, 50.0))
    );
}

// 窗口级 Modal 位于滚动内容中时仍必须使用视口坐标。
#[cfg(feature = "feedback")]
#[test]
fn modal_overlay_does_not_inherit_ancestor_scroll() {
    // 构造一个打开的 Modal，使其真实声明 OverlayKind::Modal。
    let modal = Modal::show(|_| ViewNode::leaf(Space::new().width(80.0).height(40.0))).build();
    // 把 Modal 放进高度较小的滚动根，制造稳定祖先偏移。
    let root = ViewNode::new(
        ScrollView::new(ScrollDirection::Vertical)
            .size(320.0, 120.0)
            .show_scrollbar(false),
        vec![modal],
    );
    let mut tree = ViewAdapter::build_nodes(root);
    let root_id = tree.root_id().expect("滚动根节点必须存在");
    let modal_id = tree.get(root_id).expect("滚动根节点必须可读").children()[0];
    tree.get_mut(root_id)
        .expect("滚动根节点必须可写")
        .set_frame(Rect::new(0.0, 0.0, 320.0, 120.0));
    tree.layout();
    // 使用公开滚动入口写入四十像素偏移。
    tree.get_mut(root_id)
        .expect("滚动根节点必须可写")
        .widget_mut()
        .as_any_mut()
        .downcast_mut::<ScrollView>()
        .expect("根节点必须保持 ScrollView 类型")
        .set_scroll_y(40.0);
    // Modal 自己按逻辑表面绘制 mask、dialog 与文字，根变换不得再包含 -scroll。
    assert_eq!(
        ScenePaint::node_overlay_transform(&tree, modal_id),
        Transform::identity()
    );
}
