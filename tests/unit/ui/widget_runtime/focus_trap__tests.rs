// 复用被测焦点陷阱与纯顺序 helper。
use super::{FocusTrap, next_focus_in_order};
use crate::core::Rect;
// 引入真实声明适配器与布局组件，覆盖透明包装节点的正常流尺寸。
use crate::ui::adapter::ViewAdapter;
// 引入稳定组件身份集合。
use crate::ui::widget_runtime::widget::WidgetCore;
use crate::ui::{ViewNode, WidgetId};
// 使用真实按钮作为可聚焦内容。
use crate::ui::widgets::{Button, Container};
// 引入焦点陷阱公开更新接口所需集合。
use std::collections::HashSet;

// 验证正反向导航都在作用域边界循环。
#[test]
// 测试名称说明焦点顺序的循环语义。
fn focus_order_wraps_in_both_directions() {
    // 创建首个稳定焦点身份。
    let first = WidgetId::new(2);
    // 创建中间稳定焦点身份。
    let middle = WidgetId::new(4);
    // 创建末尾稳定焦点身份。
    let last = WidgetId::new(8);
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
    focusable.insert(WidgetId::new(3));
    // 创建显式禁用的焦点陷阱。
    let mut trap = FocusTrap::new().active(false);
    // 通过运行时入口更新可聚焦后代集合。
    trap.update_focusable(focusable);
    // 禁用作用域不得返回任何下一焦点。
    assert_eq!(trap.next_focus(None, true), None);
}

// 验证透明焦点作用域由真实子按钮撑开，避免内容越过父面板底部。
#[test]
fn focus_trap_uses_child_natural_size_in_parent_flow() {
    let mut tree = ViewAdapter::build_nodes(ViewNode::new(
        Container::new().size(240.0, 100.0),
        vec![ViewNode::new(
            FocusTrap::new(),
            vec![ViewNode::leaf(Button::new("确认"))],
        )],
    ));
    let root = tree.root_id().expect("根容器必须存在");
    let trap = tree.get(root).expect("根容器必须可读").children()[0];
    let button = tree.get(trap).expect("焦点作用域必须可读").children()[0];
    tree.get_mut(root)
        .expect("根容器必须可写")
        .set_frame(Rect::new(0.0, 0.0, 240.0, 100.0));

    tree.layout();

    let trap_frame = tree.get(trap).expect("焦点作用域必须存在").frame();
    let button_frame = tree.get(button).expect("按钮必须存在").frame();
    assert!(trap_frame.h > 0.0);
    assert!(button_frame.h > 0.0);
    assert!(button_frame.y + button_frame.h <= trap_frame.y + trap_frame.h);
}
