use crate::draw::engine::cpu::canvas_2d::CpuCanvas2D;
use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::state::State;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::{Date, DatePicker, PickerMode, Weekday};
use crate::ui::{with_config, ComponentConfig};

fn render_picker(picker: &DatePicker) {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(320, 360));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let tree = WidgetTree::new();
    let mut ctx = PaintContext::new_for_test(
        &mut canvas,
        font,
        &fonts,
        &images,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        320,
        360,
    );
    let size = picker.measure(Constraints::loose(Size::new(240.0, 280.0)));
    WidgetRender::render(
        picker,
        Rect::new(32.0, 18.0, size.w, size.h),
        &mut ctx,
        &tree,
    );
}

fn render_picker_pixels(picker: &DatePicker, frame: Rect) -> (Vec<u32>, usize) {
    const WIDTH: i32 = 240;
    const HEIGHT: i32 = 320;
    let mut canvas = CpuCanvas2D::new(PixelSurface::new(WIDTH, HEIGHT));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let tree = WidgetTree::new();
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
            WIDTH,
            HEIGHT,
        );
        WidgetRender::render(picker, frame, &mut ctx, &tree);
    }
    (canvas.surface().pixels().to_vec(), WIDTH as usize)
}

fn click(picker: &mut DatePicker, x: f32, y: f32) -> EventResult {
    picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(x, y),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    })
}

