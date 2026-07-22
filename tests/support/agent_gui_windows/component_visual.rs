use super::component_visual_manifest::{CaseKind, COMPONENT_VISUAL_CASES};
use super::*;
use image::{GenericImage, Rgba, RgbaImage};
use std::path::{Path, PathBuf};

const DEFAULT_WINDOW_SIZE: (i32, i32) = (1200, 800);
const COMPACT_WINDOW_SIZE: (i32, i32) = (900, 640);
const SELECT_OPEN_QUERY: &str = "al";
const SEMANTIC_INVOKE_OPEN_CASE_IDS: &[&str] =
    &["drawer", "modal", "popconfirm", "popover", "dropdown"];

#[derive(Debug)]
struct ObservedCase {
    index: usize,
    id: String,
    name: String,
    states: Vec<String>,
    evidence: Vec<(String, String)>,
}

#[test]
fn component_visual_denominator_is_derived_from_the_demo_manifest() {
    let widget_count = COMPONENT_VISUAL_CASES
        .iter()
        .filter(|case| case.kind == CaseKind::Widget)
        .count();
    let composite_count = COMPONENT_VISUAL_CASES
        .iter()
        .filter(|case| case.kind == CaseKind::Composite)
        .count();
    let provider_count = COMPONENT_VISUAL_CASES
        .iter()
        .filter(|case| case.kind == CaseKind::Provider)
        .count();

    assert_eq!((widget_count, composite_count, provider_count), (87, 3, 2));
    assert_eq!(
        COMPONENT_VISUAL_CASE_COUNT,
        widget_count + composite_count + provider_count
    );
    assert_eq!(COMPONENT_VISUAL_CASE_COUNT, 92, "frozen 0.0.1 inventory");
    assert!(COMPONENT_VISUAL_CASES.iter().all(|case| {
        !case.id.is_empty()
            && !case.name.is_empty()
            && !case.category.is_empty()
            && !case.states.is_empty()
    }));
}

#[test]
fn expanded_evidence_accepts_only_boolean_true_from_supported_semantic_fields() {
    assert!(node_exposes_expanded(&json!({
        "state": { "expanded": true },
        "selection": { "expanded": null },
    })));
    assert!(node_exposes_expanded(&json!({
        "state": { "expanded": null },
        "selection": { "expanded": true },
    })));
    assert!(!node_exposes_expanded(&json!({
        "state": { "expanded": false },
        "selection": { "expanded": false },
    })));
    assert!(!node_exposes_expanded(&json!({
        "state": { "expanded": "true" },
        "selection": {},
    })));
}

#[test]
fn expanded_accessibility_cases_support_a_fresh_open_evidence_sequence() {
    let cases = COMPONENT_VISUAL_CASES
        .iter()
        .filter(|case| case.states.contains(&"expanded-accessibility"))
        .collect::<Vec<_>>();
    assert_eq!(
        cases.iter().map(|case| case.id).collect::<Vec<_>>(),
        [
            "select",
            "date-picker",
            "date-range-picker",
            "time-picker",
            "color-picker",
            "cascader",
            "tree-select",
            "auto-complete",
            "mentions",
        ]
    );
    for case in cases {
        for required in ["pressed", "focus", "open"] {
            assert!(
                case.states.contains(&required),
                "{} needs {required} before expanded evidence can be isolated",
                case.name
            );
        }
    }
}

#[test]
fn semantic_trigger_cases_use_invoke_for_open_evidence() {
    for id in SEMANTIC_INVOKE_OPEN_CASE_IDS {
        let case = COMPONENT_VISUAL_CASES
            .iter()
            .find(|case| case.id == *id)
            .unwrap_or_else(|| panic!("missing semantic trigger case {id}"));
        assert!(
            case.states.contains(&"open"),
            "{} must capture open",
            case.name
        );
        assert!(
            case.states
                .iter()
                .any(|state| matches!(*state, "focus" | "focus-trap")),
            "{} must prove keyboard focus after semantic Invoke",
            case.name
        );
    }
}

#[test]
fn select_open_evidence_uses_targeted_search_input() {
    let case = COMPONENT_VISUAL_CASES
        .iter()
        .find(|case| case.id == "select")
        .expect("Select visual case");
    assert!(case.states.contains(&"search-buffer"));
    assert!(case.states.contains(&"expanded-accessibility"));
    assert_eq!(targeted_open_text(case.id), Some(SELECT_OPEN_QUERY));
}

