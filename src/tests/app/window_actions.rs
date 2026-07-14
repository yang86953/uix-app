use crate::app::window_actions::{apply_window_action, configure_custom_title_bar};
use crate::app::WindowConfig;
use crate::native::test_harness::FakeWindow;
use crate::ui::event::WindowAction;
use crate::ui::view::label;

#[test]
fn custom_title_bar_configuration_hides_caption_and_preserves_client_size() {
    let mut window = FakeWindow::new(1, "custom", 640, 480);

    configure_custom_title_bar(&mut window, 800, 600).expect("configure custom title bar");

    assert!(!window.props.state.system_title_bar_visible);
    assert_eq!(window.props.state.width, 800);
    assert_eq!(window.props.state.height, 600);
    assert_eq!(window.props.state.set_size_calls, vec![(800, 600)]);
}

#[test]
fn window_actions_target_only_the_supplied_window() {
    let mut target = FakeWindow::new(1, "target", 320, 240);
    let untouched = FakeWindow::new(2, "untouched", 320, 240);

    apply_window_action(&mut target, WindowAction::BeginMoveDrag).expect("begin drag");
    apply_window_action(&mut target, WindowAction::Minimize).expect("minimize");
    apply_window_action(&mut target, WindowAction::MaximizeRestore).expect("maximize");
    apply_window_action(&mut target, WindowAction::MaximizeRestore).expect("restore");
    apply_window_action(&mut target, WindowAction::ToggleMaximizeFromTitleBar)
        .expect("title bar maximize");
    apply_window_action(&mut target, WindowAction::ToggleMaximizeFromTitleBar)
        .expect("title bar restore");
    apply_window_action(&mut target, WindowAction::RequestClose).expect("request close");

    assert_eq!(target.props.state.begin_move_drag_calls, 1);
    assert!(!target.props.state.maximized);
    assert!(!target.props.state.minimized);
    assert!(target.state.close_requested);
    assert_eq!(untouched.props.state.begin_move_drag_calls, 0);
    assert!(!untouched.state.close_requested);
}

#[test]
fn secondary_window_custom_title_bar_is_opt_in() {
    let default = WindowConfig::new("default", 320, 240, || label("default"));
    let custom = WindowConfig::new("custom", 320, 240, || label("custom")).custom_title_bar(true);

    assert!(!default.custom_title_bar);
    assert!(custom.custom_title_bar);
}
