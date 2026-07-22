use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::widgets::feedback::TriggerMode;
use crate::ui::widgets::navigation::dropdown::*;

fn render_dropdown(dropdown: &Dropdown, frame: Rect) -> String {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(400, 320));
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
            400,
            320,
        );
        ctx.with_recorder(&mut display_list, |ctx| {
            WidgetRender::render(dropdown, frame, ctx, &tree);
        });
    }
    format!("{display_list:?}")
}

fn pointer_down(button: MouseButton, y: f32) -> SystemEvent {
    SystemEvent::PointerDown {
        pos: Point::new(20.0, y),
        button,
        mods: KeyMod::NONE,
    }
}

fn key_event(key: KeyCode) -> SystemEvent {
    SystemEvent::KeyDown {
        key,
        mods: KeyMod::NONE,
    }
}

#[test]
fn dropdown_is_focusable_and_keyboard_selects_items() {
    let mut dropdown = Dropdown::new("Actions").items(vec!["Edit", "Delete", "Export"]);

    assert_eq!(WidgetComponent::tab_index(&dropdown), 1);
    assert_eq!(
        dropdown.on_event(&SystemEvent::FocusIn),
        EventResult::Handled
    );
    dropdown.on_event(&key_event(KeyCode::Down));
    assert!(dropdown.is_open());
    dropdown.on_event(&key_event(KeyCode::Down));
    dropdown.on_event(&key_event(KeyCode::Enter));

    assert_eq!(dropdown.selected_index(), Some(1));
    assert_eq!(dropdown.current_value(), Some("Delete"));
    assert!(!dropdown.is_open());
}

