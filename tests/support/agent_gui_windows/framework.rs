use super::*;

const FRAMEWORK_PAGE_ID: &str = "sidebar-page-10";
const FRAMEWORK_SCROLL_ID: &str = "framework-scroll";
const LOCALE_STATUS_ID: &str = "framework-locale-status";
const LOCALE_SAMPLE_ID: &str = "framework-locale-sample";
const LOCALE_EN_ID: &str = "framework-locale-en";
const PROVIDER_LARGE_BUTTON_ID: &str = "framework-provider-large-button";
const PROVIDER_SMALL_BUTTON_ID: &str = "framework-provider-small-button";
const TOKEN_BUTTON_ID: &str = "framework-token-button";
const EMPTY_SAMPLE_ID: &str = "framework-empty-sample";

#[test]
#[ignore = "requires an interactive Windows desktop"]
fn real_demo_switches_provider_locale_and_presents_framework_capabilities() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    // Exercise the shipping default Auto -> D3D11 GPU-native swapchain.
    // Desktop captures sample the DWM-composited client output.
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
            "request_id": "framework-hello",
            "type": "hello",
            "token": token,
            "client": { "name": "uix-demo-framework-acceptance" },
        }),
    );
    assert_success(&hello, "framework-hello");

    let listed = list_windows(&mut connection, "framework-list");
    assert_eq!(listed.len(), 1, "demo must start with one main window");
    let window_id = listed[0]["window_id"].as_u64().expect("window id");
    let generation = listed[0]["generation"].as_u64().expect("generation");
    wait_for_presented(
        &mut connection,
        "framework-first-present",
        window_id,
        generation,
        1,
    );

    let home = snapshot(&mut connection, "framework-home", window_id);
    demo.assert_expected_dpi(&home, "framework");
    capture_if_requested(&demo, "00-agent-home.png");
    let (framework_x, framework_y) =
        visible_center(node_by_automation_id(&home, FRAMEWORK_PAGE_ID));
    let navigated = perform_until_presentable(
        &demo,
        &mut connection,
        window_id,
        generation,
        "framework-open-page",
        None,
        json!({ "kind": "click_at", "x": framework_x, "y": framework_y }),
    );
    wait_for_presented(
        &mut connection,
        "framework-page-presented",
        window_id,
        generation,
        navigated["revision"].as_u64().expect("framework revision"),
    );

    let zh = snapshot(&mut connection, "framework-zh", window_id);
    assert_eq!(
        node_by_automation_id(&zh, LOCALE_STATUS_ID)["name"],
        "当前语言：简体中文"
    );
    assert_eq!(
        node_by_automation_id(&zh, LOCALE_SAMPLE_ID)["name"],
        "Locale.empty_description：暂无数据"
    );
    assert_eq!(
        node_by_automation_id(&zh, EMPTY_SAMPLE_ID)["name"],
        "List 自定义空态"
    );
    assert!(node_by_automation_id(&zh, LOCALE_EN_ID)["actions"]
        .as_array()
        .is_some_and(|actions| actions.contains(&json!("invoke"))));
    let large_height = node_by_automation_id(&zh, PROVIDER_LARGE_BUTTON_ID)["visible_bounds"]["h"]
        .as_f64()
        .expect("large provider button height");
    let small_height = node_by_automation_id(&zh, PROVIDER_SMALL_BUTTON_ID)["visible_bounds"]["h"]
        .as_f64()
        .expect("small provider button height");
    assert!(
        large_height > small_height,
        "Provider Large must exceed explicit Small: {large_height} <= {small_height}"
    );
    capture_if_requested(&demo, "02-framework-zh.png");

    let switched =
        invoke_until_presentable(&demo, &mut connection, window_id, generation, LOCALE_EN_ID);
    wait_for_presented(
        &mut connection,
        "framework-english-presented",
        window_id,
        generation,
        switched["revision"].as_u64().expect("English revision"),
    );
    let english = snapshot(&mut connection, "framework-english", window_id);
    assert_eq!(
        node_by_automation_id(&english, LOCALE_STATUS_ID)["name"],
        "当前语言：English"
    );
    assert_eq!(
        node_by_automation_id(&english, LOCALE_SAMPLE_ID)["name"],
        "Locale.empty_description：No data"
    );
    capture_if_requested(&demo, "03-framework-en.png");

    let scroll = node_by_automation_id(&english, FRAMEWORK_SCROLL_ID);
    assert!(scroll["actions"]
        .as_array()
        .is_some_and(|actions| actions.contains(&json!("scroll"))));
    let scrolled = perform_until_presentable(
        &demo,
        &mut connection,
        window_id,
        generation,
        "framework-scroll-lower",
        Some(json!({ "automation_id": FRAMEWORK_SCROLL_ID })),
        json!({ "kind": "scroll", "delta_x": 0.0, "delta_y": 560.0 }),
    );
    wait_for_presented(
        &mut connection,
        "framework-lower-presented",
        window_id,
        generation,
        scrolled["revision"].as_u64().expect("scroll revision"),
    );
    let lower = snapshot(&mut connection, "framework-lower", window_id);
    for automation_id in [TOKEN_BUTTON_ID, EMPTY_SAMPLE_ID] {
        let visible = &node_by_automation_id(&lower, automation_id)["visible_bounds"];
        assert!(
            visible["w"].as_f64().unwrap_or_default() > 0.0
                && visible["h"].as_f64().unwrap_or_default() > 0.0,
            "{automation_id} must be visible after scrolling: {visible}"
        );
    }
    capture_if_requested(&demo, "04-framework-lower.png");

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

fn capture_if_requested(demo: &DemoProcess, file_name: &str) {
    let Some(root) = std::env::var_os("UIX_GUI_EVIDENCE_DIR") else {
        return;
    };
    let path = PathBuf::from(root).join(file_name);
    foreground::capture_demo_client_png(demo, &path);
    eprintln!("GUI evidence: {}", path.display());
}