#[test]
#[ignore = "builds review boards from existing component visual evidence"]
fn build_component_visual_contact_sheets() {
    let root = evidence_root();
    let mut light = evidence_files(&root, |name| name.ends_with("-light-desktop.png"));
    let mut dark = evidence_files(&root, |name| name.ends_with("-dark-compact.png"));
    let mut interactive = evidence_files(&root, |name| {
        name.contains("-light-") && !name.ends_with("-light-desktop.png")
    });
    assert_eq!(
        light.len(),
        COMPONENT_VISUAL_CASE_COUNT,
        "light component evidence"
    );
    assert_eq!(
        dark.len(),
        COMPONENT_VISUAL_CASE_COUNT,
        "dark component evidence"
    );
    assert!(!interactive.is_empty(), "interactive component evidence");
    light.sort();
    dark.sort();
    interactive.sort();

    let output = root.join("contact-sheets");
    fs::create_dir_all(&output).expect("create component contact sheet directory");
    write_contact_sheet_group(&output, "light-desktop", &light);
    write_contact_sheet_group(&output, "dark-compact", &dark);
    write_contact_sheet_group(&output, "interactive", &interactive);
    eprintln!("component contact sheets: {}", output.display());
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes component visual evidence"]
fn real_demo_captures_every_component_and_applicable_visual_state() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let evidence_root = evidence_root();
    fs::create_dir_all(&evidence_root).expect("create component visual evidence root");
    clear_generated_evidence(&evidence_root);

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
            "request_id": "component-visual-hello",
            "type": "hello",
            "token": token,
            "client": { "name": "uix-component-visual-quality-gate" },
        }),
    );
    assert_success(&hello, "component-visual-hello");
    for action in ["pointer_move", "pointer_down", "pointer_up"] {
        assert!(hello["capabilities"]["window_actions"]
            .as_array()
            .is_some_and(|actions| actions.contains(&json!(action))));
    }

    let listed = list_windows(&mut connection, "component-visual-list");
    assert_eq!(listed.len(), 1, "demo must start with one main window");
    let window_id = listed[0]["window_id"].as_u64().expect("window id");
    let generation = listed[0]["generation"].as_u64().expect("generation");
    wait_for_presented(
        &mut connection,
        "component-visual-first-present",
        window_id,
        generation,
        1,
    );
    let initial = snapshot(&mut connection, "component-visual-initial-case", window_id);
    assert_eq!(
        node_name(node_by_automation_id(&initial, "component-qa-total")),
        COMPONENT_VISUAL_CASE_COUNT.to_string()
    );

    let mut observed = Vec::with_capacity(COMPONENT_VISUAL_CASE_COUNT);
    for index in 0..COMPONENT_VISUAL_CASE_COUNT {
        let mut case = inspect_case(&mut connection, window_id, index);
        let base_file = format!("{index:02}-{}-light-desktop.png", case.id);
        capture(&demo, &evidence_root, &base_file);
        case.evidence
            .push(("global/light-desktop".into(), base_file.clone()));
        capture_interactive_states(
            &demo,
            &mut connection,
            window_id,
            generation,
            &evidence_root,
            &mut case,
            &base_file,
        );
        observed.push(case);
        if index + 1 < COMPONENT_VISUAL_CASE_COUNT {
            invoke_and_wait(
                &demo,
                &mut connection,
                window_id,
                generation,
                "component-qa-next",
                &format!("component-visual-next-{index:02}"),
            );
        }
    }

    invoke_and_wait(
        &demo,
        &mut connection,
        window_id,
        generation,
        "component-qa-reset",
        "component-visual-reset",
    );
    invoke_and_wait(
        &demo,
        &mut connection,
        window_id,
        generation,
        "theme-toggle",
        "component-visual-dark-theme",
    );
    resize_and_wait(
        &demo,
        &mut connection,
        window_id,
        generation,
        COMPACT_WINDOW_SIZE,
        "component-visual-compact",
    );

    for (index, expected) in observed.iter_mut().enumerate() {
        let actual = inspect_case(&mut connection, window_id, index);
        assert_eq!(
            actual.id, expected.id,
            "component order changed in dark run"
        );
        assert_eq!(
            actual.name, expected.name,
            "component name changed in dark run"
        );
        let dark_file = format!("{index:02}-{}-dark-compact.png", expected.id);
        capture(&demo, &evidence_root, &dark_file);
        expected
            .evidence
            .push(("global/dark-compact".into(), dark_file));
        if index + 1 < COMPONENT_VISUAL_CASE_COUNT {
            invoke_and_wait(
                &demo,
                &mut connection,
                window_id,
                generation,
                "component-qa-next",
                &format!("component-visual-dark-next-{index:02}"),
            );
        }
    }

    write_matrix(&evidence_root, &observed);
    let total_declared_states: usize = observed.iter().map(|case| case.states.len()).sum();
    assert!(
        observed.iter().all(|case| {
            case.states
                .iter()
                .all(|state| case.evidence.iter().any(|(covered, _)| covered == state))
        }),
        "every declared component state must map to evidence"
    );
    assert_eq!(observed.len(), COMPONENT_VISUAL_CASE_COUNT);
    assert!(total_declared_states >= COMPONENT_VISUAL_CASE_COUNT * 2);

    resize_and_wait(
        &demo,
        &mut connection,
        window_id,
        generation,
        DEFAULT_WINDOW_SIZE,
        "component-visual-default-restore",
    );
    drop(connection);
    demo.close_and_wait();
    eprintln!(
        "component visual evidence root: {}; components={}; declared_states={}",
        evidence_root.display(),
        observed.len(),
        total_declared_states
    );
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes Upload visual evidence"]
fn real_demo_captures_upload_list_visual() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let evidence_root = evidence_root();
    fs::create_dir_all(&evidence_root).expect("create component visual evidence root");

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
            "request_id": "upload-visual-hello",
            "type": "hello",
            "token": token,
            "client": { "name": "uix-upload-visual-quality-gate" },
        }),
    );
    assert_success(&hello, "upload-visual-hello");
    let listed = list_windows(&mut connection, "upload-visual-list");
    let window_id = listed[0]["window_id"].as_u64().expect("window id");
    let generation = listed[0]["generation"].as_u64().expect("generation");
    wait_for_presented(
        &mut connection,
        "upload-visual-first-present",
        window_id,
        generation,
        1,
    );

    let mut upload_index = None;
    for index in 0..COMPONENT_VISUAL_CASE_COUNT {
        let case = inspect_case(&mut connection, window_id, index);
        if case.id == "upload" {
            upload_index = Some(index);
            assert_eq!(case.name, "Upload");
            assert!(case.states.iter().any(|state| state == "file-list"));
            assert!(case.states.iter().any(|state| state == "image-preview"));
            assert!(case.states.iter().any(|state| state == "manual"));
            break;
        }
        invoke_and_wait(
            &demo,
            &mut connection,
            window_id,
            generation,
            "component-qa-next",
            &format!("upload-visual-next-{index:02}"),
        );
    }
    let upload_index = upload_index.expect("Upload component case");
    capture(&demo, &evidence_root, "upload-list-light-desktop.png");

    let before = snapshot(&mut connection, "upload-visual-before-remove", window_id);
    let target = node_by_automation_id(&before, "component-qa-target");
    let bounds = &target["visible_bounds"];
    let x = bounds["x"].as_f64().expect("upload x") + bounds["w"].as_f64().expect("upload width")
        - 14.0;
    let y = bounds["y"].as_f64().expect("upload y") + 116.0;
    perform_and_wait(
        &demo,
        &mut connection,
        window_id,
        generation,
        "upload-visual-remove",
        None,
        json!({ "kind": "click_at", "x": x, "y": y }),
    );
    let after = snapshot(&mut connection, "upload-visual-after-remove", window_id);
    let value = node_by_automation_id(&after, "component-qa-target")["state"]["value_text"]
        .as_str()
        .expect("upload queue value");
    assert!(!value.contains("button-light.png"), "removed row: {value}");
    assert!(
        value.contains("demo.png"),
        "preview row should remain: {value}"
    );
    capture(&demo, &evidence_root, "upload-list-light-removed.png");

    invoke_and_wait(
        &demo,
        &mut connection,
        window_id,
        generation,
        "theme-toggle",
        "upload-visual-dark-theme",
    );
    resize_and_wait(
        &demo,
        &mut connection,
        window_id,
        generation,
        COMPACT_WINDOW_SIZE,
        "upload-visual-compact",
    );
    assert_eq!(
        inspect_case(&mut connection, window_id, upload_index).id,
        "upload"
    );
    capture(&demo, &evidence_root, "upload-list-dark-compact.png");

    drop(connection);
    demo.close_and_wait();
}

