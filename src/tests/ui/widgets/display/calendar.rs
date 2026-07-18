use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::locale::Locale;
use crate::ui::widgets::{Calendar, Date};

fn render_calendar_in(
    calendar: &Calendar,
    locale: &Locale,
    frame: Rect,
    surface_size: (i32, i32),
) -> String {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(surface_size.0, surface_size.1));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let tree = WidgetTree::new();
    let mut display_list = crate::draw::painting::DisplayList::new();
    {
        let mut ctx = PaintContext::new_for_test(
            &mut canvas,
            font,
            &fonts,
            &images,
            &tokens,
            96.0,
            1.0,
            Orientation::YDown,
            surface_size.0,
            surface_size.1,
        );
        crate::ui::locale::with_locale(locale, || {
            ctx.with_recorder(&mut display_list, |ctx| {
                WidgetRender::render(calendar, frame, ctx, &tree);
            });
        });
    }
    format!("{display_list:?}")
}

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

#[test]
fn constrained_calendar_centers_clips_and_uses_rendered_hit_geometry() {
    let mut calendar = Calendar::new().default_displayed(2026, 7);
    let display_list = render_calendar_in(
        &calendar,
        &crate::ui::locale::zh_cn(),
        Rect::new(10.0, 5.0, 140.0, 100.0),
        (180, 120),
    );

    assert!(
        display_list.contains("PushClip { rect: Rect { x: 10.0, y: 5.0, w: 140.0, h: 100.0 } }"),
        "Calendar must clip every draw to the assigned frame: {display_list}"
    );
    assert!(display_list.contains("2026年7月"));
    let control = calendar
        .control_rect_for_test()
        .expect("rendered interaction control");
    assert!((control.x - 20.0).abs() < 0.01);
    assert!((control.y - 0.0).abs() < 0.01);
    assert!((control.w - 100.0).abs() < 0.01);
    assert!((control.h - 100.0).abs() < 0.01);

    assert_eq!(
        calendar.on_event(&SystemEvent::PointerDown {
            pos: Point::new(5.0, 50.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled,
        "blank area inside the assigned frame must not use nominal-size hit zones"
    );
    assert_eq!(
        calendar.on_event(&SystemEvent::PointerDown {
            pos: Point::new(control.x + 1.0, control.y + 1.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(calendar.displayed_month(), (2026, 6));

    let first_day = calendar
        .day_center_for_test(1)
        .expect("first day center after navigation");
    assert_eq!(
        calendar.on_event(&SystemEvent::PointerDown {
            pos: first_day,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(calendar.selected_date(), Some(Date::new(2026, 6, 1)));
}

#[test]
fn calendar_localizes_and_measures_english_month_and_weekday_text() {
    let calendar = Calendar::new().default_displayed(2026, 9).cell_size(20.0);
    let display_list = render_calendar_in(
        &calendar,
        &crate::ui::locale::en_us(),
        Rect::new(0.0, 0.0, 140.0, 160.0),
        (140, 160),
    );

    assert!(
        display_list.contains("September 2026"),
        "English Calendar title should use the active Locale: {display_list}"
    );
    assert!(display_list.contains("Mon"));
    assert!(display_list.contains("Sun"));
    assert!(!display_list.contains("年"));
}
