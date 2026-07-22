use super::*;

const RECOVERY_STATUS_ID: &str = "runtime-graphics-recovery-status";
const INJECT_DEVICE_LOST_ID: &str = "runtime-inject-device-lost";
const VERIFY_RECOVERED_ID: &str = "runtime-verify-recovered-interaction";

#[test]
#[ignore = "requires an interactive Windows desktop and the test-harness feature"]
fn real_demo_recovers_from_injected_device_loss_and_accepts_followup_interaction() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut demo =
        DemoProcess::spawn_with_args(DEFAULT_VULKAN_GRAPHICS, &["--graphics-recovery-acceptance"]);
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
            "request_id": "graphics-recovery-hello",
            "type": "hello",
            "token": token,
            "client": { "name": "uix-demo-graphics-recovery-acceptance" },
        }),
    );
    assert_success(&hello, "graphics-recovery-hello");

    let listed = exchange(
        &mut connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "graphics-recovery-list",
            "type": "list_windows",
        }),
    );
    assert_success(&listed, "graphics-recovery-list");
    let windows = listed["windows"].as_array().expect("listed windows");
    assert_eq!(windows.len(), 1);
    let window_id = windows[0]["window_id"].as_u64().expect("window id");
    let generation = windows[0]["generation"].as_u64().expect("generation");
    wait_for_presented(
        &mut connection,
        "graphics-recovery-first-present",
        window_id,
        generation,
        1,
    );

    let home = snapshot(&mut connection, "graphics-recovery-home", window_id);
    demo.assert_expected_dpi(&home, "graphics-recovery");
    let (runtime_x, runtime_y) = visible_center(node_by_automation_id(&home, "sidebar-page-1"));
    let navigated = perform_until_presentable(
        &demo,
        &mut connection,
        window_id,
        generation,
        "graphics-recovery-open-runtime",
        None,
        json!({ "kind": "click_at", "x": runtime_x, "y": runtime_y }),
    );
    wait_for_presented(
        &mut connection,
        "graphics-recovery-runtime-presented",
        window_id,
        generation,
        navigated["revision"].as_u64().expect("runtime revision"),
    );

    let before = snapshot(&mut connection, "graphics-recovery-before", window_id);
    assert_eq!(
        node_by_automation_id(&before, RECOVERY_STATUS_ID)["name"],
        "图形恢复验收：等待注入"
    );
    assert_eq!(
        node_by_automation_id(&before, VERIFY_RECOVERED_ID)["state"]["disabled"],
        true
    );

    let injected = invoke_until_presentable(
        &demo,
        &mut connection,
        window_id,
        generation,
        INJECT_DEVICE_LOST_ID,
    );
    let injected_revision = injected["revision"]
        .as_u64()
        .expect("injected status revision");
    wait_for_presented(
        &mut connection,
        "graphics-recovery-injected-presented",
        window_id,
        generation,
        injected_revision,
    );

    let recovered = snapshot(&mut connection, "graphics-recovery-recovered", window_id);
    assert_eq!(
        node_by_automation_id(&recovered, RECOVERY_STATUS_ID)["name"],
        "图形恢复验收：已注入，等待恢复后交互"
    );
    assert_eq!(
        node_by_automation_id(&recovered, VERIFY_RECOVERED_ID)["state"]["disabled"],
        false
    );

    let verified = invoke_until_presentable(
        &demo,
        &mut connection,
        window_id,
        generation,
        VERIFY_RECOVERED_ID,
    );
    wait_for_presented(
        &mut connection,
        "graphics-recovery-followup-presented",
        window_id,
        generation,
        verified["revision"]
            .as_u64()
            .expect("verified status revision"),
    );
    let after = snapshot(&mut connection, "graphics-recovery-after", window_id);
    assert_eq!(
        node_by_automation_id(&after, RECOVERY_STATUS_ID)["name"],
        "图形恢复验收：恢复后交互成功"
    );

    drop(connection);
    let output = demo.close_and_wait();
    assert!(
        output.contains("test-harness injected graphics device loss"),
        "real window run must reach the typed injected failure boundary; output={output}"
    );
    println!(
        "graphics recovery capture: backend=vulkan; typed_device_lost=true; recovered_revision={injected_revision}; followup_revision={}",
        verified["revision"]
            .as_u64()
            .expect("verified status revision")
    );
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
