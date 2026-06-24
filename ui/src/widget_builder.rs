use uix_graphics::Color;
use crate::{compute_flex_layout, AlignItems, FlexChild, FlexDirection, FlexInput, JustifyContent};
use uix_core::{EdgeInsets, Rect, Size};
use crate::widget::{Widget, WidgetId, WidgetTree};

/// Builder for constructing widget trees declaratively.
pub struct WidgetBuilder {
    children: Vec<Box<dyn Widget>>,
    direction: FlexDirection,
    justify: JustifyContent,
    align: AlignItems,
    padding: EdgeInsets,
    gap: f32,
    bg_color: Option<Color>,
    width: Option<f32>,
    height: Option<f32>,
    #[allow(dead_code)]
    min_width: Option<f32>,
    #[allow(dead_code)]
    min_height: Option<f32>,
    flex_grow: f32,
    flex_shrink: f32,
    visible: bool,
    opacity: f32,
}

impl Widget for ContainerWidget {
    fn preferred_size(&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        let w = self.width.unwrap_or(0.0);
        let h = self.height.unwrap_or(0.0);
        Size::new(w, h)
    }

    fn render(
        &self,
        frame: Rect,
        ctx: &mut crate::render_context::RenderContext,
        _tree: &WidgetTree,
    ) {
        if !self.visible {
            return;
        }
        // Fill background
        if let Some(bg) = self.bg_color {
            ctx.fill_rect(frame, bg, None);
        }
    }

    // NOTE(布局): ContainerWidget 现在与 Container 一样从子 Widget 读取
    // flex_grow/flex_shrink，并支持 wrap。保留独立实现而非委托给 Container，
    // 避免破坏 ContainerWidget 的简化语义。
    fn layout_children(
        &self,
        frame: Rect,
        children: &[WidgetId],
        tree: &WidgetTree,
    ) -> Vec<(WidgetId, Rect)> {
        if children.is_empty() { return Vec::new(); }

        // 过滤不可见子节点：不可见的 widget 不参与布局，不占空间
        let visible_children: Vec<WidgetId> = children.iter().copied()
            .filter(|&cid| tree.get(cid).map(|n| n.visible()).unwrap_or(false))
            .collect();
        if visible_children.is_empty() {
            return Vec::new();
        }

        let child_sizes: Vec<Size> = visible_children
            .iter()
            .map(|&cid| {
                tree.get(cid)
                    .map(|c| c.preferred_size(None))
                    .unwrap_or_default()
            })
            .collect();

        let flex_children: Vec<FlexChild> = visible_children
            .iter()
            .map(|&cid| {
                let w = tree.get(cid);
                FlexChild {
                    flex_grow: w.map(|c| c.inner().flex_grow()).unwrap_or(0.0),
                    flex_shrink: w.map(|c| c.inner().flex_shrink()).unwrap_or(0.0),
                    ..FlexChild::default()
                }
            })
            .collect();

        let input = FlexInput {
            direction: self.direction,
            wrap: self.wrap,
            gap: self.gap,
            padding: self.padding,
            container: frame,
            children: flex_children,
            child_sizes,
            justify_content: self.justify,
            align_items: self.align,
            ..FlexInput::default()
        };

        let output = compute_flex_layout(&input);
        visible_children
            .iter()
            .zip(output.child_rects)
            .map(|(&cid, rect)| (cid, rect))
            .collect()
    }
