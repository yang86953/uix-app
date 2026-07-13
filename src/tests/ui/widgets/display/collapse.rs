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
