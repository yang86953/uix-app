use crate::draw::backend::cpu::pixel_surface::PixelSurface;
use crate::draw::backend::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::geometry::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::core::widget::WidgetCore;
use crate::ui::traits::WidgetTextInput;
use crate::ui::widgets::navigation::pagination::*;

fn render_pagination(pagination: &Pagination) -> String {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(640, 64));
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
            64,
        );
        ctx.with_recorder(&mut display_list, |ctx| {
            WidgetRender::render(pagination, Rect::new(0.0, 0.0, 640.0, 40.0), ctx, &tree);
        });
    }
    format!("{display_list:?}")
}

#[test]
fn pagination_click_emits_change_semantic_event() {
    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(Pagination::new(85, 10)));
    tree.get_mut(id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 260.0, 40.0));

    let page = Rc::new(RefCell::new(String::new()));
    let page_for_handler = page.clone();
    tree.handler_table()
        .on(id, SemanticKind::Change, move |event| {
            if let Some(value) = event.text_payload() {
                *page_for_handler.borrow_mut() = value.to_string();
            }
        });

    tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(70.0, 16.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(&*page.borrow(), "2");
}

#[test]
fn pagination_normalizes_zero_page_size_and_extreme_current() {
    let mut pagination = Pagination::new(100, 0).page_size(0).current(usize::MAX);

    assert_eq!(pagination.total_pages(), 100);
    assert_eq!(pagination.get_current(), 100);
    assert_eq!(
        pagination.visible_range(100, pagination.get_current()),
        vec![1, 0, 99, 100]
    );

    assert_eq!(
        pagination.on_event(&SystemEvent::PointerDown {
            pos: Point::new(170.0, 0.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(pagination.get_current(), 100);

    assert_eq!(Pagination::new(100, 10).current(0).get_current(), 1);
}

#[test]
fn pagination_is_focusable_and_keyboard_changes_page() {
    let mut pagination = Pagination::new(100, 10).current(5);

    assert_eq!(WidgetComponent::tab_index(&pagination), 1);
    assert_eq!(
        pagination.on_event(&SystemEvent::FocusIn),
        EventResult::Handled
    );
    pagination.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    });
    assert_eq!(pagination.get_current(), 6);
    pagination.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Home,
        mods: KeyMod::NONE,
    });
    assert_eq!(pagination.get_current(), 1);
    pagination.on_event(&SystemEvent::KeyDown {
        key: KeyCode::End,
        mods: KeyMod::NONE,
    });
    assert_eq!(pagination.get_current(), 10);
}

#[test]
fn pagination_page_size_changer_is_interactive_and_emits_change() {
    let mut pagination = Pagination::new(100, 10)
        .show_total(false)
        .show_size_changer(true)
        .page_size_options(vec![10, 25, 50]);
    let event = SystemEvent::PointerDown {
        pos: Point::new(210.0, 16.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };

    assert_eq!(pagination.on_event(&event), EventResult::Handled);
    assert_eq!(pagination.get_page_size(), 25);
    let semantic = pagination
        .semantic_event(ComponentId::new(9), &event)
        .expect("page-size change should emit event");
    assert_eq!(semantic.text_payload(), Some("page_size=25"));

    pagination.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Up,
        mods: KeyMod::NONE,
    });
    assert_eq!(pagination.get_page_size(), 10);
}

#[test]
fn pagination_snapshot_and_accessibility_expose_runtime_page() {
    let pagination = Pagination::new(95, 10).current(3);
    let fields = pagination.snapshot_fields();

    assert!(matches!(
        fields,
        SnapshotFields::Pagination {
            current: 3,
            page_size: 10,
            ..
        }
    ));
    let accessibility = fields.accessibility();
    assert_eq!(accessibility.state.value_now, Some(3.0));
    assert_eq!(accessibility.state.value_max, Some(10.0));
}

