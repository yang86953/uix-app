use crate::core::{Point, Rect};
use crate::draw::backend::cpu::canvas_2d::CpuCanvas2D;
use crate::draw::backend::cpu::pixel_surface::PixelSurface;
use crate::draw::geometry::spatial::Orientation;
use crate::draw::scene::ScenePaint;
use crate::native::traits::input::{KeyCode, KeyMod, MouseButton};
use crate::tests::common::{Color, DesignTokens, FontHandle, FontService, ImageService};
use crate::ui::core::widget::WidgetCore;
use crate::ui::event::{SemanticKind, WindowAction};
use crate::ui::semantic_action::{SemanticAction, SemanticActionKind};
use crate::ui::view::{
    button, label, row, window_control, window_control_named, window_drag_region, StyleExt,
    ViewAdapter,
};
use crate::ui::{
    AccessibilityRole, AlignItems, JustifyContent, OverlayKind, PaintContext, SystemEvent,
    WidgetTree, WindowControl,
};

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

fn key_event(key: KeyCode, down: bool) -> SystemEvent {
    if down {
        SystemEvent::KeyDown {
            key,
            mods: KeyMod::NONE,
        }
    } else {
        SystemEvent::KeyUp {
            key,
            mods: KeyMod::NONE,
        }
    }
}

fn pointer_double_click(pos: Point, button: MouseButton) -> SystemEvent {
    SystemEvent::PointerDoubleClick {
        pos,
        button,
        mods: KeyMod::NONE,
    }
}

fn render_control_corner(tree: &WidgetTree) -> u32 {
    let root = tree.root_id().expect("window control root");
    let frame = tree.get(root).expect("window control node").frame();
    let mut target = CpuCanvas2D::new(PixelSurface::new(44, 32));
    let fonts = FontService::new();
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    {
        let mut ctx = PaintContext::new_for_test(
            &mut target,
            FontHandle::default(),
            &fonts,
            &images,
            &tokens,
            96.0,
            1.0,
            Orientation::YDown,
            44,
            32,
        );
        ScenePaint::paint(tree, root, frame, &mut ctx);
    }
    target.surface().pixels()[45]
}

#[test]
fn drag_region_maps_native_title_bar_pointer_gestures() {
    let mut tree = ViewAdapter::build(window_drag_region(label("title")).height(32.0));
    tree.root_mut()
        .expect("drag region root")
        .set_frame(Rect::new(0.0, 0.0, 240.0, 32.0));
    tree.layout();

    let pos = Point::new(12.0, 12.0);
    tree.dispatch_event(&pointer_event(pos, true, MouseButton::Right));
    assert!(tree.take_window_actions().is_empty());
    tree.dispatch_event(&pointer_event(pos, false, MouseButton::Right));
    assert_eq!(
        tree.take_window_actions(),
        vec![WindowAction::ShowSystemMenuFromTitleBar]
    );
    assert!(tree.overlay_stack().is_empty());

    tree.dispatch_event(&pointer_event(pos, true, MouseButton::Left));
    assert_eq!(
        tree.take_window_actions(),
        vec![WindowAction::BeginMoveDrag]
    );
}

#[test]
fn drag_region_leaves_interactive_descendants_in_control() {
    let mut tree = ViewAdapter::build(
        window_drag_region(
            window_control(WindowControl::Close, label("close"))
                .width(44.0)
                .height(32.0),
        )
        .width(240.0)
        .height(32.0),
    );
    tree.root_mut()
        .expect("drag region root")
        .set_frame(Rect::new(0.0, 0.0, 240.0, 32.0));
    tree.layout();

    let pos = Point::new(20.0, 16.0);
    tree.dispatch_event(&pointer_event(pos, true, MouseButton::Left));
    assert!(tree.take_window_actions().is_empty());
    tree.dispatch_event(&pointer_event(pos, false, MouseButton::Left));
    assert_eq!(tree.take_window_actions(), vec![WindowAction::RequestClose]);
}

#[test]
fn window_drag_actions_preserve_existing_keyboard_focus() {
    let mut tree = ViewAdapter::build(
        row(vec![
            button("focused").width(60.0).height(32.0),
            window_drag_region(label("title")).width(180.0).height(32.0),
        ])
        .width(240.0)
        .height(32.0),
    );
    let root = tree.root_id().expect("title bar root");
    tree.root_mut()
        .expect("title bar root")
        .set_frame(Rect::new(0.0, 0.0, 240.0, 32.0));
    tree.layout();
    let focused = tree
        .get(root)
        .and_then(|node| node.children().first().copied())
        .expect("focused control");
    tree.set_focus(Some(focused));

    tree.dispatch_event(&pointer_event(
        Point::new(120.0, 16.0),
        true,
        MouseButton::Left,
    ));

    assert_eq!(
        tree.take_window_actions(),
        vec![WindowAction::BeginMoveDrag]
    );
    assert_eq!(tree.managers().focus.focused_component(), Some(focused));

    tree.dispatch_event(&pointer_event(
        Point::new(120.0, 16.0),
        false,
        MouseButton::Left,
    ));
    tree.dispatch_event(&pointer_double_click(
        Point::new(120.0, 16.0),
        MouseButton::Left,
    ));
    assert_eq!(
        tree.take_window_actions(),
        vec![WindowAction::ToggleMaximizeFromTitleBar]
    );
    assert_eq!(tree.managers().focus.focused_component(), Some(focused));

    tree.dispatch_event(&pointer_event(
        Point::new(120.0, 16.0),
        true,
        MouseButton::Right,
    ));
    assert!(tree.take_window_actions().is_empty());
    tree.dispatch_event(&pointer_event(
        Point::new(120.0, 16.0),
        false,
        MouseButton::Right,
    ));
    assert_eq!(
        tree.take_window_actions(),
        vec![WindowAction::ShowSystemMenuFromTitleBar]
    );
    assert_eq!(tree.managers().focus.focused_component(), Some(focused));
}

