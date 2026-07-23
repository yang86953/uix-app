use crate::draw::backend::cpu::pixel_surface::PixelSurface;
use crate::draw::backend::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::geometry::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::state::State;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::navigation::tabs::*;
use crate::ui::LayoutChild;

fn render_tabs(tabs: &Tabs, frame: Rect) -> String {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(640, 320));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let tree = WidgetTree::new();
    let mut display_list = crate::draw::command::DisplayList::new();
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
            640,
            320,
        );
        ctx.with_recorder(&mut display_list, |ctx| {
            WidgetRender::render(tabs, frame, ctx, &tree);
        });
    }
    format!("{display_list:?}")
}

fn key_event(key: KeyCode) -> SystemEvent {
    SystemEvent::KeyDown {
        key,
        mods: KeyMod::NONE,
    }
}

#[test]
fn tabs_are_focusable_and_keyboard_navigation_wraps() {
    let mut tabs = Tabs::new()
        .tab("Overview", "overview")
        .tab("Details", "details")
        .active(1);

    assert_eq!(WidgetComponent::tab_index(&tabs), 1);
    assert_eq!(tabs.on_event(&SystemEvent::FocusIn), EventResult::Handled);
    tabs.on_event(&key_event(KeyCode::Right));
    assert_eq!(tabs.current_key(), Some("overview"));
    tabs.on_event(&key_event(KeyCode::Left));
    assert_eq!(tabs.current_key(), Some("details"));
    tabs.on_event(&key_event(KeyCode::Home));
    assert_eq!(tabs.active_index(), 0);
    tabs.on_event(&key_event(KeyCode::End));
    assert_eq!(tabs.active_index(), 1);
}

#[test]
fn tabs_keyboard_change_emits_active_key_and_snapshot_exposes_state() {
    let mut tabs = Tabs::new()
        .tab("Overview", "overview")
        .tab("Details", "details");
    let event = key_event(KeyCode::Right);
    tabs.on_event(&event);

    let semantic = tabs
        .semantic_event(ComponentId::new(4), &event)
        .expect("tab change should emit semantic event");
    assert_eq!(semantic.text_payload(), Some("details"));
    assert!(matches!(
        tabs.snapshot_fields(),
        SnapshotFields::Tabs {
            active_index: 1,
            tabs,
            ..
        } if tabs[1].key == "details"
    ));
    assert_eq!(
        tabs.snapshot_fields()
            .accessibility()
            .state
            .value_text
            .as_deref(),
        Some("Details")
    );
}

#[test]
fn tabs_reconcile_clamps_preserved_selection_when_items_shrink() {
    let mut tabs = Tabs::new()
        .tab("One", "one")
        .tab("Two", "two")
        .tab("Three", "three")
        .active(2);

    tabs.sync_from(Tabs::new().tab("Only", "only"));

    assert_eq!(tabs.active_index(), 0);
    assert_eq!(tabs.current_key(), Some("only"));
}

#[test]
fn string_active_key_binding_reads_and_writes_state() {
    let active = State::new("details".to_string());
    let mut tabs = Tabs::new()
        .tab("Overview", "overview")
        .tab("Details", "details")
        .active_key(&active);

    assert_eq!(tabs.current_key(), Some("details"));
    tabs.on_event(&key_event(KeyCode::Right));
    assert_eq!(tabs.current_key(), Some("overview"));
    assert_eq!(active.get(), "overview");

    active.set("details".to_string());
    tabs.on_event(&SystemEvent::FocusIn);
    assert_eq!(tabs.current_key(), Some("details"));
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TestPage {
    Overview,
    Details,
    Missing,
}

impl std::fmt::Display for TestPage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Overview => "overview",
            Self::Details => "details",
            Self::Missing => "missing",
        })
    }
}

#[test]
fn controlled_tabs_keep_typed_keys_in_state_and_string_keys_in_snapshots() {
    let active = State::new(TestPage::Details);
    let mut tabs = Tabs::controlled(
        [
            ("Overview", TestPage::Overview),
            ("Details", TestPage::Details),
        ],
        &active,
    );

    assert_eq!(tabs.current_key(), Some("details"));
    tabs.on_event(&key_event(KeyCode::Left));
    assert_eq!(active.get(), TestPage::Overview);
    assert!(matches!(
        tabs.snapshot_fields(),
        SnapshotFields::Tabs {
            active_key: Some(key),
            active_index: 0,
            ..
        } if key == "overview"
    ));
}

