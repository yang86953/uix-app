use crate::tests::common::*;
use crate::ui::core::widget::WidgetCore;
use crate::ui::widgets::{Tag, TagColor};
use crate::ui::{AccessibilityRole, EventHandler, WidgetComponent};

fn render_tag(tag: &Tag) -> Vec<u32> {
    let mut canvas = crate::draw::backend::cpu::shared_rasterizer::SharedRasterizer::new(
        crate::draw::backend::cpu::pixel_surface::PixelSurface::new(120, 36),
    );
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
        crate::draw::geometry::spatial::Orientation::YDown,
        120,
        36,
    );
    WidgetRender::render(tag, Rect::new(4.0, 8.0, 112.0, 20.0), &mut ctx, &tree);
    canvas.surface().pixels().to_vec()
}

fn render_tag_display(tag: &Tag, frame: Rect, surface_size: (i32, i32)) -> String {
    let mut canvas = crate::draw::backend::cpu::shared_rasterizer::SharedRasterizer::new(
        crate::draw::backend::cpu::pixel_surface::PixelSurface::new(surface_size.0, surface_size.1),
    );
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
            crate::draw::geometry::spatial::Orientation::YDown,
            surface_size.0,
            surface_size.1,
        );
        ctx.with_recorder(&mut display_list, |ctx| {
            WidgetRender::render(tag, frame, ctx, &tree);
        });
    }
    format!("{display_list:?}")
}

fn key(key: KeyCode) -> SystemEvent {
    SystemEvent::KeyDown {
        key,
        mods: KeyMod::NONE,
    }
}

