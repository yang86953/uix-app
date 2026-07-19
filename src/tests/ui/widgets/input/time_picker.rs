use crate::draw::engine::cpu::canvas_2d::CpuCanvas2D;
use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::state::State;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::{Time, TimePicker};
use crate::ui::{with_config, ComponentConfig};

fn render_picker(picker: &TimePicker) {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(300, 320));
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
        300,
        320,
    );
    let size = picker.measure(Constraints::loose(Size::new(240.0, 280.0)));
    WidgetRender::render(
        picker,
        Rect::new(28.0, 16.0, size.w, size.h),
        &mut ctx,
        &tree,
    );
}

fn render_picker_pixels(picker: &TimePicker, frame: Rect) -> (Vec<u32>, usize) {
    const WIDTH: i32 = 300;
    const HEIGHT: i32 = 280;
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

fn click(picker: &mut TimePicker, x: f32, y: f32) -> EventResult {
    picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(x, y),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    })
}

#[test]
fn bound_time_picker_writes_pointer_selection_and_reads_external_updates() {
    let selected = State::new(Time::new(1, 10));
    let mut picker = TimePicker::new().value(&selected);
    render_picker(&picker);

    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(4.0, 4.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert!(picker.is_open());
    let trigger = Rect::new(28.0, 16.0, 120.0, 32.0);
    let hit_frame = EventHandler::hit_test_frame(&picker, trigger);
    assert!(hit_frame.contains(Point::new(36.0, 180.0)));
    assert_eq!(WidgetRender::dirty_rect(&picker, trigger), hit_frame);

    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(16.0, 102.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(selected.get(), Time::new(2, 10));
    assert_eq!(picker.current_value(), Time::new(2, 10));

    selected.set(Time::new(23, 55));
    picker.sync_from(TimePicker::new().value(&selected));
    assert_eq!(picker.current_value(), Time::new(23, 55));
}

#[test]
fn time_normalizes_fields_and_now_is_a_valid_utc_minute() {
    assert_eq!(Time::new(24, 60), Time::new(23, 59));
    let now = Time::now();
    assert!(now.hour < 24);
    assert!(now.minute < 60);
}

#[test]
fn configured_midnight_is_not_treated_as_an_empty_value() {
    let picker = TimePicker::new().default_value(Time::new(0, 0));
    assert!(matches!(
        picker.snapshot_fields(),
        SnapshotFields::TimePicker {
            value: Some(value),
            ..
        } if value == "00:00"
    ));
}

#[test]
fn external_time_state_reconciles_and_updates_semantic_value() {
    let selected = State::new(Time::new(9, 30));
    let mut tree = ViewAdapter::build_nodes(ViewAdapter::capture_root(|| {
        ViewNode::leaf(TimePicker::new().value(&selected))
    }));
    let root = tree.root_id().expect("time picker root");
    tree.reset_invalidation();

    selected.set(Time::new(10, 45));
    assert!(tree.take_reconcile_requested());
    let next = ViewAdapter::capture_root(|| ViewNode::leaf(TimePicker::new().value(&selected)));
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let picker = tree
        .get(root)
        .expect("time picker node")
        .component()
        .as_any()
        .downcast_ref::<TimePicker>()
        .expect("TimePicker component");
    assert_eq!(picker.current_value(), Time::new(10, 45));
    let accessibility =
        crate::ui::ComponentConfigSnapshot::from_component(root, picker).accessibility();
    assert_eq!(accessibility.state.value_text.as_deref(), Some("10:45"));
    assert!(tree
        .invalidation()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .node_needs_paint(root));
}

#[test]
fn provider_size_and_explicit_override_move_time_popup_with_the_trigger() {
    let selected = State::new(Time::new(1, 10));
    let large = ComponentConfig::new().component_size(ControlSize::Large);
    let max = Constraints::loose(Size::new(1_000.0, 1_000.0));
    let mut picker = with_config(&large, || TimePicker::new().value(&selected));
    assert_eq!(picker.measure(max).h, 40.0);
    render_picker(&picker);

    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(4.0, 20.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(16.0, 110.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(selected.get(), Time::new(2, 10));

    let small = with_config(&large, || TimePicker::new().size(ControlSize::Small));
    assert_eq!(small.measure(max).h, 24.0);
}

#[test]
fn minute_column_exposes_every_minute_instead_of_only_five_minute_steps() {
    let selected = State::new(Time::new(8, 0));
    let mut picker = TimePicker::new().value(&selected);
    render_picker(&picker);
    assert_eq!(click(&mut picker, 4.0, 4.0), EventResult::Handled);
    assert_eq!(click(&mut picker, 90.0, 146.0), EventResult::Handled);
    assert_eq!(selected.get(), Time::new(8, 3));
}

#[test]
fn opening_late_time_scrolls_both_columns_to_the_selected_rows() {
    let selected = State::new(Time::new(23, 55));
    let mut picker = TimePicker::new().value(&selected);
    render_picker(&picker);
    assert_eq!(click(&mut picker, 4.0, 4.0), EventResult::Handled);
    assert_eq!(click(&mut picker, 90.0, 134.0), EventResult::Handled);
    assert_eq!(selected.get(), Time::new(23, 55));
    assert!(!picker.is_open());
}

#[test]
fn minute_column_wheel_reaches_the_last_minute() {
    let selected = State::new(Time::new(8, 0));
    let mut picker = TimePicker::new().value(&selected);
    render_picker(&picker);
    assert_eq!(click(&mut picker, 4.0, 4.0), EventResult::Handled);
    assert_eq!(
        picker.on_event(&SystemEvent::Wheel {
            pos: Point::new(90.0, 100.0),
            delta: Point::new(0.0, 100.0),
        }),
        EventResult::Handled
    );
    assert_eq!(click(&mut picker, 90.0, 218.0), EventResult::Handled);
    assert_eq!(selected.get(), Time::new(8, 59));
}

#[test]
fn keyboard_switches_columns_changes_highlight_and_commits_with_enter() {
    let selected = State::new(Time::new(9, 58));
    let mut picker = TimePicker::new().value(&selected);
    assert_eq!(picker.on_event(&SystemEvent::FocusIn), EventResult::Handled);
    assert_eq!(
        picker.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    for key in [KeyCode::Right, KeyCode::Down, KeyCode::Enter] {
        assert_eq!(
            picker.on_event(&SystemEvent::KeyDown {
                key,
                mods: KeyMod::NONE,
            }),
            EventResult::Handled
        );
    }
    assert_eq!(selected.get(), Time::new(9, 59));
    assert!(!picker.is_open());
}

#[test]
fn focus_open_state_and_popover_overlay_are_observable() {
    let mut picker = TimePicker::new();
    let id = ComponentId::new(13);
    let frame = Rect::new(20.0, 40.0, 80.0, 12.0);
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
    assert_eq!(
        picker.snapshot_fields().accessibility().state.expanded,
        Some(true)
    );
    let overlay = WidgetRender::overlay_entry(&picker, id, frame)
        .expect("an open time picker must paint above later siblings");
    assert_eq!(overlay.kind(), crate::ui::OverlayKind::Popover);
    assert_eq!(
        overlay.bounds_rect(),
        Some(EventHandler::hit_test_frame(&picker, frame))
    );
    assert!(overlay
        .bounds_rect()
        .is_some_and(|bounds| bounds.w >= 120.0));
}

#[test]
fn constrained_trigger_and_scrolled_popup_paint_stay_inside_overlay_bounds() {
    let mut picker = TimePicker::new()
        .size(ControlSize::Large)
        .placeholder("WWWWWWWWWWWWWWWWWWWWWWWW")
        .default_value(Time::new(23, 59));
    let frame = Rect::new(0.0, 0.0, 60.0, 10.0);
    let _ = render_picker_pixels(&picker, frame);
    assert_eq!(click(&mut picker, 4.0, 4.0), EventResult::Handled);
    let (pixels, stride) = render_picker_pixels(&picker, frame);
    let bounds = EventHandler::hit_test_frame(&picker, frame);

    for y in 0..280usize {
        for x in 0..300usize {
            if x >= bounds.w as usize || y >= bounds.h as usize {
                assert_eq!(
                    pixels[y * stride + x],
                    0,
                    "time picker paint leaked at ({x}, {y}); bounds={bounds:?}"
                );
            }
        }
    }
}

#[test]
fn pointer_leave_clears_preview_and_unrelated_keys_are_not_swallowed() {
    let mut picker = TimePicker::new().default_value(Time::new(1, 10));
    render_picker(&picker);
    assert_eq!(click(&mut picker, 4.0, 4.0), EventResult::Handled);
    assert_eq!(
        picker.on_event(&SystemEvent::PointerMove {
            pos: Point::new(16.0, 102.0),
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        picker.on_event(&SystemEvent::PointerLeave),
        EventResult::Handled
    );
    assert_eq!(
        picker.on_event(&SystemEvent::KeyDown {
            key: KeyCode::A,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
}
