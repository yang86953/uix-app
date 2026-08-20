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
    assert_eq!(tree.cursor_for_widget(Some(children[0])), CursorType::Hand);
    // 显式 Arrow 必须覆盖根手形，不能被当作无声明。
    assert_eq!(
        // 查询第二个子节点的有效光标。
        tree.cursor_for_widget(Some(children[1])),
        // 预期平台默认箭头。
        CursorType::Arrow
    );
    // 没有悬停目标时始终回退平台默认箭头。
    assert_eq!(tree.cursor_for_widget(None), CursorType::Arrow);
}
