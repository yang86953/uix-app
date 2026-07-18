use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::core::widget::WidgetCore;
use crate::ui::widgets::display::collapse::*;
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

fn click(collapse: &mut Collapse, pos: Point) {
    assert_eq!(collapse.on_event(&pointer_down(pos)), EventResult::Handled);
    assert_eq!(collapse.on_event(&pointer_up(pos)), EventResult::Handled);
}

fn render_collapse_in(collapse: &Collapse, frame: Rect, surface_size: (i32, i32)) -> String {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(surface_size.0, surface_size.1));
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
            surface_size.0,
            surface_size.1,
        );
        ctx.with_recorder(&mut display_list, |ctx| {
            WidgetRender::render(collapse, frame, ctx, &tree);
        });
    }
    format!("{display_list:?}")
}

#[test]
fn collapse_advertises_animation_capability() {
    let collapse = Collapse::new().panels(vec![CollapsePanel::new("Panel", "content")]);

    assert!(collapse
        .capabilities()
        .contains(WidgetCapabilities::ANIMATION));
    assert!(collapse.as_animation().is_some());
}

#[test]
fn collapse_click_starts_panel_transition_and_marks_paint_dirty() {
    let mut collapse = Collapse::new().panels(vec![CollapsePanel::new("Panel", "content")]);

    click(&mut collapse, Point::new(4.0, 4.0));
    assert!(collapse.panels[0].expanded);

    let initial_opacity = collapse.transitions[0].opacity_progress;
    assert!(WidgetAnimation::update_animation(&mut collapse, 0.05));
    assert!(collapse.transitions[0].opacity_progress > initial_opacity);
    assert_eq!(
        WidgetAnimation::dirty_bounds(&collapse, Rect::new(0.0, 0.0, 320.0, 36.0)),
        Rect::new(0.0, 0.0, 320.0, 70.0)
    );
}

#[test]
fn collapse_tree_update_marks_animation_paint_dirty() {
    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(
        Collapse::new().panels(vec![CollapsePanel::new("Panel", "content")]),
    ));
    tree.get_mut(id)
        .expect("collapse root")
        .set_frame(Rect::new(0.0, 0.0, 320.0, 36.0));
    tree.dispatch_event(&pointer_down(Point::new(4.0, 4.0)));
    tree.dispatch_event(&pointer_up(Point::new(4.0, 4.0)));
    tree.invalidation().lock().unwrap().clear();

    assert!(tree.update(1.0 / 60.0));

    let queue = tree.invalidation().lock().unwrap();
    assert!(queue.has_paint_or_composite());
    assert!(queue.node_needs_paint(id));
}

#[test]
fn collapse_collapse_transition_releases_content_after_finish() {
    let mut collapse =
        Collapse::new().panels(vec![CollapsePanel::new("Panel", "content").expanded()]);

    click(&mut collapse, Point::new(4.0, 4.0));
    assert!(!collapse.panels[0].expanded);
    assert!(collapse.panel_present(0, &collapse.panels[0]));

    assert!(!WidgetAnimation::update_animation(&mut collapse, 1.0));

    assert!(!collapse.panel_present(0, &collapse.panels[0]));
}

#[test]
fn collapse_is_focusable_and_keyboard_controls_focused_header() {
    let mut collapse = Collapse::new().panels(vec![
        CollapsePanel::new("First", "one"),
        CollapsePanel::new("Second", "two"),
    ]);
    let down = SystemEvent::KeyDown {
        key: KeyCode::Down,
        mods: KeyMod::NONE,
    };

    assert_eq!(WidgetComponent::tab_index(&collapse), 1);
    assert_eq!(
        collapse.on_event(&SystemEvent::FocusIn),
        EventResult::Handled
    );
    assert_eq!(collapse.on_event(&down), EventResult::Handled);
    assert_eq!(collapse.focused_header(), 1);

    let right = SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    };
    assert_eq!(collapse.on_event(&right), EventResult::Handled);
    assert_eq!(collapse.expanded_indices(), vec![1]);
    assert_eq!(
        collapse
            .semantic_event(ComponentId::new(4), &right)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("1".to_string())
    );

    collapse.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Left,
        mods: KeyMod::NONE,
    });
    assert!(collapse.expanded_indices().is_empty());
}

#[test]
fn collapse_snapshot_and_accessibility_expose_expansion() {
    let mut collapse = Collapse::new().panels(vec![
        CollapsePanel::new("First", "one"),
        CollapsePanel::new("Second", "two").expanded(),
    ]);
    collapse.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Down,
        mods: KeyMod::NONE,
    });
    let fields = collapse.snapshot_fields();

    assert!(matches!(
        fields,
        SnapshotFields::Collapse {
            ref panels,
            focused_header: 1,
            ..
        } if panels[1].expanded
    ));
    let accessibility = fields.accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Button);
    assert_eq!(accessibility.name.as_deref(), Some("Second"));
    assert_eq!(accessibility.state.value_text.as_deref(), Some("Second"));
    assert_eq!(accessibility.state.expanded, Some(true));
}