#[test]
fn pagination_total_template_receives_the_current_one_based_record_range() {
    assert_eq!(
        Pagination::new(95, 20).current(1).current_record_range(),
        1..20
    );
    assert_eq!(
        Pagination::new(95, 20).current(3).current_record_range(),
        41..60
    );
    assert_eq!(
        Pagination::new(95, 20).current(5).current_record_range(),
        81..95
    );
    assert_eq!(Pagination::new(0, 20).current_record_range(), 0..0);

    let captured = Rc::new(RefCell::new(None));
    let captured_for_template = captured.clone();
    let pagination = Pagination::new(95, 20)
        .current(3)
        .total_template(move |total, range| {
            *captured_for_template.borrow_mut() = Some((total, range.clone()));
            format!("第 {}-{} 条 / 共 {total} 条", range.start, range.end)
        });
    let display = render_pagination(&pagination);

    assert_eq!(*captured.borrow(), Some((95, 41..60)));
    assert!(display.contains("第 41-60 条 / 共 95 条"), "{display}");
}

#[test]
fn simple_pagination_renders_compact_counter_and_keeps_previous_next_interactive() {
    let regular = Pagination::new(100, 10).current(3).show_total(false);
    let mut simple = Pagination::new(100, 10)
        .current(3)
        .show_total(false)
        .simple(true);

    assert!(
        simple.measure(Constraints::unconstrained()).w
            < regular.measure(Constraints::unconstrained()).w
    );
    let display = render_pagination(&simple);
    assert!(display.contains("3 / 10"), "{display}");

    let next = SystemEvent::PointerDown {
        pos: Point::new(120.0, 16.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };
    assert_eq!(simple.on_event(&next), EventResult::Handled);
    assert_eq!(simple.get_current(), 4);
    assert_eq!(
        simple
            .semantic_event(ComponentId::new(11), &next)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("4".to_string())
    );

    assert_eq!(
        simple.on_event(&SystemEvent::PointerDown {
            pos: Point::new(14.0, 16.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(simple.get_current(), 3);
    assert_eq!(
        simple.on_event(&SystemEvent::PointerDown {
            pos: Point::new(60.0, 16.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled,
        "the compact counter is a stable, non-selecting hit region"
    );
    assert_eq!(simple.get_current(), 3);
}

#[test]
fn pagination_jumper_is_a_real_numeric_editor_with_commit_cancel_and_clamping() {
    let mut pagination = Pagination::new(95, 10).show_total(false).show_jumper(true);
    let activate = |pagination: &Pagination| SystemEvent::PointerDown {
        pos: Point::new(
            pagination.jumper_x_range().expect("jumper range").start + 4.0,
            16.0,
        ),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };

    assert_eq!(
        pagination.on_event(&activate(&pagination)),
        EventResult::Handled
    );
    assert!(WidgetTextInput::accepts_text_input(&pagination));
    assert_eq!(
        pagination.on_event(&SystemEvent::TextInput {
            text: "7".to_string(),
        }),
        EventResult::Handled
    );
    let editing_display = render_pagination(&pagination);
    assert!(editing_display.contains("text: \"7\""), "{editing_display}");
    assert!(WidgetTextInput::text_input_cursor_rect(&pagination).h > 0.0);
    let enter = SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    };
    assert_eq!(pagination.on_event(&enter), EventResult::Handled);
    assert_eq!(pagination.get_current(), 7);
    assert_eq!(
        pagination
            .semantic_event(ComponentId::new(3), &enter)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("7".to_string())
    );

    assert_eq!(
        pagination.on_event(&activate(&pagination)),
        EventResult::Handled
    );
    assert_eq!(
        pagination.on_event(&SystemEvent::TextInput {
            text: "999".to_string(),
        }),
        EventResult::Handled
    );
    assert_eq!(
        pagination.on_event(&SystemEvent::FocusOut),
        EventResult::Handled
    );
    assert_eq!(pagination.get_current(), 10);

    assert_eq!(
        pagination.on_event(&activate(&pagination)),
        EventResult::Handled
    );
    pagination.on_event(&SystemEvent::TextInput {
        text: "4".to_string(),
    });
    assert_eq!(
        pagination.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Escape,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(pagination.get_current(), 10);
    assert!(!WidgetTextInput::accepts_text_input(&pagination));
}
