use crate::draw::backend::cpu::pixel_surface::PixelSurface;
use crate::draw::backend::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::geometry::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::widgets::{SelectableItem, SelectableList};
use crate::ui::AccessibilityRole;

fn pointer_down(pos: Point) -> SystemEvent {
    SystemEvent::PointerDown {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    }
}

fn pointer_up(pos: Point) -> SystemEvent {
    SystemEvent::PointerUp {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    }
}

fn click(list: &mut SelectableList, pos: Point) {
    assert_eq!(list.on_event(&pointer_down(pos)), EventResult::Handled);
    assert_eq!(list.on_event(&pointer_up(pos)), EventResult::Handled);
}

fn render_selectable_list(list: &SelectableList, frame: Rect, surface_size: (i32, i32)) -> String {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(surface_size.0, surface_size.1));
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
            surface_size.0,
            surface_size.1,
        );
        ctx.with_recorder(&mut display_list, |ctx| {
            WidgetRender::render(list, frame, ctx, &tree);
        });
    }
    format!("{display_list:?}")
}

fn list_with_items(count: usize) -> SelectableList {
    SelectableList::new()
        .items(
            (0..count)
                .map(|index| SelectableItem::new(format!("item-{index}"), format!("Item {index}")))
                .collect(),
        )
        .footer(format!("{count} items"))
}

#[test]
fn selectable_list_keyboard_selection_scrolls_and_emits_item_id() {
    let mut list = list_with_items(100);
    list.last_frame.set(Some(Rect::new(0.0, 0.0, 220.0, 120.0)));
    let end = SystemEvent::KeyDown {
        key: KeyCode::End,
        mods: KeyMod::NONE,
    };

    assert_eq!(WidgetComponent::tab_index(&list), 1);
    assert_eq!(list.on_event(&SystemEvent::FocusIn), EventResult::Handled);
    assert_eq!(list.on_event(&end), EventResult::Handled);
    assert_eq!(list.selected_id(), Some("item-99"));
    assert!(list.body_scroll.scroll_offset() > 0.0);
    assert!(EventHandler::scroll_delta_for_dirty(&list).is_some());
    assert_eq!(
        list.semantic_event(ComponentId::new(7), &end)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("item-99".to_string())
    );
}

#[test]
fn selectable_list_snapshot_and_accessibility_report_runtime_selection() {
    let mut list = list_with_items(3).active(1);
    list.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Down,
        mods: KeyMod::NONE,
    });

    assert_eq!(list.selected_text(), Some("Item 2"));
    let fields = list.snapshot_fields();
    let SnapshotFields::SelectableList { active_index, .. } = fields.clone() else {
        panic!("expected selectable list snapshot");
    };
    assert_eq!(active_index, 2);
    let accessibility = fields.accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::List);
    assert_eq!(accessibility.state.value_text.as_deref(), Some("Item 2"));
    assert_eq!(accessibility.state.value_now, Some(3.0));
}

#[test]
fn selectable_list_header_emits_submit_and_footer_does_not_hit_rows() {
    let mut list = list_with_items(20).header_button("Add");
    list.last_frame.set(Some(Rect::new(0.0, 0.0, 220.0, 140.0)));
    let header_click = pointer_up(Point::new(20.0, 20.0));

    assert_eq!(
        list.on_event(&pointer_down(Point::new(20.0, 20.0))),
        EventResult::Handled
    );
    assert!(list
        .semantic_event(ComponentId::new(5), &header_click)
        .is_none());
    assert_eq!(list.on_event(&header_click), EventResult::Handled);
    let event = list
        .semantic_event(ComponentId::new(5), &header_click)
        .expect("header action should emit a semantic event");
    assert_eq!(event.kind, SemanticKind::Submit);
    assert_eq!(event.text_payload(), Some("Add"));

    assert_eq!(
        list.on_event(&pointer_down(Point::new(20.0, 130.0))),
        EventResult::NotHandled
    );
}

#[test]
fn selectable_list_pointer_hit_is_two_dimensional_and_commits_on_release() {
    let mut list = list_with_items(3).header_button("Add").active(0);
    list.last_frame.set(Some(Rect::new(0.0, 0.0, 220.0, 160.0)));
    let second_row = Point::new(20.0, 48.0 + list.item_stride() + 10.0);

    assert_eq!(
        list.on_event(&pointer_down(Point::new(230.0, second_row.y))),
        EventResult::NotHandled
    );
    assert_eq!(
        list.on_event(&pointer_down(second_row)),
        EventResult::Handled
    );
    assert_eq!(list.selected_id(), Some("item-0"));
    assert!(list
        .semantic_event(ComponentId::new(8), &pointer_down(second_row))
        .is_none());
    assert_eq!(
        list.on_event(&pointer_up(Point::new(230.0, second_row.y))),
        EventResult::Handled
    );
    assert_eq!(list.selected_id(), Some("item-0"));

    click(&mut list, second_row);
    assert_eq!(list.selected_id(), Some("item-1"));
    assert_eq!(
        list.semantic_event(ComponentId::new(8), &pointer_up(second_row))
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("item-1".to_owned())
    );
}

#[test]
fn constrained_selectable_list_clips_and_elides_without_negative_geometry() {
    let list = SelectableList::new()
        .items(vec![SelectableItem::new(
            "quality",
            "一段很长的中英文质量验收项目 mixed identifier",
        )])
        .header_button("一个很长的批量选择操作")
        .footer("一个很长的验收统计页脚");

    for frame in [
        Rect::new(10.0, 8.0, 100.0, 90.0),
        Rect::new(0.0, 0.0, 8.0, 20.0),
        Rect::new(0.0, 0.0, f32::NAN, -10.0),
    ] {
        let display_list = render_selectable_list(&list, frame, (140, 120));
        assert!(!display_list.contains("NaN"), "{display_list}");
        assert!(
            !display_list.contains("w: -") && !display_list.contains("h: -"),
            "{display_list}"
        );
    }

    let display_list = render_selectable_list(&list, Rect::new(10.0, 8.0, 100.0, 90.0), (140, 120));
    assert!(display_list.contains('…'), "{display_list}");
    assert!(
        display_list.contains("PushClip { rect: Rect { x: 10.0, y: 8.0, w: 100.0, h: 90.0 } }"),
        "{display_list}"
    );
}

#[test]
fn selectable_list_reconcile_preserves_selection_by_item_id() {
    let mut list = SelectableList::new()
        .items(vec![
            SelectableItem::new("visual", "Visual"),
            SelectableItem::new("semantic", "Semantic"),
        ])
        .active(1);

    list.sync_from(
        SelectableList::new()
            .items(vec![
                SelectableItem::new("semantic", "Updated semantic"),
                SelectableItem::new("visual", "Updated visual"),
            ])
            .active(1),
    );

    assert_eq!(list.selected_id(), Some("semantic"));
    assert_eq!(list.selected_text(), Some("Updated semantic"));
}

#[test]
fn empty_selectable_list_without_header_is_not_focusable() {
    let list = SelectableList::new();

    assert_eq!(WidgetComponent::tab_index(&list), 0);
    assert_eq!(
        WidgetComponent::tab_index(&SelectableList::new().header_button("Add")),
        1
    );
}
