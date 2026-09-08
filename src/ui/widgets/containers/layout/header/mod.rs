//! Header 顶部栏的布局、标题绘制与声明式视觉。

use super::{
    layout_shell_children, layout_shell_children_into, measure_shell_children,
    measure_shell_children_into,
};
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Color;
use crate::ui::layout::{FlexDirection, LayoutChild};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::widget::WidgetTree;
use crate::ui::{SnapshotFields, View, ViewNode, WidgetId};
use crate::widget;

// 保存 Header 的默认高度、标题排版与子树方向。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct HeaderVisual {
    default_height: f32,
    title_font_size: f32,
    title_left_padding: f32,
    title_vertical_factor: f32,
    child_direction: FlexDirection,
}

crate::uix_items!("src/ui/widgets/containers/layout/header/header.uix");

pub(crate) const fn header_child_direction() -> FlexDirection {
    FlexDirection::Column
}

widget! {
    /// 页面顶部栏。
    pub struct Header {
        height: f32,
        bg_color: Option<Color>,
        title: Option<String>,
        /// 同目录 UIX 生成的唯一静态视觉表。
        pub(crate) visual: &'static HeaderVisual,
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
        if let Some(title) = &self.title {
            let color = ctx.tokens().color_text();
            let font_size = self.visual.title_font_size;
            let y = frame.y + (frame.h - font_size) * self.visual.title_vertical_factor;
            ctx.draw_text(
                title,
                Point::new(frame.x + self.visual.title_left_padding, y),
                color,
                font_size,
            );
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

impl Default for Header {
    fn default() -> Self {
        Self::new(HEADER_VISUAL_REF.default_height)
    }
}

impl Header {
    /// 创建指定逻辑高度且不覆盖主题背景的顶部栏。
    pub fn new(height: f32) -> Self {
        Self {
            height,
            bg_color: None,
            title: None,
            visual: HEADER_VISUAL_REF,
        }
    }

    /// 设置顶部栏背景颜色。
    pub fn bg(mut self, color: Color) -> Self {
        self.bg_color = Some(color);
        self
    }

    /// 声明式标题文字（左侧渲染）。
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(0.0, self.height)
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.height = next.height;
        self.bg_color = next.bg_color;
        self.title = next.title;
        self.visual = next.visual;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Header {
            height: self.height,
            bg_color: self.bg_color,
        }
    }
}

// 把 Header Rust 内核与 UIX 静态视觉组合为单一叶节点。
fn build_header_view(mut kernel: Header, visual: &'static HeaderVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Header {
    fn build(self) -> ViewNode {
        build_header_uix_root(self)
    }
}

fn build_header_uix_root(kernel: Header) -> ViewNode {
    crate::uix!("src/ui/widgets/containers/layout/header/header.uix")
}
