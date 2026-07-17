use super::*;
use std::path::Path;

const PAGE_SLUGS: [&str; 13] = [
    "home",
    "runtime",
    "general",
    "layout",
    "navigation",
    "input",
    "data",
    "feedback",
    "charts",
    "other",
    "framework",
    "gallery",
    "component-qa",
];
const COMPACT_PAGE_INDEXES: [usize; 7] = [0, 3, 5, 6, 7, 10, 11];
const DEFAULT_WINDOW_SIZE: (i32, i32) = (1200, 800);
const COMPACT_WINDOW_SIZE: (i32, i32) = (900, 640);

#[test]
#[ignore = "requires an interactive Windows desktop and writes visual evidence"]
fn real_demo_captures_all_pages_for_visual_review() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let evidence_root = evidence_root();
    let mut demo = DemoProcess::spawn(DEFAULT_VULKAN_GRAPHICS);
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
            "request_id": "visual-hello",
            "type": "hello",
            "token": token,
            "client": { "name": "uix-demo-visual-acceptance" },
        }),
    );
    assert_success(&hello, "visual-hello");

    let listed = list_windows(&mut connection, "visual-list");
    assert_eq!(listed.len(), 1, "demo must start with one main window");
    let window_id = listed[0]["window_id"].as_u64().expect("window id");
    let generation = listed[0]["generation"].as_u64().expect("generation");
    wait_for_presented(
        &mut connection,
        "visual-first-present",
        window_id,
        generation,
        1,
    );
    let settled = snapshot(&mut connection, "visual-initial-settle", window_id);
    wait_for_presented(
        &mut connection,
        "visual-initial-settle-presented",
        window_id,
        generation,
        settled["revision"].as_u64().expect("settled revision"),
    );

    for (page_index, slug) in PAGE_SLUGS.iter().enumerate() {
        navigate_to_page(&demo, &mut connection, window_id, generation, page_index);
        capture(
            &demo,
            &evidence_root,
            &format!("desktop-1200x800-{page_index:02}-{slug}-top.png"),
        );
        scroll_page_to_bottom(&demo, &mut connection, window_id, generation, page_index);
        capture(
            &demo,
            &evidence_root,
            &format!("desktop-1200x800-{page_index:02}-{slug}-bottom.png"),
        );
    }

    for page_index in COMPACT_PAGE_INDEXES {
        let slug = PAGE_SLUGS[page_index];
        navigate_to_page(&demo, &mut connection, window_id, generation, page_index);
        resize_and_wait(
            &demo,
            &mut connection,
            window_id,
            generation,
            COMPACT_WINDOW_SIZE,
            &format!("visual-compact-{page_index}-resize"),
        );
        capture(
            &demo,
            &evidence_root,
            &format!("compact-900x640-{page_index:02}-{slug}-top.png"),
        );
        scroll_page_to_bottom(&demo, &mut connection, window_id, generation, page_index);
        capture(
            &demo,
            &evidence_root,
            &format!("compact-900x640-{page_index:02}-{slug}-bottom.png"),
        );
        resize_and_wait(
            &demo,
            &mut connection,
            window_id,
            generation,
            DEFAULT_WINDOW_SIZE,
            &format!("visual-default-{page_index}-restore"),
        );
    }

    capture_compact_sidebar_state(
        &demo,
        &mut connection,
        window_id,
        generation,
        &evidence_root,
    );
    capture_feedback_modal_states(
        &demo,
        &mut connection,
        window_id,
        generation,
        &evidence_root,
        "light",
        true,
    );

    let themed = invoke_until_presentable(
        &demo,
        &mut connection,
        window_id,
        generation,
        "theme-toggle",
    );
    wait_for_presented(
        &mut connection,
        "visual-dark-theme-presented",
        window_id,
        generation,
        themed["revision"].as_u64().expect("theme revision"),
    );
    navigate_to_page(&demo, &mut connection, window_id, generation, 0);
    scroll_page_to_top(&demo, &mut connection, window_id, generation, 0);
    capture(&demo, &evidence_root, "state-dark-home-top.png");
    navigate_to_page(&demo, &mut connection, window_id, generation, 5);
    scroll_page_to_top(&demo, &mut connection, window_id, generation, 5);
    capture(&demo, &evidence_root, "state-dark-input-top.png");
    scroll_page_to_bottom(&demo, &mut connection, window_id, generation, 5);
    capture(&demo, &evidence_root, "state-dark-input-bottom.png");
    navigate_to_page(&demo, &mut connection, window_id, generation, 6);
    scroll_page_to_bottom(&demo, &mut connection, window_id, generation, 6);
    capture(&demo, &evidence_root, "state-dark-data-bottom.png");
    capture_feedback_modal_states(
        &demo,
        &mut connection,
        window_id,
        generation,
        &evidence_root,
        "dark",
        false,
    );

    drop(connection);
    demo.close_and_wait();
    eprintln!("visual evidence root: {}", evidence_root.display());
}