#[test]
fn drag_region_keeps_nested_interactive_context_menu() {
    let mut tree = ViewAdapter::build(
        window_drag_region(button("nested").width(80.0).height(32.0))
            .width(240.0)
            .height(32.0),
    );
    tree.root_mut()
        .expect("drag region root")
        .set_frame(Rect::new(0.0, 0.0, 240.0, 32.0));
    tree.layout();

    let pos = Point::new(20.0, 16.0);
    tree.dispatch_event(&pointer_event(pos, true, MouseButton::Right));
    tree.dispatch_event(&pointer_event(pos, false, MouseButton::Right));
    assert!(tree.take_window_actions().is_empty());
    assert_eq!(
        tree.overlay_stack().top().map(|entry| entry.kind()),
        Some(OverlayKind::ContextMenu)
    );
}

#[test]
fn drag_region_keeps_nested_explicit_context_menu_handler() {
    let called = Rc::new(Cell::new(false));
    let called_for_handler = called.clone();
    let content = row([label("title")]).width(240.0).height(32.0).on_semantic(
        SemanticKind::ContextMenu,
        move |event| {
            called_for_handler.set(true);
            event.prevent_default();
        },
    );
    let mut tree = ViewAdapter::build(window_drag_region(content).width(240.0).height(32.0));
    tree.root_mut()
        .expect("drag region root")
        .set_frame(Rect::new(0.0, 0.0, 240.0, 32.0));
    tree.layout();

    let pos = Point::new(200.0, 16.0);
    tree.dispatch_event(&pointer_event(pos, true, MouseButton::Right));
    tree.dispatch_event(&pointer_event(pos, false, MouseButton::Right));

    assert!(called.get());
    assert!(tree.take_window_actions().is_empty());
    assert!(tree
        .overlay_stack()
        .iter()
        .all(|entry| entry.kind() != OverlayKind::ContextMenu));
}

#[test]
fn drag_region_cancels_system_menu_when_released_outside() {
    let mut tree = ViewAdapter::build(window_drag_region(label("title")).height(32.0));
    tree.root_mut()
        .expect("drag region root")
        .set_frame(Rect::new(0.0, 0.0, 240.0, 32.0));
    tree.layout();

    tree.dispatch_event(&pointer_event(
        Point::new(20.0, 16.0),
        true,
        MouseButton::Right,
    ));
    tree.dispatch_event(&pointer_event(
        Point::new(280.0, 16.0),
        false,
        MouseButton::Right,
    ));
    assert!(tree.take_window_actions().is_empty());
}

#[test]
fn drag_region_double_click_keeps_nested_control_action() {
    let mut tree = ViewAdapter::build(
        window_drag_region(
            window_control(WindowControl::Minimize, label("minimize"))
                .width(44.0)
                .height(32.0),
        )
        .width(240.0)
        .height(32.0),
    );
    tree.root_mut()
        .expect("drag region root")
        .set_frame(Rect::new(0.0, 0.0, 240.0, 32.0));
    tree.layout();

    let pos = Point::new(20.0, 16.0);
    tree.dispatch_event(&pointer_double_click(pos, MouseButton::Left));
    assert!(tree.take_window_actions().is_empty());
    tree.dispatch_event(&pointer_event(pos, false, MouseButton::Left));
    assert_eq!(tree.take_window_actions(), vec![WindowAction::Minimize]);
}

