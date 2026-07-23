//! Affix 固定定位容器。
//!
//! 组件保留子树的自然布局占位，并在滚动超过自然位置后把子树固定到
//! 最近视口的 `offset_top`。调用方用同一滚动 `State` 声明当前 offset，
//! 无需逐帧手工修改子节点 frame。

use std::cell::Cell;

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::api::PaintContext;
use crate::ui::children::WidgetChildren;
use crate::ui::core::widget::WidgetCore;
use crate::ui::layout::engine::{child_from_tree_with_constraints, LayoutChild};
use crate::ui::{ComponentId, SnapshotFields, WidgetComponent, WidgetTree};

component! {
    /// 在最近滚动视口内吸顶的子树容器。
    pub struct Affix {
        children: WidgetChildren,
        offset_top: f32,
        scroll_y: f32,
        controlled_scroll_y: Option<f32>,
        natural_offset_y: Cell<f32>,
        cached_child_size: Cell<Size>,
        affixed: Cell<bool>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.cached_child_size.get())
    }

    child_overflow_expands_parent => (&self) -> bool { false }

    build => (&self) -> Vec<Box<dyn WidgetComponent>> {
        self.children.take()
    }

    render => (&self, _frame: Rect, _ctx: &mut PaintContext, _tree: &WidgetTree) {}

    measure_children => (&self, frame: Rect, children: &[ComponentId], tree: &WidgetTree)
        -> Vec<LayoutChild>
    {
        let max_width = if frame.w > 0.0 { frame.w } else { f32::MAX };
        let constraints = Constraints::loose(Size::new(max_width, f32::MAX));
        children
            .iter()
            .copied()
            .map(|id| child_from_tree_with_constraints(id, tree, constraints))
            .collect()
    }

    layout_children => (&self, frame: Rect, children: &[LayoutChild], tree: &WidgetTree)
        -> Vec<(ComponentId, Rect)>
    {
        if children.is_empty() {
            self.cached_child_size.set(Size::zero());
            self.affixed.set(false);
            return Vec::new();
        }

        let viewport_y = self.nearest_viewport_y(children, tree).unwrap_or(0.0);
        self.natural_offset_y.set((frame.y - viewport_y).max(0.0));
        self.refresh_affixed();

        let width = children
            .iter()
            .map(|child| child.measured_size.w.max(0.0))
            .fold(0.0, f32::max);
        let height = children
            .iter()
            .map(|child| child.measured_size.h.max(0.0))
            .sum::<f32>();
        self.cached_child_size.set(Size::new(width, height));

        let child_width = if frame.w > 0.0 { frame.w } else { width };
        let mut y = frame.y + self.sticky_compensation();
        children
            .iter()
            .map(|child| {
                let height = child.measured_size.h.max(0.0);
                let rect = Rect::new(frame.x, y, child_width, height);
                y += height;
                (child.id, rect)
            })
            .collect()
    }
}

impl Affix {
    pub fn new(offset_top: f32) -> Self {
        Self {
            children: WidgetChildren::new(),
            offset_top: Self::normalize_axis(offset_top),
            scroll_y: 0.0,
            controlled_scroll_y: None,
            natural_offset_y: Cell::new(0.0),
            cached_child_size: Cell::new(Size::zero()),
            affixed: Cell::new(false),
        }
    }

    /// 声明最近视口的当前纵向滚动位置。
    pub fn scroll_y(mut self, scroll_y: f32) -> Self {
        self.scroll_y = Self::normalize_axis(scroll_y);
        self.controlled_scroll_y = Some(self.scroll_y);
        self.refresh_affixed();
        self
    }

    /// 直接持有组件实例时更新滚动位置，返回吸顶状态是否变化。
    pub fn update_scroll(&mut self, scroll_y: f32) -> bool {
        self.scroll_y = Self::normalize_axis(scroll_y);
        self.refresh_affixed()
    }

    /// 直接持有组件实例时注入其自然位置与占位高度。
    pub fn set_child_bounds(&mut self, y: f32, height: f32) {
        self.natural_offset_y.set(Self::normalize_axis(y));
        self.cached_child_size
            .set(Size::new(0.0, Self::normalize_axis(height)));
        self.refresh_affixed();
    }

    pub fn is_affixed(&self) -> bool {
        self.affixed.get()
    }

    pub fn offset_top(mut self, offset_top: f32) -> Self {
        self.offset_top = Self::normalize_axis(offset_top);
        self.refresh_affixed();
        self
    }

    pub fn current_scroll_y(&self) -> f32 {
        self.scroll_y
    }

    /// 返回子树相对最近视口的当前可见 Y。
    pub fn child_y(&self) -> f32 {
        if self.is_affixed() {
            self.offset_top
        } else {
            (self.natural_offset_y.get() - self.scroll_y).max(0.0)
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.offset_top = next.offset_top;
        self.controlled_scroll_y = next.controlled_scroll_y;
        if let Some(scroll_y) = self.controlled_scroll_y {
            self.scroll_y = scroll_y;
        }
        self.refresh_affixed();
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Affix {
            offset_top: self.offset_top,
            scroll_y: self.scroll_y,
            affixed: self.affixed.get(),
        }
    }

    fn nearest_viewport_y(&self, children: &[LayoutChild], tree: &WidgetTree) -> Option<f32> {
        let self_id = tree.get(children.first()?.id)?.parent()?;
        let mut current = tree.get(self_id)?.parent();
        while let Some(id) = current {
            let node = tree.get(id)?;
            if node.viewport_scroll_offset().is_some() {
                return Some(node.frame().y);
            }
            current = node.parent();
        }
        None
    }

    fn sticky_compensation(&self) -> f32 {
        (self.scroll_y + self.offset_top - self.natural_offset_y.get()).max(0.0)
    }

    fn refresh_affixed(&self) -> bool {
        let affixed = self.sticky_compensation() > 0.01;
        let changed = self.affixed.get() != affixed;
        self.affixed.set(affixed);
        changed
    }

    fn normalize_axis(value: f32) -> f32 {
        if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        }
    }
}

impl Default for Affix {
    fn default() -> Self {
        Self::new(0.0)
    }
}