fn evidence_root() -> PathBuf {
    std::env::var_os("UIX_VISUAL_EVIDENCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/debug-captures/uix-demo-visual"))
}

fn capture(demo: &DemoProcess, root: &Path, file_name: &str) {
    let path = root.join(file_name);
    foreground::capture_demo_client_png(demo, &path);
    eprintln!("GUI evidence: {}", path.display());
}

fn navigate_to_page(
    demo: &DemoProcess,
    connection: &mut BufReader<File>,
    window_id: u64,
    generation: u64,
    page_index: usize,
) {
    let mut before = snapshot(
        connection,
        &format!("visual-page-{page_index}-before"),
        window_id,
    );
    let automation_id = format!("sidebar-page-{page_index}");
    let mut target = node_by_automation_id(&before, &automation_id);
    if target["state"]["selected"] == true {
        return;
    }
    if target["visible_bounds"].is_null() {
        scroll_target(
            demo,
            connection,
            window_id,
            generation,
            "sidebar-scroll",
            if page_index < PAGE_SLUGS.len() / 2 {
                -10_000.0
            } else {
                10_000.0
            },
            &format!("visual-page-{page_index}-sidebar-reveal"),
        );
        before = snapshot(
            connection,
            &format!("visual-page-{page_index}-revealed"),
            window_id,
        );
        target = node_by_automation_id(&before, &automation_id);
        assert!(
            !target["visible_bounds"].is_null(),
            "sidebar target must be visible after reveal: {target}"
        );
    }
    let (x, y) = visible_center(target);
    let changed = perform_until_presentable(
        demo,
        connection,
        window_id,
        generation,
        &format!("visual-page-{page_index}-open"),
        None,
        json!({ "kind": "click_at", "x": x, "y": y }),
    );
    wait_for_presented(
        connection,
        &format!("visual-page-{page_index}-presented"),
        window_id,
        generation,
        changed["revision"].as_u64().expect("page revision"),
    );
}

fn scroll_page_to_bottom(
    demo: &DemoProcess,
    connection: &mut BufReader<File>,
    window_id: u64,
    generation: u64,
    page_index: usize,
) {
    let automation_id = format!("page-scroll-{page_index}");
    scroll_target(
        demo,
        connection,
        window_id,
        generation,
        &automation_id,
        10_000.0,
        &format!("visual-page-{page_index}-scroll-bottom"),
    );
}

fn scroll_page_to_top(
    demo: &DemoProcess,
    connection: &mut BufReader<File>,
    window_id: u64,
    generation: u64,
    page_index: usize,
) {
    scroll_target(
        demo,
        connection,
        window_id,
        generation,
        &format!("page-scroll-{page_index}"),
        -10_000.0,
        &format!("visual-page-{page_index}-scroll-top"),
    );
}

fn scroll_target(
    demo: &DemoProcess,
    connection: &mut BufReader<File>,
    window_id: u64,
    generation: u64,
    automation_id: &str,
    delta_y: f32,
    request_id: &str,
) {
    let before = snapshot(connection, &format!("{request_id}-before"), window_id);
    let target = node_by_automation_id(&before, automation_id);
    assert!(target["actions"]
        .as_array()
        .is_some_and(|actions| actions.contains(&json!("scroll"))));
    demo.raise_for_interaction();
    let changed = exchange(
        connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": request_id,
            "type": "perform",
            "window_id": window_id,
            "generation": generation,
            "target": { "automation_id": automation_id },
            "action": { "kind": "scroll", "delta_x": 0.0, "delta_y": delta_y },
        }),
    );
    if changed["ok"] != true {
        assert_eq!(
            changed["error"]["code"], "internal",
            "unexpected visual scroll failure: {changed}"
        );
        eprintln!("visual target {automation_id} has no overflow in the requested direction");
        return;
    }
    assert_success(&changed, request_id);
    wait_for_presented(
        connection,
        &format!("{request_id}-presented"),
        window_id,
        generation,
        changed["revision"].as_u64().expect("scroll revision"),
    );
}

