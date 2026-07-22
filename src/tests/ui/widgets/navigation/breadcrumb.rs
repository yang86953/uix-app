use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::widgets::navigation::breadcrumb::*;

fn key_event(key: KeyCode) -> SystemEvent {
    SystemEvent::KeyDown {
        key,
        mods: KeyMod::NONE,
    }
}

fn pointer_down(pos: Point) -> SystemEvent {
    SystemEvent::PointerDown {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    }
}

fn breadcrumb_items(count: usize, active: usize) -> Vec<BreadcrumbItem> {
    (0..count)
        .map(|index| {
            let item = BreadcrumbItem::new(format!("Item {index}"));
            if index == active {
                item.active()
            } else {
                item
            }
        })
        .collect()
}

fn render_breadcrumb(breadcrumb: &Breadcrumb) -> String {
    let size = breadcrumb.measure(Constraints::loose(Size::new(640.0, 320.0)));
    let mut canvas = SharedRasterizer::new(PixelSurface::new(640, 320));
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
            640,
            320,
        );
        ctx.with_recorder(&mut display_list, |ctx| {
            WidgetRender::render(breadcrumb, Rect::new(0.0, 0.0, size.w, size.h), ctx, &tree);
        });
    }
    format!("{display_list:?}")
}

#[test]
fn breadcrumb_is_picture_eligible() {
    let breadcrumb = Breadcrumb::new()
        .item(BreadcrumbItem::new("Home"))
        .item(BreadcrumbItem::new("Docs").active());

    assert_eq!(breadcrumb.picture_policy(), PicturePolicy::Eligible);
}

#[test]
fn breadcrumb_keyboard_navigation_updates_active_item_and_emits_title() {
    let mut breadcrumb = Breadcrumb::new()
        .item(BreadcrumbItem::new("Home"))
        .item(BreadcrumbItem::new("Docs").active())
        .item(BreadcrumbItem::new("API"));
    let left = SystemEvent::KeyDown {
        key: KeyCode::Left,
        mods: KeyMod::NONE,
    };

    assert_eq!(WidgetComponent::tab_index(&breadcrumb), 1);
    assert_eq!(breadcrumb.active_index(), 1);
    assert_eq!(breadcrumb.on_event(&left), EventResult::Handled);
    assert_eq!(breadcrumb.active_title(), Some("Home"));
    assert_eq!(
        breadcrumb
            .semantic_event(ComponentId::new(9), &left)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("Home".to_string())
    );

    let enter = SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    };
    assert_eq!(breadcrumb.on_event(&enter), EventResult::Handled);
    assert_eq!(
        breadcrumb
            .semantic_event(ComponentId::new(9), &enter)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("Home".to_string())
    );
}

