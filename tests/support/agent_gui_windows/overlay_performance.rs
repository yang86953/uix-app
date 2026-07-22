use super::*;

const CYCLES: usize = 12;

#[test]
#[ignore = "requires a real Windows desktop and measures repeated overlay presentation"]
fn real_demo_modal_and_drawer_first_frame_latency_does_not_accumulate() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut demo = DemoProcess::spawn_with_args(DEFAULT_VULKAN_GRAPHICS, &["--component-qa"]);
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
            "request_id": "overlay-performance-hello",
            "type": "hello",
            "token": token,
            "client": { "name": "uix-overlay-performance-regression" },
        }),
    );
    assert_success(&hello, "overlay-performance-hello");

    let listed = list_windows(&mut connection);
    assert_eq!(listed.len(), 1, "demo must start with one main window");
    let window_id = listed[0]["window_id"].as_u64().expect("window id");
    let generation = listed[0]["generation"].as_u64().expect("generation");
    wait_for_presented(&mut connection, window_id, generation, 1, "first-present");

    for component_id in ["drawer", "modal"] {
        navigate_to_case(&demo, &mut connection, window_id, generation, component_id);
        let overlay_automation_id = format!("component-qa-{component_id}-cancel");
        let mut first_frame_latencies = Vec::with_capacity(CYCLES);
        for cycle in 0..CYCLES {
            let current = wait_for_automation_state(
                &mut connection,
                window_id,
                "component-qa-target",
                true,
                &format!("{component_id}-{cycle}-closed"),
            );
            let target = node_by_automation_id(&current, "component-qa-target");
            let bounds = &target["visible_bounds"];
            let x = bounds["x"].as_f64().expect("target x")
                + bounds["w"].as_f64().expect("target width") * 0.5;
            let y = bounds["y"].as_f64().expect("target y")
                + bounds["h"].as_f64().expect("target height") * 0.5;

            let started = Instant::now();
            perform_and_wait(
                &demo,
                &mut connection,
                window_id,
                generation,
                &format!("{component_id}-{cycle}-open"),
                None,
                json!({ "kind": "click_at", "x": x, "y": y }),
            );
            first_frame_latencies.push(started.elapsed());
            let opened = wait_for_automation_state(
                &mut connection,
                window_id,
                &overlay_automation_id,
                true,
                &format!("{component_id}-{cycle}-opened"),
            );
            assert_eq!(
                node_by_automation_id(&opened, "component-qa-id")["name"],
                component_id,
                "opening an overlay must keep the selected component case"
            );
            assert!(
                automation_node_is_visible(&opened, &overlay_automation_id),
                "{component_id} cycle {cycle} did not expose its overlay content after opening"
            );

            perform_and_wait(
                &demo,
                &mut connection,
                window_id,
                generation,
                &format!("{component_id}-{cycle}-close"),
                None,
                json!({ "kind": "press_key", "key": "escape" }),
            );
            let closed = wait_for_automation_state(
                &mut connection,
                window_id,
                &overlay_automation_id,
                false,
                &format!("{component_id}-{cycle}-closed-again"),
            );
            assert!(
                !automation_node_is_visible(&closed, &overlay_automation_id),
                "{component_id} cycle {cycle} still exposed its overlay content after closing"
            );
        }
        assert_latency_stable(component_id, &first_frame_latencies);
    }

    let output = demo.close_and_wait();
    if std::env::var_os("UIX_PERF_PROBE").is_some() {
        let mut slow_full_frames = output
            .lines()
            .filter(|line| line.contains("frame_us=") && line.contains("backdrop_restore=1"))
            .collect::<Vec<_>>();
        slow_full_frames.sort_unstable_by_key(|line| {
            std::cmp::Reverse(probe_metric(line, "paint_cpu").unwrap_or_default())
        });
        assert!(
            !slow_full_frames.is_empty(),
            "performance probe did not observe an overlay backdrop restore: {output}"
        );
        for line in slow_full_frames.into_iter().take(12) {
            eprintln!("overlay backdrop frame: {line}");
        }
    }
    assert!(!output.contains("panicked at"), "demo panic: {output}");
}

fn probe_metric(line: &str, expected_key: &str) -> Option<u128> {
    line.split_whitespace().find_map(|field| {
        let (key, value) = field.split_once('=')?;
        (key == expected_key).then(|| value.parse().ok()).flatten()
    })
}

