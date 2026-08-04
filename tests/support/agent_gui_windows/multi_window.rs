use super::*;

const MAIN_THEME_STATE_ID: &str = "runtime-theme-state";
const CHILD_THEME_STATE_ID: &str = "theme-window-theme-state";

#[test]
#[ignore = "requires an interactive Windows desktop"]
fn real_demo_opens_theme_window_and_syncs_observable_state() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut demo = DemoProcess::spawn(DEFAULT_D3D11_GRAPHICS);
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
            "request_id": "multi-window-hello",
            "type": "hello",
            "token": token,
            "client": { "name": "uix-demo-multi-window-acceptance" },
        }),
    );
    assert_success(&hello, "multi-window-hello");

    let initial = list_windows(&mut connection, "multi-window-initial");
    assert_eq!(
        initial.len(),
        1,
        "demo must start with only the main window"
    );
    let main_window_id = initial[0]["window_id"].as_u64().expect("main window id");
    let main_generation = initial[0]["generation"].as_u64().expect("main generation");
    wait_for_presented(
        &mut connection,
        "multi-window-main-presented",
        main_window_id,
        main_generation,
        1,
    );

    let main_before = snapshot(
        &mut connection,
        "multi-window-main-before-navigation",
        main_window_id,
    );
    demo.assert_expected_dpi(&main_before, "multi-window-main");
    let (runtime_x, runtime_y) =
        visible_center(node_by_automation_id(&main_before, "sidebar-page-1"));
    let opened_runtime = perform_until_presentable(
        &demo,
        &mut connection,
        main_window_id,
        main_generation,
        "multi-window-open-runtime-page",
        None,
        json!({ "kind": "click_at", "x": runtime_x, "y": runtime_y }),
    );
    wait_for_presented(
        &mut connection,
        "multi-window-runtime-presented",
        main_window_id,
        main_generation,
        opened_runtime["revision"]
            .as_u64()
            .expect("runtime page revision"),
    );

    let runtime_snapshot = snapshot(&mut connection, "multi-window-runtime", main_window_id);
    assert_eq!(
        node_by_automation_id(&runtime_snapshot, MAIN_THEME_STATE_ID)["name"],
        "当前主题：亮色"
    );
    let open_target = node_by_automation_id(&runtime_snapshot, "runtime-open-theme-window");
    assert_eq!(open_target["role"], "button");
    assert!(open_target["actions"]
        .as_array()
        .is_some_and(|actions| actions.contains(&json!("invoke"))));

    let opened_child = invoke_until_presentable(
        &demo,
        &mut connection,
        main_window_id,
        main_generation,
        "runtime-open-theme-window",
    );
    let main_revision_before_toggle = opened_child["revision"]
        .as_u64()
        .expect("main revision after opening child");
    let child = wait_for_window_title(&mut connection, "UIX Theme Window");
    let child_window_id = child["window_id"].as_u64().expect("child window id");
    let child_generation = child["generation"].as_u64().expect("child generation");
    wait_for_presented(
        &mut connection,
        "multi-window-child-presented",
        child_window_id,
        child_generation,
        1,
    );

    let child_before = snapshot(
        &mut connection,
        "multi-window-child-before",
        child_window_id,
    );
    assert_eq!(
        node_by_automation_id(&child_before, CHILD_THEME_STATE_ID)["name"],
        "当前主题：亮色"
    );
    assert_eq!(
        node_by_automation_id(&child_before, "theme-window-close")["name"],
        "关闭主题联动窗口"
    );

    let (toggle_x, toggle_y) = visible_center(node_by_automation_id(
        &child_before,
        "theme-window-theme-toggle",
    ));
    let toggled = perform_until_presentable(
        &demo,
        &mut connection,
        child_window_id,
        child_generation,
        "multi-window-toggle-child-theme",
        None,
        json!({ "kind": "click_at", "x": toggle_x, "y": toggle_y }),
    );
    wait_for_presented(
        &mut connection,
        "multi-window-child-theme-presented",
        child_window_id,
        child_generation,
        toggled["revision"].as_u64().expect("child theme revision"),
    );
    let main_changed = wait_for_changed(
        &mut connection,
        "multi-window-main-theme-changed",
        main_window_id,
        main_generation,
        main_revision_before_toggle,
    );
    wait_for_presented(
        &mut connection,
        "multi-window-main-theme-presented",
        main_window_id,
        main_generation,
        main_changed,
    );

    let child_after = snapshot(&mut connection, "multi-window-child-after", child_window_id);
    let main_after = snapshot(&mut connection, "multi-window-main-after", main_window_id);
    assert_eq!(
        node_by_automation_id(&child_after, CHILD_THEME_STATE_ID)["name"],
        "当前主题：暗色"
    );
    assert_eq!(
        node_by_automation_id(&main_after, MAIN_THEME_STATE_ID)["name"],
        "当前主题：暗色"
    );

    let _ = invoke_until_presentable(
        &demo,
        &mut connection,
        child_window_id,
        child_generation,
        "theme-window-close",
    );
    wait_for_window_count(&mut connection, 1);
    drop(connection);
    demo.close_and_wait();
}

fn list_windows(connection: &mut BufReader<File>, request_id: &str) -> Vec<Value> {
    let response = exchange(
        connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": request_id,
            "type": "list_windows",
        }),
    );
    assert_success(&response, request_id);
    response["windows"]
        .as_array()
        .expect("listed windows")
        .clone()
}

fn snapshot(connection: &mut BufReader<File>, request_id: &str, window_id: u64) -> Value {
    let response = exchange(
        connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": request_id,
            "type": "snapshot",
            "window_id": window_id,
        }),
    );
    assert_success(&response, request_id);
    response["snapshot"].clone()
}

fn wait_for_presented(
    connection: &mut BufReader<File>,
    request_id: &str,
    window_id: u64,
    generation: u64,
    revision: u64,
) {
    let response = exchange(
        connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": request_id,
            "type": "wait",
            "window_id": window_id,
            "generation": generation,
            "presented_revision": revision,
            "timeout_ms": PRESENT_TIMEOUT_MS,
        }),
    );
    assert_success(&response, request_id);
    assert_eq!(response["outcome"], "presented");
}

fn wait_for_changed(
    connection: &mut BufReader<File>,
    request_id: &str,
    window_id: u64,
    generation: u64,
    revision: u64,
) -> u64 {
    let response = exchange(
        connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": request_id,
            "type": "wait",
            "window_id": window_id,
            "generation": generation,
            "after_revision": revision,
            "timeout_ms": PRESENT_TIMEOUT_MS,
        }),
    );
    assert_success(&response, request_id);
    assert_eq!(response["outcome"], "changed");
    response["window"]["revision"]
        .as_u64()
        .expect("changed revision")
}

fn wait_for_window_title(connection: &mut BufReader<File>, title: &str) -> Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut attempt = 0u32;
    loop {
        let windows = list_windows(connection, &format!("multi-window-list-{attempt}"));
        if let Some(window) = windows.into_iter().find(|window| window["title"] == title) {
            return window;
        }
        assert!(
            Instant::now() < deadline,
            "secondary window `{title}` did not appear"
        );
        attempt = attempt.wrapping_add(1);
        thread::sleep(Duration::from_millis(25));
    }
}

fn wait_for_window_count(connection: &mut BufReader<File>, expected: usize) {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut attempt = 0u32;
    loop {
        let windows = list_windows(connection, &format!("multi-window-close-{attempt}"));
        if windows.len() == expected {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "window count did not become {expected}"
        );
        attempt = attempt.wrapping_add(1);
        thread::sleep(Duration::from_millis(25));
    }
}
