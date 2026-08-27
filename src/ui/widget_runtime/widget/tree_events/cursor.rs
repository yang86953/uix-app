// 引入树事件模块持有的 WidgetTree 与节点核心契约。
use super::*;
// 引入平台无关的指针光标枚举。
use crate::platform::windowing::CursorType;

// 为 UI System 提供当前指针路由目标的纯查询能力。
impl WidgetTree {
    // 返回当前悬停节点继承后生效的平台光标。
    pub(crate) fn active_pointer_cursor(&self) -> CursorType {
        // 使用事件路由已经确定的悬停身份，确保浮层与普通树命中语义一致。
        self.cursor_for_widget(self.managers().interaction.hovered_widget())
    }

    // 从指定节点沿父链解析最近的显式光标覆盖。
    fn cursor_for_widget(&self, mut current: Option<WidgetId>) -> CursorType {
        // 子节点未覆盖时依次查询祖先声明。
        while let Some(id) = current {
            // 失效身份或已停止树直接退回默认箭头。
            let Some(node) = self.get(id) else {
                // 平台默认光标不依赖任何组件生命周期。
                return CursorType::Arrow;
            };
            // 最近的显式声明拥有继承优先级。
            if let Some(cursor) = node.cursor() {
                // Arrow 也必须作为显式覆盖返回。
                return cursor;
            }
            // 继续读取运行时树中的直接父节点。
            current = node.parent();
        }
        // 没有命中节点或整条父链都未声明时使用平台默认箭头。
        CursorType::Arrow
    }
}
