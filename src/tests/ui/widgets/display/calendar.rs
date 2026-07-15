use crate::tests::common::*;
use crate::ui::widgets::{Calendar, Date};

#[test]
fn calendar_keyboard_moves_across_month_and_commits_focused_date() {
    let mut calendar = Calendar::new().default_date(Date::new(2026, 1, 31));
    let right = SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    };

    assert_eq!(WidgetComponent::tab_index(&calendar), 1);
    assert_eq!(
        calendar.on_event(&SystemEvent::FocusIn),
        EventResult::Handled
    );
    assert_eq!(calendar.on_event(&right), EventResult::Handled);
    assert_eq!(calendar.displayed_month(), (2026, 2));
    assert_eq!(calendar.selected_date(), Some(Date::new(2026, 1, 31)));

    let enter = SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    };
    assert_eq!(calendar.on_event(&enter), EventResult::Handled);
    assert_eq!(calendar.selected_date(), Some(Date::new(2026, 2, 1)));
    assert_eq!(
        calendar
            .semantic_event(ComponentId::new(5), &enter)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("2026-02-01".to_string())
    );
}

#[test]
fn calendar_year_jump_clamps_leap_day_focus() {
    let mut calendar = Calendar::new()
        .default_date(Date::new(2024, 2, 29))
        .year_jump(true);

    assert_eq!(
        calendar.on_event(&SystemEvent::KeyDown {
            key: KeyCode::PageDown,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );

    assert_eq!(calendar.displayed_month(), (2025, 2));
    assert!(matches!(
        calendar.snapshot_fields(),
        SnapshotFields::Calendar {
            focused_day: 28,
            selected: Some(Date {
                year: 2024,
                month: 2,
                day: 29,
            }),
            ..
        }
    ));
}

#[test]
fn calendar_snapshot_and_accessibility_expose_runtime_date() {
    let calendar = Calendar::new().default_date(Date::new(2026, 7, 15));
    let fields = calendar.snapshot_fields();

    assert!(matches!(
        fields,
        SnapshotFields::Calendar {
            year: 2026,
            month: 7,
            selected: Some(Date { day: 15, .. }),
            focused_day: 15,
            ..
        }
    ));
    let accessibility = fields.accessibility();
    assert_eq!(
        accessibility.state.value_text.as_deref(),
        Some("2026-07-15; focused 2026-07-15")
    );
    assert_eq!(accessibility.state.value_now, Some(15.0));
    assert_eq!(accessibility.state.value_max, Some(31.0));
}

#[test]
fn calendar_rejects_outside_pointer_and_normalizes_cell_size() {
    let mut calendar = Calendar::new()
        .default_displayed(12_000, 13)
        .cell_size(f32::NAN);

    assert_eq!(calendar.displayed_month(), (9999, 12));
    assert_eq!(
        calendar.on_event(&SystemEvent::PointerDown {
            pos: Point::new(-1.0, 60.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    assert!(matches!(
        calendar.snapshot_fields(),
        SnapshotFields::Calendar {
            cell_size: 40.0,
            selected: None,
            ..
        }
    ));
}
