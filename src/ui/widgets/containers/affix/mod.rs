//! Affix 固定定位容器。
//!
//! 组件保留子树的自然布局占位，并在滚动超过自然位置后把子树固定到
//! 最近视口的 `offset_top`。调用方用同一滚动 `State` 声明当前 offset，
//! 无需逐帧手工修改子节点 frame。

use std::cell::Cell;

use crate::core::{Constraints, Rect, Size};
use crate::ui::children::WidgetChildren;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::tree_measure::child_from_tree_with_constraints;
use crate::ui::widget_runtime::widget::WidgetCore;
use crate::widget;
// Affix 的定制布局复用共享布局数值与 margin 归一化规则。
use crate::ui::layout::LayoutChild;
use crate::ui::layout::engine::{finite_non_negative, finite_or_zero, normalize_margin};
use crate::ui::{SnapshotFields, View, ViewNode, Widget, WidgetId, WidgetTree};

// 保存 Affix 的默认偏移与吸顶状态切换阈值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AffixVisual {
    default_offset_top: f32,
    sticky_activation_threshold: f32,
}

crate::uix_items!("src/ui/widgets/containers/affix/affix.uix");

widget! {
    /// 在最近滚动视口内吸顶的子树容器。
    pub struct Affix {
        children: WidgetChildren,
        offset_top: f32,
        scroll_y: f32,
        controlled_scroll_y: Option<f32>,
        natural_offset_y: Cell<f32>,
        cached_child_size: Cell<Size>,
        affixed: Cell<bool>,
        /// 同目录 UIX 生成的唯一静态视觉表。
        pub(crate) visual: &'static AffixVisual,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.cached_child_size.get())
    }

    child_overflow_expands_parent => (&self) -> bool { false }

    on_children_changed => (&mut self, child_count: usize) {
        // 只有空集合需要丢弃由旧子树派生的吸顶状态。
        if child_count == 0 {
            // 空子树不再占用任何固有尺寸。
            self.cached_child_size.set(Size::zero());
            // 空子树没有可用于吸顶计算的自然位置。
            self.natural_offset_y.set(0.0);
            // 没有子树时组件必须退出吸顶状态。
            self.affixed.set(false);
        }
    }

    on_child_visibility_changed => (&mut self) {
        // 吸顶占位必须跟随当前可见子树重新测量。
        self.cached_child_size.set(Size::zero());
    }

    build => (&self) -> Vec<Box<dyn Widget>> {
        self.children.take()
    }

    render => (&self, _frame: Rect, _ctx: &mut PaintContext, _tree: &WidgetTree) {}

    measure_children => (&self, frame: Rect, children: &[WidgetId], tree: &WidgetTree)
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
        -> Vec<(WidgetId, Rect)>
    {
        if children.is_empty() {
            // 空布局不再保留旧子树占位。
            self.cached_child_size.set(Size::zero());
            // 空布局没有有效的自然位置。
            self.natural_offset_y.set(0.0);
            // 空布局必须退出吸顶状态。
            self.affixed.set(false);
            // 空布局不产生任何子节点位置。
            return Vec::new();
        }

        let viewport_y = self.nearest_viewport_y(children, tree).unwrap_or(0.0);
        self.natural_offset_y.set((frame.y - viewport_y).max(0.0));
        self.refresh_affixed();

        let width = children
            .iter()
            .map(|child| {
                // 先清除非有限 margin，同时保留有限负外边距语义。
                let margin = normalize_margin(child.margin);
                // 父级固有宽度使用子项自然 border-box 加横向 margin。
                finite_non_negative(child.measured_size.w + margin.horizontal())
            })
            .fold(0.0, f32::max);
        let height = children
            .iter()
            .fold(0.0, |total, child| {
                // 每个子项使用与宽度一致的有限 margin。
                let margin = normalize_margin(child.margin);
                // 单项纵向外尺寸包含顶部和底部 margin。
                let outer_height =
                    finite_non_negative(child.measured_size.h + margin.vertical());
                // 每步有限化，避免大量子项累加溢出。
                finite_non_negative(total + outer_height)
            });
        self.cached_child_size.set(Size::new(width, height));

        // 从吸顶补偿后的有限纵坐标开始依次消费子项外边距。
        let mut y = finite_or_zero(frame.y + self.sticky_compensation());
        children
            .iter()
            .map(|child| {
                // 使用共享规则清除非法 margin 分量。
                let margin = normalize_margin(child.margin);
                // 子项实际高度保持有限非负且不包含 margin。
                let height = finite_non_negative(child.measured_size.h);
                // 有父级宽度时在 margin 内拉伸 border-box，否则保留自然宽度。
                let child_width = if frame.w > 0.0 {
                    // 左右 margin 从父级可用宽度中扣除一次。
                    finite_non_negative(frame.w - margin.horizontal())
                } else {
                    // 未分配宽度时使用子项自然 border-box 宽度。
                    finite_non_negative(child.measured_size.w)
                };
                // 左 margin 决定子项 border-box 横坐标。
                let child_x = finite_or_zero(frame.x + margin.left);
                // 顶部 margin 在写入子项 frame 前推进纵坐标。
                let child_y = finite_or_zero(y + margin.top);
                // frame 只包含 border-box，不把 margin 包进尺寸。
                let rect = Rect::new(child_x, child_y, child_width, height);
                // 底部 margin 在当前 border-box 后推开下一个兄弟。
                y = finite_or_zero(child_y + height + margin.bottom);
                (child.id, rect)
            })
            .collect()
    }
}

impl Affix {
    /// 创建使用指定视口顶部偏移量的吸顶容器。
    pub fn new(offset_top: f32) -> Self {
        Self {
            children: WidgetChildren::new(),
            offset_top: Self::normalize_axis(offset_top),
            scroll_y: 0.0,
            controlled_scroll_y: None,
            natural_offset_y: Cell::new(0.0),
            cached_child_size: Cell::new(Size::zero()),
            affixed: Cell::new(false),
            visual: AFFIX_VISUAL_REF,
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

    /// 返回子树当前是否处于吸顶状态。
    pub fn is_affixed(&self) -> bool {
        self.affixed.get()
    }

    /// 设置吸顶状态下相对视口顶部的偏移量。
    pub fn offset_top(mut self, offset_top: f32) -> Self {
        self.offset_top = Self::normalize_axis(offset_top);
        self.refresh_affixed();
        self
    }

    /// 返回容器当前采用的纵向滚动位置。
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
        self.visual = next.visual;
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
        let affixed = self.sticky_compensation() > self.visual.sticky_activation_threshold;
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
        Self::new(AFFIX_VISUAL_REF.default_offset_top)
    }
}

// 把 Affix Rust 内核与 UIX 静态视觉组合为单一叶节点。
fn build_affix_view(mut kernel: Affix, visual: &'static AffixVisual) -> ViewNode {
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

impl View for Affix {
    fn build(self) -> ViewNode {
        build_affix_uix_root(self)
    }
}

// 为 UIX 根提供稳定的 Rust 内核绑定名称。
fn build_affix_uix_root(kernel: Affix) -> ViewNode {
    crate::uix!("src/ui/widgets/containers/affix/affix.uix")
}

// 只在单元测试目标验证 Affix UIX 视觉注入契约。
#[cfg(test)]
#[path = "../../../../../tests/unit/ui/widgets/containers/affix__tests.rs"]
mod tests;