fn capture_compact_sidebar_state(
    demo: &DemoProcess,
    connection: &mut BufReader<File>,
    window_id: u64,
    generation: u64,
    root: &Path,
) {
    navigate_to_page(demo, connection, window_id, generation, 0);
    resize_and_wait(
        demo,
        connection,
        window_id,
        generation,
        COMPACT_WINDOW_SIZE,
        "visual-compact-sidebar-resize",
    );
    scroll_target(
        demo,
        connection,
        window_id,
        generation,
        "sidebar-scroll",
        10_000.0,
        "visual-compact-sidebar-bottom",
    );
    capture(demo, root, "state-compact-sidebar-bottom.png");
    resize_and_wait(
        demo,
        connection,
        window_id,
        generation,
        DEFAULT_WINDOW_SIZE,
        "visual-compact-sidebar-restore",
    );
}

#[allow(clippy::too_many_arguments)]
fn capture_feedback_modal_states(
    demo: &DemoProcess,
    connection: &mut BufReader<File>,
    window_id: u64,
    generation: u64,
    root: &Path,
    theme: &str,
    capture_compact: bool,
) {
    navigate_to_page(demo, connection, window_id, generation, 7);
    scroll_page_to_top(demo, connection, window_id, generation, 7);
    let before = snapshot(
        connection,
        &format!("visual-{theme}-modal-before"),
        window_id,
    );
    let modal = node_by_automation_id(&before, "feedback-focus-modal");
    let bounds = &modal["visible_bounds"];
    let x = bounds["x"].as_f64().expect("modal trigger x") + 48.0;
    let y = bounds["y"].as_f64().expect("modal trigger y") + 16.0;
    let opened = perform_until_presentable(
        demo,
        connection,
        window_id,
        generation,
        &format!("visual-{theme}-modal-open"),
        None,
        json!({ "kind": "click_at", "x": x, "y": y }),
    );
    wait_for_presented(
        connection,
        &format!("visual-{theme}-modal-open-presented"),
        window_id,
        generation,
        opened["revision"].as_u64().expect("modal open revision"),
    );
    thread::sleep(Duration::from_millis(300));
    capture(demo, root, &format!("state-{theme}-feedback-modal.png"));

    if capture_compact {
        resize_and_wait(
            demo,
            connection,
            window_id,
            generation,
            COMPACT_WINDOW_SIZE,
            "visual-light-modal-compact-resize",
        );
        thread::sleep(Duration::from_millis(300));
        capture(demo, root, "state-light-feedback-modal-compact.png");
        resize_and_wait(
            demo,
            connection,
            window_id,
            generation,
            DEFAULT_WINDOW_SIZE,
            "visual-light-modal-default-restore",
        );
    }

    let closed = perform_until_presentable(
        demo,
        connection,
        window_id,
        generation,
        &format!("visual-{theme}-modal-close"),
        None,
        json!({ "kind": "press_key", "key": "escape" }),
    );
    wait_for_presented(
        connection,
        &format!("visual-{theme}-modal-close-presented"),
        window_id,
        generation,
        closed["revision"].as_u64().expect("modal close revision"),
    );
    thread::sleep(Duration::from_millis(300));
}

fn resize_and_wait(
    demo: &DemoProcess,
    connection: &mut BufReader<File>,
    window_id: u64,
    generation: u64,
    size: (i32, i32),
    request_id: &str,
) {
    let before = snapshot(connection, &format!("{request_id}-before"), window_id);
    let presented_revision = before["presented_revision"]
        .as_u64()
        .expect("presented revision");
    foreground::resize_demo_window(demo, size.0, size.1);
    wait_for_presented(
        connection,
        request_id,
        window_id,
        generation,
        presented_revision + 1,
    );
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
