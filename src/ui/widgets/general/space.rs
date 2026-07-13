//! Space widget — Ant Design style flex container with uniform gap between children.
//!
//! Provides consistent spacing for a row or column of child widgets.

use std::cell::Cell;

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
        /// layout_children 后缓存子树内容尺寸，供无固定宽高时的 measure。
        pub(crate) cached_content_size: Cell<Size>,
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
        if children.is_empty() {
            self.cached_content_size.set(Size::zero());
            return Vec::new();
        }

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

        let intrinsic_main = match self.direction {
            FlexDirection::Row | FlexDirection::RowReverse => self.fixed_width.is_none(),
            FlexDirection::Column | FlexDirection::ColumnReverse => self.fixed_height.is_none(),
        };

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
            intrinsic_main,
        };

        let output = compute_flex_layout(&input);
        self.cached_content_size.set(Size::new(
            output.total_size.w.max(0.0),
            output.total_size.h.max(0.0),
        ));
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
            cached_content_size: Cell::new(Size::zero()),
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
        let cached = self.cached_content_size.get();
        // flex_grow 子项以 0 为 basis，避免窗口缩小时仍用旧缓存撑破父级。
        let grow = self.flex_grow_val > 0.0;
        let w = self.fixed_width.unwrap_or_else(|| {
            if grow {
                0.0
            } else if cached.w > 0.0 {
                cached.w
            } else {
                0.0
            }
        });
        let h = self.fixed_height.unwrap_or_else(|| {
            if grow {
                0.0
            } else if cached.h > 0.0 {
                cached.h
            } else {
                0.0
            }
        });
        Size::new(w, h)
    }

    fn child_constraints(&self, frame: Rect) -> Constraints {
        let is_row = matches!(
            self.direction,
            FlexDirection::Row | FlexDirection::RowReverse
        );
        // 主轴始终 MAX：允许内容溢出，由 Phase 2 layout_expand 撑开。
        // 交叉轴：有固定尺寸时用 frame；否则 MAX（避免无固定宽的 Column 在
        // frame.w=0 时把 Label 等压成 0 宽）。
        let max_w = if is_row || self.fixed_width.is_none() {
            f32::MAX
        } else {
            frame.w
        };
        let max_h = if !is_row || self.fixed_height.is_none() {
            f32::MAX
        } else {
            frame.h
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
        // 保留 cached_content_size：reconcile 不重建布局缓存。
    }
}

impl Default for Space {
    fn default() -> Self {
        Self::new()
    }
}

