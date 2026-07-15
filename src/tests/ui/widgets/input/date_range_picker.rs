use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::state::State;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::{Date, DateRangePicker, PresetDate, Weekday};
use crate::ui::{with_config, ComponentConfig};

fn render_picker(picker: &DateRangePicker) {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(320, 420));
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
        420,
    );
    let size = picker.measure(Constraints::loose(Size::new(320.0, 420.0)));
    WidgetRender::render(picker, Rect::new(0.0, 0.0, size.w, size.h), &mut ctx, &tree);
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
    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(100.0, 81.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert!(picker.is_selecting_end());
    assert_eq!(start.get(), Date::new(2026, 7, 10));

    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(210.0, 81.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert!(picker.is_selecting_end());
    assert_eq!(end.get(), Date::new(2026, 7, 12));

    let _ = picker.on_event(&SystemEvent::PointerDown {
        pos: Point::new(175.0, 81.0),
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
