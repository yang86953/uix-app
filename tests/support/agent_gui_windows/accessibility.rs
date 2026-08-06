//! S4-03 accessibility evidence collector.
//!
//! Walks every component QA case in a real window and persists the semantic
//! evidence fields (role / name / state / value / actions / focus / keyboard)
//! of the case target node as one JSON file per component. The per-component
//! captures are later assembled into the 110-row accessibility manifest by
//! `scripts/build_accessibility_manifest.py` (see
//! `docs/进度/无障碍证据.md` for the schema).

use super::*;
use std::path::Path;

/// Evidence root. Defaults to the same convention as component visual so the
/// directory is stable when the env var is absent.
fn evidence_root() -> PathBuf {
    std::env::var_os("UIX_ACCESSIBILITY_EVIDENCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/debug-captures/uix-accessibility"))
}

#[derive(Debug)]
struct CaseSemantics {
    index: usize,
    id: String,
    name: String,
    states: Vec<String>,
    target: Value,
    focus_node: Option<Value>,
}

/// Compact evidence projection of a semantic node: exactly the fields the
/// S4-03 schema promises per item (role/name/state/value/actions/focus/keyboard).
fn projection(node: &Value) -> Value {
    let state = node.get("state").cloned().unwrap_or(Value::Null);
    let actions = node.get("actions").cloned().unwrap_or_else(|| json!([]));
    json!({
        "role": node.get("role").cloned().unwrap_or(Value::Null),
        "name": node.get("name").cloned().unwrap_or(Value::Null),
        "state": state,
        "value": json!({
            "value_text": state.get("value_text").cloned().unwrap_or(Value::Null),
            "value_now": state.get("value_now").cloned().unwrap_or(Value::Null),
            "value_min": state.get("value_min").cloned().unwrap_or(Value::Null),
            "value_max": state.get("value_max").cloned().unwrap_or(Value::Null),
            "checked": state.get("checked").cloned().unwrap_or(Value::Null),
            "selected": state.get("selected").cloned().unwrap_or(Value::Null),
            "expanded": state.get("expanded").cloned().unwrap_or(Value::Null),
        }),
        "actions": actions,
        "focus": json!({
            "focused": node.get("focused").cloned().unwrap_or(json!(false)),
            "focusable": actions.as_array().is_some_and(|a| a.contains(&json!("focus"))),
        }),
        "keyboard": json!({
            "actions": actions,
            "multiline": state.get("multiline").cloned().unwrap_or(json!(false)),
            "required": state.get("required").cloned().unwrap_or(json!(false)),
        }),
    })
}

