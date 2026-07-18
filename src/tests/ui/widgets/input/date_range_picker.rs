use crate::draw::engine::cpu::canvas_2d::CpuCanvas2D;
use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::state::State;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::{Date, DateRangePicker, PresetDate, Weekday};
use crate::ui::{with_config, ComponentConfig};

fn render_picker(picker: &DateRangePicker) {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(400, 480));
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
        400,
        480,
    );
    let size = picker.measure(Constraints::loose(Size::new(320.0, 420.0)));
    WidgetRender::render(
        picker,
        Rect::new(36.0, 20.0, size.w, size.h),
        &mut ctx,
        &tree,
    );
}

fn render_picker_pixels(picker: &DateRangePicker, frame: Rect) -> (Vec<u32>, usize) {
    const WIDTH: i32 = 400;
    const HEIGHT: i32 = 360;
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

fn click(picker: &mut DateRangePicker, x: f32, y: f32) -> EventResult {
    picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(x, y),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    })
}

#[test]
fn pointer_selection_commits_ordered_range_and_skips_disabled_endpoint() {
    let start = State::new(Date::new(2026, 7, 10));
    let end = State::new(Date::new(2026, 7, 12));
    let mut picker = DateRangePicker::new()
        .start(&start)
        .end(&end)
        .disabled_date(|date| date == Date::new(2026, 7, 4));
    render_picker(&picker);

    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(4.0, 4.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let trigger = Rect::new(36.0, 20.0, 280.0, 32.0);
    let hit_frame = EventHandler::hit_test_frame(&picker, trigger);
    assert!(hit_frame.contains(Point::new(48.0, 300.0)));
    assert_eq!(WidgetRender::dirty_rect(&picker, trigger), hit_frame);
    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(100.0, 105.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert!(picker.is_selecting_end());
    assert_eq!(start.get(), Date::new(2026, 7, 10));

    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(210.0, 105.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert!(picker.is_selecting_end());
    assert_eq!(end.get(), Date::new(2026, 7, 12));

    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(175.0, 105.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(start.get(), Date::new(2026, 7, 1));
    assert_eq!(end.get(), Date::new(2026, 7, 3));
    assert!(!picker.is_open());
}

#[test]
fn preset_click_commits_bound_states_and_closes_popup() {
    let start = State::new(Date::default());
    let end = State::new(Date::default());
    let preset = PresetDate::new(Date::new(2026, 8, 5), Date::new(2026, 8, 1));
    let mut picker = DateRangePicker::new()
        .start(&start)
        .end(&end)
        .presets([("发布窗口", preset)]);
    render_picker(&picker);

    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(4.0, 4.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(20.0, 298.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(start.get(), Date::new(2026, 8, 1));
    assert_eq!(end.get(), Date::new(2026, 8, 5));
    assert!(!picker.is_open());
}

#[test]
fn presets_cover_stable_calendar_boundaries() {
    let reversed = PresetDate::new(Date::new(2026, 9, 5), Date::new(2026, 9, 1));
    assert_eq!(reversed.start(), Date::new(2026, 9, 1));
    assert_eq!(reversed.end(), Date::new(2026, 9, 5));

    let week = PresetDate::this_week();
    assert_eq!(week.start().weekday(), Weekday::Monday);
    assert_eq!(week.end().weekday(), Weekday::Sunday);

    let one_day = PresetDate::last_days(0);
    assert_eq!(one_day.start(), one_day.end());
    let longest = PresetDate::last_days(u32::MAX);
    assert!(longest.start() < longest.end());
    let month = PresetDate::this_month();
    assert_eq!(month.start().day, 1);
    assert_eq!(month.start().month, month.end().month);
}

#[test]
fn external_range_state_reconciles_and_updates_accessibility_value() {
    let start = State::new(Date::new(2026, 7, 1));
    let end = State::new(Date::new(2026, 7, 3));
    let mut tree = ViewAdapter::build_nodes(ViewAdapter::capture_root(|| {
        ViewNode::leaf(DateRangePicker::new().start(&start).end(&end))
    }));
    let root = tree.root_id().expect("date range picker root");

    start.set(Date::new(2027, 8, 12));
    end.set(Date::new(2027, 8, 20));
    let next = ViewAdapter::capture_root(|| {
        ViewNode::leaf(DateRangePicker::new().start(&start).end(&end))
    });
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let picker = tree
        .get(root)
        .expect("date range picker node")
        .component()
        .as_any()
        .downcast_ref::<DateRangePicker>()
        .expect("DateRangePicker component");
    assert_eq!(
        picker.current_range(),
        Some((Date::new(2027, 8, 12), Date::new(2027, 8, 20)))
    );
    assert!(matches!(
        picker.snapshot_fields(),
        SnapshotFields::DateRangePicker {
            start: Some(start),
            end: Some(end),
            ..
        } if start == "2027-08-12" && end == "2027-08-20"
    ));
    let accessibility =
        crate::ui::ComponentConfigSnapshot::from_component(root, picker).accessibility();
    assert_eq!(
        accessibility.state.value_text.as_deref(),
        Some("2027-08-12 / 2027-08-20")
    );
}

#[test]
fn provider_size_and_explicit_override_move_range_popup_with_the_trigger() {
    let start = State::new(Date::default());
    let end = State::new(Date::default());
    let preset = PresetDate::new(Date::new(2026, 8, 1), Date::new(2026, 8, 5));
    let large = ComponentConfig::new().component_size(ControlSize::Large);
    let max = Constraints::loose(Size::new(1_000.0, 1_000.0));
    let mut picker = with_config(&large, || {
        DateRangePicker::new()
            .start(&start)
            .end(&end)
            .presets([("release", preset)])
    });
    assert_eq!(picker.measure(max).h, 40.0);
    render_picker(&picker);

    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(4.0, 20.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(20.0, 300.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(start.get(), Date::new(2026, 8, 1));
    assert_eq!(end.get(), Date::new(2026, 8, 5));

    let small = with_config(&large, || DateRangePicker::new().size(ControlSize::Small));
    assert_eq!(small.measure(max).h, 24.0);
}

#[test]
fn keyboard_focus_and_open_state_are_exposed_to_accessibility() {
    let mut picker = DateRangePicker::new();
    assert_eq!(
        picker.snapshot_fields().accessibility().state.expanded,
        Some(false)
    );
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
}

#[test]
fn open_range_picker_is_a_popover_overlay_above_later_siblings() {
    let mut picker = DateRangePicker::new().presets([(
        "发布窗口",
        PresetDate::new(Date::new(2026, 7, 1), Date::new(2026, 7, 3)),
    )]);
    let id = ComponentId::new(11);
    let frame = Rect::new(20.0, 40.0, 280.0, 32.0);
    assert!(WidgetRender::overlay_entry(&picker, id, frame).is_none());
    assert_eq!(click(&mut picker, 4.0, 4.0), EventResult::Handled);

    let overlay = WidgetRender::overlay_entry(&picker, id, frame)
        .expect("an open range picker must paint above later siblings");
    assert_eq!(overlay.kind(), crate::ui::OverlayKind::Popover);
    assert_eq!(
        overlay.bounds_rect(),
        Some(EventHandler::hit_test_frame(&picker, frame))
    );
}

#[test]
fn hover_clear_requests_repaint_and_unrelated_keys_are_not_swallowed() {
    let mut picker =
        DateRangePicker::new().default_range(Date::new(2026, 7, 1), Date::new(2026, 7, 17));
    render_picker(&picker);
    assert_eq!(click(&mut picker, 4.0, 4.0), EventResult::Handled);
    assert_eq!(
        picker.on_event(&SystemEvent::PointerMove {
            pos: Point::new(100.0, 105.0),
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        picker.on_event(&SystemEvent::PointerMove {
            pos: Point::new(140.0, 45.0),
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

#[test]
fn constrained_trigger_and_long_preset_paint_stay_inside_overlay_bounds() {
    let mut picker = DateRangePicker::new()
        .size(ControlSize::Large)
        .default_range(Date::new(2026, 7, 1), Date::new(2026, 7, 17))
        .presets([(
            "WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW",
            PresetDate::new(Date::new(2026, 7, 1), Date::new(2026, 7, 17)),
        )]);
    let frame = Rect::new(0.0, 0.0, 80.0, 12.0);
    let _ = render_picker_pixels(&picker, frame);
    assert_eq!(click(&mut picker, 4.0, 4.0), EventResult::Handled);
    let (pixels, stride) = render_picker_pixels(&picker, frame);
    let bounds = EventHandler::hit_test_frame(&picker, frame);

    for y in 0..360usize {
        for x in 0..400usize {
            if x >= bounds.w as usize || y >= bounds.h as usize {
                assert_eq!(
                    pixels[y * stride + x],
                    0,
                    "date range picker paint leaked at ({x}, {y}); bounds={bounds:?}"
                );
            }
        }
    }
}
