// 引入树事件模块持有的 WidgetTree 与节点核心契约。
use super::*;
// 引入平台无关的指针光标枚举。
use crate::platform::windowing::CursorType;

// 为 UI System 提供当前指针路由目标的纯查询能力。
impl WidgetTree {
    // 返回当前悬停节点继承后生效的平台光标。
    pub(crate) fn active_pointer_cursor(&self) -> CursorType {
        // 使用事件路由已经确定的悬停身份，确保浮层与普通树命中语义一致。
        self.cursor_for_component(self.managers().interaction.hovered_component())
    }

    // 从指定节点沿父链解析最近的显式光标覆盖。
    fn cursor_for_component(&self, mut current: Option<WidgetId>) -> CursorType {
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

// 验证光标继承、显式默认覆盖和空命中回退。
#[cfg(test)]
mod tests {
    // 引入被测查询和树内部构建入口。
    use super::*;
    // 引入声明树到运行时树的唯一适配器。
    use crate::ui::adapter::ViewAdapter;
    // 引入最小容器组件和公开声明节点。
    use crate::ui::{Container, ViewNode};

    // 子节点应继承父光标并允许显式 Arrow 覆盖。
    #[test]
    fn resolves_inherited_and_explicit_pointer_cursors() {
        // 第一个子节点不声明光标，用于验证父级继承。
        let inherited = ViewNode::leaf(Container::new());
        // 第二个子节点显式声明默认箭头，用于覆盖父级手形。
        let explicit_default = ViewNode::leaf(Container::new()).cursor(CursorType::Arrow);
        // 根节点声明手形光标并承载两个子节点。
        let root = ViewNode::new(Container::new(), vec![inherited, explicit_default])
            // 所有未覆盖后代应继承该手形光标。
            .cursor(CursorType::Hand);
        // 通过正式适配器建立运行时节点元数据。
        let mut tree = WidgetTree::default();
        // 挂载完整声明树并取得根身份。
        let root_id = tree.build(ViewAdapter::expand(root));
        // 复制直接子身份，避免后续查询持有节点借用。
        let children = tree
            // 根节点必须已成功挂载。
            .get(root_id)
            // 测试树必须保留两个直接子节点。
            .expect("根节点应存在")
            // 复制小型身份列表用于独立查询。
            .children()
            // 转为测试拥有的向量。
            .to_vec();
        // 未声明的第一个子节点继承根手形。
        assert_eq!(tree.cursor_for_component(Some(children[0])), CursorType::Hand);
        // 显式 Arrow 必须覆盖根手形，不能被当作无声明。
        assert_eq!(
            // 查询第二个子节点的有效光标。
            tree.cursor_for_component(Some(children[1])),
            // 预期平台默认箭头。
            CursorType::Arrow
        );
        // 没有悬停目标时始终回退平台默认箭头。
        assert_eq!(tree.cursor_for_component(None), CursorType::Arrow);
    }
}
