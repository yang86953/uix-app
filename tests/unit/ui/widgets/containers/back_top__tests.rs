    // 引入待验证组件与事件处理 trait。
    use super::*;
    // 引入构造指针事件所需的几何与修饰键类型。
    use crate::core::Point;
    // 引入事件处理 trait 以调用组件契约。
    use crate::ui::{EventHandler, KeyMod, MouseButton};

    // 验证绑定状态决定可见性且激活写回顶部。
    #[test]
    fn bound_state_activation_writes_top() {
        // 创建超过默认阈值的应用滚动状态。
        let scroll_y = State::new(450.0_f32);
        // 绑定公开 BackTop 组件。
        let mut back_top = BackTop::new().scroll_state(&scroll_y);
        // 初始滚动位置应使按钮可见。
        assert!(back_top.is_visible());
        // 构造主指针激活事件。
        let result = back_top.on_event(&SystemEvent::PointerDown {
            // 坐标不影响组件自身的激活语义。
            pos: Point::zero(),
            // 只允许主按钮激活。
            button: MouseButton::Left,
            // 本测试不使用修饰键。
            mods: KeyMod::NONE,
        });
        // 激活必须由组件消费。
        assert_eq!(result, EventResult::Handled);
        // 应用拥有的状态必须被写回顶部。
        assert_eq!(scroll_y.get(), 0.0);
        // 当前组件必须立即隐藏。
        assert!(!back_top.is_visible());
    }
