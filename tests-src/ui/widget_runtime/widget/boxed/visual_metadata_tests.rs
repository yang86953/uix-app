//! `ui/widget_runtime/widget/boxed/visual_metadata.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;
use crate::ui::animation::AnimationConfig;
use crate::ui::{Label, Placement};

// 构造带稳定非默认基础变换和布局帧的实际节点。
fn transformed_node() -> BoxedWidget {
    let mut node = BoxedWidget::new(Box::new(Label::new("transform")));
    node.frame = Rect::new(12.0, 18.0, 80.0, 40.0);
    node.set_visual_transform(ViewTransform {
        offset: Point::new(7.0, -3.0),
        scale: 1.25,
        affine: crate::draw::Transform::rotate(0.2),
        origin: crate::ui::TransformOrigin::default(),
    });
    node
}

#[test]
fn matrix_without_transition_matches_the_base_transform() {
    let node = transformed_node();
    let expected = node.visual_transform.matrix(node.frame);
    assert_eq!(node.visual_transform_matrix(), expected);
}

#[test]
fn matrix_with_transition_preserves_overlay_composition() {
    let mut node = transformed_node();
    node.set_enter_animation(Some(AnimationConfig::slide_in(Placement::Left, 1.0)), None);
    let player = node
        .view_transition
        .as_ref()
        .expect("非零滑入动画必须建立过渡播放器");
    let transition = ViewTransform {
        offset: player.offset,
        scale: player.scale,
        affine: crate::draw::Transform::identity(),
        origin: crate::ui::TransformOrigin::default(),
    };
    let expected = node
        .visual_transform
        .combined(transition)
        .matrix(node.frame);
    assert_eq!(node.visual_transform_matrix(), expected);
}
