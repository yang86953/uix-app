use crate::tests::common::*;
use crate::ui::core::widget::WidgetCore;
use crate::ui::widgets::display::collapse::*;

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

    assert!(matches!(
        EventHandler::on_event(
            &mut collapse,
            &SystemEvent::PointerDown {
                pos: Point::new(4.0, 4.0),
                button: MouseButton::Left,
                mods: KeyMod::NONE,
            },
        ),
        EventResult::Handled
    ));
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
    tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(4.0, 4.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
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

    EventHandler::on_event(
        &mut collapse,
        &SystemEvent::PointerDown {
            pos: Point::new(4.0, 4.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        },
    );
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
