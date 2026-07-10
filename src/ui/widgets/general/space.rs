//! Space widget — Ant Design style flex container with uniform gap between children.
//!
//! Provides consistent spacing for a row or column of child widgets.

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::ui::children::WidgetChildren;
use crate::ui::layout::engine::{child_from_tree_with_constraints, LayoutChild};
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

    picture_policy => (&self) -> crate::draw::compositor::PicturePolicy {
        crate::draw::compositor::PicturePolicy::Eligible
    }

    flex_grow => (&self) -> f32 { self.flex_grow_val }

    flex_shrink => (&self) -> f32 { 0.0 }

    render => (&self, _frame: Rect, _ctx: &mut PaintContext, _tree: &WidgetTree) {
        // Space itself is invisible; children are rendered by the tree.
    }

    build => (&self) -> Vec<Box<dyn WidgetComponent>> {
        self.children.take()
    }

    measure_children => (&self, frame: Rect, children: &[ComponentId], tree: &WidgetTree)
        -> Vec<LayoutChild>
    {
        let constraints = self.child_constraints(frame);
        children
            .iter()
            .copied()
            .map(|cid| child_from_tree_with_constraints(cid, tree, constraints))
            .collect()
    }

    layout_children => (&self, frame: Rect, children: &[LayoutChild], _tree: &WidgetTree)
        -> Vec<(ComponentId, Rect)>
    {
        if children.is_empty() { return Vec::new(); }

        let child_sizes: Vec<Size> = children.iter().map(|child| child.measured_size).collect();

        let flex_children: Vec<FlexChild> = children
            .iter()
            .map(|child| FlexChild {
                    flex_grow: child.flex_grow,
                    // 禁止子节点收缩——Phase 2 负责扩展容器适应内容
                    flex_shrink: 0.0,
                    align_self: child.align_self,
                    ..FlexChild::default()
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
            child_margins: vec![crate::core::EdgeInsets::zero(); children.len()],
            justify_content: self.justify,
            align_items: self.align,
            intrinsic_main: false,
        };

        let output = compute_flex_layout(&input);
        children
            .iter()
            .zip(output.child_rects)
            .map(|(child, rect)| (child.id, rect))
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

    fn child_constraints(&self, frame: Rect) -> Constraints {
        let max_w = match self.direction {
            FlexDirection::Row | FlexDirection::RowReverse => f32::MAX,
            FlexDirection::Column | FlexDirection::ColumnReverse => frame.w,
        };
        let max_h = match self.direction {
            FlexDirection::Row | FlexDirection::RowReverse => frame.h,
            FlexDirection::Column | FlexDirection::ColumnReverse => f32::MAX,
        };
        Constraints::loose(Size::new(max_w, max_h))
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

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.direction = next.direction;
        self.space_size = next.space_size;
        self.wrap = next.wrap;
        self.justify = next.justify;
        self.align = next.align;
        self.fixed_width = next.fixed_width;
        self.fixed_height = next.fixed_height;
        self.flex_grow_val = next.flex_grow_val;
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
    use crate::ui::core::widget::WidgetCore;
    use crate::ui::traits::{WidgetCapabilities, WidgetLayout};

    struct FixedChild(Size);

    impl WidgetComponent for FixedChild {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }

        fn into_any(self: Box<Self>) -> Box<dyn std::any::Any> {
            self
        }

        fn capabilities(&self) -> WidgetCapabilities {
            WidgetCapabilities::from_bits(WidgetCapabilities::LAYOUT)
        }

        crate::wc_upcast!(FixedChild; WidgetLayout);
    }

    impl WidgetLayout for FixedChild {
        fn measure(&self, constraints: Constraints) -> Size {
            constraints.clamp(self.0)
        }

        fn flex_grow(&self) -> f32 {
            2.0
        }

        fn flex_shrink(&self) -> f32 {
            0.25
        }

        fn align_self(&self) -> Option<AlignItems> {
            Some(AlignItems::End)
        }
    }

    #[test]
    fn measure_clamps_fixed_space_size() {
        let measured = Space::new()
            .width(80.0)
            .height(24.0)
            .measure(Constraints::loose(Size::new(40.0, 32.0)));

        assert_eq!(measured, Size::new(40.0, 24.0));
    }

    #[test]
    fn measure_children_respects_axes_and_preserves_flex_metadata() {
        let mut tree = WidgetTree::new();
        let child = tree.set_root(Box::new(FixedChild(Size::new(120.0, 30.0))));
        let frame = Rect::new(0.0, 0.0, 40.0, 20.0);

        let row = Space::new().measure_children(frame, &[child], &tree);
        assert_eq!(row[0].measured_size, Size::new(120.0, 20.0));
        assert_eq!(row[0].flex_grow, 2.0);
        assert_eq!(row[0].flex_shrink, 0.25);
        assert_eq!(row[0].align_self, Some(AlignItems::End));

        let column = Space::new()
            .vertical()
            .measure_children(frame, &[child], &tree);
        assert_eq!(column[0].measured_size, Size::new(40.0, 30.0));
    }

    #[test]
    fn layout_children_allow_main_axis_overflow_and_clamp_cross_axis() {
        let mut tree = WidgetTree::new();
        let root = tree.set_root(Box::new(
            Space::new()
                .width(40.0)
                .height(20.0)
                .child(FixedChild(Size::new(120.0, 30.0))),
        ));

        tree.layout();

        let child = tree.get(root).unwrap().children()[0];
        assert_eq!(tree.get(child).unwrap().frame().w, 120.0);
        assert_eq!(tree.get(child).unwrap().frame().h, 20.0);
    }

    #[test]
    fn layout_children_consumes_snapshot_instead_of_tree_measure_or_frame() {
        let mut tree = WidgetTree::new();
        let child = tree.set_root(Box::new(FixedChild(Size::new(12.0, 12.0))));
        tree.get_mut(child)
            .unwrap()
            .set_frame(Rect::new(0.0, -20.0, 120.0, 60.0));
        let snapshot = LayoutChild::new(child, Size::new(120.0, 0.0));

        let placements =
            Space::new().layout_children(Rect::new(0.0, 0.0, 40.0, 20.0), &[snapshot], &tree);

        assert_eq!(placements[0].1, Rect::new(0.0, 10.0, 120.0, 0.0));
    }
}