fn inspect_case(
    connection: &mut BufReader<File>,
    window_id: u64,
    expected_index: usize,
) -> ObservedCase {
    let snapshot = snapshot(
        connection,
        &format!("component-visual-case-{expected_index:02}"),
        window_id,
    );
    let total = node_name(node_by_automation_id(&snapshot, "component-qa-total"));
    assert_eq!(total, COMPONENT_VISUAL_CASE_COUNT.to_string());
    let position = node_name(node_by_automation_id(&snapshot, "component-qa-position"));
    assert_eq!(
        position,
        format!("{}/{}", expected_index + 1, COMPONENT_VISUAL_CASE_COUNT)
    );
    let name = node_name(node_by_automation_id(&snapshot, "component-qa-current"));
    let id = node_name(node_by_automation_id(&snapshot, "component-qa-id"));
    let state_text = node_name(node_by_automation_id(&snapshot, "component-qa-states"));
    let states = state_text
        .split_once("本组件：")
        .map(|(_, states)| states)
        .expect("component state metadata")
        .split(" / ")
        .map(str::trim)
        .filter(|state| !state.is_empty())
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    assert!(!id.is_empty(), "component id must be present");
    assert!(!name.is_empty(), "component name must be present");
    assert!(!states.is_empty(), "component states must be present");

    let target = node_by_automation_id(&snapshot, "component-qa-target");
    let bounds = &target["visible_bounds"];
    assert!(
        bounds["w"].as_f64().unwrap_or_default() > 0.0
            && bounds["h"].as_f64().unwrap_or_default() > 0.0,
        "{name} target must be visible: {bounds}"
    );

    ObservedCase {
        index: expected_index,
        id,
        name,
        states,
        evidence: Vec::new(),
    }
}

