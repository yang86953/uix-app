//! Space widget — Ant Design style flex container with uniform gap between children.
//!
//! Provides consistent spacing for a row or column of child widgets.

use crate::base::{Rect, Size};
use crate::define_widget;
use crate::graphics::{
    compute_flex_layout, AlignItems, FlexChild, FlexDirection, FlexInput, JustifyContent,
};
use crate::ui::children::WidgetChildren;
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{Widget, WidgetCore, WidgetId, WidgetTree};

/// Predefined space sizes matching Ant Design.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpaceSize {
    Small,  // 8px
    Middle, // 16px
    Large,  // 24px
    Custom(f32),
}

impl SpaceSize {
    pub fn value(&self) -> f32 {
        match self {
            Self::Small => 8.0,
            Self::Middle => 16.0,
            Self::Large => 24.0,
            Self::Custom(v) => *v,
        }
    }
}

define_widget! {
    /// Space — a flex container that adds uniform gap between its children.
    pub struct Space {
        children: WidgetChildren,
        direction: FlexDirection,
        space_size: SpaceSize,
        #[allow(dead_code)]
        wrap: bool,
        justify: JustifyContent,
        align: AlignItems,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        Size::new(
            self.fixed_width.unwrap_or(0.0),
            self.fixed_height.unwrap_or(0.0),
        )
    }

    render => (&self, _frame: Rect, _ctx: &mut RenderContext, _tree: &WidgetTree) {
        // Space itself is invisible; children are rendered by the tree.
    }

    build => (&self) -> Vec<Box<dyn Widget>> {
        self.children.take()
    }

    layout_children => (&self, frame: Rect, children: &[WidgetId], tree: &WidgetTree)
        -> Vec<(WidgetId, Rect)>
    {
        if children.is_empty() { return Vec::new(); }

        let child_sizes: Vec<Size> = children
            .iter()
            .map(|&cid| {
                let pref = tree.get(cid)
                    .map(|c| c.preferred_size(None))
                    .unwrap_or_default();
                let actual_h = tree.get(cid)
                    .map(|c| c.frame().h)
                    .unwrap_or(0.0);
                // 有明确 preferred_size 的子节点优先用 pref.h，
                // 避免面板折叠后 Phase 2 的扩展高度被误保留。
                // 对于 pref.h=0 的弹性子节点，保留实际 frame 高度。
                let h = if pref.h > 0.0 { pref.h } else { actual_h };
                Size::new(pref.w, h)
            })
            .collect();

        let flex_children: Vec<FlexChild> = children
            .iter()
            .map(|&cid| {
                let w = tree.get(cid);
                FlexChild {
                    flex_grow: w.map(|c| c.inner().flex_grow()).unwrap_or(0.0),
                    // 禁止子节点收缩——Phase 2 负责扩展容器适应内容
                    flex_shrink: 0.0,
                    ..FlexChild::default()
                }
            })
            .collect();

        let input = FlexInput {
            direction: self.direction,
            gap: self.space_size.value(),
            padding: crate::base::EdgeInsets::zero(),
            container: frame,
            children: flex_children,
            child_sizes,
            justify_content: self.justify,
            align_items: self.align,
            ..FlexInput::default()
        };

        let output = compute_flex_layout(&input);
        children
            .iter()
            .zip(output.child_rects)
            .map(|(&cid, rect)| (cid, rect))
            .collect()
    }
}

impl Space {
    pub fn new() -> Self {
        Self {
            children: WidgetChildren::new(),
            direction: FlexDirection::Row,
            space_size: SpaceSize::Small,
            wrap: false,
            justify: JustifyContent::Start,
            align: AlignItems::Center,
            fixed_width: None,
            fixed_height: None,
        }
    }

    pub fn child(self, w: impl Widget + 'static) -> Self {
        self.children.add(w);
        self
    }

    pub fn children(self, widgets: Vec<Box<dyn Widget>>) -> Self {
        self.children.set_all(widgets);
        self
    }

    pub fn direction(mut self, d: FlexDirection) -> Self {
        self.direction = d;
        self
    }
    pub fn size(mut self, s: SpaceSize) -> Self {
        self.space_size = s;
        self
    }
    pub fn justify(mut self, j: JustifyContent) -> Self {
        self.justify = j;
        self
    }
    pub fn align(mut self, a: AlignItems) -> Self {
        self.align = a;
        self
    }
    pub fn width(mut self, w: f32) -> Self {
        self.fixed_width = Some(w);
        self
    }
    pub fn height(mut self, h: f32) -> Self {
        self.fixed_height = Some(h);
        self
    }
    pub fn vertical(mut self) -> Self {
        self.direction = FlexDirection::Column;
        self
    }
}

impl Default for Space {
    fn default() -> Self {
        Self::new()
    }
}
