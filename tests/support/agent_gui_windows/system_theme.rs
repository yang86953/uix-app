use super::*;

#[test]
#[ignore = "requires an interactive Windows desktop and temporarily toggles AppsUseLightTheme"]
fn real_demo_follows_live_windows_system_theme_and_restores_preference() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut demo =
        DemoProcess::spawn_with_args(DEFAULT_VULKAN_GRAPHICS, &["--follow-system-theme"]);
    let descriptor = demo.wait_for_descriptor();
    let endpoint = descriptor["endpoint"]
        .as_str()
        .expect("descriptor endpoint");
    let token = descriptor["token"].as_str().expect("descriptor token");
    let stream = connect(endpoint, &mut demo.child);
    let mut connection = BufReader::new(stream);

    let hello = exchange(
        &mut connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "system-theme-hello",
            "type": "hello",
            "token": token,
            "client": { "name": "uix-demo-system-theme-acceptance" },
        }),
    );
    assert_success(&hello, "system-theme-hello");

    let listed = exchange(
        &mut connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "system-theme-list",
            "type": "list_windows",
        }),
    );
    assert_success(&listed, "system-theme-list");
    let windows = listed["windows"].as_array().expect("listed windows");
    assert_eq!(windows.len(), 1);
    let window_id = windows[0]["window_id"].as_u64().expect("window id");
    let generation = windows[0]["generation"].as_u64().expect("generation");

    let presented = exchange(
        &mut connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "system-theme-first-present",
            "type": "wait",
            "window_id": window_id,
            "generation": generation,
            "presented_revision": 1,
            "timeout_ms": PRESENT_TIMEOUT_MS,
        }),
    );
    assert_success(&presented, "system-theme-first-present");
    assert_eq!(presented["outcome"], "presented");

    let snapshot = exchange(
        &mut connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "system-theme-snapshot",
            "type": "snapshot",
            "window_id": window_id,
        }),
    );
    assert_success(&snapshot, "system-theme-snapshot");
    demo.assert_expected_dpi(&snapshot["snapshot"], "system-theme");
    assert_eq!(
        node_by_automation_id(&snapshot["snapshot"], "system-theme-follow-status")["name"],
        "跟随系统"
    );
    assert!(snapshot["snapshot"]["nodes"]
        .as_array()
        .expect("snapshot nodes")
        .iter()
        .all(|node| node["automation_id"] != "theme-toggle"));

    foreground::verify_system_theme_follow(&demo, &mut connection, window_id, generation);

    drop(connection);
    demo.close_and_wait();
}
