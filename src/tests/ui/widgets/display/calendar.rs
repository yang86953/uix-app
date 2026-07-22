use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::painting::PaintPass;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::locale::Locale;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::{Calendar, CalendarEvent, Date, Label};

fn render_calendar_in(
    calendar: &Calendar,
    locale: &Locale,
    frame: Rect,
    surface_size: (i32, i32),
) -> String {
    render_calendar_pass_in(calendar, locale, frame, surface_size, PaintPass::Content)
}

fn render_calendar_pass_in(
    calendar: &Calendar,
    locale: &Locale,
    frame: Rect,
    surface_size: (i32, i32),
    pass: PaintPass,
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
        ctx.set_paint_pass(pass);
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

#[test]
fn calendar_custom_date_cells_materialize_layout_and_clip_each_visible_date() {
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        Calendar::new()
            .default_displayed(2026, 7)
            .date_cell(|date, info| {
                crate::ui::view::label(format!(
                    "{}:selected={}:current={}",
                    date.format(),
                    info.is_selected,
                    info.is_current_month,
                ))
            }),
    ));
    let root = tree.root_id().expect("Calendar root");
    tree.layout();

    let root_node = tree.get(root).expect("Calendar node");
    assert_eq!(root_node.children().len(), 31);
    assert_eq!(
        root_node.children_clip(root_node.frame()),
        Some(root_node.frame())
    );

    let first = root_node.children()[0];
    let first_node = tree.get(first).expect("first custom cell host");
    let first_frame = first_node.frame();
    assert_eq!(
        first_node.key(),
        Some("calendar-cell:2026-07-01"),
        "date keys must be independent of slot order"
    );
    assert!(first_frame.w > 0.0 && first_frame.h > 0.0);
    assert_eq!(first_node.children_clip(first_frame), Some(first_frame));
    assert_eq!(first_node.children().len(), 1);
    assert_eq!(
        tree.get(first_node.children()[0])
            .expect("custom cell content")
            .frame(),
        first_frame,
        "the materialized View must fill exactly one clipped date-cell slot"
    );

    assert_eq!(tree.find_all_by_type::<Label>().len(), 31);
    assert_eq!(
        tree.find_all_by_type::<Label>()[0].1.text(),
        "2026-07-01:selected=false:current=true"
    );
}

#[test]
fn calendar_custom_cells_follow_runtime_month_selection_and_reconcile_factory() {
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        Calendar::new()
            .default_displayed(2026, 7)
            .date_cell(|date, info| {
                crate::ui::view::label(format!("old:{}:{}", date.format(), info.is_selected))
            }),
    ));
    let root = tree.root_id().expect("Calendar root");
    tree.layout();
    let july_cells = tree.get(root).expect("Calendar").children().to_vec();

    let day_15 = tree
        .get(root)
        .and_then(|node| node.component().as_any().downcast_ref::<Calendar>())
        .and_then(|calendar| calendar.day_center_for_test(15))
        .expect("day 15 center");
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: day_15,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    tree.layout();
    assert_eq!(
        tree.get(root).expect("Calendar").children(),
        july_cells.as_slice(),
        "selection context changes should reconcile keyed cells in place"
    );
    assert!(tree
        .find_all_by_type::<Label>()
        .iter()
        .any(|(_, label)| label.text() == "old:2026-07-15:true"));

    tree.set_focus(Some(root));
    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::PageUp,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    tree.layout();
    let june_cells = tree.get(root).expect("Calendar").children().to_vec();
    assert_eq!(june_cells.len(), 30);
    assert!(june_cells.iter().all(|id| !july_cells.contains(id)));
    assert_eq!(
        tree.find_all_by_type::<Label>()[0].1.text(),
        "old:2026-06-01:false"
    );

    let observed_dates = Rc::new(RefCell::new(Vec::new()));
    let observed_by_factory = Rc::clone(&observed_dates);
    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Calendar::new()
                .default_displayed(2026, 1)
                .date_cell(move |date, info| {
                    observed_by_factory.borrow_mut().push(date);
                    crate::ui::view::label(format!("new:{}:{}", date.format(), info.is_selected))
                }),
        ),
    );
    tree.layout();

    assert_eq!(
        tree.get(root)
            .and_then(|node| node.component().as_any().downcast_ref::<Calendar>())
            .expect("Calendar")
            .displayed_month(),
        (2026, 6),
        "declarative rebuild must preserve the user's live month"
    );
    assert_eq!(tree.get(root).expect("Calendar").children(), june_cells);
    assert_eq!(observed_dates.borrow().len(), 30);
    assert!(observed_dates
        .borrow()
        .iter()
        .all(|date| (date.year, date.month) == (2026, 6)));
    assert_eq!(
        tree.find_all_by_type::<Label>()[0].1.text(),
        "new:2026-06-01:false",
        "the replacement factory must run against live state, not its declared default"
    );
}

#[test]
fn calendar_clamped_month_boundary_keeps_custom_cell_identity() {
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        Calendar::new()
            .default_displayed(9999, 12)
            .date_cell(|date, _| crate::ui::view::label(date.format())),
    ));
    let root = tree.root_id().expect("Calendar root");
    tree.layout();
    let before = tree.get(root).expect("Calendar").children().to_vec();

    tree.set_focus(Some(root));
    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::PageDown,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    tree.layout();

    assert_eq!(
        tree.get(root)
            .and_then(|node| node.component().as_any().downcast_ref::<Calendar>())
            .expect("Calendar")
            .displayed_month(),
        (9999, 12)
    );
    assert_eq!(tree.get(root).expect("Calendar").children(), before);
}