#[test]
fn controlled_tabs_recover_from_an_unmatched_value_on_navigation() {
    let active = State::new(TestPage::Missing);
    let mut tabs = Tabs::controlled(
        [
            ("Overview", TestPage::Overview),
            ("Details", TestPage::Details),
        ],
        &active,
    );

    assert_eq!(tabs.current_key(), None);
    tabs.on_event(&key_event(KeyCode::Right));
    assert_eq!(tabs.current_key(), Some("overview"));
    assert_eq!(active.get(), TestPage::Overview);
}

#[test]
fn uncontrolled_tabs_reconcile_preserves_the_active_key_across_reordering() {
    let mut tabs = Tabs::new()
        .tab("Overview", "overview")
        .tab("Details", "details")
        .active(1);

    tabs.sync_from(
        Tabs::new()
            .tab("Details", "details")
            .tab("Overview", "overview"),
    );

    assert_eq!(tabs.active_index(), 0);
    assert_eq!(tabs.current_key(), Some("details"));
}

#[test]
fn external_tab_state_requests_reconcile_and_updates_the_live_component() {
    let active = State::new("overview".to_string());
    let mut tree = ViewAdapter::build_nodes(ViewAdapter::capture_root(|| {
        ViewNode::leaf(
            Tabs::new()
                .tab("Overview", "overview")
                .tab("Details", "details")
                .active_key(&active),
        )
    }));
    let root = tree.root_id().expect("tabs root");
    tree.reset_invalidation();

    active.set("details".to_string());
    assert!(tree.take_reconcile_requested());
    let next = ViewAdapter::capture_root(|| {
        ViewNode::leaf(
            Tabs::new()
                .tab("Overview", "overview")
                .tab("Details", "details")
                .active_key(&active),
        )
    });
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let tabs = tree
        .get(root)
        .expect("live tabs")
        .component()
        .as_any()
        .downcast_ref::<Tabs>()
        .expect("Tabs component");
    assert_eq!(tabs.current_key(), Some("details"));
}

