//! Space widget — Ant Design style flex container with uniform gap between children.
//!
//! Provides consistent spacing for a row or column of child widgets.

use std::cell::Cell;

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::ui::children::WidgetChildren;
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::tree_measure::child_from_tree_with_constraints;
// 导入共享的物理内容外尺寸计算。
use crate::ui::SnapshotFields;
use crate::ui::layout::LayoutChild;
use crate::ui::layout::engine::content_size_from_children;
use crate::ui::layout::{
    AlignItems, FlexChild, FlexDirection, FlexInput, JustifyContent, flex::compute_flex_layout,
};
use crate::ui::{ComponentId, WidgetComponent, WidgetTree};

/// Predefined space sizes matching Ant Design.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpaceSize {
    /// 八逻辑像素的紧凑间距。
    Small, // 8px
    /// 十六逻辑像素的标准间距。
    Middle, // 16px
    /// 二十四逻辑像素的宽松间距。
    Large, // 24px
    /// 调用方提供的自定义逻辑像素间距。
    Custom(f32),
}

impl SpaceSize {
    /// 返回此间距档位对应的逻辑像素值。
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
        let intrinsic = self.intrinsic_size();
        let clamped = constraints.clamp(intrinsic);
        let cached = self.cached_content_size.get();
        // 缓存内容是测量结果的下限，即使存在显式尺寸也不能把子树溢出压回更小的
        // measure；否则下一轮 Phase 1 会写回较小 frame，与 Phase 2 扩展振荡。
        let w = if cached.w > 0.0 {
            clamped.w.max(cached.w)
        } else {
            clamped.w
        };
        let h = if cached.h > 0.0 {
            clamped.h.max(cached.h)
        } else {
            clamped.h
        };
        Size::new(w, h)
    }

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    flex_grow => (&self) -> f32 { self.flex_grow_val }

    flex_shrink => (&self) -> f32 { 0.0 }

    on_children_changed => (&mut self, child_count: usize) {
        // 最后一个子节点移除后，旧内容尺寸不再是有效的测量下限。
        if child_count == 0 {
            // 立即归零，避免树布局跳过空节点时继续暴露陈旧尺寸。
            self.cached_content_size.set(Size::zero());
        }
    }

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

        let flex_children: Vec<FlexChild> = children
            .iter()
            .map(|child| FlexChild {
                flex_grow: child.flex_grow,
                // 禁止子节点收缩——Phase 2 负责扩展容器适应内容
                flex_shrink: 0.0,
                align_self: child.align_self,
                measured_size: child.measured_size,
                // Space 与其他 Flex 容器一致地让 margin 推开兄弟并参与固有尺寸。
                margin: child.margin,
                ..FlexChild::default()
            })
            .collect();

        let intrinsic_main = match self.direction {
            FlexDirection::Row | FlexDirection::RowReverse => self.fixed_width.is_none(),
            FlexDirection::Column | FlexDirection::ColumnReverse => self.fixed_height.is_none(),
        };
        // 交叉轴未显式指定时由最宽或最高子项撑开。
        let intrinsic_cross = match self.direction {
            // 水平排列的交叉轴是高度。
            FlexDirection::Row | FlexDirection::RowReverse => self.fixed_height.is_none(),
            // 垂直排列的交叉轴是宽度。
            FlexDirection::Column | FlexDirection::ColumnReverse => self.fixed_width.is_none(),
        };

        let input = FlexInput {
            direction: self.direction,
            wrap: self.wrap,
            gap: self.space_size.value(),
            padding: crate::core::EdgeInsets::zero(),
            container: frame,
            children: &flex_children,
            justify_content: self.justify,
            align_items: self.align,
            intrinsic_main,
            // 把独立的交叉轴固有性传给共享 Flex 求解器。
            intrinsic_cross,
        };

        let output = compute_flex_layout(&input);
        // 缓存子布局实际可见末端与正尾侧 margin，供下一轮固有测量撑开。
        let content_size = content_size_from_children(frame, &output.child_rects, children);
        // 写入不依赖求解器父级总尺寸的真实子内容范围。
        self.cached_content_size.set(content_size);
        children
            .iter()
            .zip(output.child_rects)
            .map(|(child, rect)| (child.id, rect))
            .collect()
    }
}