#[allow(clippy::too_many_arguments)]
fn capture_interactive_states(
    demo: &DemoProcess,
    connection: &mut BufReader<File>,
    window_id: u64,
    generation: u64,
    root: &Path,
    case: &mut ObservedCase,
    base_file: &str,
) {
    let initial = snapshot(
        connection,
        &format!("component-visual-{}-interactive", case.id),
        window_id,
    );
    let target = node_by_automation_id(&initial, "component-qa-target");
    let (bounds_x, bounds_y, bounds_w, bounds_h) = visible_rect(target);
    let (x, y) = match case.id.as_str() {
        "result-view" => (bounds_x + bounds_w * 0.5, bounds_y + bounds_h * 0.4 + 88.0),
        "splitter" => (bounds_x + bounds_w / 3.0, bounds_y + bounds_h * 0.5),
        "slider" => (bounds_x + bounds_w * 0.42, bounds_y + bounds_h * 0.5),
        "table" => (bounds_x + 12.0, bounds_y + 50.0),
        "tree" => (bounds_x + 80.0, bounds_y + 14.0),
        _ => visible_center(target),
    };
    let actions = target["actions"].as_array().cloned().unwrap_or_default();

    if case.id == "tooltip" {
        capture_tooltip_states(demo, connection, window_id, generation, root, case);
    } else if case.states.iter().any(|state| state == "hover") {
        perform_pointer(
            demo,
            connection,
            window_id,
            generation,
            &format!("component-visual-{}-hover", case.id),
            "pointer_move",
            x,
            y,
        );
        thread::sleep(Duration::from_millis(100));
        let file = format!("{:02}-{}-light-hover.png", case.index, case.id);
        capture(demo, root, &file);
        case.evidence.push(("hover".into(), file));
        perform_pointer(
            demo,
            connection,
            window_id,
            generation,
            &format!("component-visual-{}-hover-clear", case.id),
            "pointer_move",
            4.0,
            4.0,
        );
    }

    if case.states.iter().any(|state| state == "pressed") {
        perform_pointer(
            demo,
            connection,
            window_id,
            generation,
            &format!("component-visual-{}-pressed-move", case.id),
            "pointer_move",
            x,
            y,
        );
        perform_pointer(
            demo,
            connection,
            window_id,
            generation,
            &format!("component-visual-{}-pressed-down", case.id),
            "pointer_down",
            x,
            y,
        );
        thread::sleep(Duration::from_millis(70));
        let file = format!("{:02}-{}-light-pressed.png", case.index, case.id);
        capture(demo, root, &file);
        case.evidence.push(("pressed".into(), file));
        let commits_selection = matches!(case.id.as_str(), "table" | "tree");
        perform_pointer(
            demo,
            connection,
            window_id,
            generation,
            &format!("component-visual-{}-pressed-up", case.id),
            "pointer_up",
            if commits_selection { x } else { 4.0 },
            if commits_selection { y } else { 4.0 },
        );
        if commits_selection {
            thread::sleep(Duration::from_millis(70));
            let file = format!("{:02}-{}-light-selected.png", case.index, case.id);
            capture(demo, root, &file);
            case.evidence.push(("selected".into(), file));
        }
        if requires_expanded_evidence(case) {
            let pressed = snapshot(
                connection,
                &format!("component-visual-{}-pressed-expanded", case.id),
                window_id,
            );
            if node_exposes_expanded(node_by_automation_id(&pressed, "component-qa-target")) {
                perform_and_wait(
                    demo,
                    connection,
                    window_id,
                    generation,
                    &format!("component-visual-{}-pressed-close", case.id),
                    None,
                    json!({ "kind": "press_key", "key": "escape" }),
                );
                let closed = snapshot(
                    connection,
                    &format!("component-visual-{}-pressed-closed", case.id),
                    window_id,
                );
                assert!(
                    !node_exposes_expanded(node_by_automation_id(&closed, "component-qa-target")),
                    "{} pressed evidence must be closed before its independent open evidence",
                    case.name
                );
            }
        }
    }

    if case.states.iter().any(|state| state == "focus")
        && !matches!(case.id.as_str(), "drawer" | "tooltip")
    {
        let focus_target = if actions.contains(&json!("focus")) {
            json!({ "automation_id": "component-qa-target" })
        } else {
            assert!(
                matches!(case.id.as_str(), "nav-group" | "navigation"),
                "{} declares focus but component-qa-target does not expose semantic focus: {actions:?}",
                case.name
            );
            let descendant = first_focusable_descendant(&initial, target);
            json!({ "node_id": descendant["node_id"] })
        };
        perform_and_wait(
            demo,
            connection,
            window_id,
            generation,
            &format!("component-visual-{}-focus", case.id),
            Some(focus_target),
            json!({ "kind": "focus" }),
        );
        let file = format!("{:02}-{}-light-focus.png", case.index, case.id);
        capture(demo, root, &file);
        case.evidence.push(("focus".into(), file));
    }

    if case.states.iter().any(|state| state == "changed") && case.id == "carousel" {
        perform_and_wait(
            demo,
            connection,
            window_id,
            generation,
            "component-visual-carousel-changed",
            None,
            json!({
                "kind": "click_at",
                "x": bounds_x + bounds_w - 15.0,
                "y": bounds_y + bounds_h * 0.5,
            }),
        );
        let file = format!("{:02}-{}-light-changed.png", case.index, case.id);
        capture(demo, root, &file);
        case.evidence.push(("changed".into(), file));
    }

    if case.id == "focus-trap" {
        capture_focus_trap_cycle(demo, connection, window_id, generation, root, case);
    }

    if case.id == "tree" && case.states.iter().any(|state| state == "expanded") {
        perform_and_wait(
            demo,
            connection,
            window_id,
            generation,
            "component-visual-tree-expanded",
            None,
            json!({ "kind": "press_key", "key": "right" }),
        );
        let file = format!("{:02}-{}-light-expanded.png", case.index, case.id);
        capture(demo, root, &file);
        case.evidence.push(("expanded".into(), file));
    }

    if case.id == "table" && case.states.iter().any(|state| state == "sorted") {
        perform_and_wait(
            demo,
            connection,
            window_id,
            generation,
            "component-visual-table-sorted",
            None,
            json!({
                "kind": "click_at",
                "x": bounds_x + 90.0,
                "y": bounds_y + 15.0,
            }),
        );
        let file = format!("{:02}-{}-light-sorted.png", case.index, case.id);
        capture(demo, root, &file);
        case.evidence.push(("sorted".into(), file));
    }

    if case.states.iter().any(|state| state == "open") && case.id != "tooltip" {
        let (open_target, open_action) = if let Some(text) = targeted_open_text(&case.id) {
            assert!(
                actions.contains(&json!("insert_text")),
                "{} target must expose semantic insert_text: {actions:?}",
                case.name
            );
            (
                Some(json!({ "automation_id": "component-qa-target" })),
                json!({ "kind": "insert_text", "text": text }),
            )
        } else if uses_semantic_invoke_to_open(case) {
            assert!(
                actions.contains(&json!("invoke")),
                "{} closed trigger must expose semantic Invoke: {actions:?}",
                case.name
            );
            (
                Some(json!({ "automation_id": "component-qa-target" })),
                json!({ "kind": "invoke" }),
            )
        } else {
            match case.id.as_str() {
                "cascader" | "tree-select" => (
                    None,
                    json!({ "kind": "press_key", "key": "down", "modifiers": [] }),
                ),
                _ => (None, json!({ "kind": "click_at", "x": x, "y": y })),
            }
        };
        perform_and_wait(
            demo,
            connection,
            window_id,
            generation,
            &format!("component-visual-{}-open", case.id),
            open_target,
            open_action,
        );
        thread::sleep(Duration::from_millis(180));
        let opened = snapshot(
            connection,
            &format!("component-visual-{}-opened", case.id),
            window_id,
        );
        assert_eq!(
            node_name(node_by_automation_id(&opened, "component-qa-id")),
            case.id,
            "overlay interaction must not leave the component case"
        );
        let opened_target = node_by_automation_id(&opened, "component-qa-target");
        if requires_expanded_evidence(case) {
            assert!(
                node_exposes_expanded(opened_target),
                "{} open evidence must expose aria-expanded=true: {opened_target}",
                case.name
            );
        }
        let file = format!("{:02}-{}-light-open.png", case.index, case.id);
        capture(demo, root, &file);
        let changed_ratio = changed_pixel_ratio(&root.join(base_file), &root.join(&file));
        assert!(
            changed_ratio >= 0.005,
            "{} open evidence changed only {:.3}% of pixels",
            case.name,
            changed_ratio * 100.0
        );
        case.evidence.push(("open".into(), file.clone()));
        if case.states.iter().any(|state| state == "overlay") {
            case.evidence.push(("overlay".into(), file.clone()));
        }
        if case.id == "popconfirm" {
            case.evidence.push(("confirm".into(), file));
            perform_and_wait(
                demo,
                connection,
                window_id,
                generation,
                "component-visual-popconfirm-cancel-focus",
                None,
                json!({ "kind": "press_key", "key": "right" }),
            );
            let cancel_file = format!("{:02}-{}-light-cancel.png", case.index, case.id);
            capture(demo, root, &cancel_file);
            case.evidence.push(("cancel".into(), cancel_file));
        }
        if case.id == "modal" {
            capture_modal_focus_trap(demo, connection, window_id, generation, root, case);
        }
        if case.id == "drawer" {
            perform_and_wait(
                demo,
                connection,
                window_id,
                generation,
                "component-visual-drawer-focus",
                None,
                json!({ "kind": "press_key", "key": "tab" }),
            );
            wait_for_focused_node(
                connection,
                window_id,
                "component-qa-drawer-cancel",
                Duration::from_secs(10),
            );
            let focus_file = format!("{:02}-{}-light-focus.png", case.index, case.id);
            capture(demo, root, &focus_file);
            case.evidence.push(("focus".into(), focus_file));
        }
        perform_and_wait(
            demo,
            connection,
            window_id,
            generation,
            &format!("component-visual-{}-close", case.id),
            None,
            json!({ "kind": "press_key", "key": "escape" }),
        );
        thread::sleep(Duration::from_millis(400));
    }

    if case.states.iter().any(|state| state == "scrolled") {
        let scroll_snapshot = snapshot(
            connection,
            &format!("component-visual-{}-scroll-target", case.id),
            window_id,
        );
        let scroll_target = node_by_automation_id(&scroll_snapshot, "component-qa-target");
        let scroll_actions = scroll_target["actions"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        assert!(
            scroll_actions.contains(&json!("scroll")),
            "{} must expose semantic scroll in its scrollable state: {scroll_actions:?}",
            case.name
        );
        perform_and_wait(
            demo,
            connection,
            window_id,
            generation,
            &format!("component-visual-{}-scroll", case.id),
            Some(json!({ "automation_id": "component-qa-target" })),
            // Agent semantic actions use the framework-normalized convention:
            // positive Y scrolls the viewport down. Native wheel input is the
            // boundary that converts the opposite Windows wheel convention.
            json!({ "kind": "scroll", "delta_x": 0.0, "delta_y": 180.0 }),
        );
        let file = format!("{:02}-{}-light-scrolled.png", case.index, case.id);
        capture(demo, root, &file);
        case.evidence.push(("scrolled".into(), file));
    }

    for state in &case.states {
        if !case.evidence.iter().any(|(covered, _)| covered == state) {
            case.evidence.push((state.clone(), base_file.to_string()));
        }
    }

    for state in &case.states {
        if requires_distinct_evidence(case, state) {
            assert!(
                case.evidence
                    .iter()
                    .any(|(covered, file)| covered == state && file != base_file),
                "{} state {state} requires dedicated visual evidence",
                case.name
            );
        }
    }
}

fn first_focusable_descendant<'a>(snapshot: &'a Value, ancestor: &Value) -> &'a Value {
    let ancestor_id = ancestor["node_id"].as_str().expect("ancestor node id");
    let nodes = snapshot["nodes"].as_array().expect("snapshot nodes");
    let parents = nodes
        .iter()
        .filter_map(|node| {
            Some((
                node["node_id"].as_str()?.to_owned(),
                node["parent"].as_str().map(str::to_owned),
            ))
        })
        .collect::<std::collections::HashMap<_, _>>();

    nodes
        .iter()
        .find(|node| {
            if !node["actions"]
                .as_array()
                .is_some_and(|actions| actions.contains(&json!("focus")))
                || node["visible_bounds"].is_null()
            {
                return false;
            }
            let mut current = node["parent"].as_str();
            while let Some(parent) = current {
                if parent == ancestor_id {
                    return true;
                }
                current = parents.get(parent).and_then(|parent| parent.as_deref());
            }
            false
        })
        .expect("composite focus case must expose a visible focusable descendant")
}