fn automation_node_is_visible(snapshot: &Value, automation_id: &str) -> bool {
    snapshot["nodes"]
        .as_array()
        .expect("snapshot nodes")
        .iter()
        .find(|node| node["automation_id"] == automation_id)
        .is_some_and(|node| !node["visible_bounds"].is_null())
}

fn wait_for_automation_state(
    connection: &mut BufReader<File>,
    window_id: u64,
    automation_id: &str,
    expected_visible: bool,
    request_prefix: &str,
) -> Value {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut attempt = 0;
    loop {
        let current = snapshot(
            connection,
            window_id,
            &format!("{request_prefix}-{attempt}"),
        );
        if automation_node_is_visible(&current, automation_id) == expected_visible {
            return current;
        }
        if Instant::now() >= deadline {
            let node = current["nodes"]
                .as_array()
                .expect("snapshot nodes")
                .iter()
                .find(|node| node["automation_id"] == automation_id);
            let component = current["nodes"]
                .as_array()
                .expect("snapshot nodes")
                .iter()
                .find(|node| node["automation_id"] == "component-qa-id");
            panic!(
                "automation node `{automation_id}` did not reach visible={expected_visible}; node={node:?}; component={component:?}"
            );
        }
        attempt += 1;
        thread::sleep(Duration::from_millis(50));
    }
}

fn navigate_to_case(
    demo: &DemoProcess,
    connection: &mut BufReader<File>,
    window_id: u64,
    generation: u64,
    expected: &str,
) {
    for index in 0..COMPONENT_VISUAL_CASE_COUNT {
        let current = snapshot(
            connection,
            window_id,
            &format!("navigate-{expected}-{index}"),
        );
        if node_by_automation_id(&current, "component-qa-id")["name"] == expected {
            return;
        }
        perform_and_wait(
            demo,
            connection,
            window_id,
            generation,
            &format!("navigate-{expected}-{index}-next"),
            Some(json!({ "automation_id": "component-qa-next" })),
            json!({ "kind": "invoke" }),
        );
    }
    panic!("component QA case `{expected}` was not found");
}

#[allow(clippy::too_many_arguments)]
fn perform_and_wait(
    _demo: &DemoProcess,
    connection: &mut BufReader<File>,
    window_id: u64,
    generation: u64,
    request_id: &str,
    target: Option<Value>,
    action: Value,
) {
    let mut request = json!({
        "schema": "uix.agent.v1",
        "request_id": request_id,
        "type": "perform",
        "window_id": window_id,
        "generation": generation,
        "action": action,
    });
    if let Some(target) = target {
        request["target"] = target;
    }
    let response = exchange(connection, request);
    assert_success(&response, request_id);
    let revision = response["revision"].as_u64().expect("perform revision");
    wait_for_presented(connection, window_id, generation, revision, request_id);
}

fn list_windows(connection: &mut BufReader<File>) -> Vec<Value> {
    let response = exchange(
        connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "overlay-performance-list",
            "type": "list_windows",
        }),
    );
    assert_success(&response, "overlay-performance-list");
    response["windows"]
        .as_array()
        .expect("listed windows")
        .clone()
}

fn snapshot(connection: &mut BufReader<File>, window_id: u64, request_id: &str) -> Value {
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
    window_id: u64,
    generation: u64,
    revision: u64,
    request_prefix: &str,
) {
    let request_id = format!("{request_prefix}-presented");
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
    assert_success(&response, &request_id);
    assert_eq!(response["outcome"], "presented");
}

fn assert_latency_stable(component_id: &str, samples: &[Duration]) {
    let window = 4;
    let first = median(&samples[..window]);
    let last = median(&samples[samples.len() - window..]);
    let allowed = (first.saturating_mul(3) / 2).max(first + Duration::from_millis(100));
    eprintln!(
        "{component_id} first-frame latency: first={first:?} last={last:?} samples={samples:?}"
    );
    assert!(
        last <= allowed,
        "{component_id} first-frame latency accumulated: first={first:?} last={last:?} allowed={allowed:?}"
    );
}

fn median(samples: &[Duration]) -> Duration {
    let mut samples = samples.to_vec();
    samples.sort_unstable();
    samples[samples.len() / 2]
}
