// 复用被测模块中的勾选框组件与事件类型。
use super::*;
// 引入事件行为 trait 与指针坐标类型。
use crate::core::Point;
use crate::ui::{KeyMod, View, widget_runtime::traits::EventHandler};
// 比较构建前后是否共享 UIX 静态视觉。
use std::ptr;

// 验证默认实例和 UIX 构建节点只使用同一份视觉事实。
#[test]
fn checkbox_uses_colocated_uix_visual() {
    // 构建前不得复制第二份默认视觉表。
    let checkbox = Checkbox::default();
    assert!(ptr::eq(checkbox.visual, CHECKBOX_VISUAL_REF));
    // UIX 注入后的叶内核仍指向同一静态记录。
    let node = View::build(checkbox);
    let checkbox = node
        .widget
        .as_any()
        .downcast_ref::<Checkbox>()
        .expect("UIX 根应保留 Checkbox 内核");
    assert!(ptr::eq(checkbox.visual, CHECKBOX_VISUAL_REF));
}

// 键盘激活：KeyDown 只武装，配对的 KeyUp 才完成切换（与 Button 一致）。
#[test]
fn keyboard_activation_completes_on_matching_key_up() {
    // 构造未勾选勾选框。
    let mut checkbox = Checkbox::new("同意");
    // KeyDown 按下激活键。
    let _ = EventHandler::on_event(
        &mut checkbox,
        &SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        },
    );
    // 按下阶段不得立即切换。
    assert!(!checkbox.checked);
    // 配对的 Enter 释放完成切换。
    let _ = EventHandler::on_event(
        &mut checkbox,
        &SystemEvent::KeyUp {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        },
    );
    assert!(checkbox.checked);
}

// 指针激活：PointerDown 只武装，PointerUp 才完成切换。
#[test]
fn pointer_activation_completes_on_pointer_up() {
    // 构造未勾选勾选框。
    let mut checkbox = Checkbox::new("同意");
    // PointerDown 按下。
    let _ = EventHandler::on_event(
        &mut checkbox,
        &SystemEvent::PointerDown {
            pos: Point::new(8.0, 8.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        },
    );
    // 按下阶段不得立即切换。
    assert!(!checkbox.checked);
    // PointerUp 完成切换。
    let _ = EventHandler::on_event(
        &mut checkbox,
        &SystemEvent::PointerUp {
            pos: Point::new(8.0, 8.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        },
    );
    assert!(checkbox.checked);
}
