use crate::tests::common::*;
use crate::ui::view::{embed, ViewAdapter};
use crate::ui::widgets::{ResultType, ResultView};
use crate::ui::{AccessibilityRole, WidgetComponent};
use std::cell::Cell;
use std::rc::Rc;

#[test]
fn result_action_is_focusable_and_only_claims_its_button_region() {
    let mut result = ResultView::new(ResultType::Error)
        .title("保存失败")
        .extra_text("重试");
    let outside = SystemEvent::PointerDown {
        pos: Point::new(20.0, 20.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };
    let action = SystemEvent::PointerDown {
        pos: Point::new(200.0, 208.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };

    assert_eq!(WidgetComponent::tab_index(&result), 1);
    assert_eq!(result.on_event(&outside), EventResult::NotHandled);
    assert_eq!(result.on_event(&action), EventResult::Handled);
    let accessibility = result.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Button);
    assert_eq!(accessibility.name.as_deref(), Some("重试"));
    assert_eq!(accessibility.state.value_text.as_deref(), Some("保存失败"));
}

#[test]
fn result_pointer_and_keyboard_activation_reach_public_click_handler() {
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    let mut tree = ViewAdapter::build(
        embed(ResultView::new(ResultType::Info).extra_text("继续")).on_click_fn(move || {
            observed.set(observed.get() + 1);
        }),
    );
    let id = tree.root_id().expect("result root");
    tree.get_mut(id)
        .expect("result node")
        .set_frame(Rect::new(0.0, 0.0, 400.0, 300.0));
    let pos = Point::new(200.0, 208.0);

    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerUp {
            pos,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(calls.get(), 1);

    let body = Point::new(20.0, 20.0);
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: body,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    let _ = tree.dispatch_event(&SystemEvent::PointerUp {
        pos: body,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(calls.get(), 1);

    tree.set_focus(Some(id));
    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Space,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyUp {
            key: KeyCode::Space,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(calls.get(), 2);
}

#[test]
fn result_without_action_remains_a_noninteractive_status() {
    let mut result = ResultView::new(ResultType::Success).title("完成");
    assert_eq!(WidgetComponent::tab_index(&result), 0);
    assert_eq!(
        result.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    assert_eq!(
        result.snapshot_fields().accessibility().role,
        AccessibilityRole::Status
    );
}