fn visible_rect(node: &Value) -> (f64, f64, f64, f64) {
    let bounds = &node["visible_bounds"];
    (
        bounds["x"].as_f64().expect("visible x"),
        bounds["y"].as_f64().expect("visible y"),
        bounds["w"].as_f64().expect("visible width"),
        bounds["h"].as_f64().expect("visible height"),
    )
}

fn capture_tooltip_states(
    demo: &DemoProcess,
    connection: &mut BufReader<File>,
    window_id: u64,
    generation: u64,
    root: &Path,
    case: &mut ObservedCase,
) {
    for (state, automation_id) in [
        ("bottom", "component-qa-tooltip-bottom"),
        ("left", "component-qa-tooltip-left"),
        ("right", "component-qa-tooltip-right"),
    ] {
        let current = snapshot(
            connection,
            &format!("component-visual-tooltip-{state}-target"),
            window_id,
        );
        let (x, y) = visible_center(node_by_automation_id(&current, automation_id));
        perform_pointer(
            demo,
            connection,
            window_id,
            generation,
            &format!("component-visual-tooltip-{state}"),
            "pointer_move",
            x,
            y,
        );
        thread::sleep(Duration::from_millis(120));
        let file = format!("{:02}-tooltip-light-{state}.png", case.index);
        capture(demo, root, &file);
        case.evidence.push((state.into(), file));
        perform_pointer(
            demo,
            connection,
            window_id,
            generation,
            &format!("component-visual-tooltip-{state}-clear"),
            "pointer_move",
            4.0,
            4.0,
        );
    }

    perform_and_wait(
        demo,
        connection,
        window_id,
        generation,
        "component-visual-tooltip-focus",
        Some(json!({ "automation_id": "component-qa-target" })),
        json!({ "kind": "focus" }),
    );
    let focused = snapshot(connection, "component-visual-tooltip-focused", window_id);
    assert_eq!(
        node_by_automation_id(&focused, "component-qa-target")["focused"],
        true
    );
    let file = format!("{:02}-tooltip-light-top-focus.png", case.index);
    capture(demo, root, &file);
    case.evidence.push(("top".into(), file.clone()));
    case.evidence.push(("focus".into(), file));
}

