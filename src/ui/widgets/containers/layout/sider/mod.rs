//! Sider 侧边栏的折叠尺寸、子树布局与声明式视觉。

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

// 保存 Sider 的展开、折叠默认宽度与子树方向。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SiderVisual {
    default_width: f32,
    default_collapsed_width: f32,
    child_direction: FlexDirection,
}

crate::uix_items!("src/ui/widgets/containers/layout/sider/sider.uix");

pub(crate) const fn sider_child_direction() -> FlexDirection {
    FlexDirection::Column
}

widget! {
    /// 页面侧边栏。
    pub struct Sider {
        width: f32,
        bg_color: Option<Color>,
        collapsible: bool,
        collapsed: bool,
        collapsed_width: f32,
        /// 同目录 UIX 生成的唯一静态视觉表。
        pub(crate) visual: &'static SiderVisual,
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

impl Sider {
    /// 创建指定展开宽度且默认不可折叠的侧边栏。
    pub fn new(width: f32) -> Self {
        Self {
            width,
            bg_color: None,
            collapsible: false,
            collapsed: false,
            collapsed_width: SIDER_VISUAL_REF.default_collapsed_width,
            visual: SIDER_VISUAL_REF,
        }
    }

    pub fn bg(mut self, color: Color) -> Self {
        self.bg_color = Some(color);
        self
    }

    pub fn collapsible(mut self, value: bool) -> Self {
        self.collapsible = value;
        self
    }

    pub fn collapsed(mut self, value: bool) -> Self {
        self.collapsed = value;
        self
    }

    pub fn collapsed_width(mut self, width: f32) -> Self {
        self.collapsed_width = width;
        self
    }

    fn intrinsic_size(&self) -> Size {
        if self.collapsed {
            Size::new(self.collapsed_width, 0.0)
        } else {
            Size::new(self.width, 0.0)
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.width = next.width;
        self.bg_color = next.bg_color;
        self.collapsible = next.collapsible;
        self.collapsed = next.collapsed;
        self.collapsed_width = next.collapsed_width;
        self.visual = next.visual;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Sider {
            width: self.width,
            bg_color: self.bg_color,
            collapsible: self.collapsible,
            collapsed: self.collapsed,
            collapsed_width: self.collapsed_width,
        }
    }
}

impl Default for Sider {
    fn default() -> Self {
        Self::new(SIDER_VISUAL_REF.default_width)
    }
}

fn build_sider_view(mut kernel: Sider, visual: &'static SiderVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Sider {
    fn build(self) -> ViewNode {
        build_sider_uix_root(self)
    }
}

fn build_sider_uix_root(kernel: Sider) -> ViewNode {
    crate::uix!("src/ui/widgets/containers/layout/sider/sider.uix")
}