impl Space {
    pub(crate) fn explicit_size_locks(&self) -> (bool, bool) {
        (self.fixed_width.is_some(), self.fixed_height.is_some())
    }

    /// 创建水平、居中对齐、不换行且使用紧凑间距的空容器。
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

    /// 追加一个由此容器拥有的子组件。
    pub fn child(self, w: impl WidgetComponent + 'static) -> Self {
        self.children.add(w);
        self
    }

    /// 替换此容器拥有的全部子组件。
    pub fn children(self, widgets: Vec<Box<dyn WidgetComponent>>) -> Self {
        self.children.set_all(widgets);
        self
    }

    /// 设置子组件沿主轴排列的方向。
    pub fn direction(mut self, d: FlexDirection) -> Self {
        self.direction = d;
        self
    }
    /// 设置相邻子组件之间的统一间距。
    pub fn size(mut self, s: SpaceSize) -> Self {
        self.space_size = s;
        self
    }
    /// 设置子组件在主轴上的空间分配方式。
    pub fn justify(mut self, j: JustifyContent) -> Self {
        self.justify = j;
        self
    }
    /// 设置子组件在交叉轴上的默认对齐方式。
    pub fn align(mut self, a: AlignItems) -> Self {
        self.align = a;
        self
    }
    /// 设置首选宽度；正值锁定宽度，零按未指定尺寸处理。
    pub fn width(mut self, w: f32) -> Self {
        self.fixed_width = Some(w);
        self
    }
    /// 设置首选高度；正值锁定高度，零按未指定尺寸处理。
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
    /// 将排列方向切换为从上到下的垂直方向。
    pub fn vertical(mut self) -> Self {
        self.direction = FlexDirection::Column;
        self
    }
    /// 设置主轴空间不足时是否把子组件换到下一行或列。
    pub fn wrap(mut self, v: bool) -> Self {
        self.wrap = v;
        self
    }

    fn intrinsic_size(&self) -> Size {
        let cached = self.cached_content_size.get();
        // flex_grow 子项以 0 为 basis，避免窗口缩小时仍用旧缓存撑破父级。
        let grow = self.flex_grow_val > 0.0;
        // 显式 >0 是定高/定宽，尊重之，不被内容溢出撑大；
        // None 或 0 视为 indefinite，用 cached（子项溢出）撑开。
        let w = match self.fixed_width {
            Some(fw) if fw > 0.0 => fw,
            _ => {
                if grow {
                    0.0
                } else if cached.w > 0.0 {
                    cached.w
                } else {
                    0.0
                }
            }
        };
        let h = match self.fixed_height {
            Some(fh) if fh > 0.0 => fh,
            _ => {
                if grow {
                    0.0
                } else if cached.h > 0.0 {
                    cached.h
                } else {
                    0.0
                }
            }
        };
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

// Space 布局参数与缓存的内部回归测试。
#[cfg(test)]
mod tests {
    // 导入当前模块的 Space 与布局类型。
    use super::*;
    // 导入调用布局 trait 所需的公开接口。
    use crate::ui::WidgetLayout;

    // 验证 Space 把子项 margin 传入共享 Flex 并计入缓存。
    #[test]
    fn child_margins_affect_positions_and_cached_content_size() {
        // 构造默认水平且无固定尺寸的 Space。
        let space = Space::new();
        // 构造二十乘十的自然尺寸子项。
        let mut child = LayoutChild::new(ComponentId::new(1), Size::new(20.0, 10.0));
        // 四侧 margin 使自然外尺寸达到三十乘二十。
        child.margin = crate::core::EdgeInsets::new(2.0, 3.0, 8.0, 7.0);
        // 空树足以满足无树读取的布局入口。
        let tree = WidgetTree::new();
        // 在零尺寸 bootstrap frame 中执行组件布局。
        let positions = space.layout_children(Rect::zero(), &[child], &tree);
        // 子项应从左上 margin 后开始并保留自然尺寸。
        assert_eq!(positions[0].1, Rect::new(2.0, 3.0, 20.0, 10.0));
        // 缓存必须记录包含右下 margin 的完整外尺寸。
        assert_eq!(space.cached_content_size.get(), Size::new(30.0, 20.0));
    }
}
