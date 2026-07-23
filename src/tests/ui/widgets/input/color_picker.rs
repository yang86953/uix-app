use crate::draw::backend::cpu::canvas_2d::CpuCanvas2D;
use crate::draw::backend::cpu::pixel_surface::PixelSurface;
use crate::draw::backend::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::geometry::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::state::State;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::ColorPicker;
use crate::ui::{with_config, ComponentConfig};

fn render_picker(picker: &ColorPicker) {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(240, 280));
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
        240,
        280,
    );
    let size = picker.measure(Constraints::loose(Size::new(240.0, 280.0)));
    WidgetRender::render(picker, Rect::new(0.0, 0.0, size.w, size.h), &mut ctx, &tree);
}

fn render_picker_pixels(picker: &ColorPicker, frame: Rect) -> (Vec<u32>, usize) {
    const WIDTH: i32 = 280;
    const HEIGHT: i32 = 180;
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

fn left_click(picker: &mut ColorPicker, x: f32, y: f32) -> EventResult {
    picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(x, y),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    })
}

fn key_down(picker: &mut ColorPicker, key: KeyCode) -> EventResult {
    picker.on_event(&SystemEvent::KeyDown {
        key,
        mods: KeyMod::NONE,
    })
}

#[test]
fn bound_color_picker_writes_preset_selection_and_reads_external_updates() {
    let selected = State::new(Color::blue());
    let mut picker = ColorPicker::new().value(&selected);
    render_picker(&picker);

    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(4.0, 4.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert!(picker.is_open());

    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(9.0, 49.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let first_preset = Color::from_rgb(0xF5, 0x22, 0x22);
    assert_eq!(selected.get(), first_preset);
    assert_eq!(picker.current_value(), first_preset);

    selected.set(Color::green());
    picker.sync_from(ColorPicker::new().value(&selected));
    assert_eq!(picker.current_value(), Color::green());
}

#[test]
fn uncontrolled_color_picker_keeps_its_runtime_value_during_reconcile() {
    let mut picker = ColorPicker::new().default_value(Color::red());
    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(4.0, 4.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(33.0, 49.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let runtime_value = picker.current_value();

    picker.sync_from(ColorPicker::new().default_value(Color::blue()));
    assert_eq!(picker.current_value(), runtime_value);
}

#[test]
fn external_color_state_reconciles_and_updates_semantic_value() {
    let selected = State::new(Color::red());
    let mut tree = ViewAdapter::build_nodes(ViewAdapter::capture_root(|| {
        ViewNode::leaf(ColorPicker::new().value(&selected))
    }));
    let root = tree.root_id().expect("color picker root");
    tree.reset_invalidation();

    selected.set(Color::from_rgba(0x16, 0x77, 0xFF, 0x80));
    assert!(tree.take_reconcile_requested());
    let next = ViewAdapter::capture_root(|| ViewNode::leaf(ColorPicker::new().value(&selected)));
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let picker = tree
        .get(root)
        .expect("color picker node")
        .component()
        .as_any()
        .downcast_ref::<ColorPicker>()
        .expect("ColorPicker component");
    assert_eq!(picker.current_value(), selected.get());
    assert!(matches!(
        picker.snapshot_fields(),
        SnapshotFields::ColorPicker { value, .. } if value == selected.get()
    ));
    let accessibility =
        crate::ui::ComponentConfigSnapshot::from_component(root, picker).accessibility();
    assert_eq!(accessibility.state.value_text.as_deref(), Some("#1677FF80"));
    assert!(tree
        .invalidation()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .node_needs_paint(root));
}

#[test]
fn provider_size_and_explicit_override_move_color_popup_with_the_trigger() {
    let selected = State::new(Color::blue());
    let large = ComponentConfig::new().component_size(ControlSize::Large);
    let max = Constraints::loose(Size::new(1_000.0, 1_000.0));
    let mut picker = with_config(&large, || ColorPicker::new().value(&selected));
    assert_eq!(picker.measure(max), Size::new(40.0, 40.0));
    render_picker(&picker);

    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(4.0, 20.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(9.0, 53.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(selected.get(), Color::from_rgb(0xF5, 0x22, 0x22));

    let small = with_config(&large, || ColorPicker::new().size(ControlSize::Small));
    assert_eq!(small.measure(max), Size::new(24.0, 24.0));
}

#[test]
fn constrained_trigger_paints_and_hits_only_inside_its_actual_frame() {
    let mut picker = ColorPicker::new()
        .size(ControlSize::Large)
        .default_value(Color::from_rgb(0x16, 0x77, 0xFF));
    let frame = Rect::new(10.0, 10.0, 12.0, 6.0);
    let (pixels, stride) = render_picker_pixels(&picker, frame);

    for y in 0..180usize {
        for x in 0..280usize {
            if !(10..22).contains(&x) || !(10..16).contains(&y) {
                assert_eq!(
                    pixels[y * stride + x],
                    0,
                    "closed constrained ColorPicker leaked paint at ({x}, {y})"
                );
            }
        }
    }

    assert_eq!(left_click(&mut picker, 20.0, 2.0), EventResult::NotHandled);
    assert!(!picker.is_open());
}

#[test]
fn open_state_and_popover_overlay_are_exposed_to_accessibility() {
    let mut picker = ColorPicker::new();
    let id = ComponentId::new(14);
    let frame = Rect::new(20.0, 40.0, 12.0, 6.0);
    assert_eq!(
        picker.snapshot_fields().accessibility().state.expanded,
        Some(false)
    );
    assert!(WidgetRender::overlay_entry(&picker, id, frame).is_none());

    picker.open();
    assert_eq!(
        picker.snapshot_fields().accessibility().state.expanded,
        Some(true)
    );
    let overlay = WidgetRender::overlay_entry(&picker, id, frame)
        .expect("an open color palette must paint above later siblings");
    assert_eq!(overlay.kind(), crate::ui::OverlayKind::Popover);
    assert_eq!(
        overlay.bounds_rect(),
        Some(EventHandler::hit_test_frame(&picker, frame))
    );
}

#[test]
fn keyboard_moves_across_the_palette_grid_and_commits_the_highlight() {
    let selected = State::new(Color::from_rgb(0xF5, 0x22, 0x22));
    let mut picker = ColorPicker::new().value(&selected);
    assert_eq!(picker.on_event(&SystemEvent::FocusIn), EventResult::Handled);
    for key in [
        KeyCode::Enter,
        KeyCode::Right,
        KeyCode::Down,
        KeyCode::Enter,
    ] {
        assert_eq!(key_down(&mut picker, key), EventResult::Handled);
    }

    assert_eq!(selected.get(), Color::from_rgb(0xEB, 0x2F, 0x96));
    assert!(!picker.is_open());
}

#[test]
fn palette_right_padding_neither_previews_nor_selects_the_next_row() {
    let mut picker = ColorPicker::new().default_value(Color::from_rgb(1, 2, 3));
    render_picker(&picker);
    assert_eq!(left_click(&mut picker, 4.0, 4.0), EventResult::Handled);
    let _ = WidgetAnimation::update_animation(&mut picker, 1.0);
    let frame = Rect::new(0.0, 0.0, 32.0, 32.0);
    let (before, _) = render_picker_pixels(&picker, frame);

    assert_eq!(
        picker.on_event(&SystemEvent::PointerMove {
            pos: Point::new(200.0, 49.0),
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    let (after, _) = render_picker_pixels(&picker, frame);
    assert_eq!(
        after, before,
        "right padding must not highlight preset index 8"
    );
}

#[test]
fn selected_preset_has_a_visible_marker_inside_the_palette() {
    let mut selected = ColorPicker::new().default_value(Color::from_rgb(0xF5, 0x22, 0x22));
    let mut custom = ColorPicker::new().default_value(Color::from_rgb(1, 2, 3));
    selected.open();
    custom.open();
    let _ = WidgetAnimation::update_animation(&mut selected, 1.0);
    let _ = WidgetAnimation::update_animation(&mut custom, 1.0);

    let frame = Rect::new(0.0, 0.0, 32.0, 32.0);
    let (selected_pixels, stride) = render_picker_pixels(&selected, frame);
    let (custom_pixels, _) = render_picker_pixels(&custom, frame);
    let panel_differs = (36..124usize).any(|y| {
        (0..208usize).any(|x| selected_pixels[y * stride + x] != custom_pixels[y * stride + x])
    });
    assert!(
        panel_differs,
        "the current preset needs a marker independent of the trigger swatch"
    );
}

#[test]
fn transparent_value_uses_a_checkerboard_instead_of_looking_empty() {
    let picker = ColorPicker::new().default_value(Color::transparent());
    let (pixels, stride) = render_picker_pixels(&picker, Rect::new(0.0, 0.0, 32.0, 32.0));
    let mut interior_colors = std::collections::BTreeSet::new();
    for y in 6..26usize {
        for x in 6..26usize {
            let pixel = pixels[y * stride + x];
            if pixel != 0 {
                interior_colors.insert(pixel);
            }
        }
    }
    assert!(
        interior_colors.len() >= 2,
        "transparent colors need a two-tone checkerboard, got {interior_colors:?}"
    );
}

#[test]
fn pointer_leave_and_unrelated_motion_only_handle_visible_state_changes() {
    let mut picker = ColorPicker::new();
    assert_eq!(
        picker.on_event(&SystemEvent::PointerMove {
            pos: Point::new(200.0, 100.0),
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    assert_eq!(
        picker.on_event(&SystemEvent::PointerLeave),
        EventResult::NotHandled
    );
    assert_eq!(key_down(&mut picker, KeyCode::A), EventResult::NotHandled);
}
