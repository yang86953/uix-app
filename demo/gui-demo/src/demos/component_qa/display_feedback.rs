use uix::prelude::*;

use super::COMPONENT_VISUAL_CASE_COUNT;

fn inventory_count_label() -> String {
    format!("{} 个组件", COMPONENT_VISUAL_CASE_COUNT)
}

fn sample_tree() -> Vec<TreeNode> {
    let mut components = TreeNode::new("组件（展开后可滚动）", "components");
    for index in 0..32 {
        components = components.add(TreeNode::new(
            &format!("组件测试项 {index:02}"),
            &format!("component-{index:02}"),
        ));
    }
    vec![
        components,
        TreeNode::new("质量", "quality")
            .add(TreeNode::new("视觉", "visual"))
            .add(TreeNode::new("交互", "interaction")),
    ]
}

fn carousel_slide(text: &str, color: ColorValue) -> ViewNode {
    column([label(text).font_size(22.0).padding_v(70.0)])
        .align(AlignItems::Center)
        .bg(color)
}

fn carousel_compact_slide(text: &str, color: ColorValue) -> ViewNode {
    column([label(text).font_size(14.0).padding_v(28.0)])
        .align(AlignItems::Center)
        .bg(color)
}

fn carousel_frame(
    carousel: ViewNode,
    width: f32,
    height: f32,
    automation_id: &'static str,
) -> ViewNode {
    grid([carousel.automation_id(automation_id)])
        .columns(vec![GridTrack::Fr(1.0)])
        .rows(vec![GridTrack::Fr(1.0)])
        .width(width)
        .height(height)
        .align_self(AlignItems::Start)
}

fn collapse_frame(
    collapse: Collapse,
    width: f32,
    height: f32,
    automation_id: &'static str,
) -> ViewNode {
    grid([embed(collapse).automation_id(automation_id)])
        .columns(vec![GridTrack::Fr(1.0)])
        .rows(vec![GridTrack::Fr(1.0)])
        .width(width)
        .height(height)
        .align_self(AlignItems::Start)
}

fn descriptions_frame(
    descriptions: Descriptions,
    width: f32,
    height: f32,
    automation_id: &'static str,
) -> ViewNode {
    grid([embed(descriptions).automation_id(automation_id)])
        .columns(vec![GridTrack::Fr(1.0)])
        .rows(vec![GridTrack::Fr(1.0)])
        .width(width)
        .height(height)
        .align_self(AlignItems::Start)
}

fn empty_frame(empty: Empty, width: f32, height: f32, automation_id: &'static str) -> ViewNode {
    grid([embed(empty).automation_id(automation_id)])
        .columns(vec![GridTrack::Fr(1.0)])
        .rows(vec![GridTrack::Fr(1.0)])
        .width(width)
        .height(height)
        .align_self(AlignItems::Start)
}

fn image_frame(image: Image, width: f32, height: f32, automation_id: &'static str) -> ViewNode {
    grid([embed(image).automation_id(automation_id)])
        .columns(vec![GridTrack::Fr(1.0)])
        .rows(vec![GridTrack::Fr(1.0)])
        .width(width)
        .height(height)
        .align_self(AlignItems::Start)
}

fn list_frame(list: List, width: f32, height: f32, automation_id: &'static str) -> ViewNode {
    grid([embed(list).automation_id(automation_id)])
        .columns(vec![GridTrack::Fr(1.0)])
        .rows(vec![GridTrack::Fr(1.0)])
        .width(width)
        .height(height)
        .align_self(AlignItems::Start)
}

fn result_frame(
    result: ResultView,
    width: f32,
    height: f32,
    automation_id: &'static str,
) -> ViewNode {
    grid([embed(result).automation_id(automation_id)])
        .columns(vec![GridTrack::Fr(1.0)])
        .rows(vec![GridTrack::Fr(1.0)])
        .width(width)
        .height(height)
        .align_self(AlignItems::Start)
}

fn selectable_list_frame(
    list: SelectableList,
    width: f32,
    height: f32,
    automation_id: &'static str,
) -> ViewNode {
    grid([embed(list).automation_id(automation_id)])
        .columns(vec![GridTrack::Fr(1.0)])
        .rows(vec![GridTrack::Fr(1.0)])
        .width(width)
        .height(height)
        .align_self(AlignItems::Start)
}

fn skeleton_frame(
    skeleton: Skeleton,
    width: f32,
    height: f32,
    automation_id: &'static str,
) -> ViewNode {
    grid([embed(skeleton).automation_id(automation_id)])
        .columns(vec![GridTrack::Fr(1.0)])
        .rows(vec![GridTrack::Fr(1.0)])
        .width(width)
        .height(height)
        .align_self(AlignItems::Start)
}

fn tag_frame(tag: Tag, width: f32, height: f32, automation_id: &'static str) -> ViewNode {
    grid([embed(tag).automation_id(automation_id)])
        .columns(vec![GridTrack::Fr(1.0)])
        .rows(vec![GridTrack::Fr(1.0)])
        .width(width)
        .height(height)
        .align_self(AlignItems::Start)
}