fn capture_focus_trap_cycle(
    demo: &DemoProcess,
    connection: &mut BufReader<File>,
    window_id: u64,
    generation: u64,
    root: &Path,
    case: &mut ObservedCase,
) {
    perform_and_wait(
        demo,
        connection,
        window_id,
        generation,
        "component-visual-focus-trap-first",
        Some(json!({ "automation_id": "component-qa-focus-first" })),
        json!({ "kind": "focus" }),
    );
    for (step, expected) in [
        ("second", "component-qa-focus-second"),
        ("third", "component-qa-focus-third"),
        ("wrap", "component-qa-focus-first"),
    ] {
        perform_and_wait(
            demo,
            connection,
            window_id,
            generation,
            &format!("component-visual-focus-trap-{step}"),
            None,
            json!({ "kind": "press_key", "key": "tab" }),
        );
        let current = snapshot(
            connection,
            &format!("component-visual-focus-trap-{step}-snapshot"),
            window_id,
        );
        assert_eq!(node_by_automation_id(&current, expected)["focused"], true);
        if step == "third" {
            let file = format!("{:02}-focus-trap-light-focus-cycle.png", case.index);
            capture(demo, root, &file);
            case.evidence.push(("focus-cycle".into(), file));
        }
    }
}

fn capture_modal_focus_trap(
    demo: &DemoProcess,
    connection: &mut BufReader<File>,
    window_id: u64,
    generation: u64,
    root: &Path,
    case: &mut ObservedCase,
) {
    perform_and_wait(
        demo,
        connection,
        window_id,
        generation,
        "component-visual-modal-focus-cancel",
        None,
        json!({ "kind": "press_key", "key": "tab" }),
    );
    wait_for_focused_node(
        connection,
        window_id,
        "component-qa-modal-cancel",
        Duration::from_secs(10),
    );
    perform_and_wait(
        demo,
        connection,
        window_id,
        generation,
        "component-visual-modal-focus-confirm",
        None,
        json!({ "kind": "press_key", "key": "tab" }),
    );
    wait_for_focused_node(
        connection,
        window_id,
        "component-qa-modal-confirm",
        Duration::from_secs(10),
    );
    let confirm = snapshot(
        connection,
        "component-visual-modal-confirm-focused",
        window_id,
    );
    assert_eq!(
        node_by_automation_id(&confirm, "component-qa-modal-confirm")["focused"],
        true
    );
    let file = format!("{:02}-modal-light-focus-trap.png", case.index);
    capture(demo, root, &file);
    case.evidence.push(("focus-trap".into(), file));
    perform_and_wait(
        demo,
        connection,
        window_id,
        generation,
        "component-visual-modal-focus-wrap",
        None,
        json!({ "kind": "press_key", "key": "tab" }),
    );
    wait_for_focused_node(
        connection,
        window_id,
        "component-qa-modal-cancel",
        Duration::from_secs(10),
    );
    let wrapped = snapshot(
        connection,
        "component-visual-modal-cancel-refocused",
        window_id,
    );
    assert_eq!(
        node_by_automation_id(&wrapped, "component-qa-modal-cancel")["focused"],
        true
    );
}