#[test]
fn dropdown_pointer_and_keyboard_selection_emit_change_values() {
    let mut dropdown = Dropdown::new("Actions").items(vec!["Edit", "Delete"]);
    dropdown.open();
    let event = SystemEvent::PointerDown {
        pos: Point::new(20.0, 47.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };

    assert_eq!(dropdown.on_event(&event), EventResult::Handled);
    let semantic = dropdown
        .semantic_event(ComponentId::new(3), &event)
        .expect("selection should emit change");
    assert_eq!(semantic.text_payload(), Some("Edit"));
}

#[test]
fn dropdown_snapshot_and_accessibility_expose_runtime_state() {
    let mut dropdown = Dropdown::new("Actions").items(vec!["Edit", "Delete"]);
    dropdown.open();
    dropdown.on_event(&key_event(KeyCode::End));

    let fields = dropdown.snapshot_fields();
    assert!(matches!(
        fields,
        SnapshotFields::Dropdown {
            open: true,
            selected_index: None,
            highlighted_index: Some(1),
            ..
        }
    ));
    let accessibility = fields.accessibility();
    assert_eq!(accessibility.name.as_deref(), Some("Actions"));
    assert_eq!(accessibility.state.expanded, Some(true));
}

#[test]
fn dropdown_escape_closes_without_selection() {
    let mut dropdown = Dropdown::new("Actions").items(vec!["Edit"]);
    dropdown.open();

    assert_eq!(
        dropdown.on_event(&key_event(KeyCode::Escape)),
        EventResult::Handled
    );
    assert!(!dropdown.is_open());
    assert_eq!(dropdown.current_value(), None);
}

#[test]
fn divider_and_disabled_rows_are_real_geometry_and_never_selectable() {
    let mut dropdown = Dropdown::new("Actions").items(vec![
        DropdownItem::new("Edit"),
        DropdownItem::divider(),
        DropdownItem::new("Delete").disabled(true),
        DropdownItem::new("Export"),
    ]);
    dropdown.open();
    let frame = Rect::new(0.0, 0.0, 160.0, 32.0);

    assert_eq!(dropdown.hit_test_frame(frame).h, 130.0);
    let rendered = render_dropdown(&dropdown, frame);
    assert!(
        rendered.contains("h: 1.0"),
        "divider must emit a one-pixel rule: {rendered}"
    );

    assert_eq!(
        dropdown.on_event(&pointer_down(MouseButton::Left, 85.0)),
        EventResult::NotHandled,
        "disabled row must reject pointer selection"
    );
    assert_eq!(dropdown.current_value(), None);
    assert!(dropdown.is_open());

    dropdown.on_event(&key_event(KeyCode::Down));
    dropdown.on_event(&key_event(KeyCode::Enter));
    assert_eq!(dropdown.current_value(), Some("Export"));
    assert_eq!(dropdown.selected_index(), Some(3));
}

#[test]
fn nested_item_expands_in_place_and_child_selection_emits_its_value() {
    let mut dropdown = Dropdown::new("Actions").items(vec![
        DropdownItem::new("More").children(vec![
            DropdownItem::new("Import"),
            DropdownItem::new("Export"),
        ]),
        DropdownItem::new("Delete"),
    ]);
    dropdown.open();
    let collapsed = dropdown.hit_test_frame(Rect::new(0.0, 0.0, 160.0, 32.0));

    assert_eq!(
        dropdown.on_event(&pointer_down(MouseButton::Left, 47.0)),
        EventResult::Handled
    );
    assert!(dropdown.is_open(), "expanding a parent must keep menu open");
    let expanded = dropdown.hit_test_frame(Rect::new(0.0, 0.0, 160.0, 32.0));
    assert_eq!(expanded.h - collapsed.h, 60.0);

    let child_event = pointer_down(MouseButton::Left, 77.0);
    assert_eq!(dropdown.on_event(&child_event), EventResult::Handled);
    assert_eq!(dropdown.current_value(), Some("Import"));
    assert_eq!(
        dropdown
            .semantic_event(ComponentId::new(12), &child_event)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("Import".to_string())
    );
}

#[test]
fn item_icon_replaces_plain_label_offset_with_real_icon_geometry() {
    let mut plain = Dropdown::new("Actions").items(vec![DropdownItem::new("Export")]);
    plain.open();
    let mut with_icon =
        Dropdown::new("Actions").items(vec![DropdownItem::new("Export").icon("download")]);
    with_icon.open();
    let frame = Rect::new(0.0, 0.0, 160.0, 32.0);
    let plain_render = render_dropdown(&plain, frame);
    let icon_render = render_dropdown(&with_icon, frame);

    assert_ne!(icon_render, plain_render);
    assert!(
        icon_render.matches("DrawText { text:").count()
            > plain_render.matches("DrawText { text:").count(),
        "icon must emit an actual icon-font glyph: {icon_render}"
    );
}

#[test]
fn every_dropdown_trigger_mode_uses_its_declared_activation_event() {
    let mut click = Dropdown::new("Click").trigger(TriggerMode::Click);
    assert_eq!(
        click.on_event(&pointer_down(MouseButton::Left, 16.0)),
        EventResult::Handled
    );
    assert!(click.is_open());

    let mut hover = Dropdown::new("Hover").trigger(TriggerMode::Hover);
    assert_eq!(
        hover.on_event(&SystemEvent::PointerEnter),
        EventResult::Handled
    );
    assert!(hover.is_open());
    assert_eq!(
        hover.on_event(&SystemEvent::PointerLeave),
        EventResult::Handled
    );
    assert!(!hover.is_open());

    let mut focus = Dropdown::new("Focus").trigger(TriggerMode::Focus);
    assert_eq!(focus.on_event(&SystemEvent::FocusIn), EventResult::Handled);
    assert!(focus.is_open());
    assert_eq!(focus.on_event(&SystemEvent::FocusOut), EventResult::Handled);
    assert!(!focus.is_open());

    let mut context = Dropdown::new("Context").trigger(TriggerMode::ContextMenu);
    assert_eq!(
        context.on_event(&pointer_down(MouseButton::Left, 16.0)),
        EventResult::NotHandled
    );
    assert!(!context.is_open());
    assert_eq!(
        context.on_event(&pointer_down(MouseButton::Right, 16.0)),
        EventResult::Handled
    );
    assert!(context.is_open());
}
