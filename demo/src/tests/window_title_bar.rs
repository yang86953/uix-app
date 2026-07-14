use super::*;
use uix::ui::test_harness::{AutomationActionKind, TestApp};

#[test]
fn demo_title_bar_exposes_custom_window_controls() {
    let app = TestApp::new((INIT_W as f32, INIT_H as f32), || {
        super::super::window_title_bar::demo_title_bar()
    });
    let snapshot = app.snapshot();

    for (automation_id, accessible_name) in [
        ("window-control-minimize", "最小化窗口"),
        ("window-control-maximize-restore", "最大化或还原窗口"),
        ("window-control-close", "关闭窗口"),
    ] {
        let control = snapshot.find(automation_id).expect("title bar control");
        assert_eq!(
            control.accessibility.role,
            uix::prelude::AccessibilityRole::Button
        );
        assert_eq!(control.accessibility.name.as_deref(), Some(accessible_name));
        assert!(control.supports(AutomationActionKind::Invoke));
        assert!(control.is_visible());
    }

    let drag_region = snapshot
        .find("window-titlebar-drag")
        .expect("title bar drag region");
    assert!(drag_region.is_visible());
    assert!(drag_region.frame.w > 46.0 * 3.0);
}