#[test]
fn accordion_normalizes_multiple_initially_expanded_panels() {
    let collapse = Collapse::new()
        .panels(vec![
            CollapsePanel::new("First", "one").expanded(),
            CollapsePanel::new("Second", "two").expanded(),
        ])
        .accordion();

    assert_eq!(collapse.expanded_indices(), vec![0]);
}

#[test]
fn empty_collapse_is_not_focusable() {
    assert_eq!(WidgetComponent::tab_index(&Collapse::new()), 0);
}

#[test]
fn collapse_measures_wrapped_content_from_the_constrained_width() {
    let collapse = Collapse::new().panels(vec![
        CollapsePanel::new(
            "很长的面板标题需要保持在可用宽度内",
            "这是一段很长的中文正文，需要根据真实可用宽度自动换行并增加组件高度。",
        )
        .expanded(),
        CollapsePanel::new("Second", "body"),
    ]);

    let wide = collapse.measure(Constraints::loose(Size::new(240.0, 500.0)));
    let narrow = collapse.measure(Constraints::loose(Size::new(120.0, 500.0)));

    assert_eq!(wide.w, 240.0);
    assert_eq!(narrow.w, 120.0);
    assert!(
        narrow.h > wide.h,
        "narrow content must wrap: {narrow:?} <= {wide:?}"
    );
}

#[test]
fn collapse_render_clips_elides_and_uses_lucide_chevrons() {
    let collapse = Collapse::new().panels(vec![
        CollapsePanel::new(
            "A very long Collapse header that must stay inside",
            "Long body content must wrap inside the assigned frame instead of covering siblings.",
        )
        .expanded(),
        CollapsePanel::new("Second", "body"),
    ]);
    let display_list =
        render_collapse_in(&collapse, Rect::new(10.0, 5.0, 120.0, 100.0), (160, 120));

    assert!(
        display_list.contains("PushClip { rect: Rect { x: 10.0, y: 5.0, w: 120.0, h: 100.0 } }"),
        "Collapse must clip all paint to its frame: {display_list}"
    );
    assert!(
        display_list.contains('…'),
        "long header must elide: {display_list}"
    );
    assert!(
        display_list.contains("DrawTextWrapped"),
        "body must use wrapped text: {display_list}"
    );
    assert!(
        !display_list.contains('▶') && !display_list.contains('▼'),
        "text arrows must not return: {display_list}"
    );
    assert!(
        !display_list.contains("w: -") && !display_list.contains("h: -"),
        "{display_list}"
    );
}

#[test]
fn collapse_pointer_geometry_tracks_wrapped_content() {
    let mut collapse = Collapse::new().panels(vec![
        CollapsePanel::new(
            "First",
            "这是一段足够长的中文正文，会在窄面板内换成多行并推动第二个标题向下。",
        )
        .expanded(),
        CollapsePanel::new("Second", "body"),
    ]);
    let measured = collapse.measure(Constraints::loose(Size::new(120.0, 500.0)));
    let _ = render_collapse_in(
        &collapse,
        Rect::new(0.0, 0.0, measured.w, measured.h),
        (160, measured.h.ceil() as i32 + 10),
    );
    let second_header = Point::new(20.0, measured.h - 18.0);

    click(&mut collapse, second_header);

    assert_eq!(collapse.expanded_indices(), vec![0, 1]);
}

#[test]
fn collapse_animation_completion_requests_layout_shrink() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(
        Collapse::new().panels(vec![CollapsePanel::new("Panel", "content").expanded()]),
    ));
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 240.0, 70.0));

    tree.dispatch_event(&pointer_down(Point::new(4.0, 4.0)));
    tree.dispatch_event(&pointer_up(Point::new(4.0, 4.0)));
    tree.invalidation().lock().unwrap().clear();
    assert!(!tree.update(1.0));

    assert!(
        tree.invalidation()
            .lock()
            .unwrap()
            .layout_roots()
            .contains(&root),
        "finishing the leave transition must request the collapsed height"
    );
}

#[test]
fn collapse_reconcile_tracks_unique_headers_across_reorder() {
    let mut collapse = Collapse::new().panels(vec![
        CollapsePanel::new("First", "one").expanded(),
        CollapsePanel::new("Second", "two"),
    ]);
    collapse.on_event(&SystemEvent::FocusIn);
    collapse.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Down,
        mods: KeyMod::NONE,
    });

    collapse.sync_from(Collapse::new().panels(vec![
        CollapsePanel::new("Second", "updated two"),
        CollapsePanel::new("First", "updated one"),
    ]));

    assert_eq!(collapse.expanded_indices(), vec![1]);
    assert_eq!(collapse.focused_header(), 0);
}