fn capture_case(
    demo: &DemoProcess,
    connection: &mut BufReader<File>,
    window_id: u64,
    generation: u64,
    index: usize,
) -> CaseSemantics {
    let current = snapshot(
        connection,
        &format!("accessibility-case-{index:02}"),
        window_id,
    );
    let total = node_name(node_by_automation_id(&current, "component-qa-total"));
    assert_eq!(total, COMPONENT_VISUAL_CASE_COUNT.to_string());
    let id = node_name(node_by_automation_id(&current, "component-qa-id"));
    let name = node_name(node_by_automation_id(&current, "component-qa-current"));
    let state_text = node_name(node_by_automation_id(&current, "component-qa-states"));
    let states = state_text
        .split_once("本组件：")
        .map(|(_, states)| states)
        .expect("component state metadata")
        .split(" / ")
        .map(str::trim)
        .filter(|state| !state.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    assert!(!id.is_empty() && !name.is_empty() && !states.is_empty());

    let target = node_by_automation_id(&current, "component-qa-target").clone();
    let target_actions = target["actions"].as_array().cloned().unwrap_or_default();

    // Focus evidence: focus the target (or a focusable descendant for
    // composites) and capture the focused node.
    let focus_node = if target_actions.contains(&json!("focus")) {
        perform_and_wait(
            demo,
            connection,
            window_id,
            generation,
            &format!("accessibility-{id}-focus"),
            Some(json!({ "automation_id": "component-qa-target" })),
            json!({ "kind": "focus" }),
        );
        let after = snapshot(
            connection,
            &format!("accessibility-{id}-focused"),
            window_id,
        );
        let focused = node_by_automation_id(&after, "component-qa-target").clone();
        assert_eq!(
            focused["focused"], true,
            "{name} must be focused after focus"
        );
        Some(focused)
    } else {
        None
    };

    CaseSemantics {
        index,
        id,
        name,
        states,
        target,
        focus_node,
    }
}

fn write_case_evidence(root: &Path, case: &CaseSemantics) {
    let dir = root.join("components");
    fs::create_dir_all(&dir).expect("create components dir");
    let file = dir.join(format!("{:02}-{}.json", case.index, case.id));
    let value = json!({
        "schema": "uix.accessibility.component.v1",
        "case_index": case.index,
        "case_id": case.id,
        "component": case.name,
        "declared_states": case.states,
        "target": projection(&case.target),
        "focus": case.focus_node.as_ref().map(projection).unwrap_or(Value::Null),
    });
    fs::write(
        &file,
        serde_json::to_string_pretty(&value).expect("serialize component evidence"),
    )
    .expect("write component evidence");
    eprintln!("accessibility component evidence: {}", file.display());
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes accessibility evidence"]
fn real_demo_captures_component_semantic_evidence() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let evidence_root = evidence_root();
    fs::create_dir_all(&evidence_root).expect("create accessibility evidence root");
    let components_dir = evidence_root.join("components");
    if let Ok(entries) = fs::read_dir(&components_dir) {
        for entry in entries.flatten() {
            fs::remove_file(entry.path()).expect("remove stale component evidence");
        }
    }

    let mut demo = DemoProcess::spawn_with_args(DEFAULT_D3D11_GRAPHICS, &["--component-qa"]);
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
            "request_id": "accessibility-hello",
            "type": "hello",
            "token": token,
            "client": { "name": "uix-accessibility-evidence" },
        }),
    );
    assert_success(&hello, "accessibility-hello");

    let listed = list_windows(&mut connection, "accessibility-list");
    assert_eq!(listed.len(), 1, "demo must start with one main window");
    let window_id = listed[0]["window_id"].as_u64().expect("window id");
    let generation = listed[0]["generation"].as_u64().expect("generation");
    wait_for_presented(
        &mut connection,
        "accessibility-first-present",
        window_id,
        generation,
        1,
    );

    for index in 0..COMPONENT_VISUAL_CASE_COUNT {
        let case = capture_case(&demo, &mut connection, window_id, generation, index);
        write_case_evidence(&evidence_root, &case);
        if index + 1 < COMPONENT_VISUAL_CASE_COUNT {
            invoke_and_wait(
                &demo,
                &mut connection,
                window_id,
                generation,
                "component-qa-next",
                &format!("accessibility-next-{index:02}"),
            );
        }
    }

    drop(connection);
    demo.close_and_wait();
    eprintln!(
        "accessibility component evidence root: {}; components={}",
        evidence_root.display(),
        COMPONENT_VISUAL_CASE_COUNT
    );
}

// ---- helpers mirroring the component visual harness (private per module) ----

/// PAGE_CHARTS index in `demo/gui-demo/src/common/page.rs` (sidebar automation id
/// ``sidebar-page-8``).
const CHARTS_PAGE_IDX: usize = 8;

/// Advanced chart scene titles rendered by `demo/gui-demo/src/demos/charts.rs`.
/// The collector matches semantic image nodes by these titles; the assembler
/// maps them back to ledger rows.
const CHARTS_SCENE_TITLES: &[&str] = &[
    "磁盘占用趋势", // AreaChart
    "散点分布",     // ScatterChart
    "气泡分布",     // ScatterChart (bubble)
    "能力雷达",     // RadarChart
    "热力矩阵",     // Heatmap
    "转化漏斗",     // FunnelChart
    "月度盈亏",     // WaterfallChart
    "双轴组合",     // ComboChart
    "磁盘占用",     // Treemap
    "完成度",       // Gauge
    "预算使用",     // Gauge
];

