use crate::core::{Point, Rect};
use crate::native::traits::input::{KeyMod, MouseButton};
use crate::ui::core::widget::WidgetCore;
use crate::ui::event::WindowAction;
use crate::ui::semantic_action::{SemanticAction, SemanticActionKind};
use crate::ui::view::{
    button, label, window_control, window_control_named, window_drag_region, ViewAdapter,
};
use crate::ui::{AccessibilityRole, SystemEvent, WindowControl};

fn pointer_event(pos: Point, down: bool, button: MouseButton) -> SystemEvent {
    if down {
        SystemEvent::PointerDown {
            pos,
            button,
            mods: KeyMod::NONE,
        }
    } else {
        SystemEvent::PointerUp {
            pos,
            button,
            mods: KeyMod::NONE,
        }
    }
}

#[test]
fn drag_region_emits_move_only_for_left_pointer_down() {
    let mut tree = ViewAdapter::build(window_drag_region(label("title")).height(32.0));
    tree.root_mut()
        .expect("drag region root")
        .set_frame(Rect::new(0.0, 0.0, 240.0, 32.0));
    tree.layout();

    let pos = Point::new(12.0, 12.0);
    tree.dispatch_event(&pointer_event(pos, true, MouseButton::Right));
    assert!(tree.take_window_actions().is_empty());

    tree.dispatch_event(&pointer_event(pos, true, MouseButton::Left));
    assert_eq!(
        tree.take_window_actions(),
        vec![WindowAction::BeginMoveDrag]
    );
}

#[test]
fn control_region_owns_nested_presentation_and_emits_on_release() {
    // 即便内容本身是 Button，命中也由窗口控制包装器接管，避免嵌套交互吞事件。
    let mut tree = ViewAdapter::build(
        window_control(WindowControl::Close, button("×"))
            .width(44.0)
            .height(32.0),
    );
    tree.root_mut()
        .expect("window control root")
        .set_frame(Rect::new(0.0, 0.0, 44.0, 32.0));
    tree.layout();

    let pos = Point::new(20.0, 16.0);
    tree.dispatch_event(&pointer_event(pos, true, MouseButton::Left));
    assert!(tree.take_window_actions().is_empty());
    tree.dispatch_event(&pointer_event(pos, false, MouseButton::Left));

    assert_eq!(tree.take_window_actions(), vec![WindowAction::RequestClose]);
}

#[test]
fn control_region_cancels_when_pointer_is_released_outside() {
    let mut tree = ViewAdapter::build(
        window_control(WindowControl::Close, label("close"))
            .width(44.0)
            .height(32.0),
    );
    tree.root_mut()
        .expect("window control root")
        .set_frame(Rect::new(0.0, 0.0, 44.0, 32.0));
    tree.layout();

    tree.dispatch_event(&pointer_event(
        Point::new(20.0, 16.0),
        true,
        MouseButton::Left,
    ));
    tree.dispatch_event(&pointer_event(
        Point::new(80.0, 16.0),
        false,
        MouseButton::Left,
    ));
    assert!(tree.take_window_actions().is_empty());

    // 之前的外部释放不得留下 armed 状态，让后续孤立 PointerUp 误触发。
    tree.dispatch_event(&pointer_event(
        Point::new(20.0, 16.0),
        false,
        MouseButton::Left,
    ));
    assert!(tree.take_window_actions().is_empty());
}

#[test]
fn each_window_control_maps_to_its_platform_neutral_action() {
    let cases = [
        (WindowControl::Minimize, WindowAction::Minimize),
        (
            WindowControl::MaximizeRestore,
            WindowAction::MaximizeRestore,
        ),
        (WindowControl::Close, WindowAction::RequestClose),
    ];

    for (control, expected) in cases {
        let mut tree = ViewAdapter::build(
            window_control(control, label("control"))
                .width(60.0)
                .height(32.0),
        );
        tree.root_mut()
            .expect("window control root")
            .set_frame(Rect::new(0.0, 0.0, 60.0, 32.0));
        tree.layout();
        let pos = Point::new(8.0, 8.0);
        tree.dispatch_event(&pointer_event(pos, true, MouseButton::Left));
        tree.dispatch_event(&pointer_event(pos, false, MouseButton::Left));
        assert_eq!(tree.take_window_actions(), vec![expected]);
    }
}

#[test]
fn named_window_control_is_an_invokable_accessible_button() {
    let mut tree = ViewAdapter::build(
        window_control_named(WindowControl::Close, "关闭窗口", label("×"))
            .width(44.0)
            .height(32.0),
    );
    let root = tree.root().expect("window control root").id();
    tree.root_mut()
        .expect("window control root")
        .set_frame(Rect::new(0.0, 0.0, 44.0, 32.0));
    tree.layout();

    let snapshot = tree.semantic_snapshot_body();
    let control = snapshot
        .nodes
        .iter()
        .find(|node| node.id == root)
        .expect("window control semantic node");
    assert_eq!(control.accessibility.role, AccessibilityRole::Button);
    assert_eq!(control.accessibility.name.as_deref(), Some("关闭窗口"));
    assert!(control.actions.contains(&SemanticActionKind::Invoke));

    tree.perform_semantic_action(root, &SemanticAction::Invoke)
        .expect("invoke close control");
    assert_eq!(tree.take_window_actions(), vec![WindowAction::RequestClose]);
}
