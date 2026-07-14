use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::state::State;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::{Date, DatePicker};

fn render_picker(picker: &DatePicker) {
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
    WidgetRender::render(picker, Rect::new(0.0, 0.0, 160.0, 32.0), &mut ctx, &tree);
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

    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(80.0, 81.0),
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