#[test]
fn control_region_owns_nested_presentation_and_emits_on_release() {
    // 即便内容本身是 Button，命中也由窗口控制包装器接管，避免嵌套交互吞事件。
    let mut tree = ViewAdapter::build(
        window_control(WindowControl::Close, button("Close"))
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
fn control_region_requires_matching_activation_input() {
    let mut tree = ViewAdapter::build(
        window_control(WindowControl::Close, label("close"))
            .width(44.0)
            .height(32.0),
    );
    tree.root_mut()
        .expect("window control root")
        .set_frame(Rect::new(0.0, 0.0, 44.0, 32.0));
    tree.layout();
    let pos = Point::new(20.0, 16.0);

    tree.dispatch_event(&pointer_event(pos, true, MouseButton::Left));
    tree.dispatch_event(&key_event(KeyCode::Enter, false));
    assert!(tree.take_window_actions().is_empty());
    tree.dispatch_event(&pointer_event(pos, false, MouseButton::Left));
    assert_eq!(tree.take_window_actions(), vec![WindowAction::RequestClose]);

    tree.dispatch_event(&key_event(KeyCode::Enter, true));
    tree.dispatch_event(&key_event(KeyCode::Space, false));
    assert!(tree.take_window_actions().is_empty());
    tree.dispatch_event(&key_event(KeyCode::Enter, false));
    assert_eq!(tree.take_window_actions(), vec![WindowAction::RequestClose]);
}

#[test]
fn control_region_renders_custom_hover_and_active_backgrounds() {
    let mut tree = ViewAdapter::build(
        window_control(WindowControl::Close, label(""))
            .width(44.0)
            .height(32.0)
            .bg(Color::red())
            .bg_hover(Color::green())
            .bg_active(Color::blue()),
    );
    tree.root_mut()
        .expect("window control root")
        .set_frame(Rect::new(0.0, 0.0, 44.0, 32.0));
    tree.layout();
    let pos = Point::new(20.0, 16.0);

    assert_eq!(render_control_corner(&tree), Color::red().premultiplied());
    tree.dispatch_event(&SystemEvent::PointerMove {
        pos,
        mods: KeyMod::NONE,
    });
    assert_eq!(render_control_corner(&tree), Color::green().premultiplied());
    tree.dispatch_event(&pointer_event(pos, true, MouseButton::Left));
    assert_eq!(render_control_corner(&tree), Color::blue().premultiplied());
    tree.dispatch_event(&pointer_event(pos, false, MouseButton::Left));
    assert_eq!(render_control_corner(&tree), Color::green().premultiplied());
    tree.dispatch_event(&SystemEvent::PointerMove {
        pos: Point::new(80.0, 16.0),
        mods: KeyMod::NONE,
    });
    assert_eq!(render_control_corner(&tree), Color::red().premultiplied());
}

#[test]
fn control_region_renders_focus_and_keeps_active_priority() {
    let mut tree = ViewAdapter::build(
        window_control(WindowControl::Close, label(""))
            .width(44.0)
            .height(32.0)
            .bg(Color::red())
            .bg_focus(Color::from_rgb(255, 215, 0))
            .bg_active(Color::blue()),
    );
    tree.root_mut()
        .expect("window control root")
        .set_frame(Rect::new(0.0, 0.0, 44.0, 32.0));
    tree.layout();

    assert_eq!(render_control_corner(&tree), Color::red().premultiplied());
    tree.dispatch_event(&key_event(KeyCode::Tab, true));
    assert_eq!(
        render_control_corner(&tree),
        Color::from_rgb(255, 215, 0).premultiplied()
    );

    tree.dispatch_event(&key_event(KeyCode::Enter, true));
    assert_eq!(render_control_corner(&tree), Color::blue().premultiplied());
    tree.dispatch_event(&key_event(KeyCode::Enter, false));
    assert_eq!(
        render_control_corner(&tree),
        Color::from_rgb(255, 215, 0).premultiplied()
    );
    assert_eq!(tree.take_window_actions(), vec![WindowAction::RequestClose]);

    tree.dispatch_event(&pointer_event(
        Point::new(80.0, 16.0),
        true,
        MouseButton::Left,
    ));
    assert_eq!(render_control_corner(&tree), Color::red().premultiplied());
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
fn window_control_content_can_be_centered_in_its_bounds() {
    let mut tree = ViewAdapter::build(
        window_control(WindowControl::Minimize, label("control"))
            .width(60.0)
            .height(32.0)
            .align(AlignItems::Center)
            .justify(JustifyContent::Center),
    );
    let root = tree.root_id().expect("window control root");
    tree.root_mut()
        .expect("window control root")
        .set_frame(Rect::new(0.0, 0.0, 60.0, 32.0));
    tree.layout();

    let child = tree
        .get(root)
        .and_then(|node| node.children().first())
        .and_then(|id| tree.get(*id))
        .expect("window control content");
    let child_frame = child.frame();
    assert!((child_frame.x + child_frame.w * 0.5 - 30.0).abs() < 0.01);
    assert!((child_frame.y + child_frame.h * 0.5 - 16.0).abs() < 0.01);
}

#[test]
fn named_window_control_is_an_invokable_accessible_button() {
    let mut tree = ViewAdapter::build(
        window_control_named(
            WindowControl::Close,
            "关闭窗口",
            button("nested presentation"),
        )
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
    assert_eq!(
        snapshot
            .nodes
            .iter()
            .filter(|node| node.accessibility.role == AccessibilityRole::Button)
            .count(),
        1,
        "窗口控制的展示子树不得形成嵌套按钮语义"
    );

    tree.perform_semantic_action(root, &SemanticAction::Invoke)
        .expect("invoke close control");
    assert_eq!(tree.take_window_actions(), vec![WindowAction::RequestClose]);
}
use std::cell::Cell;
use std::rc::Rc;