#[test]
fn breadcrumb_pointer_hit_testing_skips_separator_and_uses_custom_separator_width() {
    let mut breadcrumb = Breadcrumb::new()
        .items(vec![
            BreadcrumbItem::new("A").active(),
            BreadcrumbItem::new("B"),
        ])
        .separator(">>");
    let first_width = 15.5;
    let separator_width = 24.0;

    assert_eq!(
        breadcrumb.on_event(&SystemEvent::PointerDown {
            pos: Point::new(first_width + 1.0, 10.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    assert_eq!(
        breadcrumb.on_event(&SystemEvent::PointerDown {
            pos: Point::new(first_width + separator_width + 1.0, 10.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(breadcrumb.active_title(), Some("B"));
    assert_eq!(
        breadcrumb
            .snapshot_fields()
            .accessibility()
            .state
            .value_text
            .as_deref(),
        Some("B")
    );
}

#[test]
fn empty_breadcrumb_is_not_focusable_or_pointer_interactive() {
    let mut breadcrumb = Breadcrumb::new();

    assert_eq!(WidgetComponent::tab_index(&breadcrumb), 0);
    assert_eq!(
        breadcrumb.on_event(&SystemEvent::PointerDown {
            pos: Point::new(1.0, 1.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
}

#[test]
fn breadcrumb_icon_uses_a_real_slot_and_lucide_paint_without_overlapping_text() {
    let plain = Breadcrumb::new()
        .item(BreadcrumbItem::new("Home"))
        .item(BreadcrumbItem::new("Docs").active());
    let icon = Breadcrumb::new()
        .item(BreadcrumbItem::new("Home").icon("home"))
        .item(BreadcrumbItem::new("Docs").active());

    assert_eq!(
        icon.measure(Constraints::unconstrained()).w
            - plain.measure(Constraints::unconstrained()).w,
        20.0
    );
    let display = render_breadcrumb(&icon);
    assert!(display.contains("Home"));
    assert!(
        display.contains(crate::ui::widgets::icon::icon_char("home"))
            || display.contains("\\u{e0f5}"),
        "the Lucide home glyph must be recorded: {display}"
    );

    let (first, icon_rect, title) = icon
        .item_content_rects_for_test(0)
        .expect("visible icon item geometry");
    let icon_rect = icon_rect.expect("icon slot");
    let (second, _, _) = icon
        .item_content_rects_for_test(1)
        .expect("second item geometry");
    assert!(icon_rect.x + icon_rect.w < title.x);
    assert!(title.x + title.w <= first.x + first.w);
    assert!(first.x + first.w < second.x);
}

#[test]
fn breadcrumb_max_items_preserves_first_current_and_last_with_one_overflow_trigger() {
    let breadcrumb = Breadcrumb::new().items(breadcrumb_items(6, 3)).max_items(3);

    assert_eq!(
        breadcrumb.visible_slots_for_test(),
        vec![Some(0), None, Some(3), Some(5)]
    );
    assert_eq!(
        breadcrumb
            .overflow_rows_for_test()
            .into_iter()
            .map(|(index, _)| index)
            .collect::<Vec<_>>(),
        vec![1, 2, 4]
    );
    assert_eq!(breadcrumb.measure(Constraints::unconstrained()).h, 22.0);
}

#[test]
fn breadcrumb_overflow_pointer_selects_hidden_item_and_emits_its_title() {
    let mut breadcrumb = Breadcrumb::new().items(breadcrumb_items(6, 5)).max_items(3);
    let trigger = breadcrumb
        .overflow_trigger_rect_for_test()
        .expect("overflow trigger");

    assert_eq!(
        breadcrumb.on_event(&pointer_down(Point::new(
            trigger.x + trigger.w * 0.5,
            trigger.y + trigger.h * 0.5,
        ))),
        EventResult::Handled
    );
    assert_eq!(breadcrumb.overflow_state_for_test(), (true, Some(1)));
    assert_eq!(breadcrumb.measure(Constraints::unconstrained()).h, 106.0);
    let display = render_breadcrumb(&breadcrumb);
    assert!(display.contains("Item 1"));
    assert!(display.contains("Item 2"));
    assert!(display.contains("Item 3"));

    let (_, row) = breadcrumb
        .overflow_rows_for_test()
        .into_iter()
        .find(|(index, _)| *index == 2)
        .expect("hidden item row");
    let click = pointer_down(Point::new(row.x + 8.0, row.y + row.h * 0.5));
    assert_eq!(breadcrumb.on_event(&click), EventResult::Handled);
    assert_eq!(breadcrumb.active_title(), Some("Item 2"));
    assert_eq!(breadcrumb.overflow_state_for_test(), (false, None));
    assert_eq!(
        breadcrumb
            .semantic_event(ComponentId::new(17), &click)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("Item 2".to_string())
    );
}

#[test]
fn breadcrumb_overflow_keyboard_opens_navigates_selects_and_escapes() {
    let mut breadcrumb = Breadcrumb::new().items(breadcrumb_items(6, 5)).max_items(3);

    assert_eq!(
        breadcrumb.on_event(&key_event(KeyCode::Down)),
        EventResult::Handled
    );
    assert_eq!(breadcrumb.overflow_state_for_test(), (true, Some(1)));
    assert!(EventHandler::take_layout_request(&mut breadcrumb));
    assert!(!EventHandler::take_layout_request(&mut breadcrumb));
    breadcrumb.on_event(&key_event(KeyCode::Down));
    assert_eq!(breadcrumb.overflow_state_for_test(), (true, Some(2)));
    let enter = key_event(KeyCode::Enter);
    assert_eq!(breadcrumb.on_event(&enter), EventResult::Handled);
    assert_eq!(breadcrumb.active_title(), Some("Item 2"));
    assert_eq!(breadcrumb.overflow_state_for_test(), (false, None));
    assert_eq!(
        breadcrumb
            .semantic_event(ComponentId::new(18), &enter)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("Item 2".to_string())
    );

    assert_eq!(
        breadcrumb.on_event(&key_event(KeyCode::Up)),
        EventResult::Handled
    );
    assert_eq!(breadcrumb.overflow_state_for_test(), (true, Some(4)));
    assert_eq!(
        breadcrumb.on_event(&key_event(KeyCode::Escape)),
        EventResult::Handled
    );
    assert_eq!(breadcrumb.overflow_state_for_test(), (false, None));
    assert!(EventHandler::take_layout_request(&mut breadcrumb));
    assert!(!EventHandler::take_layout_request(&mut breadcrumb));
    assert_eq!(breadcrumb.active_title(), Some("Item 2"));
}

#[test]
fn breadcrumb_overflow_closes_on_external_exit_and_reconcile_clears_transient_state() {
    let mut breadcrumb = Breadcrumb::new().items(breadcrumb_items(6, 5)).max_items(3);

    breadcrumb.on_event(&key_event(KeyCode::Down));
    assert_eq!(
        breadcrumb.on_event(&SystemEvent::PointerLeave),
        EventResult::Handled
    );
    assert_eq!(breadcrumb.overflow_state_for_test(), (false, None));

    breadcrumb.on_event(&key_event(KeyCode::Down));
    assert_eq!(
        breadcrumb.on_event(&SystemEvent::FocusOut),
        EventResult::Handled
    );
    assert_eq!(breadcrumb.overflow_state_for_test(), (false, None));

    breadcrumb.on_event(&key_event(KeyCode::Down));
    assert_eq!(
        breadcrumb.on_event(&SystemEvent::WindowBlur),
        EventResult::Handled
    );
    assert_eq!(breadcrumb.overflow_state_for_test(), (false, None));

    breadcrumb.on_event(&key_event(KeyCode::Down));
    assert_eq!(
        breadcrumb.on_event(&pointer_down(Point::new(600.0, 250.0))),
        EventResult::Handled
    );
    assert_eq!(breadcrumb.overflow_state_for_test(), (false, None));

    breadcrumb.on_event(&key_event(KeyCode::Down));
    breadcrumb.on_event(&key_event(KeyCode::Enter));
    breadcrumb.sync_from(Breadcrumb::new().items(breadcrumb_items(6, 0)).max_items(3));
    assert_eq!(breadcrumb.overflow_state_for_test(), (false, None));
    assert_eq!(breadcrumb.active_title(), Some("Item 1"));
    assert!(
        breadcrumb
            .semantic_event(ComponentId::new(19), &key_event(KeyCode::Enter))
            .is_none(),
        "reconcile must clear a stale pending overflow selection"
    );
}

#[test]
fn breadcrumb_small_max_items_clamps_to_the_required_path_anchors() {
    for max_items in [1, 2] {
        let breadcrumb = Breadcrumb::new()
            .items(breadcrumb_items(6, 3))
            .max_items(max_items);
        assert_eq!(
            breadcrumb.visible_slots_for_test(),
            vec![Some(0), None, Some(3), Some(5)]
        );
    }

    for max_items in [0, 6, 10] {
        let mut breadcrumb = Breadcrumb::new()
            .items(breadcrumb_items(6, 5))
            .max_items(max_items);
        assert_eq!(
            breadcrumb.visible_slots_for_test(),
            (0..6).map(Some).collect::<Vec<_>>()
        );
        assert_eq!(
            breadcrumb.on_event(&key_event(KeyCode::Down)),
            EventResult::NotHandled
        );
        assert_eq!(breadcrumb.overflow_state_for_test(), (false, None));
    }
}
