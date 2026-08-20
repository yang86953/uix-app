use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{ComponentId, EventResult, SystemEvent, WidgetTree};
use std::collections::HashSet;

/// 按给定顺序返回相邻焦点并在两端循环；无当前项时从导航方向起点开始。
pub fn next_focus_in_order(
    focusable: &[ComponentId],
    current: Option<ComponentId>,
    forward: bool,
) -> Option<ComponentId> {
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

component! {
    /// Dispatch-managed focus trap metadata and cycling helper.
    pub struct FocusTrap {
        /// Whether focus trapping is active.
        active: bool,
        /// Focusable widgets captured after layout.
        focusable_ids: HashSet<ComponentId>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(Size::new(0.0, 0.0))
    }

    layout_children => (&self, frame: Rect, children: &[crate::ui::LayoutChild], _tree: &WidgetTree)
        -> Vec<(ComponentId, Rect)>
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
    pub fn update_focusable(&mut self, ids: HashSet<ComponentId>) {
        self.focusable_ids = ids;
    }

    /// 按组件身份排序后返回作用域内的相邻焦点；禁用或空作用域返回 `None`。
    pub fn next_focus(&self, current: Option<ComponentId>, forward: bool) -> Option<ComponentId> {
        if !self.active {
            return None;
        }
        let mut sorted: Vec<ComponentId> = self.focusable_ids.iter().copied().collect();
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
