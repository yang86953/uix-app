use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::ui::component::paint_context::PaintContext;
use crate::ui::{ComponentId, EventResult, SystemEvent, WidgetTree};
use std::collections::HashSet;

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
    pub fn new() -> Self {
        Self {
            active: true,
            focusable_ids: HashSet::new(),
        }
    }

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

    pub fn next_focus(&self, current: Option<ComponentId>, forward: bool) -> Option<ComponentId> {
        if !self.active {
            return None;
        }
        let mut sorted: Vec<ComponentId> = self.focusable_ids.iter().copied().collect();
        sorted.sort();
        next_focus_in_order(&sorted, current, forward)
    }

    pub fn wrap(
        self,
        node: crate::ui::component::widget::WidgetNode,
    ) -> crate::ui::component::widget::WidgetNode {
        crate::ui::component::widget::WidgetNode::new(Box::new(self), vec![node])
    }
}

// 验证焦点陷阱的正反向循环与禁用边界。
#[cfg(test)]
// 将焦点顺序契约限制在组件模块内部测试。
mod tests {
    // 复用被测焦点陷阱与纯顺序 helper。
    use super::{FocusTrap, next_focus_in_order};
    // 引入稳定组件身份集合。
    use crate::ui::ComponentId;
    // 引入焦点陷阱公开更新接口所需集合。
    use std::collections::HashSet;

    // 验证正反向导航都在作用域边界循环。
    #[test]
    // 测试名称说明焦点顺序的循环语义。
    fn focus_order_wraps_in_both_directions() {
        // 创建首个稳定焦点身份。
        let first = ComponentId::new(2);
        // 创建中间稳定焦点身份。
        let middle = ComponentId::new(4);
        // 创建末尾稳定焦点身份。
        let last = ComponentId::new(8);
        // 按升序保存焦点身份。
        let ids = [first, middle, last];
        // 从末项向前导航必须回到首项。
        assert_eq!(next_focus_in_order(&ids, Some(ids[2]), true), Some(ids[0]));
        // 从首项反向导航必须回到末项。
        assert_eq!(next_focus_in_order(&ids, Some(ids[0]), false), Some(ids[2]));
    }

    // 验证禁用作用域不接管焦点选择。
    #[test]
    // 测试名称说明 active 配置的运行时职责。
    fn inactive_trap_does_not_select_focus() {
        // 创建包含一个可聚焦身份的集合。
        let mut focusable = HashSet::new();
        // 登记稳定焦点身份。
        focusable.insert(ComponentId::new(3));
        // 创建显式禁用的焦点陷阱。
        let mut trap = FocusTrap::new().active(false);
        // 通过运行时入口更新可聚焦后代集合。
        trap.update_focusable(focusable);
        // 禁用作用域不得返回任何下一焦点。
        assert_eq!(trap.next_focus(None, true), None);
    }
}
