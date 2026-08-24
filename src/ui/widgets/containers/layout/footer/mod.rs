//! Footer 底部栏的固定高度、子树布局与声明式视觉。

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

// 保存 Footer 的默认高度与子树方向。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct FooterVisual {
    default_height: f32,
    child_direction: FlexDirection,
}

crate::uix_items!("src/ui/widgets/containers/layout/footer/footer.uix");

pub(crate) const fn footer_child_direction() -> FlexDirection {
    FlexDirection::Column
}

widget! {
    /// 页面底部栏。
    pub struct Footer {
        height: f32,
        bg_color: Option<Color>,
        /// 同目录 UIX 生成的唯一静态视觉表。
        pub(crate) visual: &'static FooterVisual,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

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

    measure_children => (&self, frame: Rect, children: &[WidgetId], tree: &WidgetTree)
        -> Vec<LayoutChild>
    {
        measure_shell_children(frame, children, tree)
    }

    measure_children_into => (
        &self,
        frame: Rect,
        children: &[WidgetId],
        tree: &WidgetTree,
        output: &mut Vec<LayoutChild>
    ) {
        measure_shell_children_into(frame, children, tree, output);
    }
}

impl Footer {
    /// 创建指定逻辑高度且不覆盖主题背景的底部栏。
    pub fn new(height: f32) -> Self {
        Self {
            height,
            bg_color: None,
            visual: FOOTER_VISUAL_REF,
        }
    }

    pub fn bg(mut self, color: Color) -> Self {
        self.bg_color = Some(color);
        self
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(0.0, self.height)
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.height = next.height;
        self.bg_color = next.bg_color;
        self.visual = next.visual;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Footer {
            height: self.height,
            bg_color: self.bg_color,
        }
    }
}

impl Default for Footer {
    fn default() -> Self {
        Self::new(FOOTER_VISUAL_REF.default_height)
    }
}

fn build_footer_view(mut kernel: Footer, visual: &'static FooterVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Footer {
    fn build(self) -> ViewNode {
        build_footer_uix_root(self)
    }
}

fn build_footer_uix_root(kernel: Footer) -> ViewNode {
    crate::uix!("src/ui/widgets/containers/layout/footer/footer.uix")
}
