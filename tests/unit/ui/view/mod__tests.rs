    // 引入当前模块公开与内部测试边界。
    use super::*;
    // 引入可共享修改的测试观察值。
    use std::cell::RefCell;
    // 引入单线程共享所有权。
    use std::rc::Rc;

    // 验证 Change 事件只转发文本载荷。
    #[test]
    fn on_change_fn_forwards_text_payload() {
        // 保存处理器观察到的最新文本。
        let observed = Rc::new(RefCell::new(String::new()));
        // 克隆所有权给静态事件闭包。
        let callback_observed = Rc::clone(&observed);
        // 创建最小输入节点并登记公开 Change 处理器。
        let mut node = ViewNode::leaf(crate::ui::widgets::Input::new("")).on_change_fn(
            // 把回调文本复制到测试观察值。
            move |value| *callback_observed.borrow_mut() = value.to_string(),
        );
        // 构造带文本载荷的语义变更事件。
        let mut event = SemanticEvent::change(crate::core::ComponentId::new(1), "Belldandy");
        // 调用节点登记的唯一处理器。
        (node.handlers[0].handler)(&mut event);
        // 回调必须收到完整当前文本。
        assert_eq!(&*observed.borrow(), "Belldandy");
    }
