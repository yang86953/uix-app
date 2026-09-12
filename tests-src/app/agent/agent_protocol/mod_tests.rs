//! `app/agent/agent_protocol/mod.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

#[test]
fn hello_catalog_advertises_only_supported_background_window_actions() {
    let actions = [
        json!({ "kind": "resize_window", "width": 800, "height": 600 }),
        json!({ "kind": "close_window" }),
    ];
    for action in actions {
        let kind = action["kind"]
            .as_str()
            .unwrap_or_else(|| unreachable!("fixture action kind must be a string"));
        assert!(AGENT_WINDOW_ACTIONS.contains(&kind));
        assert!(matches!(
            parse_action(&action),
            Ok(ParsedAgentAction::Window(_))
        ));
    }
    // Legacy wire parsing is not authority to advertise native-window access
    // on the isolated background surface. Keep this prohibition explicit.
    for kind in [
        "move_window",
        "maximize_window",
        "minimize_window",
        "restore_window",
    ] {
        assert!(!AGENT_WINDOW_ACTIONS.contains(&kind));
    }
}

#[test]
fn hello_catalog_freezes_window_state_field_names() {
    assert_eq!(
        AGENT_WINDOW_STATE_FIELDS,
        [
            "logical_width",
            "logical_height",
            "maximized",
            "minimized",
            "fullscreen",
            "focused",
        ]
    );
}

#[test]
fn hello_catalog_freezes_request_types_and_screenshot_capability() {
    assert_eq!(
        AGENT_REQUEST_TYPES,
        [
            "hello",
            "list_windows",
            "snapshot",
            "screenshot",
            "perform",
            "confirm",
            "wait",
        ]
    );
}