fn timeline_frame(
    timeline: Timeline,
    width: f32,
    height: f32,
    automation_id: &'static str,
) -> ViewNode {
    grid([embed(timeline).automation_id(automation_id)])
        .columns(vec![GridTrack::Fr(1.0)])
        .rows(vec![GridTrack::Fr(1.0)])
        .width(width)
        .height(height)
        .align_self(AlignItems::Start)
}

fn tree_frame(tree: Tree, width: f32, height: f32, automation_id: &'static str) -> ViewNode {
    grid([embed(tree).automation_id(automation_id)])
        .columns(vec![GridTrack::Fr(1.0)])
        .rows(vec![GridTrack::Fr(1.0)])
        .width(width)
        .height(height)
        .align_self(AlignItems::Start)
}

fn rich_text_frame(
    rich_text: RichText,
    width: f32,
    height: f32,
    automation_id: &'static str,
) -> ViewNode {
    grid([embed(rich_text)
        .width(width)
        .height(height)
        .automation_id(automation_id)])
    .columns(vec![GridTrack::Fr(1.0)])
    .rows(vec![GridTrack::Fr(1.0)])
    .width(width)
    .height(height)
    .align_self(AlignItems::Start)
}

fn alert_frame(alert: Alert, width: f32, height: f32, automation_id: &'static str) -> ViewNode {
    grid([embed(alert)
        .width(width)
        .height(height)
        .automation_id(automation_id)])
    .columns(vec![GridTrack::Fr(1.0)])
    .rows(vec![GridTrack::Fr(1.0)])
    .width(width)
    .height(height)
    .align_self(AlignItems::Start)
}

fn message_frame(
    message: Message,
    width: f32,
    height: f32,
    automation_id: &'static str,
) -> ViewNode {
    ViewNode::new(
        Grid::new()
            .columns(vec![GridTrack::Fr(1.0)])
            .rows(vec![GridTrack::Fr(1.0)])
            .justify(JustifyContent::Stretch),
        vec![embed(message).automation_id(automation_id)],
    )
    .width(width)
    .height(height)
    .align_self(AlignItems::Start)
}

fn notification_frame(
    notification: Notification,
    width: f32,
    height: f32,
    automation_id: &'static str,
) -> ViewNode {
    ViewNode::new(
        Grid::new()
            .columns(vec![GridTrack::Fr(1.0)])
            .rows(vec![GridTrack::Fr(1.0)])
            .justify(JustifyContent::Stretch),
        vec![embed(notification).automation_id(automation_id)],
    )
    .width(width)
    .height(height)
    .align_self(AlignItems::Start)
}

fn spin_frame(spin: Spin, width: f32, height: f32, automation_id: &'static str) -> ViewNode {
    ViewNode::new(
        Grid::new()
            .columns(vec![GridTrack::Fr(1.0)])
            .rows(vec![GridTrack::Fr(1.0)])
            .justify(JustifyContent::Stretch),
        vec![embed(spin).automation_id(automation_id)],
    )
    .width(width)
    .height(height)
    .align_self(AlignItems::Start)
}

fn avatar_frame(avatar: Avatar, width: f32, height: f32, automation_id: &'static str) -> ViewNode {
    grid([embed(avatar).automation_id(automation_id)])
        .columns(vec![GridTrack::Fr(1.0)])
        .rows(vec![GridTrack::Fr(1.0)])
        .width(width)
        .height(height)
        .align_self(AlignItems::Start)
}

fn calendar_frame(
    calendar: Calendar,
    width: f32,
    height: f32,
    automation_id: &'static str,
) -> ViewNode {
    grid([embed(calendar).automation_id(automation_id)])
        .columns(vec![GridTrack::Fr(1.0)])
        .rows(vec![GridTrack::Fr(1.0)])
        .width(width)
        .height(height)
        .align_self(AlignItems::Start)
}

fn card_frame(card: Card, width: f32, height: f32, automation_id: &'static str) -> ViewNode {
    grid([embed(card).automation_id(automation_id)])
        .columns(vec![GridTrack::Fr(1.0)])
        .rows(vec![GridTrack::Fr(1.0)])
        .width(width)
        .height(height)
        .align_self(AlignItems::Start)
}

// 展示与反馈类测试场景拆入独立文件，保持主文件处于行数上限内。
#[path = "build_display_a.rs"]
mod build_display_a;
#[path = "build_display_b.rs"]
mod build_display_b;
#[path = "build_feedback.rs"]
mod build_feedback;

pub fn build(id: &str, tk: &DesignTokens) -> Option<ViewNode> {
    // 展示类与反馈类分组构建，任一命中即返回场景。
    build_display_a::build(id, tk)
        .or_else(|| build_display_b::build(id, tk))
        .or_else(|| build_feedback::build(id, tk))
}
