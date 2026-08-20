    // 复用被测模块中的开关组件与事件类型。
    use super::*;
    // 引入事件行为 trait 与指针坐标类型。
    use crate::core::Point;
    use crate::ui::{KeyMod, widget_runtime::traits::EventHandler};

    // 键盘激活：KeyDown 只武装，配对的 KeyUp 才完成切换（与 Button 一致）。
    #[test]
    fn keyboard_activation_completes_on_matching_key_up() {
        // 构造未开启开关。
        let mut switch = Switch::new();
        // KeyDown 按下激活键。
        let _ = EventHandler::on_event(
            &mut switch,
            &SystemEvent::KeyDown {
                key: KeyCode::Space,
                mods: KeyMod::NONE,
            },
        );
        // 按下阶段不得立即切换。
        assert!(!switch.checked);
        // 非配对释放键不完成切换。
        let _ = EventHandler::on_event(
            &mut switch,
            &SystemEvent::KeyUp {
                key: KeyCode::Enter,
                mods: KeyMod::NONE,
            },
        );
        assert!(!switch.checked);
        // 配对的 Space 释放完成切换。
        let _ = EventHandler::on_event(
            &mut switch,
            &SystemEvent::KeyUp {
                key: KeyCode::Space,
                mods: KeyMod::NONE,
            },
        );
        assert!(switch.checked);
    }

    // 指针激活：PointerDown 只武装，PointerUp 才完成切换；移出取消手势。
    #[test]
    fn pointer_activation_completes_on_pointer_up_and_cancels_on_leave() {
        // 构造未开启开关。
        let mut switch = Switch::new();
        // PointerDown 按下。
        let _ = EventHandler::on_event(
            &mut switch,
            &SystemEvent::PointerDown {
                pos: Point::new(8.0, 8.0),
                button: MouseButton::Left,
                mods: KeyMod::NONE,
            },
        );
        // 按下阶段不得立即切换。
        assert!(!switch.checked);
        // 指针移出取消未完成的按下手势。
        let _ = EventHandler::on_event(&mut switch, &SystemEvent::PointerLeave);
        // 随后到达的 PointerUp 不再完成切换。
        let _ = EventHandler::on_event(
            &mut switch,
            &SystemEvent::PointerUp {
                pos: Point::new(8.0, 8.0),
                button: MouseButton::Left,
                mods: KeyMod::NONE,
            },
        );
        assert!(!switch.checked);

        // 完整按下/释放序列才完成切换。
        let _ = EventHandler::on_event(
            &mut switch,
            &SystemEvent::PointerDown {
                pos: Point::new(8.0, 8.0),
                button: MouseButton::Left,
                mods: KeyMod::NONE,
            },
        );
        let _ = EventHandler::on_event(
            &mut switch,
            &SystemEvent::PointerUp {
                pos: Point::new(8.0, 8.0),
                button: MouseButton::Left,
                mods: KeyMod::NONE,
            },
        );
        assert!(switch.checked);
    }