#[test]
fn bound_date_picker_writes_pointer_selection_and_reads_external_updates() {
    let selected = State::new(Date::new(2026, 7, 10));
    let mut picker = DatePicker::new().value(&selected);
    render_picker(&picker);

    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(4.0, 4.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert!(picker.is_open());
    let trigger = Rect::new(32.0, 18.0, 160.0, 32.0);
    let hit_frame = EventHandler::hit_test_frame(&picker, trigger);
    assert!(hit_frame.contains(Point::new(40.0, 180.0)));
    assert_eq!(WidgetRender::dirty_rect(&picker, trigger), hit_frame);

    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(60.0, 105.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(selected.get(), Date::new(2026, 7, 1));
    assert_eq!(picker.current_value(), Date::new(2026, 7, 1));

    selected.set(Date::new(2027, 8, 12));
    picker.sync_from(DatePicker::new().value(&selected));
    assert_eq!(picker.current_value(), Date::new(2027, 8, 12));
}

#[test]
fn date_normalizes_month_before_clamping_day() {
    assert_eq!(Date::new(2026, 0, 31), Date::new(2026, 1, 31));
    assert_eq!(Date::new(2025, 2, 31), Date::new(2025, 2, 28));

    let today = Date::today();
    assert!((1..=12).contains(&today.month));
    assert!((1..=31).contains(&today.day));
    assert_eq!(Date::new(2026, 7, 13).weekday(), Weekday::Monday);
    assert_eq!(Date::new(1970, 1, 1).weekday(), Weekday::Thursday);
    assert_eq!(Date::new(1, 1, 1).weekday(), Weekday::Monday);
    assert!(Date::new(2026, 7, 18).weekday().is_weekend());
}

#[test]
fn picker_modes_write_period_start_and_disabled_dates_do_not_commit() {
    let selected = State::new(Date::new(2026, 7, 10));
    let mut picker = DatePicker::new()
        .value(&selected)
        .mode(PickerMode::Week)
        .disabled_date(|date| date == Date::new(2026, 7, 1));
    render_picker(&picker);

    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(4.0, 4.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(60.0, 105.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(selected.get(), Date::new(2026, 7, 10));
    assert!(picker.is_open());

    picker.sync_from(DatePicker::new().value(&selected).mode(PickerMode::Week));
    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(60.0, 105.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(selected.get(), Date::new(2026, 6, 29));
    assert!(!picker.is_open());

    selected.set(Date::new(2026, 7, 10));
    picker.sync_from(DatePicker::new().value(&selected).mode(PickerMode::Month));
    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(4.0, 4.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(100.0, 105.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(selected.get(), Date::new(2026, 7, 1));
    assert_eq!(picker.picker_mode(), PickerMode::Month);
}

#[test]
fn external_date_state_reconciles_and_updates_semantic_value() {
    let selected = State::new(Date::new(2026, 7, 10));
    let mut tree = ViewAdapter::build_nodes(ViewAdapter::capture_root(|| {
        ViewNode::leaf(DatePicker::new().value(&selected))
    }));
    let root = tree.root_id().expect("date picker root");
    tree.reset_invalidation();

    selected.set(Date::new(2027, 8, 12));
    assert!(tree.take_reconcile_requested());
    let next = ViewAdapter::capture_root(|| ViewNode::leaf(DatePicker::new().value(&selected)));
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let picker = tree
        .get(root)
        .expect("date picker node")
        .component()
        .as_any()
        .downcast_ref::<DatePicker>()
        .expect("DatePicker component");
    assert_eq!(picker.current_value(), Date::new(2027, 8, 12));
    assert!(matches!(
        picker.snapshot_fields(),
        SnapshotFields::DatePicker {
            value: Some(value),
            ..
        } if value == "2027-08-12"
    ));
    let accessibility =
        crate::ui::ComponentConfigSnapshot::from_component(root, picker).accessibility();
    assert_eq!(
        accessibility.state.value_text.as_deref(),
        Some("2027-08-12")
    );
    assert!(tree
        .invalidation()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .node_needs_paint(root));
}

#[test]
fn provider_size_and_explicit_override_move_date_popup_with_the_trigger() {
    let selected = State::new(Date::new(2026, 7, 10));
    let large = ComponentConfig::new().component_size(ControlSize::Large);
    let max = Constraints::loose(Size::new(1_000.0, 1_000.0));
    let mut picker = with_config(&large, || DatePicker::new().value(&selected));
    assert_eq!(picker.measure(max).h, 40.0);
    render_picker(&picker);

    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(4.0, 20.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(60.0, 113.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(selected.get(), Date::new(2026, 7, 1));

    let small = with_config(&large, || DatePicker::new().size(ControlSize::Small));
    assert_eq!(small.measure(max).h, 24.0);
}

#[test]
fn calendar_weekday_header_and_first_date_row_have_distinct_hit_geometry() {
    let selected = State::new(Date::new(2026, 7, 17));
    let mut picker = DatePicker::new().value(&selected);
    render_picker(&picker);

    assert_eq!(click(&mut picker, 4.0, 4.0), EventResult::Handled);
    assert_eq!(
        click(&mut picker, 60.0, 45.0),
        EventResult::Handled,
        "clicking the centered month title must not activate either navigation button"
    );
    assert_eq!(click(&mut picker, 60.0, 105.0), EventResult::Handled);
    assert_eq!(
        selected.get(),
        Date::new(2026, 7, 1),
        "the first date row must start below the weekday labels"
    );
}

#[test]
fn calendar_navigation_hit_zones_match_the_header_edge_icons() {
    let selected = State::new(Date::new(2026, 7, 17));
    let mut picker = DatePicker::new().value(&selected);
    render_picker(&picker);

    assert_eq!(click(&mut picker, 4.0, 4.0), EventResult::Handled);
    assert_eq!(click(&mut picker, 12.0, 45.0), EventResult::Handled);
    assert_eq!(click(&mut picker, 18.0, 105.0), EventResult::Handled);
    assert_eq!(
        selected.get(),
        Date::new(2026, 6, 1),
        "the left header icon hit zone must navigate to the previous month"
    );
}

#[test]
fn constrained_trigger_keeps_a_readable_minimum_width_calendar_popup() {
    let mut picker = DatePicker::new().default_value(Date::new(2026, 7, 17));
    let frame = Rect::new(10.0, 20.0, 80.0, 12.0);
    let _ = render_picker_pixels(&picker, Rect::new(0.0, 0.0, frame.w, frame.h));
    assert_eq!(click(&mut picker, 4.0, 4.0), EventResult::Handled);

    let hit_frame = EventHandler::hit_test_frame(&picker, frame);
    assert!(
        hit_frame.w >= 160.0,
        "the seven-column popup must not collapse to the constrained trigger width: {hit_frame:?}"
    );
}

#[test]
fn constrained_closed_trigger_clips_text_icon_and_focus_to_its_actual_frame() {
    let mut picker = DatePicker::new()
        .placeholder("WWWWWWWWWWWWWWWWWWWW")
        .size(ControlSize::Large);
    assert_eq!(picker.on_event(&SystemEvent::FocusIn), EventResult::Handled);
    let frame = Rect::new(0.0, 0.0, 60.0, 10.0);
    let (pixels, stride) = render_picker_pixels(&picker, frame);

    for y in 0..320usize {
        for x in 0..240usize {
            if x >= frame.w as usize || y >= frame.h as usize {
                assert_eq!(
                    pixels[y * stride + x],
                    0,
                    "date picker trigger paint leaked at ({x}, {y})"
                );
            }
        }
    }
}

#[test]
fn focus_and_open_state_are_observable_to_keyboard_and_accessibility() {
    let mut picker = DatePicker::new();
    let id = ComponentId::new(7);
    let frame = Rect::new(20.0, 40.0, 160.0, 32.0);
    assert_eq!(
        picker.snapshot_fields().accessibility().state.expanded,
        Some(false)
    );
    assert!(WidgetRender::overlay_entry(&picker, id, frame).is_none());
    assert_eq!(picker.on_event(&SystemEvent::FocusIn), EventResult::Handled);
    assert_eq!(
        picker.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(picker.is_open());
    assert_eq!(
        picker.snapshot_fields().accessibility().state.expanded,
        Some(true)
    );
    let overlay = WidgetRender::overlay_entry(&picker, id, frame)
        .expect("an open date picker must paint above later siblings");
    assert_eq!(overlay.kind(), crate::ui::OverlayKind::Popover);
    assert_eq!(
        overlay.bounds_rect(),
        Some(EventHandler::hit_test_frame(&picker, frame))
    );
}

#[test]
fn clearing_calendar_hover_requests_repaint_and_unrelated_keys_are_not_swallowed() {
    let mut picker = DatePicker::new().default_value(Date::new(2026, 7, 17));
    render_picker(&picker);
    assert_eq!(click(&mut picker, 4.0, 4.0), EventResult::Handled);

    assert_eq!(
        picker.on_event(&SystemEvent::PointerMove {
            pos: Point::new(60.0, 105.0),
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        picker.on_event(&SystemEvent::PointerMove {
            pos: Point::new(80.0, 45.0),
            mods: KeyMod::NONE,
        }),
        EventResult::Handled,
        "clearing the previous hover must invalidate its highlight"
    );
    assert_eq!(
        picker.on_event(&SystemEvent::KeyDown {
            key: KeyCode::A,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
}
