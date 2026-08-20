    // 复用被测模块中的单选组组件与事件类型。
    use super::*;
    // 引入事件行为 trait 与修饰键类型。
    use crate::ui::{KeyMod, widget_runtime::traits::EventHandler};

    // 方向键导航有界：边界处不再循环回绕。
    #[test]
    fn arrow_navigation_is_bounded_at_edges() {
        // 构造三个选项、选中首项的单选组。
        let mut radio = Radio::new().options(["A", "B", "C"]).default_selected(0);
        // 在首项向左移动。
        let _ = EventHandler::on_event(
            &mut radio,
            &SystemEvent::KeyDown {
                key: KeyCode::Left,
                mods: KeyMod::NONE,
            },
        );
        // 有界语义下停在首项，不回绕到末项。
        assert_eq!(radio.current_index(), Some(0));
        // 在末项向右移动。
        let mut radio = Radio::new().options(["A", "B", "C"]).default_selected(2);
        let _ = EventHandler::on_event(
            &mut radio,
            &SystemEvent::KeyDown {
                key: KeyCode::Right,
                mods: KeyMod::NONE,
            },
        );
        // 有界语义下停在末项，不回绕到首项。
        assert_eq!(radio.current_index(), Some(2));
        // 组内中间项仍可正常双向移动。
        let mut radio = Radio::new().options(["A", "B", "C"]).default_selected(1);
        let _ = EventHandler::on_event(
            &mut radio,
            &SystemEvent::KeyDown {
                key: KeyCode::Down,
                mods: KeyMod::NONE,
            },
        );
        assert_eq!(radio.current_index(), Some(2));
    }

    // Home/End 分别跳到组内首项与末项。
    #[test]
    fn home_and_end_jump_to_group_edges() {
        // 构造三个选项、选中中项的单选组。
        let mut radio = Radio::new().options(["A", "B", "C"]).default_selected(1);
        // Home 跳到首项。
        let _ = EventHandler::on_event(
            &mut radio,
            &SystemEvent::KeyDown {
                key: KeyCode::Home,
                mods: KeyMod::NONE,
            },
        );
        assert_eq!(radio.current_index(), Some(0));
        // End 跳到末项。
        let _ = EventHandler::on_event(
            &mut radio,
            &SystemEvent::KeyDown {
                key: KeyCode::End,
                mods: KeyMod::NONE,
            },
        );
        assert_eq!(radio.current_index(), Some(2));
    }
