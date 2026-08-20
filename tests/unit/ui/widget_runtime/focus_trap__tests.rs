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
