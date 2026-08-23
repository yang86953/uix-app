use crate::core::{Constraints, Rect, Size};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widget_runtime::tree_measure::child_from_tree_with_natural_constraints;
use crate::ui::{EventResult, SystemEvent, WidgetId, WidgetTree};
use crate::widget;
use std::collections::HashSet;

/// 按给定顺序返回相邻焦点并在两端循环；无当前项时从导航方向起点开始。
pub fn next_focus_in_order(
    focusable: &[WidgetId],
    current: Option<WidgetId>,
    forward: bool,
) -> Option<WidgetId> {
    if focusable.is_empty() {
        return None;
    }
    let edge = if forward { 0 } else { focusable.len() - 1 };
    if let Some(cur_id) = current {
        let pos = focusable.iter().position(|&id| id == cur_id);
        match pos {
            Some(p) => {
                if forward {
                    Some(focusable[(p + 1) % focusable.len()])
                } else {
                    Some(focusable[(p + focusable.len() - 1) % focusable.len()])
                }
            }
            None => Some(focusable[edge]),
        }
    } else {
        Some(focusable[edge])
    }
}

widget! {
    /// Dispatch-managed focus trap metadata and cycling helper.
    pub struct FocusTrap {
        /// Whether focus trapping is active.
        active: bool,
        /// Focusable widgets captured after layout.
        focusable_ids: HashSet<WidgetId>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(0.0, 0.0))
    }

    // 焦点作用域是透明包装节点，必须由直接子树的自然尺寸撑开正常流占位。
    measure_from_children => (&self, constraints: Constraints, children: &[WidgetId], tree: &WidgetTree)
        -> Option<Size>
    {
        // 多个直接子节点按现有重叠布局语义取最大外框，而不是错误累加高度。
        let measured = children
            .iter()
            .copied()
            .map(|child_id| {
                child_from_tree_with_natural_constraints(
                    child_id,
                    tree,
                    Constraints::unconstrained(),
                )
                .measured_size
            })
            .fold(Size::zero(), |size, child| {
                Size::new(size.w.max(child.w), size.h.max(child.h))
            });
        Some(constraints.clamp(measured))
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(WidgetId, Rect)>
    {
        children.iter().map(|child| (child.id, frame)).collect()
    }

    render => (&self, _frame: Rect, _ctx: &mut PaintContext, _tree: &WidgetTree) {}

    on_event => (&mut self, _event: &SystemEvent) -> EventResult {
        EventResult::NotHandled
    }

}

impl Default for FocusTrap {
    fn default() -> Self {
        Self::new()
    }
}

impl FocusTrap {
    /// 创建默认启用且尚未收集可聚焦后代的焦点陷阱。
    pub fn new() -> Self {
        Self {
            active: true,
            focusable_ids: HashSet::new(),
        }
    }

    /// 设置该作用域是否接管焦点循环。
    pub fn active(mut self, v: bool) -> Self {
        self.active = v;
        self
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active
    }

    /// Updates the focusable widget list after layout.
    pub fn update_focusable(&mut self, ids: HashSet<WidgetId>) {
        self.focusable_ids = ids;
    }

    /// 按组件身份排序后返回作用域内的相邻焦点；禁用或空作用域返回 `None`。
    pub fn next_focus(&self, current: Option<WidgetId>, forward: bool) -> Option<WidgetId> {
        if !self.active {
            return None;
        }
        let mut sorted: Vec<WidgetId> = self.focusable_ids.iter().copied().collect();
        sorted.sort();
        next_focus_in_order(&sorted, current, forward)
    }

    /// 将一个节点包装为焦点陷阱的唯一直接子节点。
    pub fn wrap(
        self,
        node: crate::ui::widget_runtime::widget::WidgetNode,
    ) -> crate::ui::widget_runtime::widget::WidgetNode {
        crate::ui::widget_runtime::widget::WidgetNode::new(Box::new(self), vec![node])
    }
}

// 验证焦点陷阱的正反向循环与禁用边界。
#[cfg(test)]
// 将焦点顺序契约限制在组件模块内部测试。
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../tests/unit/ui/widget_runtime/focus_trap__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