fn key_up(key: KeyCode) -> SystemEvent {
    SystemEvent::KeyUp {
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

fn pointer_up(pos: Point) -> SystemEvent {
    SystemEvent::PointerUp {
        pos,
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    }
}

#[test]
fn tag_measure_counts_cjk_scalars_and_reserved_action_regions() {
    assert_eq!(
        Tag::new("管理员").measure(Constraints::unconstrained()),
        Size::new(52.0, 20.0)
    );
    assert_eq!(
        Tag::new("管理员")
            .closable()
            .measure(Constraints::unconstrained()),
        Size::new(72.0, 20.0)
    );
    assert_eq!(
        Tag::new("管理员")
            .checkable(true)
            .measure(Constraints::unconstrained()),
        Size::new(66.0, 20.0)
    );
    assert_eq!(
        Tag::new("管理员")
            .closable()
            .checkable(true)
            .measure(Constraints::unconstrained()),
        Size::new(86.0, 20.0)
    );
}

#[test]
fn tag_measure_uses_custom_font_size_for_cjk_text() {
    assert_eq!(
        Tag::new("管理员")
            .font_size(20.0)
            .measure(Constraints::unconstrained()),
        Size::new(76.0, 28.0)
    );
}

#[test]
fn invalid_tag_font_sizes_fall_back_in_measure_and_render() {
    let expected_size = Tag::new("管理员").measure(Constraints::unconstrained());
    let expected_pixels = render_tag(&Tag::new("Status").closable());

    for invalid in [f32::NAN, f32::INFINITY, 0.0, -4.0] {
        assert_eq!(
            Tag::new("管理员")
                .font_size(invalid)
                .measure(Constraints::unconstrained()),
            expected_size
        );
        assert_eq!(
            render_tag(&Tag::new("Status").closable().font_size(invalid)),
            expected_pixels
        );
    }
}

#[test]
fn checkable_tag_toggles_from_pointer_and_keyboard_with_change_payloads() {
    let mut tag = Tag::new("发布").checkable(true);
    let pointer = pointer_down(Point::new(8.0, 8.0));
    let pointer_release = pointer_up(Point::new(8.0, 8.0));

    assert_eq!(WidgetComponent::tab_index(&tag), 1);
    assert_eq!(tag.on_event(&pointer), EventResult::Handled);
    assert!(!tag.is_checked());
    assert!(tag.semantic_event(ComponentId::new(2), &pointer).is_none());
    assert_eq!(tag.on_event(&pointer_release), EventResult::Handled);
    assert!(tag.is_checked());
    assert_eq!(
        tag.semantic_event(ComponentId::new(2), &pointer_release)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("checked".into())
    );

    let space = key(KeyCode::Space);
    assert_eq!(tag.on_event(&space), EventResult::Handled);
    assert!(tag.is_checked());
    let space_up = key_up(KeyCode::Space);
    assert_eq!(tag.on_event(&space_up), EventResult::Handled);
    assert!(!tag.is_checked());
    assert_eq!(
        tag.semantic_event(ComponentId::new(2), &space_up)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("unchecked".into())
    );
    let accessibility = tag.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Checkbox);
    assert_eq!(accessibility.state.checked, Some(false));
}

#[test]
fn closable_tag_separates_body_from_close_region_and_can_reopen() {
    let mut tag = Tag::new("临时").closable();
    let body = pointer_down(Point::new(4.0, 4.0));
    let close = pointer_down(Point::new(52.0, 4.0));
    let close_release = pointer_up(Point::new(52.0, 4.0));

    assert_eq!(tag.on_event(&body), EventResult::NotHandled);
    assert!(tag.is_visible());
    assert_eq!(tag.on_event(&close), EventResult::Handled);
    assert!(tag.is_visible());
    assert_eq!(tag.on_event(&close_release), EventResult::Handled);
    assert!(!tag.is_visible());
    assert!(EventHandler::take_layout_request(&mut tag));
    assert_eq!(WidgetComponent::tab_index(&tag), 0);
    assert_eq!(
        tag.semantic_event(ComponentId::new(3), &close_release)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("closed".into())
    );

    tag.open();
    assert!(tag.is_visible());
    assert!(EventHandler::take_layout_request(&mut tag));
    assert_eq!(WidgetComponent::tab_index(&tag), 1);
    assert_eq!(tag.on_event(&key(KeyCode::Enter)), EventResult::Handled);
    assert!(tag.is_visible());
    assert_eq!(tag.on_event(&key_up(KeyCode::Enter)), EventResult::Handled);
    assert!(!tag.is_visible());
}

#[test]
fn tag_requires_matching_pointer_and_key_release_before_committing() {
    let mut tag = Tag::new("选择项").checkable(true);

    assert_eq!(
        tag.on_event(&pointer_down(Point::new(8.0, 8.0))),
        EventResult::Handled
    );
    assert_eq!(
        tag.on_event(&SystemEvent::PointerLeave),
        EventResult::Handled
    );
    assert_eq!(
        tag.on_event(&pointer_up(Point::new(8.0, 8.0))),
        EventResult::NotHandled
    );
    assert!(!tag.is_checked());

    assert_eq!(
        tag.on_event(&pointer_down(Point::new(8.0, 8.0))),
        EventResult::Handled
    );
    assert_eq!(
        tag.on_event(&pointer_up(Point::new(200.0, 8.0))),
        EventResult::Handled
    );
    assert!(!tag.is_checked());

    assert_eq!(tag.on_event(&key(KeyCode::Space)), EventResult::Handled);
    assert_eq!(tag.on_event(&SystemEvent::FocusOut), EventResult::Handled);
    assert_eq!(
        tag.on_event(&key_up(KeyCode::Space)),
        EventResult::NotHandled
    );
    assert!(!tag.is_checked());

    assert_eq!(tag.on_event(&key(KeyCode::Space)), EventResult::Handled);
    assert_eq!(tag.on_event(&key(KeyCode::Space)), EventResult::Handled);
    assert_eq!(
        tag.on_event(&key_up(KeyCode::Enter)),
        EventResult::NotHandled
    );
    assert!(!tag.is_checked());
    assert_eq!(
        tag.on_event(&key_up(KeyCode::Space)),
        EventResult::NotHandled
    );
    assert!(!tag.is_checked());

    assert_eq!(tag.on_event(&key(KeyCode::Space)), EventResult::Handled);
    assert_eq!(tag.on_event(&key_up(KeyCode::Space)), EventResult::Handled);
    assert!(tag.is_checked());
}

#[test]
fn closable_tag_hit_testing_matches_the_visible_twenty_pixel_slot() {
    let mut tag = Tag::new("临时").closable();
    let _ = render_tag_display(&tag, Rect::new(0.0, 0.0, 72.0, 20.0), (80, 28));

    assert_eq!(
        tag.on_event(&pointer_down(Point::new(51.0, 10.0))),
        EventResult::NotHandled
    );
    assert_eq!(
        tag.on_event(&pointer_down(Point::new(52.0, 10.0))),
        EventResult::Handled
    );
    assert_eq!(
        tag.on_event(&pointer_up(Point::new(51.0, 10.0))),
        EventResult::Handled
    );
    assert!(tag.is_visible());
}

#[test]
fn constrained_tag_clips_elides_and_uses_contrasting_custom_foreground() {
    let constrained = Tag::new("很长的中英文混合标签 mixed value")
        .checkable(true)
        .default_checked(true)
        .closable();
    let display_list =
        render_tag_display(&constrained, Rect::new(10.0, 6.0, 80.0, 16.0), (100, 32));
    assert!(
        display_list.contains("PushClip { rect: Rect { x: 10.0, y: 6.0, w: 80.0, h: 16.0 } }"),
        "{display_list}"
    );
    assert!(display_list.contains('…'), "{display_list}");
    assert!(!display_list.contains("NaN"), "{display_list}");
    assert!(
        !display_list.contains("w: -") && !display_list.contains("h: -"),
        "{display_list}"
    );

    let invalid = render_tag_display(
        &Tag::new("invalid").closable(),
        Rect::new(0.0, 0.0, f32::NAN, -10.0),
        (20, 20),
    );
    assert!(!invalid.contains("NaN"), "{invalid}");
    assert!(!invalid.contains("w: -") && !invalid.contains("h: -"));

    let light = render_tag_display(
        &Tag::new("可读").custom_color(Color::white()),
        Rect::new(0.0, 0.0, 48.0, 20.0),
        (56, 28),
    );
    assert!(
        light.contains("color: Color { r: 0, g: 0, b: 0, a: 255 }"),
        "{light}"
    );
}

#[test]
fn reconcile_preserves_runtime_visibility_and_checked_state() {
    let mut tag = Tag::new("old").default_checked(true).closable();
    tag.set_checked(false);
    tag.close();
    tag.sync_from(Tag::new("new").default_checked(true).closable());
    assert!(!tag.is_visible());
    assert!(!tag.is_checked());

    tag.open();
    tag.sync_from(Tag::new("static").checkable(false));
    assert!(tag.is_visible());
    assert!(!tag.is_checked());
    assert_eq!(WidgetComponent::tab_index(&tag), 0);
}

#[test]
fn tree_clears_interaction_when_tag_hides_itself() {
    let mut tree = WidgetTree::new();
    let tag = tree.set_root(Box::new(Tag::new("临时").closable()));
    tree.get_mut(tag)
        .expect("tag")
        .set_frame(Rect::new(0.0, 0.0, 100.0, 32.0));
    tree.set_focus(Some(tag));
    tree.dispatch_event(&SystemEvent::PointerMove {
        pos: Point::new(20.0, 16.0),
        mods: KeyMod::NONE,
    });
    assert_eq!(tree.managers().focus.focused_component(), Some(tag));
    assert_eq!(tree.managers().interaction.hovered_component(), Some(tag));

    tree.dispatch_event(&key(KeyCode::Escape));

    assert!(!tree.get(tag).expect("tag").visible());
    assert_eq!(tree.managers().focus.focused_component(), None);
    assert_eq!(tree.managers().interaction.hovered_component(), None);

    tree.get_mut(tag)
        .expect("tag")
        .component_mut()
        .as_any_mut()
        .downcast_mut::<Tag>()
        .expect("Tag")
        .open();
    tree.dispatch_event(&SystemEvent::PointerMove {
        pos: Point::new(60.0, 16.0),
        mods: KeyMod::NONE,
    });
    tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(60.0, 16.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert!(tree.get(tag).expect("tag").visible());
    tree.dispatch_event(&SystemEvent::PointerUp {
        pos: Point::new(60.0, 16.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert!(!tree.get(tag).expect("tag").visible());
    assert_eq!(tree.managers().interaction.pressed_component(), None);
    assert!(!tree.managers().drag.is_potential());
    assert_eq!(tree.managers().focus.focused_component(), None);
}

#[test]
fn extended_color_and_icon_participate_in_snapshot_and_measurement() {
    let plain = Tag::new("Admin").color(TagColor::Geekblue);
    let icon = Tag::new("Admin").color(TagColor::Geekblue).icon("star");
    assert!(
        icon.measure(Constraints::unconstrained()).w
            > plain.measure(Constraints::unconstrained()).w
    );
    assert!(matches!(
        icon.snapshot_fields(),
        SnapshotFields::Tag {
            color: TagColor::Geekblue,
            ref icon,
            ..
        } if icon == "star"
    ));
}
