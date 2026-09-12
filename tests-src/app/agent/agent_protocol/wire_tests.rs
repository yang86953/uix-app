//! `app/agent/agent_protocol/wire.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

fn window(closed: bool) -> AgentWindowInfo {
    AgentWindowInfo {
        window_id: WindowId::new(7),
        generation: 3,
        title: "fixture".to_owned(),
        visible: !closed,
        presentable: !closed,
        focused: false,
        logical_width: 800,
        logical_height: 600,
        maximized: false,
        minimized: false,
        fullscreen: false,
        revision: 9,
        presented_revision: 8,
        closed,
    }
}

#[test]
fn only_closed_wait_terminal_requests_connection_close() {
    let changed = wait_success("changed".to_owned(), "changed", &window(false));
    assert!(!changed.close_connection());
    let closed = wait_success("closed".to_owned(), "closed", &window(true));
    assert!(closed.close_connection());
}

#[test]
fn window_wire_value_includes_negotiated_logical_size_and_modes() {
    let value = window_info_value(&window(false));
    assert_eq!(value["logical_width"], 800);
    assert_eq!(value["logical_height"], 600);
    assert_eq!(value["maximized"], false);
    assert_eq!(value["minimized"], false);
    assert_eq!(value["fullscreen"], false);
}
