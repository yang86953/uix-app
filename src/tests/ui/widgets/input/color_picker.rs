use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
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