#[test]
fn calendar_events_paint_only_the_displayed_dates_and_follow_month_navigation() {
    let mut calendar = Calendar::new().default_displayed(2026, 7).events(vec![
        CalendarEvent::new(Date::new(2026, 7, 15), "July", Color::red()),
        CalendarEvent::new(Date::new(2026, 8, 1), "August", Color::green()),
    ]);
    let frame = Rect::new(0.0, 0.0, 280.0, 280.0);
    let july = render_calendar_in(&calendar, &crate::ui::locale::en_us(), frame, (280, 280));
    assert_eq!(july.matches("FillCircle").count(), 1, "{july}");
    assert!(july.contains("color: Color { r: 255, g: 0, b: 0, a: 255 }"));
    assert!(!july.contains("color: Color { r: 0, g: 255, b: 0, a: 255 }"));

    assert_eq!(
        calendar.on_event(&SystemEvent::KeyDown {
            key: KeyCode::PageDown,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    let august = render_calendar_in(&calendar, &crate::ui::locale::en_us(), frame, (280, 280));
    assert_eq!(august.matches("FillCircle").count(), 1, "{august}");
    assert!(!august.contains("color: Color { r: 255, g: 0, b: 0, a: 255 }"));
    assert!(august.contains("color: Color { r: 0, g: 255, b: 0, a: 255 }"));
}

#[test]
fn calendar_events_cap_visible_markers_and_report_overflow() {
    let date = Date::new(2026, 7, 15);
    let calendar = Calendar::new().default_displayed(2026, 7).events(vec![
        CalendarEvent::new(date, "one", Color::from_rgb(11, 21, 31)),
        CalendarEvent::new(date, "two", Color::from_rgb(12, 22, 32)),
        CalendarEvent::new(date, "three", Color::from_rgb(13, 23, 33)),
        CalendarEvent::new(date, "four", Color::from_rgb(14, 24, 34)),
    ]);
    let display = render_calendar_in(
        &calendar,
        &crate::ui::locale::en_us(),
        Rect::new(0.0, 0.0, 280.0, 280.0),
        (280, 280),
    );

    assert_eq!(display.matches("FillCircle").count(), 3, "{display}");
    assert!(display.contains("text: \"+1\""), "{display}");
    assert!(display.contains("color: Color { r: 13, g: 23, b: 33, a: 255 }"));
    assert!(!display.contains("color: Color { r: 14, g: 24, b: 34, a: 255 }"));
}

#[test]
fn calendar_events_overlay_custom_cells_in_the_after_children_pass() {
    let calendar = Calendar::new()
        .default_displayed(2026, 7)
        .date_cell(|date, _| crate::ui::view::label(date.day.to_string()))
        .events(vec![CalendarEvent::new(
            Date::new(2026, 7, 15),
            "custom",
            Color::from_rgb(41, 51, 61),
        )]);
    let frame = Rect::new(0.0, 0.0, 280.0, 280.0);
    let content = render_calendar_pass_in(
        &calendar,
        &crate::ui::locale::en_us(),
        frame,
        (280, 280),
        PaintPass::Content,
    );
    let overlay = render_calendar_pass_in(
        &calendar,
        &crate::ui::locale::en_us(),
        frame,
        (280, 280),
        PaintPass::AfterChildren,
    );

    assert_eq!(content.matches("FillCircle").count(), 0, "{content}");
    assert_eq!(overlay.matches("FillCircle").count(), 1, "{overlay}");
    assert!(overlay.contains("color: Color { r: 41, g: 51, b: 61, a: 255 }"));
    assert!(!overlay.contains("July 2026"));
}

#[test]
fn calendar_event_reconcile_preserves_live_month_and_replaces_marker_data() {
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        Calendar::new()
            .default_displayed(2026, 7)
            .events(vec![CalendarEvent::new(
                Date::new(2026, 8, 1),
                "old",
                Color::red(),
            )]),
    ));
    let root = tree.root_id().expect("Calendar root");
    tree.layout();
    tree.set_focus(Some(root));
    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::PageDown,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );

    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(Calendar::new().default_displayed(2026, 1).events(vec![
            CalendarEvent::new(Date::new(2026, 8, 2), "new", Color::blue()),
        ])),
    );
    let calendar = tree
        .get(root)
        .and_then(|node| node.component().as_any().downcast_ref::<Calendar>())
        .expect("Calendar");
    assert_eq!(calendar.displayed_month(), (2026, 8));

    let display = render_calendar_in(
        calendar,
        &crate::ui::locale::en_us(),
        Rect::new(0.0, 0.0, 280.0, 280.0),
        (280, 280),
    );
    assert_eq!(display.matches("FillCircle").count(), 1, "{display}");
    assert!(display.contains("color: Color { r: 0, g: 0, b: 255, a: 255 }"));
    assert!(!display.contains("color: Color { r: 255, g: 0, b: 0, a: 255 }"));
}
