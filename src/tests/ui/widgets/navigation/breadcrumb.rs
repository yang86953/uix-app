use crate::tests::common::*;
use crate::ui::widgets::navigation::breadcrumb::*;

#[test]
fn breadcrumb_is_picture_eligible() {
    let breadcrumb = Breadcrumb::new()
        .item(BreadcrumbItem::new("Home"))
        .item(BreadcrumbItem::new("Docs").active());

    assert_eq!(breadcrumb.picture_policy(), PicturePolicy::Eligible);
}

#[test]
fn breadcrumb_keyboard_navigation_updates_active_item_and_emits_title() {
    let mut breadcrumb = Breadcrumb::new()
        .item(BreadcrumbItem::new("Home"))
        .item(BreadcrumbItem::new("Docs").active())
        .item(BreadcrumbItem::new("API"));
    let left = SystemEvent::KeyDown {
        key: KeyCode::Left,
        mods: KeyMod::NONE,
    };

    assert_eq!(WidgetComponent::tab_index(&breadcrumb), 1);
    assert_eq!(breadcrumb.active_index(), 1);
    assert_eq!(breadcrumb.on_event(&left), EventResult::Handled);
    assert_eq!(breadcrumb.active_title(), Some("Home"));
    assert_eq!(
        breadcrumb
            .semantic_event(ComponentId::new(9), &left)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("Home".to_string())
    );

    let enter = SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    };
    assert_eq!(breadcrumb.on_event(&enter), EventResult::Handled);
    assert_eq!(
        breadcrumb
            .semantic_event(ComponentId::new(9), &enter)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("Home".to_string())
    );
}

#[test]
fn breadcrumb_pointer_hit_testing_skips_separator_and_uses_custom_separator_width() {
    let mut breadcrumb = Breadcrumb::new()
        .items(vec![
            BreadcrumbItem::new("A").active(),
            BreadcrumbItem::new("B"),
        ])
        .separator(">>");
    let first_width = 15.5;
    let separator_width = 24.0;

    assert_eq!(
        breadcrumb.on_event(&SystemEvent::PointerDown {
            pos: Point::new(first_width + 1.0, 10.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    assert_eq!(
        breadcrumb.on_event(&SystemEvent::PointerDown {
            pos: Point::new(first_width + separator_width + 1.0, 10.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(breadcrumb.active_title(), Some("B"));
    assert_eq!(
        breadcrumb
            .snapshot_fields()
            .accessibility()
            .state
            .value_text
            .as_deref(),
        Some("B")
    );
}

#[test]
fn empty_breadcrumb_is_not_focusable_or_pointer_interactive() {
    let mut breadcrumb = Breadcrumb::new();

    assert_eq!(WidgetComponent::tab_index(&breadcrumb), 0);
    assert_eq!(
        breadcrumb.on_event(&SystemEvent::PointerDown {
            pos: Point::new(1.0, 1.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
}