#[test]
#[ignore = "requires an interactive Windows desktop and writes accessibility evidence"]
fn real_demo_captures_charts_semantic_evidence() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let evidence_root = evidence_root();
    fs::create_dir_all(&evidence_root).expect("create accessibility evidence root");

    let mut demo = DemoProcess::spawn_with_args(DEFAULT_D3D11_GRAPHICS, &[]);
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
            "request_id": "charts-accessibility-hello",
            "type": "hello",
            "token": token,
            "client": { "name": "uix-charts-accessibility-evidence" },
        }),
    );
    assert_success(&hello, "charts-accessibility-hello");

    let listed = list_windows(&mut connection, "charts-accessibility-list");
    let window_id = listed[0]["window_id"].as_u64().expect("window id");
    let generation = listed[0]["generation"].as_u64().expect("generation");
    wait_for_presented(
        &mut connection,
        "charts-accessibility-first-present",
        window_id,
        generation,
        1,
    );

    // Navigate to the charts page by clicking the sidebar item (NavItem
    // exposes no semantic invoke; pointer click drives page switching).
    let nav_snapshot = snapshot(
        &mut connection,
        "charts-accessibility-nav-snapshot",
        window_id,
    );
    let nav_item =
        node_by_automation_id(&nav_snapshot, &format!("sidebar-page-{CHARTS_PAGE_IDX}")).clone();
    let bounds = &nav_item["visible_bounds"];
    let (x, y) = (
        bounds["x"].as_f64().expect("nav x") + bounds["w"].as_f64().expect("nav w") * 0.5,
        bounds["y"].as_f64().expect("nav y") + bounds["h"].as_f64().expect("nav h") * 0.5,
    );
    perform_and_wait(
        &demo,
        &mut connection,
        window_id,
        generation,
        "charts-accessibility-navigate",
        None,
        json!({ "kind": "click_at", "x": x, "y": y }),
    );

    let current = snapshot(&mut connection, "charts-accessibility-snapshot", window_id);
    let nodes = current["nodes"].as_array().expect("snapshot nodes");
    let mut captured: Vec<Value> = Vec::new();
    for node in nodes {
        if node["role"] != "image" {
            continue;
        }
        let name = node["name"].as_str().unwrap_or_default();
        if CHARTS_SCENE_TITLES.contains(&name) {
            captured.push(projection(node));
        }
    }

    let missing: Vec<&str> = CHARTS_SCENE_TITLES
        .iter()
        .copied()
        .filter(|title| {
            !captured
                .iter()
                .any(|node| node["name"].as_str() == Some(title))
        })
        .collect();
    assert!(
        missing.is_empty(),
        "charts scene missing semantic nodes for: {missing:?}"
    );

    let value = json!({
        "schema": "uix.accessibility.charts.v1",
        "captured_nodes": captured.len(),
        "nodes": captured,
    });
    fs::write(
        evidence_root.join("charts-evidence.json"),
        serde_json::to_string_pretty(&value).expect("serialize charts evidence"),
    )
    .expect("write charts evidence");
    eprintln!(
        "accessibility charts evidence: {}; nodes={}",
        evidence_root.join("charts-evidence.json").display(),
        captured.len()
    );

    drop(connection);
    demo.close_and_wait();
}

fn node_name(node: &Value) -> String {
    node["name"].as_str().unwrap_or_default().to_string()
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

fn perform_and_wait(
    demo: &DemoProcess,
    connection: &mut BufReader<File>,
    window_id: u64,
    generation: u64,
    request_id: &str,
    target: Option<Value>,
    action: Value,
) -> Value {
    let changed = perform_until_presentable(
        demo, connection, window_id, generation, request_id, target, action,
    );
    let revision = changed["revision"].as_u64().expect("perform revision");
    wait_for_presented(
        connection,
        &format!("{request_id}-presented"),
        window_id,
        generation,
        revision,
    );
    changed
}

fn invoke_and_wait(
    demo: &DemoProcess,
    connection: &mut BufReader<File>,
    window_id: u64,
    generation: u64,
    automation_id: &str,
    request_id: &str,
) {
    perform_and_wait(
        demo,
        connection,
        window_id,
        generation,
        request_id,
        Some(json!({ "automation_id": automation_id })),
        json!({ "kind": "invoke" }),
    );
}
