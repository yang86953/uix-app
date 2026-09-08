//! Content 内容区的弹性布局、背景绘制与声明式视觉。

use super::{
    layout_shell_children, layout_shell_children_into, measure_shell_children,
    measure_shell_children_into,
};
use crate::core::{Constraints, Rect, Size};
use crate::draw::Color;
use crate::ui::layout::{FlexDirection, LayoutChild};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::widget::WidgetTree;
use crate::ui::{SnapshotFields, View, ViewNode, WidgetId};
use crate::widget;

// 保存 Content 的默认尺寸、弹性增长与子树方向。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ContentVisual {
    intrinsic_width: f32,
    intrinsic_height: f32,
    flex_grow: f32,
    child_direction: FlexDirection,
}

crate::uix_items!("src/ui/widgets/containers/layout/content/content.uix");

pub(crate) const fn content_child_direction() -> FlexDirection {
    FlexDirection::Column
}

widget! {
    /// 页面内容区。
    pub struct Content {
        bg_color: Option<Color>,
        /// 同目录 UIX 生成的唯一静态视觉表。
        pub(crate) visual: &'static ContentVisual,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    flex_grow => (&self) -> f32 { self.visual.flex_grow }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        if let Some(bg) = self.bg_color {
            ctx.fill_rect(frame, bg, None);
        }
    }

    layout_children => (&self, frame: Rect, children: &[LayoutChild], _tree: &WidgetTree)
        -> Vec<(WidgetId, Rect)>
    {
        layout_shell_children(self.visual.child_direction, frame, children)
    }

    layout_children_into => (
        &self,
        frame: Rect,
        children: &[LayoutChild],
        _tree: &WidgetTree,
        scratch: &mut crate::ui::LayoutEngineScratch,
        output: &mut Vec<(WidgetId, Rect)>
    ) {
        layout_shell_children_into(
            self.visual.child_direction,
            frame,
            children,
            scratch,
            output,
        );
    }

    flex_layout_axes => (&self) -> Option<(FlexDirection, crate::ui::layout::AlignItems)> {
        Some((self.visual.child_direction, crate::ui::layout::AlignItems::Stretch))
    }

    measure_children => (&self, frame: Rect, children: &[WidgetId], tree: &WidgetTree)
        -> Vec<LayoutChild>
    {
        measure_shell_children(self.visual.child_direction, frame, children, tree)
    }

    measure_children_into => (
        &self,
        frame: Rect,
        children: &[WidgetId],
        tree: &WidgetTree,
        output: &mut Vec<LayoutChild>
    ) {
        measure_shell_children_into(self.visual.child_direction, frame, children, tree, output);
    }
}

impl Default for Content {
    fn default() -> Self {
        Self::new()
    }
}

impl Content {
    /// 创建不覆盖主题背景的内容区。
    pub fn new() -> Self {
        Self {
            bg_color: None,
            visual: CONTENT_VISUAL_REF,
        }
    }

    pub fn bg(mut self, color: Color) -> Self {
        self.bg_color = Some(color);
        self
    }

    /// 便捷包裹单个子节点。
    pub fn child(
        self,
        node: impl crate::ui::IntoWidgetNode,
    ) -> crate::ui::widget_runtime::widget::WidgetNode {
        crate::ui::widget_runtime::widget::WidgetNode::new(Box::new(self), vec![node.into_node()])
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(self.visual.intrinsic_width, self.visual.intrinsic_height)
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.bg_color = next.bg_color;
        self.visual = next.visual;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Content {
            bg_color: self.bg_color,
        }
    }
}

fn build_content_view(mut kernel: Content, visual: &'static ContentVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Content {
    fn build(self) -> ViewNode {
        build_content_uix_root(self)
    }
}

fn build_content_uix_root(kernel: Content) -> ViewNode {
    crate::uix!("src/ui/widgets/containers/layout/content/content.uix")
}