#[test]
fn left_and_right_positions_use_a_vertical_tab_bar_and_reserve_content_width() {
    let children = [
        LayoutChild::new(ComponentId::new(11), Size::new(10.0, 10.0)),
        LayoutChild::new(ComponentId::new(12), Size::new(10.0, 10.0)),
    ];
    let frame = Rect::new(10.0, 20.0, 400.0, 200.0);

    let mut left = Tabs::new()
        .tab("Overview", "overview")
        .tab("Details", "details")
        .active(1)
        .tab_position(TabPosition::Left);
    let display = render_tabs(&left, Rect::new(30.0, 40.0, 400.0, 200.0));
    assert!(display.contains("Overview"));
    assert_eq!(
        left.tab_rect_for_test(0),
        Some(Rect::new(0.0, 0.0, 160.0, 40.0))
    );
    assert_eq!(
        left.tab_rect_for_test(1),
        Some(Rect::new(0.0, 40.0, 160.0, 40.0))
    );
    assert_eq!(
        WidgetLayout::layout_children(&left, frame, &children, &WidgetTree::new()),
        vec![(ComponentId::new(12), Rect::new(186.0, 28.0, 208.0, 184.0))]
    );
    assert_eq!(
        left.on_event(&SystemEvent::PointerDown {
            pos: Point::new(80.0, 20.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(left.current_key(), Some("overview"));

    let mut right = Tabs::new()
        .tab("Overview", "overview")
        .tab("Details", "details")
        .active(1)
        .tab_position(TabPosition::Right);
    render_tabs(&right, Rect::new(30.0, 40.0, 400.0, 200.0));
    assert_eq!(
        right.tab_rect_for_test(0),
        Some(Rect::new(240.0, 0.0, 160.0, 40.0))
    );
    assert_eq!(
        WidgetLayout::layout_children(&right, frame, &children, &WidgetTree::new()),
        vec![(ComponentId::new(12), Rect::new(26.0, 28.0, 208.0, 184.0))]
    );
    assert_eq!(
        right.on_event(&SystemEvent::PointerDown {
            pos: Point::new(300.0, 20.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(right.current_key(), Some("overview"));
}

#[test]
fn scrollable_tabs_wheel_and_keyboard_keep_the_active_tab_visible() {
    let mut horizontal = Tabs::new()
        .tab("Overview alpha", "a")
        .tab("Details beta", "b")
        .tab("History gamma", "c")
        .tab("Settings delta", "d")
        .scrollable(true)
        .size(180.0, 100.0);
    render_tabs(&horizontal, Rect::new(50.0, 60.0, 180.0, 100.0));
    assert_eq!(horizontal.tab_scroll_offset(), 0.0);
    assert_eq!(
        horizontal.on_event(&SystemEvent::Wheel {
            pos: Point::new(90.0, 20.0),
            delta: Point::new(0.0, 1.0),
        }),
        EventResult::Handled
    );
    assert!(horizontal.tab_scroll_offset() > 0.0);
    assert_eq!(
        horizontal.on_event(&SystemEvent::Wheel {
            pos: Point::new(90.0, 80.0),
            delta: Point::new(0.0, 1.0),
        }),
        EventResult::NotHandled,
        "wheel outside the tab strip must keep bubbling"
    );

    horizontal.on_event(&key_event(KeyCode::End));
    let last = horizontal.tab_rect_for_test(3).expect("last tab geometry");
    assert!(last.x < 180.0 && last.x + last.w > 0.0);
    assert_eq!(horizontal.current_key(), Some("d"));

    let mut vertical = Tabs::new()
        .tab("One", "one")
        .tab("Two", "two")
        .tab("Three", "three")
        .tab("Four", "four")
        .tab_position(TabPosition::Right)
        .scrollable(true)
        .size(220.0, 90.0);
    render_tabs(&vertical, Rect::new(40.0, 30.0, 220.0, 90.0));
    assert_eq!(
        vertical.on_event(&SystemEvent::Wheel {
            pos: Point::new(100.0, 45.0),
            delta: Point::new(0.0, 1.0),
        }),
        EventResult::Handled
    );
    assert_eq!(vertical.tab_scroll_offset(), 40.0);
    vertical.on_event(&key_event(KeyCode::End));
    let last = vertical
        .tab_rect_for_test(3)
        .expect("last vertical tab geometry");
    assert!(last.y < 90.0 && last.y + last.h > 0.0);

    vertical.sync_from(
        Tabs::new()
            .tab("One", "one")
            .tab("Two", "two")
            .tab_position(TabPosition::Top)
            .scrollable(true),
    );
    assert_eq!(vertical.tab_scroll_offset(), 0.0);
}

#[test]
fn editable_tabs_activate_real_add_and_close_targets_including_an_empty_bar() {
    let added = std::rc::Rc::new(std::cell::Cell::new(0usize));
    let closed = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let mut tabs = Tabs::editable()
        .tab("Alpha", "alpha")
        .tab("Beta", "beta")
        .on_add({
            let added = added.clone();
            move |_| added.set(added.get() + 1)
        })
        .on_close({
            let closed = closed.clone();
            move |key| closed.borrow_mut().push(key.to_string())
        });
    render_tabs(&tabs, Rect::new(20.0, 20.0, 320.0, 100.0));

    let close = tabs.tab_rect_for_test(0).expect("first editable tab");
    assert_eq!(
        tabs.on_event(&SystemEvent::PointerDown {
            pos: Point::new(close.x + close.w - 5.0, close.y + close.h * 0.5),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(closed.borrow().as_slice(), &["alpha"]);
    assert_eq!(tabs.current_key(), Some("beta"));

    render_tabs(&tabs, Rect::new(20.0, 20.0, 320.0, 100.0));
    let add = tabs.add_rect_for_test();
    assert_eq!(
        tabs.on_event(&SystemEvent::PointerDown {
            pos: Point::new(add.x + add.w * 0.5, add.y + add.h * 0.5),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(added.get(), 1);

    let mut empty = Tabs::editable().on_add({
        let added = added.clone();
        move |_| added.set(added.get() + 1)
    });
    render_tabs(&empty, Rect::new(0.0, 0.0, 120.0, 80.0));
    let empty_add = empty.add_rect_for_test();
    assert_eq!(
        empty.on_event(&SystemEvent::PointerDown {
            pos: Point::new(empty_add.x + 4.0, empty_add.y + 4.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(added.get(), 2);
}

#[test]
fn tab_icons_consume_real_paint_geometry_and_vertical_keys_follow_the_main_axis() {
    let plain = Tabs::new().tabs(vec![Tab::new("Home")]);
    let icon = Tabs::new().tabs(vec![Tab::new("Home").icon("home")]);
    render_tabs(&plain, Rect::new(0.0, 0.0, 240.0, 100.0));
    let icon_display = render_tabs(&icon, Rect::new(0.0, 0.0, 240.0, 100.0));
    assert!(
        icon.tab_rect_for_test(0).expect("icon tab").w
            > plain.tab_rect_for_test(0).expect("plain tab").w
    );
    assert!(icon_display.contains("Home"));

    let mut vertical = Tabs::new()
        .tab("One", "one")
        .tab("Two", "two")
        .tab_position(TabPosition::Left);
    vertical.on_event(&key_event(KeyCode::Down));
    assert_eq!(vertical.current_key(), Some("two"));
    vertical.on_event(&key_event(KeyCode::Up));
    assert_eq!(vertical.current_key(), Some("one"));
}
