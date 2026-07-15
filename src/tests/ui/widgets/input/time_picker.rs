use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::state::State;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::{Time, TimePicker};
use crate::ui::{with_config, ComponentConfig};

fn render_picker(picker: &TimePicker) {
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
