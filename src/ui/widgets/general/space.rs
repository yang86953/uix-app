//! Space widget — Ant Design style flex container with uniform gap between children.
//!
//! Provides consistent spacing for a row or column of child widgets.

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::ui::children::WidgetChildren;
use crate::ui::core::widget::WidgetCore;
use crate::ui::layout::{
    flex::compute_flex_layout, AlignItems, FlexChild, FlexDirection, FlexInput, JustifyContent,
};
use crate::ui::SnapshotFields;
use crate::ui::{ComponentId, WidgetComponent, WidgetTree};

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

component! {
    /// Space — a flex container that adds uniform gap between its children.
    pub struct Space {
        children: WidgetChildren,
        direction: FlexDirection,
        space_size: SpaceSize,
        wrap: bool,
        justify: JustifyContent,
        align: AlignItems,
        fixed_width: Option<f32>,
        fixed_height: Option<f32>,
        flex_grow_val: f32,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    flex_grow => (&self) -> f32 { self.flex_grow_val }

    flex_shrink => (&self) -> f32 { 0.0 }

    render => (&self, _frame: Rect, _ctx: &mut PaintContext, _tree: &WidgetTree) {
        // Space itself is invisible; children are rendered by the tree.
    }

    build => (&self) -> Vec<Box<dyn WidgetComponent>> {
        self.children.take()
    }

    layout_children => (&self, frame: Rect, children: &[ComponentId], tree: &WidgetTree)
        -> Vec<(ComponentId, Rect)>
    {
        if children.is_empty() { return Vec::new(); }

        let child_sizes: Vec<Size> = children
            .iter()
            .map(|&cid| {
                let pref = tree.get(cid)
                    .map(|c| c.measure(Constraints::unconstrained()))
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
                    flex_grow: w.and_then(|c| c.as_layout()).map(|l| l.flex_grow()).unwrap_or(0.0),
                    // 禁止子节点收缩——Phase 2 负责扩展容器适应内容
                    flex_shrink: 0.0,
                    ..FlexChild::default()
                }
            })
            .collect();

            let input = FlexInput {
                direction: self.direction,
                wrap: self.wrap,
                gap: self.space_size.value(),
            padding: crate::core::EdgeInsets::zero(),
            container: frame,
            children: flex_children,
            child_sizes,
            justify_content: self.justify,
            align_items: self.align,
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
            flex_grow_val: 0.0,
        }
    }

    pub fn child(self, w: impl WidgetComponent + 'static) -> Self {
        self.children.add(w);
        self
    }

    pub fn children(self, widgets: Vec<Box<dyn WidgetComponent>>) -> Self {
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
    /// 设置 flex-grow 值，使 Space 在 flex 布局中填充剩余空间。
    /// 用于响应式布局替代固定宽度。
    pub fn flex_grow(mut self, v: f32) -> Self {
        self.flex_grow_val = v;
        self
    }
    pub fn vertical(mut self) -> Self {
        self.direction = FlexDirection::Column;
        self
    }
    pub fn wrap(mut self, v: bool) -> Self {
        self.wrap = v;
        self
    }

    fn intrinsic_size(&self) -> Size {
        Size::new(
            self.fixed_width.unwrap_or(0.0),
            self.fixed_height.unwrap_or(0.0),
        )
    }
}

impl Space {
    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Space {
            direction: self.direction,
            space_size: self.space_size,
            wrap: self.wrap,
            justify: self.justify,
            align: self.align,
            fixed_width: self.fixed_width,
            fixed_height: self.fixed_height,
            flex_grow: self.flex_grow_val,
        }
    }
}

impl Default for Space {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::traits::WidgetLayout;

    #[test]
    fn measure_clamps_fixed_space_size() {
        let measured = Space::new()
            .width(80.0)
            .height(24.0)
            .measure(Constraints::loose(Size::new(40.0, 32.0)));

        assert_eq!(measured, Size::new(40.0, 24.0));
    }
}