fn node_exposes_expanded(node: &Value) -> bool {
    node.pointer("/state/expanded").and_then(Value::as_bool) == Some(true)
        || node.pointer("/selection/expanded").and_then(Value::as_bool) == Some(true)
}

fn requires_expanded_evidence(case: &ObservedCase) -> bool {
    case.states
        .iter()
        .any(|state| state == "expanded-accessibility")
        || case.id == "select"
        || uses_semantic_invoke_to_open(case)
}

fn uses_semantic_invoke_to_open(case: &ObservedCase) -> bool {
    SEMANTIC_INVOKE_OPEN_CASE_IDS.contains(&case.id.as_str())
}

fn targeted_open_text(case_id: &str) -> Option<&'static str> {
    match case_id {
        "select" => Some(SELECT_OPEN_QUERY),
        "mentions" => Some("@"),
        _ => None,
    }
}

fn requires_distinct_evidence(case: &ObservedCase, state: &str) -> bool {
    matches!(
        state,
        "hover"
            | "pressed"
            | "focus"
            | "open"
            | "scrolled"
            | "changed"
            | "focus-cycle"
            | "confirm"
            | "cancel"
    ) || (case.id == "tree" && matches!(state, "expanded" | "selected"))
        || (case.id == "table" && matches!(state, "selected" | "sorted"))
        || (case.id == "modal" && state == "focus-trap")
        || (case.id == "tooltip" && matches!(state, "top" | "bottom" | "left" | "right"))
        || (matches!(case.id.as_str(), "drawer" | "modal") && state == "overlay")
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

fn perform_pointer(
    demo: &DemoProcess,
    connection: &mut BufReader<File>,
    window_id: u64,
    generation: u64,
    request_id: &str,
    kind: &str,
    x: f64,
    y: f64,
) {
    perform_and_wait(
        demo,
        connection,
        window_id,
        generation,
        request_id,
        None,
        json!({ "kind": kind, "x": x, "y": y }),
    );
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

fn node_name(node: &Value) -> String {
    node["name"].as_str().unwrap_or_default().to_string()
}

fn evidence_root() -> PathBuf {
    std::env::var_os("UIX_COMPONENT_QA_EVIDENCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/debug-captures/uix-component-visual"))
}

fn clear_generated_evidence(root: &Path) {
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir()
                && path
                    .file_name()
                    .is_some_and(|name| name == "contact-sheets")
            {
                fs::remove_dir_all(&path).expect("remove stale contact sheets");
            } else if path
                .extension()
                .is_some_and(|extension| extension == "png" || extension == "csv")
            {
                fs::remove_file(&path).expect("remove stale component evidence");
            }
        }
    }
}

