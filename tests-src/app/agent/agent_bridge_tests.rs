//! `app/agent/agent_bridge.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

#[test]
fn window_state_publication_updates_same_generation_without_native_identity() {
    let directory = AgentBridgeDirectory::default();
    assert!(directory.enable());
    let registration = directory
        .register_window(
            WindowId::new(7),
            "fixture".to_owned(),
            true,
            true,
            false,
            800,
            600,
            false,
            false,
            false,
        )
        .expect("enabled directory must register a window");
    registration.publish_window_state(AgentWindowState {
        visible: true,
        presentable: true,
        focused: true,
        logical_width: 1000,
        logical_height: 700,
        device_pixel_ratio: 1.0,
        maximized: true,
        minimized: false,
        fullscreen: false,
    });

    let windows = directory
        .list_windows()
        .expect("live directory must list windows");
    assert_eq!(windows.len(), 1);
    let window = &windows[0];
    assert_eq!((window.logical_width, window.logical_height), (1000, 700));
    assert!(window.maximized);
    assert!(!window.minimized);
    assert!(!window.fullscreen);
}
