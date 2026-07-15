use crate::tests::common::*;
use crate::ui::view::{embed, ViewAdapter};
use crate::ui::widgets::BackTop;
use crate::ui::AccessibilityRole;

#[test]
fn hidden_back_top_is_not_visible_focusable_or_interactive() {
    let mut back_top = BackTop::new();

    assert!(!WidgetComponent::visible(&back_top));
    assert_eq!(WidgetComponent::tab_index(&back_top), 0);
    assert_eq!(
        back_top.measure(Constraints::loose(Size::new(100.0, 100.0))),
        Size::zero()
    );
    assert_eq!(
        back_top.on_event(&SystemEvent::PointerDown {
            pos: Point::new(10.0, 10.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    let accessibility = back_top.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Button);
    assert!(accessibility.state.disabled);
}

#[test]
fn controlled_scroll_position_drives_visibility_and_normalizes_threshold() {
    let back_top = BackTop::new().visibility_height(f32::NAN).scroll_y(401.0);

    assert!(WidgetComponent::visible(&back_top));
    assert_eq!(WidgetComponent::tab_index(&back_top), 1);
    assert_eq!(
        back_top.snapshot_fields(),
        SnapshotFields::BackTop {
            visibility_height: 400.0,
            visible: true,
        }
    );
}

#[test]
fn manual_visibility_reports_changes_and_is_preserved_without_controlled_scroll() {
    let mut back_top = BackTop::new().visibility_height(100.0);

    assert!(back_top.update_visibility(101.0));
    assert!(!back_top.update_visibility(200.0));
    assert!(back_top.is_visible());
}

#[test]
fn visible_back_top_keyboard_activation_uses_standard_click_handler() {
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    let mut tree = ViewAdapter::build(
        embed(BackTop::new().scroll_y(500.0)).on_click_fn(move || observed.set(observed.get() + 1)),
    );
    let root = tree.root_id().expect("back top root");
    tree.set_focus(Some(root));

    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(calls.get(), 0);
    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyUp {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(calls.get(), 1);
}