fn capture(demo: &DemoProcess, root: &Path, file_name: &str) {
    let path = root.join(file_name);
    foreground::capture_demo_client_png(demo, &path);
    eprintln!("component GUI evidence: {}", path.display());
}

fn changed_pixel_ratio(before: &Path, after: &Path) -> f64 {
    let before = image::open(before)
        .expect("open baseline component evidence")
        .to_rgba8();
    let after = image::open(after)
        .expect("open changed component evidence")
        .to_rgba8();
    assert_eq!(
        before.dimensions(),
        after.dimensions(),
        "component evidence dimensions must remain stable"
    );
    let changed = before
        .pixels()
        .zip(after.pixels())
        .filter(|(left, right)| left.0[..3] != right.0[..3])
        .count();
    changed as f64 / before.pixels().len().max(1) as f64
}

fn evidence_files(root: &Path, keep: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    fs::read_dir(root)
        .expect("read component evidence root")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|extension| extension == "png")
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(&keep)
        })
        .collect()
}

fn write_contact_sheet_group(output: &Path, label: &str, files: &[PathBuf]) {
    const COLUMNS: usize = 4;
    const ROWS: usize = 4;
    const CELL_WIDTH: u32 = 300;
    const CELL_HEIGHT: u32 = 200;
    const PER_SHEET: usize = COLUMNS * ROWS;

    for (sheet_index, chunk) in files.chunks(PER_SHEET).enumerate() {
        let mut sheet = RgbaImage::from_pixel(
            CELL_WIDTH * COLUMNS as u32,
            CELL_HEIGHT * ROWS as u32,
            Rgba([24, 24, 24, 255]),
        );
        for (cell_index, path) in chunk.iter().enumerate() {
            let source = image::open(path).expect("open component evidence PNG");
            let thumbnail = source.thumbnail(CELL_WIDTH, CELL_HEIGHT).to_rgba8();
            let column = cell_index % COLUMNS;
            let row = cell_index / COLUMNS;
            let x = column as u32 * CELL_WIDTH + (CELL_WIDTH - thumbnail.width()) / 2;
            let y = row as u32 * CELL_HEIGHT + (CELL_HEIGHT - thumbnail.height()) / 2;
            sheet
                .copy_from(&thumbnail, x, y)
                .expect("compose contact sheet");
        }
        let first = sheet_index * PER_SHEET;
        let last = first + chunk.len() - 1;
        let path = output.join(format!("{label}-{first:03}-{last:03}.png"));
        sheet.save(&path).expect("save component contact sheet");
    }
}

fn csv_field(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn write_matrix(root: &Path, cases: &[ObservedCase]) {
    let path = root.join("component-visual-matrix.csv");
    let mut file = File::create(&path).expect("create component visual matrix");
    writeln!(
        file,
        "index,component_id,component_name,state,evidence,result"
    )
    .expect("write matrix header");
    for case in cases {
        for (state, evidence) in &case.evidence {
            writeln!(
                file,
                "{},{},{},{},{},PASS",
                case.index + 1,
                csv_field(&case.id),
                csv_field(&case.name),
                csv_field(state),
                csv_field(evidence),
            )
            .expect("write matrix row");
        }
    }
    eprintln!("component visual matrix: {}", path.display());
}
