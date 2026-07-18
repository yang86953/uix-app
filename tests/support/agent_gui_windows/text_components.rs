use super::*;

struct ComponentQaSession {
    demo: DemoProcess,
    connection: BufReader<File>,
    window_id: u64,
    generation: u64,
}

impl ComponentQaSession {
    fn open(case_index: usize, request_prefix: &str) -> Self {
        let mut demo = DemoProcess::spawn_with_args(DEFAULT_VULKAN_GRAPHICS, &["--component-qa"]);
        let descriptor = demo.wait_for_descriptor();
        let endpoint = descriptor["endpoint"]
            .as_str()
            .expect("descriptor endpoint");
        let token = descriptor["token"].as_str().expect("descriptor token");
        let stream = connect(endpoint, &mut demo.child);
        let mut connection = BufReader::new(stream);
        let hello_id = format!("{request_prefix}-hello");
        let hello = exchange(
            &mut connection,
            json!({
                "schema": "uix.agent.v1",
                "request_id": hello_id,
                "type": "hello",
                "token": token,
                "client": { "name": "uix-text-component-regression" },
            }),
        );
        assert_success(&hello, &format!("{request_prefix}-hello"));

        let list_id = format!("{request_prefix}-list");
        let listed = exchange(
            &mut connection,
            json!({
                "schema": "uix.agent.v1",
                "request_id": list_id,
                "type": "list_windows",
            }),
        );
        assert_success(&listed, &format!("{request_prefix}-list"));
        let window = &listed["windows"].as_array().expect("listed windows")[0];
        let window_id = window["window_id"].as_u64().expect("window id");
        let generation = window["generation"].as_u64().expect("generation");
        wait_for_revision(&mut connection, request_prefix, window_id, generation, 1);

        for step in 0..case_index {
            let request_id = format!("{request_prefix}-next-{step}");
            let changed = perform_until_presentable(
                &demo,
                &mut connection,
                window_id,
                generation,
                &request_id,
                Some(json!({ "automation_id": "component-qa-next" })),
                json!({ "kind": "invoke" }),
            );
            let revision = changed["revision"].as_u64().expect("perform revision");
            wait_for_revision(
                &mut connection,
                &request_id,
                window_id,
                generation,
                revision,
            );
        }

        Self {
            demo,
            connection,
            window_id,
            generation,
        }
    }

    fn snapshot(&mut self, request_id: &str) -> Value {
        let response = exchange(
            &mut self.connection,
            json!({
                "schema": "uix.agent.v1",
                "request_id": request_id,
                "type": "snapshot",
                "window_id": self.window_id,
            }),
        );
        assert_success(&response, request_id);
        response["snapshot"].clone()
    }

    fn capture(&self, directory: &str, file_name: &str) -> PathBuf {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("debug-captures")
            .join(directory);
        fs::create_dir_all(&root).expect("create text component evidence root");
        let path = root.join(file_name);
        foreground::capture_demo_client_png(&self.demo, &path);
        eprintln!("text component GUI evidence: {}", path.display());
        path
    }

    fn ensure_foreground(&self) {
        self.demo.raise_for_interaction();
        foreground::request_foreground_focus(self.demo.window_handle());
    }

    fn move_pointer_to(&mut self, automation_id: &str, request_prefix: &str) -> Value {
        let snapshot = self.snapshot(&format!("{request_prefix}-before"));
        let (x, y) = visible_center(node_by_automation_id(&snapshot, automation_id));
        let moved = perform_until_presentable(
            &self.demo,
            &mut self.connection,
            self.window_id,
            self.generation,
            request_prefix,
            None,
            json!({ "kind": "pointer_move", "x": x, "y": y }),
        );
        let revision = moved["revision"].as_u64().expect("pointer move revision");
        wait_for_revision(
            &mut self.connection,
            request_prefix,
            self.window_id,
            self.generation,
            revision,
        );
        thread::sleep(Duration::from_millis(300));
        snapshot
    }

    fn move_pointer_to_offset(
        &mut self,
        automation_id: &str,
        x_fraction: f64,
        y_offset: f64,
        request_prefix: &str,
    ) -> Value {
        let snapshot = self.snapshot(&format!("{request_prefix}-bounds"));
        let bounds = &node_by_automation_id(&snapshot, automation_id)["visible_bounds"];
        let x = bounds["x"].as_f64().expect("bounds x")
            + bounds["w"].as_f64().expect("bounds width") * x_fraction;
        let y = bounds["y"].as_f64().expect("bounds y") + y_offset;
        let moved = perform_until_presentable(
            &self.demo,
            &mut self.connection,
            self.window_id,
            self.generation,
            request_prefix,
            None,
            json!({ "kind": "pointer_move", "x": x, "y": y }),
        );
        let revision = moved["revision"].as_u64().expect("pointer move revision");
        wait_for_revision(
            &mut self.connection,
            request_prefix,
            self.window_id,
            self.generation,
            revision,
        );
        self.snapshot(&format!("{request_prefix}-after"))
    }

    fn toggle(&mut self, automation_id: &str, request_prefix: &str) -> Value {
        let changed = perform_until_presentable(
            &self.demo,
            &mut self.connection,
            self.window_id,
            self.generation,
            request_prefix,
            Some(json!({ "automation_id": automation_id })),
            json!({ "kind": "toggle" }),
        );
        let revision = changed["revision"].as_u64().expect("toggle revision");
        wait_for_revision(
            &mut self.connection,
            request_prefix,
            self.window_id,
            self.generation,
            revision,
        );
        self.snapshot(&format!("{request_prefix}-after"))
    }

    fn select(&mut self, automation_id: &str, value: &str, request_prefix: &str) -> Value {
        let changed = perform_until_presentable(
            &self.demo,
            &mut self.connection,
            self.window_id,
            self.generation,
            request_prefix,
            Some(json!({ "automation_id": automation_id })),
            json!({ "kind": "select", "value": value }),
        );
        let revision = changed["revision"].as_u64().expect("select revision");
        wait_for_revision(
            &mut self.connection,
            request_prefix,
            self.window_id,
            self.generation,
            revision,
        );
        self.snapshot(&format!("{request_prefix}-after"))
    }

    fn press_key(&mut self, key: &str, request_prefix: &str) -> Value {
        let changed = perform_until_presentable(
            &self.demo,
            &mut self.connection,
            self.window_id,
            self.generation,
            request_prefix,
            None,
            json!({ "kind": "press_key", "key": key }),
        );
        let revision = changed["revision"].as_u64().expect("press key revision");
        wait_for_revision(
            &mut self.connection,
            request_prefix,
            self.window_id,
            self.generation,
            revision,
        );
        self.snapshot(&format!("{request_prefix}-after"))
    }

    fn set_value(&mut self, automation_id: &str, value: &str, request_prefix: &str) -> Value {
        let changed = perform_until_presentable(
            &self.demo,
            &mut self.connection,
            self.window_id,
            self.generation,
            request_prefix,
            Some(json!({ "automation_id": automation_id })),
            json!({ "kind": "set_value", "value": value }),
        );
        let revision = changed["revision"].as_u64().expect("set value revision");
        wait_for_revision(
            &mut self.connection,
            request_prefix,
            self.window_id,
            self.generation,
            revision,
        );
        self.snapshot(&format!("{request_prefix}-after"))
    }

    fn insert_text(&mut self, automation_id: &str, value: &str, request_prefix: &str) -> Value {
        let changed = perform_until_presentable(
            &self.demo,
            &mut self.connection,
            self.window_id,
            self.generation,
            request_prefix,
            Some(json!({ "automation_id": automation_id })),
            json!({ "kind": "insert_text", "text": value }),
        );
        let revision = changed["revision"].as_u64().expect("insert text revision");
        wait_for_revision(
            &mut self.connection,
            request_prefix,
            self.window_id,
            self.generation,
            revision,
        );
        self.snapshot(&format!("{request_prefix}-after"))
    }

    fn increment(&mut self, automation_id: &str, request_prefix: &str) -> Value {
        let changed = perform_until_presentable(
            &self.demo,
            &mut self.connection,
            self.window_id,
            self.generation,
            request_prefix,
            Some(json!({ "automation_id": automation_id })),
            json!({ "kind": "increment" }),
        );
        let revision = changed["revision"].as_u64().expect("increment revision");
        wait_for_revision(
            &mut self.connection,
            request_prefix,
            self.window_id,
            self.generation,
            revision,
        );
        self.snapshot(&format!("{request_prefix}-after"))
    }

    fn focus(&mut self, automation_id: &str, request_prefix: &str) -> Value {
        let changed = perform_until_presentable(
            &self.demo,
            &mut self.connection,
            self.window_id,
            self.generation,
            request_prefix,
            Some(json!({ "automation_id": automation_id })),
            json!({ "kind": "focus" }),
        );
        let revision = changed["revision"].as_u64().expect("focus revision");
        wait_for_revision(
            &mut self.connection,
            request_prefix,
            self.window_id,
            self.generation,
            revision,
        );
        self.snapshot(&format!("{request_prefix}-after"))
    }

    fn invoke(&mut self, automation_id: &str, request_prefix: &str) -> Value {
        let changed = perform_until_presentable(
            &self.demo,
            &mut self.connection,
            self.window_id,
            self.generation,
            request_prefix,
            Some(json!({ "automation_id": automation_id })),
            json!({ "kind": "invoke" }),
        );
        let revision = changed["revision"].as_u64().expect("invoke revision");
        wait_for_revision(
            &mut self.connection,
            request_prefix,
            self.window_id,
            self.generation,
            revision,
        );
        self.snapshot(&format!("{request_prefix}-after"))
    }

    fn click_right_slot(&mut self, automation_id: &str, request_prefix: &str) -> Value {
        self.move_pointer_to(automation_id, &format!("{request_prefix}-hover"));
        let snapshot = self.snapshot(&format!("{request_prefix}-bounds"));
        let bounds = &node_by_automation_id(&snapshot, automation_id)["visible_bounds"];
        let x = bounds["x"].as_f64().expect("bounds x")
            + bounds["w"].as_f64().expect("bounds width")
            - 10.0;
        let y = bounds["y"].as_f64().expect("bounds y")
            + bounds["h"].as_f64().expect("bounds height") * 0.5;
        let changed = perform_until_presentable(
            &self.demo,
            &mut self.connection,
            self.window_id,
            self.generation,
            request_prefix,
            None,
            json!({ "kind": "click_at", "x": x, "y": y }),
        );
        let revision = changed["revision"].as_u64().expect("click revision");
        wait_for_revision(
            &mut self.connection,
            request_prefix,
            self.window_id,
            self.generation,
            revision,
        );
        self.snapshot(&format!("{request_prefix}-after"))
    }

    fn click_at_fraction(
        &mut self,
        automation_id: &str,
        x_fraction: f64,
        y_fraction: f64,
        request_prefix: &str,
    ) -> Value {
        let snapshot = self.snapshot(&format!("{request_prefix}-bounds"));
        let bounds = &node_by_automation_id(&snapshot, automation_id)["visible_bounds"];
        let x = bounds["x"].as_f64().expect("bounds x")
            + bounds["w"].as_f64().expect("bounds width") * x_fraction;
        let y = bounds["y"].as_f64().expect("bounds y")
            + bounds["h"].as_f64().expect("bounds height") * y_fraction;
        let changed = perform_until_presentable(
            &self.demo,
            &mut self.connection,
            self.window_id,
            self.generation,
            request_prefix,
            None,
            json!({ "kind": "click_at", "x": x, "y": y }),
        );
        let revision = changed["revision"].as_u64().expect("click revision");
        wait_for_revision(
            &mut self.connection,
            request_prefix,
            self.window_id,
            self.generation,
            revision,
        );
        self.snapshot(&format!("{request_prefix}-after"))
    }

    fn click_at_offset(
        &mut self,
        automation_id: &str,
        x_fraction: f64,
        y_offset: f64,
        request_prefix: &str,
    ) -> Value {
        let snapshot = self.snapshot(&format!("{request_prefix}-bounds"));
        let bounds = &node_by_automation_id(&snapshot, automation_id)["visible_bounds"];
        let x = bounds["x"].as_f64().expect("bounds x")
            + bounds["w"].as_f64().expect("bounds width") * x_fraction;
        let y = bounds["y"].as_f64().expect("bounds y") + y_offset;
        let changed = perform_until_presentable(
            &self.demo,
            &mut self.connection,
            self.window_id,
            self.generation,
            request_prefix,
            None,
            json!({ "kind": "click_at", "x": x, "y": y }),
        );
        let revision = changed["revision"].as_u64().expect("click revision");
        wait_for_revision(
            &mut self.connection,
            request_prefix,
            self.window_id,
            self.generation,
            revision,
        );
        self.snapshot(&format!("{request_prefix}-after"))
    }

    fn close(mut self) {
        drop(self.connection);
        self.demo.close_and_wait();
    }
}

fn wait_for_revision(
    connection: &mut BufReader<File>,
    request_prefix: &str,
    window_id: u64,
    generation: u64,
    revision: u64,
) {
    let request_id = format!("{request_prefix}-presented-{revision}");
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
    assert_success(&response, &format!("{request_prefix}-presented-{revision}"));
    assert_eq!(response["outcome"], "presented");
}

fn assert_following_content_below(
    snapshot: &Value,
    target_id: &str,
    following_id: &str,
    minimum_height: f64,
) {
    let target = &node_by_automation_id(snapshot, target_id)["visible_bounds"];
    let following = &node_by_automation_id(snapshot, following_id)["visible_bounds"];
    let target_y = target["y"].as_f64().expect("target y");
    let target_h = target["h"].as_f64().expect("target height");
    let following_y = following["y"].as_f64().expect("following y");
    assert!(
        target_h >= minimum_height,
        "text target must reserve every line: {target}"
    );
    assert!(
        following_y >= target_y + target_h,
        "following content must stay below text: target={target}, following={following}"
    );
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes Button CJK evidence"]
fn real_demo_button_reserves_its_cjk_text_width() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut session = ComponentQaSession::open(0, "button-cjk");
    let snapshot = session.snapshot("button-cjk-snapshot");
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-id")["name"],
        "button"
    );
    let target = &node_by_automation_id(&snapshot, "component-qa-target")["visible_bounds"];
    assert!(
        target["w"].as_f64().is_some_and(|width| width >= 114.0),
        "CJK button must reserve its text width plus padding: {target}"
    );
    session.capture("uix-button-cjk", "button-cjk-light-desktop.png");
    session.close();
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes Input text evidence"]
fn real_demo_input_preserves_unicode_accessories_and_multiline_editing() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut session = ComponentQaSession::open(21, "input-text");
    let snapshot = session.snapshot("input-text-snapshot");
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-id")["name"],
        "input"
    );

    let target = node_by_automation_id(&snapshot, "component-qa-target");
    assert_eq!(target["visible_bounds"]["w"].as_f64(), Some(122.0));
    assert_eq!(target["visible_bounds"]["h"].as_f64(), Some(32.0));
    assert_eq!(target["role"], "text_box");
    assert_eq!(target["name"], "请输入");
    assert!(target["actions"]
        .as_array()
        .is_some_and(|actions| actions.contains(&json!("set_value"))));

    let clearable = node_by_automation_id(&snapshot, "component-qa-input-clearable");
    assert_eq!(clearable["visible_bounds"]["w"].as_f64(), Some(100.0));
    assert_eq!(clearable["state"]["value_text"], "待清除");
    let password = node_by_automation_id(&snapshot, "component-qa-input-password");
    assert_eq!(password["visible_bounds"]["w"].as_f64(), Some(104.0));
    assert_eq!(password["state"]["password"], true);
    assert!(password["state"]["value_text"].is_null());
    let textarea = node_by_automation_id(&snapshot, "component-qa-input-textarea");
    assert_eq!(textarea["visible_bounds"]["w"].as_f64(), Some(80.0));
    assert_eq!(textarea["visible_bounds"]["h"].as_f64(), Some(60.0));
    assert_eq!(textarea["state"]["multiline"], true);
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-input-small")["visible_bounds"]["h"],
        24.0
    );
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-input-large")["visible_bounds"]["h"],
        40.0
    );

    let changed = session.set_value("component-qa-target", "测试", "input-text-set");
    assert_eq!(
        node_by_automation_id(&changed, "component-qa-target")["state"]["value_text"],
        "测试"
    );
    let cleared = session.click_right_slot("component-qa-input-clearable", "input-text-clear");
    assert!(
        node_by_automation_id(&cleared, "component-qa-input-clearable")["state"]["value_text"]
            .is_null()
    );
    let normalized =
        session.set_value("component-qa-input-textarea", "甲\r\n乙", "input-text-crlf");
    assert_eq!(
        node_by_automation_id(&normalized, "component-qa-input-textarea")["state"]["value_text"],
        "甲\n乙"
    );
    let focused = session.focus("component-qa-input-textarea", "input-text-focus");
    assert_eq!(
        node_by_automation_id(&focused, "component-qa-input-textarea")["focused"],
        true
    );
    session.capture("uix-input-text", "input-text-light-focus.png");
    session.close();
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes InputNumber evidence"]
fn real_demo_input_number_preserves_precision_and_full_control_geometry() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut session = ComponentQaSession::open(22, "input-number");
    let snapshot = session.snapshot("input-number-snapshot");
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-id")["name"],
        "input-number"
    );

    let target = node_by_automation_id(&snapshot, "component-qa-target");
    assert_eq!(target["visible_bounds"]["w"].as_f64(), Some(112.0));
    assert_eq!(target["visible_bounds"]["h"].as_f64(), Some(32.0));
    assert_eq!(target["role"], "spin_button");
    assert_eq!(target["name"], "精确数值");
    assert_eq!(target["state"]["value_now"].as_f64(), Some(1.2345));
    assert_eq!(target["state"]["value_min"].as_f64(), Some(-10.0));
    assert_eq!(target["state"]["value_max"].as_f64(), Some(10.0));
    assert!(target["actions"]
        .as_array()
        .is_some_and(|actions| actions.contains(&json!("insert_text"))
            && actions.contains(&json!("increment"))
            && actions.contains(&json!("decrement"))));

    let decimal = node_by_automation_id(&snapshot, "component-qa-input-number-decimal");
    assert_eq!(decimal["visible_bounds"]["w"].as_f64(), Some(112.0));
    assert_eq!(decimal["state"]["value_now"].as_f64(), Some(0.0));
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-input-number-small")["visible_bounds"]["h"],
        24.0
    );
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-input-number-small")["visible_bounds"]["w"],
        104.0
    );
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-input-number-large")["visible_bounds"]["h"],
        40.0
    );
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-input-number-large")["visible_bounds"]["w"],
        120.0
    );
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-input-number-long")["visible_bounds"]["w"],
        112.0
    );
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-input-number-disabled")["state"]["disabled"],
        true
    );

    let incremented = session.increment("component-qa-target", "input-number-increment");
    assert_eq!(
        node_by_automation_id(&incremented, "component-qa-target")["state"]["value_now"].as_f64(),
        Some(1.3345)
    );

    for index in 0..3 {
        let stepped = session.increment(
            "component-qa-input-number-decimal",
            &format!("input-number-decimal-step-{index}"),
        );
        let expected = f64::from(index + 1) / 10.0;
        assert_eq!(
            node_by_automation_id(&stepped, "component-qa-input-number-decimal")["state"]
                ["value_now"]
                .as_f64(),
            Some(expected)
        );
    }

    for index in 0..3 {
        session.press_key("backspace", &format!("input-number-clear-{index}"));
    }
    session.insert_text(
        "component-qa-input-number-decimal",
        "7.25",
        "input-number-edit",
    );
    let committed = session.press_key("enter", "input-number-commit");
    assert_eq!(
        node_by_automation_id(&committed, "component-qa-input-number-decimal")["state"]
            ["value_now"]
            .as_f64(),
        Some(7.25)
    );

    let pointer_up = session.click_at_fraction(
        "component-qa-input-number-decimal",
        0.9,
        0.2,
        "input-number-pointer-up",
    );
    assert_eq!(
        node_by_automation_id(&pointer_up, "component-qa-input-number-decimal")["state"]
            ["value_now"]
            .as_f64(),
        Some(7.35)
    );
    let pointer_down = session.click_at_fraction(
        "component-qa-input-number-decimal",
        0.9,
        0.8,
        "input-number-pointer-down",
    );
    assert_eq!(
        node_by_automation_id(&pointer_down, "component-qa-input-number-decimal")["state"]
            ["value_now"]
            .as_f64(),
        Some(7.25)
    );

    let focused = session.focus("component-qa-target", "input-number-focus");
    assert_eq!(
        node_by_automation_id(&focused, "component-qa-target")["focused"],
        true
    );
    session.capture("uix-input-number", "input-number-light-focus.png");
    session.close();
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes Select evidence"]
fn real_demo_select_preserves_search_commit_clips_text_and_flips_popup() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut session = ComponentQaSession::open(23, "select");
    let snapshot = session.snapshot("select-snapshot");
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-id")["name"],
        "select"
    );

    let target = node_by_automation_id(&snapshot, "component-qa-target");
    assert_eq!(target["visible_bounds"]["w"].as_f64(), Some(120.0));
    assert_eq!(target["visible_bounds"]["h"].as_f64(), Some(32.0));
    assert_eq!(target["role"], "combobox");
    assert_eq!(target["name"], "搜索成员");
    assert_eq!(target["state"]["value_text"], "Beta");
    assert_eq!(
        target["selection"]["options"],
        json!(["Alpha", "Alpine", "Beta"])
    );
    assert_eq!(target["selection"]["selected_indices"], json!([2]));
    assert_eq!(target["selection"]["expanded"], false);
    assert!(target["actions"]
        .as_array()
        .is_some_and(|actions| actions.contains(&json!("focus"))
            && actions.contains(&json!("insert_text"))
            && actions.contains(&json!("select"))));

    let multiple = node_by_automation_id(&snapshot, "component-qa-select-multiple");
    assert!(multiple["visible_bounds"]["w"]
        .as_f64()
        .is_some_and(|width| (120.0..=160.0).contains(&width)));
    assert_eq!(multiple["selection"]["multiple"], true);
    assert_eq!(multiple["selection"]["selected_indices"], json!([]));
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-select-small")["visible_bounds"]["h"],
        24.0
    );
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-select-large")["visible_bounds"]["h"],
        40.0
    );
    assert!(
        node_by_automation_id(&snapshot, "component-qa-select-long")["visible_bounds"]["w"]
            .as_f64()
            .is_some_and(|width| (80.0..=140.0).contains(&width))
    );
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-select-disabled")["state"]["disabled"],
        true
    );

    session.focus("component-qa-target", "select-focus");
    let queried = session.insert_text("component-qa-target", "al", "select-query");
    let queried_target = node_by_automation_id(&queried, "component-qa-target");
    assert_eq!(queried_target["state"]["value_text"], "al");
    assert_eq!(queried_target["selection"]["selected_indices"], json!([2]));
    assert_eq!(queried_target["selection"]["expanded"], true);

    let navigated = session.press_key("down", "select-query-down");
    let navigated_target = node_by_automation_id(&navigated, "component-qa-target");
    assert_eq!(navigated_target["state"]["value_text"], "al");
    assert_eq!(
        navigated_target["selection"]["selected_indices"],
        json!([2]),
        "arrow navigation must not publish before Enter"
    );
    let committed = session.press_key("enter", "select-query-enter");
    let committed_target = node_by_automation_id(&committed, "component-qa-target");
    assert_eq!(committed_target["state"]["value_text"], "Alpine");
    assert_eq!(
        committed_target["selection"]["selected_indices"],
        json!([1])
    );
    assert_eq!(committed_target["selection"]["expanded"], false);

    session.click_at_fraction("component-qa-target", 0.5, 0.5, "select-open");
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-select", "select-light-open.png");

    session.click_at_fraction(
        "component-qa-select-multiple",
        0.5,
        0.5,
        "select-multiple-open",
    );
    let multi_selected = session.click_at_offset(
        "component-qa-select-multiple",
        0.5,
        32.0 + 14.0,
        "select-multiple-first",
    );
    let multiple = node_by_automation_id(&multi_selected, "component-qa-select-multiple");
    assert_eq!(multiple["selection"]["selected_indices"], json!([0]));
    assert_eq!(multiple["selection"]["expanded"], true);
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-select", "select-multiple-open.png");

    session.click_at_fraction("component-qa-select-empty", 0.5, 0.5, "select-empty-open");
    let empty = session.snapshot("select-empty-open-snapshot");
    assert_eq!(
        node_by_automation_id(&empty, "component-qa-select-empty")["selection"]["expanded"],
        true
    );
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-select", "select-empty-open.png");
    session.press_key("escape", "select-empty-close");

    let bottom_bounds =
        &node_by_automation_id(&snapshot, "component-qa-select-bottom")["visible_bounds"];
    let bottom_y = bottom_bounds["y"].as_f64().expect("bottom select y");
    let bottom_h = bottom_bounds["h"].as_f64().expect("bottom select height");
    assert!(
        bottom_y + bottom_h + 280.0 > 800.0,
        "QA bottom Select must require upward placement: {bottom_bounds}"
    );
    session.click_at_fraction("component-qa-select-bottom", 0.5, 0.5, "select-bottom-open");
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-select", "select-flip-above.png");
    let flipped_selection = session.click_at_offset(
        "component-qa-select-bottom",
        0.5,
        -14.0,
        "select-bottom-last-visible",
    );
    let bottom = node_by_automation_id(&flipped_selection, "component-qa-select-bottom");
    assert_eq!(bottom["selection"]["selected_indices"], json!([9]));
    assert_eq!(bottom["state"]["value_text"], "选项十");
    assert_eq!(bottom["selection"]["expanded"], false);
    session.close();
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes Segmented evidence"]
fn real_demo_segmented_uses_compact_cjk_geometry_and_keyboard_wrap() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut session = ComponentQaSession::open(37, "segmented");
    let snapshot = session.snapshot("segmented-snapshot");
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-id")["name"],
        "segmented"
    );

    thread::sleep(Duration::from_millis(300));
    session.capture("uix-segmented", "segmented-after.png");

    let target = node_by_automation_id(&snapshot, "component-qa-target");
    assert_eq!(target["visible_bounds"]["w"].as_f64(), Some(200.0));
    assert_eq!(target["visible_bounds"]["h"].as_f64(), Some(32.0));
    assert_eq!(target["role"], "radio_group");
    assert_eq!(
        target["selection"]["options"],
        json!(["每日", "每周", "每月", "每年"])
    );
    assert_eq!(target["selection"]["selected_indices"], json!([2]));

    let disabled_option =
        node_by_automation_id(&snapshot, "component-qa-segmented-disabled-option");
    assert_eq!(disabled_option["selection"]["disabled_indices"], json!([1]));
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-segmented-disabled")["state"]["disabled"],
        true
    );
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-segmented-small")["visible_bounds"]["h"],
        24.0
    );
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-segmented-large")["visible_bounds"]["h"],
        40.0
    );
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-segmented-long")["visible_bounds"]["w"],
        180.0
    );
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-segmented-constrained")["visible_bounds"]
            ["h"],
        20.0
    );
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-segmented-constrained")["selection"]
            ["selected_indices"],
        json!([0])
    );

    let full_frame = session.click_at_offset(
        "component-qa-segmented-constrained",
        0.85,
        10.0,
        "segmented-full-frame-second",
    );
    assert_eq!(
        node_by_automation_id(&full_frame, "component-qa-segmented-constrained")["selection"]
            ["selected_indices"],
        json!([1])
    );
    session.capture("uix-segmented", "segmented-full-frame.png");

    let long_second = session.click_at_offset(
        "component-qa-segmented-long",
        0.90,
        16.0,
        "segmented-long-second",
    );
    assert_eq!(
        node_by_automation_id(&long_second, "component-qa-segmented-long")["selection"]
            ["selected_indices"],
        json!([1])
    );
    session.capture("uix-segmented", "segmented-long-second.png");

    session.focus("component-qa-target", "segmented-focus");
    let next = session.press_key("right", "segmented-next");
    assert_eq!(
        node_by_automation_id(&next, "component-qa-target")["selection"]["selected_indices"],
        json!([3])
    );
    let wrapped = session.press_key("right", "segmented-wrap");
    assert_eq!(
        node_by_automation_id(&wrapped, "component-qa-target")["selection"]["selected_indices"],
        json!([0])
    );

    session.focus(
        "component-qa-segmented-disabled-option",
        "segmented-disabled-option-focus",
    );
    let skipped = session.press_key("right", "segmented-disabled-option-skip");
    assert_eq!(
        node_by_automation_id(&skipped, "component-qa-segmented-disabled-option")["selection"]
            ["selected_indices"],
        json!([2])
    );
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-segmented", "segmented-keyboard.png");
    session.close();
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes FormItem text-layout evidence"]
fn real_demo_form_item_clips_long_text_and_reserves_help_rows() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut session = ComponentQaSession::open(39, "form-item-text-layout");
    let snapshot = session.snapshot("form-item-text-layout-snapshot");
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-id")["name"],
        "form-item"
    );

    let target = node_by_automation_id(&snapshot, "component-qa-target");
    assert_eq!(target["role"], "group");
    assert_eq!(target["name"], "用户名");
    assert_eq!(target["state"]["required"], true);
    assert_eq!(target["visible_bounds"]["h"], 60.0);

    let long = node_by_automation_id(&snapshot, "component-qa-form-item-long");
    assert_eq!(long["visible_bounds"]["w"], 220.0);
    assert_eq!(long["visible_bounds"]["h"], 60.0);
    let narrow = node_by_automation_id(&snapshot, "component-qa-form-item-narrow");
    assert_eq!(narrow["visible_bounds"]["w"], 160.0);
    assert_eq!(narrow["visible_bounds"]["h"], 60.0);
    assert_eq!(narrow["state"]["required"], true);
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-form-item-inline")["visible_bounds"]["h"],
        60.0
    );

    thread::sleep(Duration::from_millis(300));
    session.capture("uix-form-item", "form-item-text-layout.png");
    session.close();
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes Form layout evidence"]
fn real_demo_form_renders_vertical_and_horizontal_items() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut session = ComponentQaSession::open(38, "form-layout");
    let snapshot = session.snapshot("form-layout-snapshot");
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-id")["name"],
        "form"
    );
    let target = node_by_automation_id(&snapshot, "component-qa-target");
    assert_eq!(target["visible_bounds"]["h"], 200.0);

    thread::sleep(Duration::from_millis(300));
    session.capture("uix-form", "form-layout.png");
    session.close();
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes Avatar text-layout evidence"]
fn real_demo_avatar_scales_cjk_text_and_clips_constrained_geometry() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut session = ComponentQaSession::open(40, "avatar-text-layout");
    let snapshot = session.snapshot("avatar-text-layout-snapshot");
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-id")["name"],
        "avatar"
    );

    let target = node_by_automation_id(&snapshot, "component-qa-target");
    assert_eq!(target["role"], "image");
    assert_eq!(target["name"], "UI");
    assert_eq!(target["visible_bounds"]["w"], 32.0);
    assert_eq!(target["visible_bounds"]["h"], 32.0);
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-avatar-cjk")["visible_bounds"]["w"],
        48.0
    );
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-avatar-large")["visible_bounds"]["w"],
        64.0
    );
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-avatar-square")["visible_bounds"]["h"],
        48.0
    );
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-avatar-long")["name"],
        "研发中心"
    );
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-avatar-fallback")["name"],
        "回退"
    );
    let image = node_by_automation_id(&snapshot, "component-qa-avatar-image");
    assert_eq!(image["name"], "图");
    assert_eq!(image["visible_bounds"]["w"], 48.0);
    assert_eq!(image["visible_bounds"]["h"], 48.0);
    let square_image = node_by_automation_id(&snapshot, "component-qa-avatar-square-image");
    assert_eq!(square_image["name"], "图");
    assert_eq!(square_image["visible_bounds"]["w"], 48.0);
    assert_eq!(square_image["visible_bounds"]["h"], 48.0);
    let constrained = node_by_automation_id(&snapshot, "component-qa-avatar-constrained");
    assert_eq!(constrained["visible_bounds"]["w"], 64.0);
    assert_eq!(constrained["visible_bounds"]["h"], 24.0);

    thread::sleep(Duration::from_millis(300));
    session.capture("uix-avatar", "avatar-text-layout-light.png");
    let dark = session.invoke("theme-toggle", "avatar-dark-theme");
    assert_eq!(
        node_by_automation_id(&dark, "component-qa-id")["name"],
        "avatar"
    );
    assert_eq!(
        node_by_automation_id(&dark, "component-qa-avatar-constrained")["visible_bounds"]["h"],
        24.0
    );
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-avatar", "avatar-text-layout-dark.png");
    session.close();
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes Calendar layout evidence"]
fn real_demo_calendar_localizes_scales_and_keeps_pointer_keyboard_geometry_aligned() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut session = ComponentQaSession::open(42, "calendar-layout");
    let snapshot = session.snapshot("calendar-layout-snapshot");
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-id")["name"],
        "calendar"
    );

    let target = node_by_automation_id(&snapshot, "component-qa-target");
    assert_eq!(target["role"], "group");
    assert_eq!(target["visible_bounds"]["w"], 154.0);
    assert_eq!(target["visible_bounds"]["h"], 172.0);
    assert_eq!(
        target["state"]["value_text"],
        "2026-07-15; focused 2026-07-15"
    );
    let english = node_by_automation_id(&snapshot, "component-qa-calendar-english");
    assert_eq!(english["visible_bounds"]["w"], 140.0);
    assert_eq!(english["visible_bounds"]["h"], 160.0);
    assert_eq!(
        english["state"]["value_text"],
        "2026-09-30; focused 2026-09-30"
    );
    let constrained = node_by_automation_id(&snapshot, "component-qa-calendar-constrained");
    assert_eq!(constrained["visible_bounds"]["w"], 140.0);
    assert_eq!(constrained["visible_bounds"]["h"], 100.0);
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-calendar-year-jump")["state"]["value_text"],
        "9999-12-31; focused 9999-12-31"
    );

    thread::sleep(Duration::from_millis(300));
    session.capture("uix-calendar", "calendar-layout-light.png");
    let focused = session.focus("component-qa-target", "calendar-focus");
    assert_eq!(
        node_by_automation_id(&focused, "component-qa-target")["focused"],
        true
    );
    let moved = session.press_key("right", "calendar-keyboard-right");
    assert_eq!(
        node_by_automation_id(&moved, "component-qa-target")["state"]["value_text"],
        "2026-07-15; focused 2026-07-16"
    );
    let committed = session.press_key("enter", "calendar-keyboard-commit");
    assert_eq!(
        node_by_automation_id(&committed, "component-qa-target")["state"]["value_text"],
        "2026-07-16; focused 2026-07-16"
    );
    let previous = session.click_at_fraction(
        "component-qa-target",
        0.05,
        0.04,
        "calendar-pointer-previous",
    );
    assert_eq!(
        node_by_automation_id(&previous, "component-qa-target")["state"]["value_text"],
        "2026-07-16; focused 2026-06-16"
    );
    let restored = session.press_key("page_down", "calendar-restore-month");
    assert_eq!(
        node_by_automation_id(&restored, "component-qa-target")["state"]["value_text"],
        "2026-07-16; focused 2026-07-16"
    );
    session.capture("uix-calendar", "calendar-layout-keyboard.png");

    let dark = session.invoke("theme-toggle", "calendar-dark-theme");
    assert_eq!(
        node_by_automation_id(&dark, "component-qa-calendar-constrained")["visible_bounds"]["h"],
        100.0
    );
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-calendar", "calendar-layout-dark.png");
    session.close();
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes Card layout evidence"]
fn real_demo_card_fits_text_clips_content_and_exposes_keyboard_actions() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut session = ComponentQaSession::open(43, "card-layout");
    let snapshot = session.snapshot("card-layout-snapshot");
    let target_body_bounds = snapshot["nodes"]
        .as_array()
        .expect("card snapshot nodes")
        .iter()
        .find(|node| node["name"] == "正文层级与安全边距")
        .map(|node| node["visible_bounds"].clone())
        .unwrap_or_else(|| panic!("missing Card body label in snapshot: {snapshot}"));
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-id")["name"],
        "card"
    );

    let target = node_by_automation_id(&snapshot, "component-qa-target");
    assert_eq!(target["role"], "group");
    assert_eq!(target["name"], "质量卡片");
    assert_eq!(target["visible_bounds"]["w"], 210.0);
    assert_eq!(target["visible_bounds"]["h"], 128.0);

    let long = node_by_automation_id(&snapshot, "component-qa-card-long");
    assert_eq!(long["name"], "A very long title must fit safely");
    assert_eq!(long["visible_bounds"]["w"], 180.0);
    assert_eq!(long["visible_bounds"]["h"], 110.0);

    let constrained = node_by_automation_id(&snapshot, "component-qa-card-constrained");
    assert_eq!(constrained["visible_bounds"]["w"], 120.0);
    assert_eq!(constrained["visible_bounds"]["h"], 64.0);
    let padded = node_by_automation_id(&snapshot, "component-qa-card-padding");
    assert_eq!(padded["visible_bounds"]["w"], 92.0);
    assert_eq!(padded["visible_bounds"]["h"], 72.0);

    let actions = node_by_automation_id(&snapshot, "component-qa-card-actions");
    assert_eq!(actions["role"], "group");
    assert_eq!(actions["name"], "Deployment");
    assert_eq!(actions["state"]["value_text"], "Open detailed settings");
    assert!(actions["actions"]
        .as_array()
        .is_some_and(|available| available.contains(&json!("focus"))));

    thread::sleep(Duration::from_millis(300));
    let light_path = session.capture("uix-card", "card-layout-light.png");
    let light_image = image::open(&light_path)
        .expect("open Card light evidence")
        .to_rgba8();
    let body_x = target_body_bounds["x"].as_f64().expect("Card body x") as u32;
    let body_y = target_body_bounds["y"].as_f64().expect("Card body y") as u32;
    let body_w = target_body_bounds["w"].as_f64().expect("Card body width") as u32;
    let body_h = target_body_bounds["h"].as_f64().expect("Card body height") as u32;
    let body_ink = (body_y..body_y + body_h)
        .flat_map(|y| (body_x..body_x + body_w).map(move |x| (x, y)))
        .filter(|(x, y)| {
            let pixel = light_image.get_pixel(*x, *y);
            u16::from(pixel[0]) + u16::from(pixel[1]) + u16::from(pixel[2]) < 600
        })
        .count();
    assert!(
        body_ink > 0,
        "Card body text must remain visible after the clipped child pass"
    );
    let hovered = session.move_pointer_to("component-qa-card-hover", "card-hover");
    assert_eq!(
        node_by_automation_id(&hovered, "component-qa-card-hover")["name"],
        "Elevation 2"
    );
    session.capture("uix-card", "card-layout-hover.png");

    let focused = session.focus("component-qa-card-actions", "card-actions-focus");
    assert_eq!(
        node_by_automation_id(&focused, "component-qa-card-actions")["focused"],
        true
    );
    let moved = session.press_key("right", "card-actions-right");
    assert_eq!(
        node_by_automation_id(&moved, "component-qa-card-actions")["state"]["value_text"],
        "Cancel operation"
    );
    let submitted = session.press_key("enter", "card-actions-submit");
    assert_eq!(
        node_by_automation_id(&submitted, "component-qa-card-actions")["state"]["value_text"],
        "Cancel operation"
    );
    session.capture("uix-card", "card-layout-keyboard.png");

    let dark = session.invoke("theme-toggle", "card-dark-theme");
    assert_eq!(
        node_by_automation_id(&dark, "component-qa-card-constrained")["visible_bounds"]["h"],
        64.0
    );
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-card", "card-layout-dark.png");
    session.close();
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes Checkbox CJK evidence"]
fn real_demo_checkbox_uses_compact_cjk_width_and_toggles() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut session = ComponentQaSession::open(24, "checkbox-cjk");
    let snapshot = session.snapshot("checkbox-cjk-snapshot");
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-id")["name"],
        "checkbox"
    );
    let target = &node_by_automation_id(&snapshot, "component-qa-target")["visible_bounds"];
    assert_eq!(target["w"].as_f64(), Some(74.0));
    assert_eq!(target["h"].as_f64(), Some(32.0));
    let empty = &node_by_automation_id(&snapshot, "component-qa-checkbox-empty")["visible_bounds"];
    assert_eq!(empty["w"].as_f64(), Some(16.0));
    assert_eq!(empty["h"].as_f64(), Some(32.0));

    assert!(
        node_by_automation_id(&snapshot, "component-qa-target")["actions"]
            .as_array()
            .is_some_and(|actions| actions.contains(&json!("toggle")))
    );
    let toggled = session.toggle("component-qa-target", "checkbox-cjk-toggle");
    let target = node_by_automation_id(&toggled, "component-qa-target");
    assert_eq!(target["role"], "checkbox");
    assert_eq!(target["name"], "同意协议");
    assert_eq!(target["state"]["checked"], true);
    session.capture("uix-checkbox-cjk", "checkbox-cjk-light-desktop.png");
    session.close();
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes Radio CJK evidence"]
fn real_demo_radio_uses_unicode_width_and_selects_with_focus() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut session = ComponentQaSession::open(25, "radio-cjk");
    let snapshot = session.snapshot("radio-cjk-snapshot");
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-id")["name"],
        "radio"
    );

    let target = node_by_automation_id(&snapshot, "component-qa-target");
    assert_eq!(target["visible_bounds"]["w"].as_f64(), Some(168.0));
    assert_eq!(target["visible_bounds"]["h"].as_f64(), Some(32.0));
    assert_eq!(target["role"], "radio_group");
    assert_eq!(target["name"], "水果");
    assert_eq!(target["state"]["value_text"], "香蕉");
    assert!(target["actions"]
        .as_array()
        .is_some_and(|actions| actions.contains(&json!("select"))));

    let small = &node_by_automation_id(&snapshot, "component-qa-radio-small")["visible_bounds"];
    assert!(small["w"]
        .as_f64()
        .is_some_and(|width| (width - 135.04999).abs() < 0.01));
    assert_eq!(small["h"].as_f64(), Some(24.0));
    let large = &node_by_automation_id(&snapshot, "component-qa-radio-large")["visible_bounds"];
    assert!(large["w"]
        .as_f64()
        .is_some_and(|width| (width - 199.70665).abs() < 0.01));
    assert_eq!(large["h"].as_f64(), Some(40.0));
    let vertical =
        &node_by_automation_id(&snapshot, "component-qa-radio-vertical")["visible_bounds"];
    assert_eq!(vertical["w"].as_f64(), Some(120.0));
    assert_eq!(vertical["h"].as_f64(), Some(96.0));

    let selected = session.select("component-qa-target", "2", "radio-cjk-select");
    let target = node_by_automation_id(&selected, "component-qa-target");
    assert_eq!(target["state"]["value_text"], "樱桃");

    let wrapped = session.press_key("right", "radio-cjk-wrap");
    let target = node_by_automation_id(&wrapped, "component-qa-target");
    assert_eq!(target["state"]["value_text"], "苹果");
    session.capture("uix-radio-cjk", "radio-cjk-light-focus.png");
    session.close();
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes Switch geometry evidence"]
fn real_demo_switch_preserves_all_sizes_and_toggles() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut session = ComponentQaSession::open(26, "switch-geometry");
    let snapshot = session.snapshot("switch-geometry-snapshot");
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-id")["name"],
        "switch"
    );

    let target = &node_by_automation_id(&snapshot, "component-qa-target")["visible_bounds"];
    assert_eq!(target["w"].as_f64(), Some(40.0));
    assert_eq!(target["h"].as_f64(), Some(32.0));
    let small = &node_by_automation_id(&snapshot, "component-qa-switch-small")["visible_bounds"];
    assert_eq!(small["w"].as_f64(), Some(28.0));
    assert_eq!(small["h"].as_f64(), Some(24.0));
    let large = &node_by_automation_id(&snapshot, "component-qa-switch-large")["visible_bounds"];
    assert_eq!(large["w"].as_f64(), Some(52.0));
    assert_eq!(large["h"].as_f64(), Some(40.0));

    assert!(
        node_by_automation_id(&snapshot, "component-qa-target")["actions"]
            .as_array()
            .is_some_and(|actions| actions.contains(&json!("toggle")))
    );
    let toggled = session.toggle("component-qa-target", "switch-geometry-toggle");
    let target = node_by_automation_id(&toggled, "component-qa-target");
    assert_eq!(target["role"], "switch");
    assert_eq!(target["state"]["checked"], true);
    session.capture("uix-switch-geometry", "switch-geometry-light-desktop.png");
    session.close();
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes Slider evidence"]
fn real_demo_slider_aligns_steps_and_clips_constrained_geometry() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut session = ComponentQaSession::open(27, "slider");
    let snapshot = session.snapshot("slider-snapshot");
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-id")["name"],
        "slider"
    );

    let target = node_by_automation_id(&snapshot, "component-qa-target");
    assert_eq!(target["visible_bounds"]["w"].as_f64(), Some(200.0));
    assert_eq!(target["visible_bounds"]["h"].as_f64(), Some(32.0));
    assert_eq!(target["role"], "slider");
    assert_eq!(target["state"]["value_now"].as_f64(), Some(42.0));
    assert_eq!(target["state"]["value_min"].as_f64(), Some(0.0));
    assert_eq!(target["state"]["value_max"].as_f64(), Some(100.0));
    assert!(target["actions"]
        .as_array()
        .is_some_and(|actions| actions.contains(&json!("focus"))
            && actions.contains(&json!("increment"))
            && actions.contains(&json!("decrement"))));

    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-slider-minimum")["state"]["value_now"],
        0.0
    );
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-slider-small")["visible_bounds"]["h"],
        24.0
    );
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-slider-large")["visible_bounds"]["h"],
        40.0
    );
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-slider-decimal")["state"]["value_now"],
        0.3
    );
    let constrained =
        &node_by_automation_id(&snapshot, "component-qa-slider-constrained")["visible_bounds"];
    assert_eq!(constrained["w"].as_f64(), Some(80.0));
    assert_eq!(constrained["h"].as_f64(), Some(12.0));

    thread::sleep(Duration::from_millis(300));
    session.capture("uix-slider", "slider-after.png");

    let incremented = session.increment("component-qa-target", "slider-increment");
    assert_eq!(
        node_by_automation_id(&incremented, "component-qa-target")["state"]["value_now"],
        45.0,
        "off-grid keyboard input must advance to the next range-aligned step"
    );
    let decimal = session.increment("component-qa-slider-decimal", "slider-decimal-increment");
    assert_eq!(
        node_by_automation_id(&decimal, "component-qa-slider-decimal")["state"]["value_now"],
        0.4
    );
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-slider", "slider-keyboard.png");
    session.close();
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes Rate evidence"]
fn real_demo_rate_preserves_half_unicode_and_constrained_geometry() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut session = ComponentQaSession::open(28, "rate");
    let snapshot = session.snapshot("rate-snapshot");
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-id")["name"],
        "rate"
    );

    let target = node_by_automation_id(&snapshot, "component-qa-target");
    assert_eq!(target["visible_bounds"]["w"].as_f64(), Some(120.0));
    assert_eq!(target["visible_bounds"]["h"].as_f64(), Some(32.0));
    assert_eq!(target["role"], "slider");
    assert_eq!(target["state"]["value_now"].as_f64(), Some(3.0));
    assert_eq!(target["state"]["value_min"].as_f64(), Some(0.0));
    assert_eq!(target["state"]["value_max"].as_f64(), Some(5.0));
    assert!(target["actions"]
        .as_array()
        .is_some_and(|actions| actions.contains(&json!("focus"))
            && actions.contains(&json!("increment"))
            && actions.contains(&json!("decrement"))));

    let half = node_by_automation_id(&snapshot, "component-qa-rate-half");
    assert_eq!(half["visible_bounds"]["w"].as_f64(), Some(120.0));
    assert_eq!(half["visible_bounds"]["h"].as_f64(), Some(32.0));
    assert_eq!(half["state"]["value_now"].as_f64(), Some(5.0));
    assert_eq!(half["state"]["value_max"].as_f64(), Some(10.0));
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-rate-disabled")["state"]["disabled"],
        true
    );

    let custom = &node_by_automation_id(&snapshot, "component-qa-rate-custom")["visible_bounds"];
    assert!(
        custom["w"].as_f64().is_some_and(|width| width >= 152.0),
        "two custom Unicode cells must reserve their rendered text width: {custom}"
    );
    assert_eq!(custom["h"].as_f64(), Some(32.0));
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-rate-empty")["state"]["value_now"],
        0.0
    );
    let small = &node_by_automation_id(&snapshot, "component-qa-rate-small")["visible_bounds"];
    assert_eq!(small["w"].as_f64(), Some(96.0));
    assert_eq!(small["h"].as_f64(), Some(24.0));
    let large = &node_by_automation_id(&snapshot, "component-qa-rate-large")["visible_bounds"];
    assert_eq!(large["w"].as_f64(), Some(144.0));
    assert_eq!(large["h"].as_f64(), Some(40.0));
    let constrained =
        &node_by_automation_id(&snapshot, "component-qa-rate-constrained")["visible_bounds"];
    assert_eq!(constrained["w"].as_f64(), Some(80.0));
    assert_eq!(constrained["h"].as_f64(), Some(12.0));

    thread::sleep(Duration::from_millis(300));
    session.capture("uix-rate", "rate-after.png");

    let incremented = session.increment("component-qa-target", "rate-increment");
    assert_eq!(
        node_by_automation_id(&incremented, "component-qa-target")["state"]["value_now"],
        4.0
    );
    let half_incremented = session.increment("component-qa-rate-half", "rate-half-increment");
    assert_eq!(
        node_by_automation_id(&half_incremented, "component-qa-rate-half")["state"]["value_now"],
        6.0
    );
    let cleared =
        session.click_at_fraction("component-qa-target", 0.7, 0.5, "rate-clearable-click");
    assert_eq!(
        node_by_automation_id(&cleared, "component-qa-target")["state"]["value_now"],
        0.0
    );
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-rate", "rate-keyboard-clear.png");
    session.close();
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes DatePicker evidence"]
fn real_demo_date_picker_separates_calendar_text_and_clips_constrained_trigger() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut session = ComponentQaSession::open(29, "date-picker");
    let snapshot = session.snapshot("date-picker-snapshot");
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-id")["name"],
        "date-picker"
    );

    let target = node_by_automation_id(&snapshot, "component-qa-target");
    assert_eq!(target["visible_bounds"]["w"].as_f64(), Some(160.0));
    assert_eq!(target["visible_bounds"]["h"].as_f64(), Some(32.0));
    assert_eq!(target["role"], "combobox");
    assert_eq!(target["state"]["value_text"], "2026-07-17");
    assert_eq!(target["state"]["expanded"], false);
    assert!(target["actions"]
        .as_array()
        .is_some_and(|actions| actions.contains(&json!("focus"))));

    let empty = node_by_automation_id(&snapshot, "component-qa-date-empty");
    assert_eq!(empty["visible_bounds"]["w"].as_f64(), Some(160.0));
    assert_eq!(empty["visible_bounds"]["h"].as_f64(), Some(32.0));
    assert!(empty["state"]["value_text"].is_null());
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-date-small")["visible_bounds"]["h"],
        24.0
    );
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-date-large")["visible_bounds"]["h"],
        40.0
    );
    let constrained =
        &node_by_automation_id(&snapshot, "component-qa-date-constrained")["visible_bounds"];
    assert_eq!(constrained["w"].as_f64(), Some(80.0));
    assert_eq!(constrained["h"].as_f64(), Some(12.0));

    let focused = session.focus("component-qa-target", "date-picker-focus");
    assert_eq!(
        node_by_automation_id(&focused, "component-qa-target")["state"]["expanded"],
        false
    );
    let opened = session.press_key("enter", "date-picker-open");
    assert_eq!(
        node_by_automation_id(&opened, "component-qa-target")["state"]["expanded"],
        true
    );
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-date-picker", "date-picker-open.png");

    let selected = session.click_at_offset(
        "component-qa-target",
        0.37,
        105.0,
        "date-picker-select-first-row",
    );
    let target = node_by_automation_id(&selected, "component-qa-target");
    assert_eq!(target["state"]["value_text"], "2026-07-01");
    assert_eq!(target["state"]["expanded"], false);

    let _ = session.focus(
        "component-qa-date-constrained",
        "date-picker-constrained-focus",
    );
    let constrained_open = session.press_key("enter", "date-picker-constrained-open");
    assert_eq!(
        node_by_automation_id(&constrained_open, "component-qa-date-constrained")["state"]
            ["expanded"],
        true
    );
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-date-picker", "date-picker-constrained-open.png");
    let constrained_closed = session.press_key("escape", "date-picker-constrained-close");
    assert_eq!(
        node_by_automation_id(&constrained_closed, "component-qa-date-constrained")["state"]
            ["expanded"],
        false
    );
    session.close();
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes DateRangePicker evidence"]
fn real_demo_date_range_picker_previews_commits_and_overlays_long_presets() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut session = ComponentQaSession::open(30, "date-range-picker");
    let snapshot = session.snapshot("date-range-picker-snapshot");
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-id")["name"],
        "date-range-picker"
    );

    let target = node_by_automation_id(&snapshot, "component-qa-target");
    assert_eq!(target["visible_bounds"]["w"].as_f64(), Some(280.0));
    assert_eq!(target["visible_bounds"]["h"].as_f64(), Some(32.0));
    assert_eq!(target["role"], "combobox");
    assert_eq!(target["state"]["value_text"], "2026-07-01 / 2026-07-17");
    assert_eq!(target["state"]["expanded"], false);
    assert!(target["actions"]
        .as_array()
        .is_some_and(|actions| actions.contains(&json!("focus"))));

    let empty = node_by_automation_id(&snapshot, "component-qa-date-range-empty");
    assert_eq!(empty["visible_bounds"]["w"].as_f64(), Some(280.0));
    assert_eq!(empty["visible_bounds"]["h"].as_f64(), Some(32.0));
    assert!(empty["state"]["value_text"].is_null());
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-date-range-small")["visible_bounds"]["h"],
        24.0
    );
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-date-range-large")["visible_bounds"]["h"],
        40.0
    );
    let constrained =
        &node_by_automation_id(&snapshot, "component-qa-date-range-constrained")["visible_bounds"];
    assert_eq!(constrained["w"].as_f64(), Some(80.0));
    assert_eq!(constrained["h"].as_f64(), Some(12.0));

    let _ = session.focus("component-qa-target", "date-range-focus");
    let opened = session.press_key("enter", "date-range-open");
    assert_eq!(
        node_by_automation_id(&opened, "component-qa-target")["state"]["expanded"],
        true
    );
    let pending = session.click_at_offset("component-qa-target", 0.365, 105.0, "date-range-start");
    assert_eq!(
        node_by_automation_id(&pending, "component-qa-target")["state"]["expanded"],
        true,
        "the first date click must keep the popup open for the end date"
    );
    let preview =
        session.move_pointer_to_offset("component-qa-target", 0.635, 105.0, "date-range-preview");
    assert_eq!(
        node_by_automation_id(&preview, "component-qa-target")["state"]["expanded"],
        true
    );
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-date-range-picker", "date-range-preview.png");

    let committed = session.click_at_offset("component-qa-target", 0.635, 105.0, "date-range-end");
    let target = node_by_automation_id(&committed, "component-qa-target");
    assert_eq!(target["state"]["value_text"], "2026-07-01 / 2026-07-03");
    assert_eq!(target["state"]["expanded"], false);

    let reopened = session.click_at_fraction("component-qa-target", 0.5, 0.5, "date-range-reopen");
    assert_eq!(
        node_by_automation_id(&reopened, "component-qa-target")["state"]["expanded"],
        true
    );
    let preset =
        session.click_at_offset("component-qa-target", 0.5, 326.0, "date-range-long-preset");
    let target = node_by_automation_id(&preset, "component-qa-target");
    assert_eq!(target["state"]["value_text"], "2026-06-29 / 2026-07-03");
    assert_eq!(target["state"]["expanded"], false);
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-date-range-picker", "date-range-preset.png");

    let _ = session.focus(
        "component-qa-date-range-constrained",
        "date-range-constrained-focus",
    );
    let constrained_open = session.press_key("enter", "date-range-constrained-open");
    assert_eq!(
        node_by_automation_id(&constrained_open, "component-qa-date-range-constrained")["state"]
            ["expanded"],
        true
    );
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-date-range-picker", "date-range-constrained-open.png");
    let constrained_closed = session.press_key("escape", "date-range-constrained-close");
    assert_eq!(
        node_by_automation_id(&constrained_closed, "component-qa-date-range-constrained")["state"]
            ["expanded"],
        false
    );
    session.close();
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes TimePicker evidence"]
fn real_demo_time_picker_reaches_exact_late_minutes_and_clips_constrained_trigger() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut session = ComponentQaSession::open(31, "time-picker");
    let snapshot = session.snapshot("time-picker-snapshot");
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-id")["name"],
        "time-picker"
    );

    let target = node_by_automation_id(&snapshot, "component-qa-target");
    assert_eq!(target["visible_bounds"]["w"].as_f64(), Some(120.0));
    assert_eq!(target["visible_bounds"]["h"].as_f64(), Some(32.0));
    assert_eq!(target["role"], "combobox");
    assert_eq!(target["state"]["value_text"], "14:30");
    assert_eq!(target["state"]["expanded"], false);
    assert!(target["actions"]
        .as_array()
        .is_some_and(|actions| actions.contains(&json!("focus"))));

    let empty = node_by_automation_id(&snapshot, "component-qa-time-empty");
    assert_eq!(empty["visible_bounds"]["w"].as_f64(), Some(120.0));
    assert_eq!(empty["visible_bounds"]["h"].as_f64(), Some(32.0));
    assert!(empty["state"]["value_text"].is_null());
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-time-late")["state"]["value_text"],
        "23:59"
    );
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-time-small")["visible_bounds"]["h"],
        24.0
    );
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-time-large")["visible_bounds"]["h"],
        40.0
    );
    let constrained =
        &node_by_automation_id(&snapshot, "component-qa-time-constrained")["visible_bounds"];
    assert_eq!(constrained["w"].as_f64(), Some(80.0));
    assert_eq!(constrained["h"].as_f64(), Some(12.0));

    let _ = session.focus("component-qa-target", "time-picker-focus");
    let opened = session.press_key("enter", "time-picker-open");
    assert_eq!(
        node_by_automation_id(&opened, "component-qa-target")["state"]["expanded"],
        true
    );
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-time-picker", "time-picker-open.png");

    let _ = session.press_key("right", "time-picker-minute-column");
    let _ = session.press_key("down", "time-picker-minute-down");
    let committed = session.press_key("enter", "time-picker-commit-minute");
    let target = node_by_automation_id(&committed, "component-qa-target");
    assert_eq!(target["state"]["value_text"], "14:31");
    assert_eq!(target["state"]["expanded"], false);

    let _ = session.focus("component-qa-time-late", "time-picker-late-focus");
    let late_open = session.press_key("enter", "time-picker-late-open");
    assert_eq!(
        node_by_automation_id(&late_open, "component-qa-time-late")["state"]["expanded"],
        true
    );
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-time-picker", "time-picker-late.png");
    let _ = session.press_key("escape", "time-picker-late-close");

    let _ = session.focus(
        "component-qa-time-constrained",
        "time-picker-constrained-focus",
    );
    let constrained_open = session.press_key("enter", "time-picker-constrained-open");
    assert_eq!(
        node_by_automation_id(&constrained_open, "component-qa-time-constrained")["state"]
            ["expanded"],
        true
    );
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-time-picker", "time-picker-constrained-open.png");
    let constrained_closed = session.press_key("escape", "time-picker-constrained-close");
    assert_eq!(
        node_by_automation_id(&constrained_closed, "component-qa-time-constrained")["state"]
            ["expanded"],
        false
    );
    session.close();
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes ColorPicker evidence"]
fn real_demo_color_picker_marks_selection_navigates_grid_and_clips_constrained_swatch() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut session = ComponentQaSession::open(32, "color-picker");
    let snapshot = session.snapshot("color-picker-snapshot");
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-id")["name"],
        "color-picker"
    );

    let target = node_by_automation_id(&snapshot, "component-qa-target");
    assert_eq!(target["visible_bounds"]["w"].as_f64(), Some(32.0));
    assert_eq!(target["visible_bounds"]["h"].as_f64(), Some(32.0));
    assert_eq!(target["role"], "combobox");
    assert_eq!(target["state"]["value_text"], "#1677FFFF");
    assert_eq!(target["state"]["expanded"], false);
    assert!(target["actions"]
        .as_array()
        .is_some_and(|actions| actions.contains(&json!("focus"))));

    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-color-light")["state"]["value_text"],
        "#F0F0F0FF"
    );
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-color-alpha")["state"]["value_text"],
        "#1677FF60"
    );
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-color-small")["visible_bounds"]["h"],
        24.0
    );
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-color-large")["visible_bounds"]["h"],
        40.0
    );
    let constrained =
        &node_by_automation_id(&snapshot, "component-qa-color-constrained")["visible_bounds"];
    assert_eq!(constrained["w"].as_f64(), Some(12.0));
    assert_eq!(constrained["h"].as_f64(), Some(6.0));
    session.capture("uix-color-picker", "color-picker-alpha.png");

    let _ = session.focus("component-qa-target", "color-picker-focus");
    let opened = session.press_key("enter", "color-picker-open");
    assert_eq!(
        node_by_automation_id(&opened, "component-qa-target")["state"]["expanded"],
        true
    );
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-color-picker", "color-picker-open.png");

    let _ = session.press_key("right", "color-picker-next");
    let _ = session.press_key("down", "color-picker-next-row");
    let committed = session.press_key("enter", "color-picker-commit");
    let target = node_by_automation_id(&committed, "component-qa-target");
    assert_eq!(target["state"]["value_text"], "#B37FEBFF");
    assert_eq!(target["state"]["expanded"], false);

    let _ = session.focus("component-qa-color-light", "color-picker-light-focus");
    let light_open = session.press_key("enter", "color-picker-light-open");
    assert_eq!(
        node_by_automation_id(&light_open, "component-qa-color-light")["state"]["expanded"],
        true
    );
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-color-picker", "color-picker-light-selected.png");
    let _ = session.press_key("escape", "color-picker-light-close");

    let _ = session.focus(
        "component-qa-color-constrained",
        "color-picker-constrained-focus",
    );
    let constrained_open = session.press_key("enter", "color-picker-constrained-open");
    assert_eq!(
        node_by_automation_id(&constrained_open, "component-qa-color-constrained")["state"]
            ["expanded"],
        true
    );
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-color-picker", "color-picker-constrained-open.png");
    let constrained_closed = session.press_key("escape", "color-picker-constrained-close");
    assert_eq!(
        node_by_automation_id(&constrained_closed, "component-qa-color-constrained")["state"]
            ["expanded"],
        false
    );
    session.close();
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes Cascader evidence"]
fn real_demo_cascader_keeps_columns_clips_text_and_selects_by_keyboard_and_pointer() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut session = ComponentQaSession::open(33, "cascader");
    let snapshot = session.snapshot("cascader-snapshot");
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-id")["name"],
        "cascader"
    );

    let target = node_by_automation_id(&snapshot, "component-qa-target");
    assert_eq!(target["visible_bounds"]["w"].as_f64(), Some(120.0));
    assert_eq!(target["visible_bounds"]["h"].as_f64(), Some(32.0));
    assert_eq!(target["role"], "combobox");
    assert!(target["state"]["value_text"].is_null());
    assert_eq!(target["state"]["expanded"], false);
    assert!(target["actions"]
        .as_array()
        .is_some_and(|actions| actions.contains(&json!("focus"))));

    let empty = node_by_automation_id(&snapshot, "component-qa-cascader-empty");
    assert_eq!(empty["visible_bounds"]["w"].as_f64(), Some(120.0));
    assert_eq!(empty["visible_bounds"]["h"].as_f64(), Some(32.0));
    assert!(empty["state"]["value_text"].is_null());
    assert!(
        node_by_automation_id(&snapshot, "component-qa-cascader-disabled")["state"]["value_text"]
            .is_null()
    );
    let constrained =
        &node_by_automation_id(&snapshot, "component-qa-cascader-constrained")["visible_bounds"];
    assert_eq!(constrained["w"].as_f64(), Some(80.0));
    assert_eq!(constrained["h"].as_f64(), Some(12.0));

    session.ensure_foreground();
    let _ = session.focus("component-qa-target", "cascader-focus");
    let opened = session.press_key("enter", "cascader-open");
    assert_eq!(
        node_by_automation_id(&opened, "component-qa-target")["state"]["expanded"],
        true
    );
    let branch = session.press_key("enter", "cascader-enter-root");
    let target = node_by_automation_id(&branch, "component-qa-target");
    assert_eq!(target["state"]["value_text"], "北京");
    assert_eq!(target["state"]["expanded"], true);
    thread::sleep(Duration::from_millis(300));
    let after_wait = session.snapshot("cascader-after-two-columns-wait");
    let target = node_by_automation_id(&after_wait, "component-qa-target");
    assert_eq!(
        target["state"]["expanded"], true,
        "Cascader closed while settling its two-column animation: {target}"
    );
    session.capture("uix-cascader", "cascader-two-columns.png");
    let after_capture = session.snapshot("cascader-after-two-columns-capture");
    let target = node_by_automation_id(&after_capture, "component-qa-target");
    assert_eq!(
        target["state"]["expanded"], true,
        "Cascader closed while collecting two-column evidence: {target}"
    );
    assert_eq!(
        target["focused"], true,
        "Cascader lost keyboard focus while collecting evidence: {target}"
    );

    let keyboard_commit = session.press_key("enter", "cascader-enter-child");
    let target = node_by_automation_id(&keyboard_commit, "component-qa-target");
    assert_eq!(target["state"]["value_text"], "北京 / 海淀");
    assert_eq!(target["state"]["expanded"], false);

    let _ = session.press_key("enter", "cascader-reopen");
    let switched =
        session.click_at_offset("component-qa-target", 0.15, 82.0, "cascader-switch-root");
    let target = node_by_automation_id(&switched, "component-qa-target");
    assert_eq!(target["state"]["value_text"], "上海");
    assert_eq!(target["state"]["expanded"], true);
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-cascader", "cascader-pointer-branch.png");
    let pointer_commit =
        session.click_at_offset("component-qa-target", 1.84, 82.0, "cascader-select-child");
    let target = node_by_automation_id(&pointer_commit, "component-qa-target");
    assert_eq!(target["state"]["value_text"], "上海 / 徐汇");
    assert_eq!(target["state"]["expanded"], false);

    let _ = session.focus("component-qa-cascader-long", "cascader-long-focus");
    let long_open = session.press_key("enter", "cascader-long-open");
    assert_eq!(
        node_by_automation_id(&long_open, "component-qa-cascader-long")["state"]["expanded"],
        true
    );
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-cascader", "cascader-long-option.png");
    let _ = session.press_key("escape", "cascader-long-close");

    let _ = session.focus("component-qa-cascader-scroll", "cascader-scroll-focus");
    let _ = session.press_key("enter", "cascader-scroll-open");
    let _ = session.press_key("end", "cascader-scroll-end");
    let scrolled_commit = session.press_key("enter", "cascader-scroll-commit");
    let scrolled = node_by_automation_id(&scrolled_commit, "component-qa-cascader-scroll");
    assert_eq!(scrolled["state"]["value_text"], "区域 9");
    assert_eq!(scrolled["state"]["expanded"], false);

    let _ = session.focus(
        "component-qa-cascader-constrained",
        "cascader-constrained-focus",
    );
    let constrained_open = session.press_key("enter", "cascader-constrained-open");
    assert_eq!(
        node_by_automation_id(&constrained_open, "component-qa-cascader-constrained")["state"]
            ["expanded"],
        true
    );
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-cascader", "cascader-constrained-open.png");
    let constrained_closed = session.press_key("escape", "cascader-constrained-close");
    assert_eq!(
        node_by_automation_id(&constrained_closed, "component-qa-cascader-constrained")["state"]
            ["expanded"],
        false
    );
    session.close();
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes TreeSelect evidence"]
fn real_demo_tree_select_clips_text_scrolls_and_selects_by_keyboard_and_pointer() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut session = ComponentQaSession::open(34, "tree-select");
    let snapshot = session.snapshot("tree-select-snapshot");
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-id")["name"],
        "tree-select"
    );

    let target = node_by_automation_id(&snapshot, "component-qa-target");
    assert_eq!(target["visible_bounds"]["w"].as_f64(), Some(200.0));
    assert_eq!(target["visible_bounds"]["h"].as_f64(), Some(32.0));
    assert_eq!(target["role"], "combobox");
    assert!(target["state"]["value_text"].is_null());
    assert_eq!(target["state"]["expanded"], false);
    assert!(target["actions"]
        .as_array()
        .is_some_and(|actions| actions.contains(&json!("focus"))));

    let empty = node_by_automation_id(&snapshot, "component-qa-tree-select-empty");
    assert_eq!(empty["visible_bounds"]["w"].as_f64(), Some(200.0));
    assert_eq!(empty["visible_bounds"]["h"].as_f64(), Some(32.0));
    assert!(empty["state"]["value_text"].is_null());
    assert!(
        node_by_automation_id(&snapshot, "component-qa-tree-select-disabled")["state"]
            ["value_text"]
            .is_null()
    );
    let constrained =
        &node_by_automation_id(&snapshot, "component-qa-tree-select-constrained")["visible_bounds"];
    assert_eq!(constrained["w"].as_f64(), Some(80.0));
    assert_eq!(constrained["h"].as_f64(), Some(12.0));

    session.ensure_foreground();
    let _ = session.focus("component-qa-target", "tree-select-focus");
    let opened = session.press_key("enter", "tree-select-open");
    assert_eq!(
        node_by_automation_id(&opened, "component-qa-target")["state"]["expanded"],
        true
    );
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-tree-select", "tree-select-hierarchy.png");

    let _ = session.press_key("end", "tree-select-end");
    let keyboard_commit = session.press_key("enter", "tree-select-keyboard-commit");
    let target = node_by_automation_id(&keyboard_commit, "component-qa-target");
    assert_eq!(target["state"]["value_text"], "Go");
    assert_eq!(target["state"]["expanded"], false);

    let _ = session.press_key("enter", "tree-select-reopen");
    let pointer_commit = session.click_at_offset(
        "component-qa-target",
        0.5,
        102.0,
        "tree-select-pointer-commit",
    );
    let target = node_by_automation_id(&pointer_commit, "component-qa-target");
    assert_eq!(target["state"]["value_text"], "Vue");
    assert_eq!(target["state"]["expanded"], false);

    let _ = session.focus(
        "component-qa-tree-select-disabled",
        "tree-select-disabled-focus",
    );
    let _ = session.press_key("enter", "tree-select-disabled-open");
    let disabled = session.click_at_offset(
        "component-qa-tree-select-disabled",
        0.5,
        46.0,
        "tree-select-disabled-click",
    );
    let disabled = node_by_automation_id(&disabled, "component-qa-tree-select-disabled");
    assert!(disabled["state"]["value_text"].is_null());
    assert_eq!(disabled["state"]["expanded"], true);
    let _ = session.press_key("escape", "tree-select-disabled-close");

    let _ = session.focus("component-qa-tree-select-empty", "tree-select-empty-focus");
    let empty_open = session.press_key("enter", "tree-select-empty-open");
    assert_eq!(
        node_by_automation_id(&empty_open, "component-qa-tree-select-empty")["state"]["expanded"],
        true
    );
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-tree-select", "tree-select-empty.png");
    let _ = session.press_key("escape", "tree-select-empty-close");

    let _ = session.focus("component-qa-tree-select-long", "tree-select-long-focus");
    let long_open = session.press_key("enter", "tree-select-long-open");
    assert_eq!(
        node_by_automation_id(&long_open, "component-qa-tree-select-long")["state"]["expanded"],
        true
    );
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-tree-select", "tree-select-long.png");
    let _ = session.press_key("escape", "tree-select-long-close");

    let _ = session.focus(
        "component-qa-tree-select-scroll",
        "tree-select-scroll-focus",
    );
    let _ = session.press_key("enter", "tree-select-scroll-open");
    let _ = session.press_key("end", "tree-select-scroll-end");
    let scrolled_commit = session.press_key("enter", "tree-select-scroll-commit");
    let scrolled = node_by_automation_id(&scrolled_commit, "component-qa-tree-select-scroll");
    assert_eq!(scrolled["state"]["value_text"], "节点 39");
    assert_eq!(scrolled["state"]["expanded"], false);
    let selected_reopen = session.press_key("enter", "tree-select-scroll-reopen");
    assert_eq!(
        node_by_automation_id(&selected_reopen, "component-qa-tree-select-scroll")["state"]
            ["expanded"],
        true
    );
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-tree-select", "tree-select-selected-reopen.png");
    let _ = session.press_key("escape", "tree-select-scroll-reopen-close");

    let _ = session.focus(
        "component-qa-tree-select-constrained",
        "tree-select-constrained-focus",
    );
    let constrained_open = session.press_key("enter", "tree-select-constrained-open");
    assert_eq!(
        node_by_automation_id(&constrained_open, "component-qa-tree-select-constrained")["state"]
            ["expanded"],
        true
    );
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-tree-select", "tree-select-constrained-open.png");
    let constrained_closed = session.press_key("escape", "tree-select-constrained-close");
    assert_eq!(
        node_by_automation_id(&constrained_closed, "component-qa-tree-select-constrained")["state"]
            ["expanded"],
        false
    );
    session.close();
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes AutoComplete evidence"]
fn real_demo_autocomplete_edits_filters_scrolls_and_clips_text() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut session = ComponentQaSession::open(35, "autocomplete");
    let snapshot = session.snapshot("autocomplete-snapshot");
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-id")["name"],
        "auto-complete"
    );

    let target = node_by_automation_id(&snapshot, "component-qa-target");
    assert_eq!(target["visible_bounds"]["w"].as_f64(), Some(200.0));
    assert_eq!(target["visible_bounds"]["h"].as_f64(), Some(32.0));
    assert_eq!(target["role"], "combobox");
    assert!(target["state"]["value_text"].is_null());
    assert_eq!(target["state"]["expanded"], false);
    assert!(target["actions"].as_array().is_some_and(
        |actions| actions.contains(&json!("focus")) && actions.contains(&json!("insert_text"))
    ));

    let empty = node_by_automation_id(&snapshot, "component-qa-autocomplete-empty");
    assert_eq!(empty["visible_bounds"]["w"].as_f64(), Some(200.0));
    assert_eq!(empty["visible_bounds"]["h"].as_f64(), Some(32.0));
    assert!(empty["state"]["value_text"].is_null());
    let constrained = &node_by_automation_id(&snapshot, "component-qa-autocomplete-constrained")
        ["visible_bounds"];
    assert_eq!(constrained["w"].as_f64(), Some(80.0));
    assert_eq!(constrained["h"].as_f64(), Some(12.0));

    session.ensure_foreground();
    let _ = session.focus("component-qa-target", "autocomplete-focus");
    let filtered = session.insert_text("component-qa-target", "上", "autocomplete-filter");
    let target = node_by_automation_id(&filtered, "component-qa-target");
    assert_eq!(target["state"]["value_text"], "上");
    assert_eq!(target["state"]["expanded"], true);
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-autocomplete", "autocomplete-filtered.png");

    let committed = session.press_key("enter", "autocomplete-commit");
    let target = node_by_automation_id(&committed, "component-qa-target");
    assert_eq!(target["state"]["value_text"], "上海");
    assert_eq!(target["state"]["expanded"], false);

    let _ = session.focus("component-qa-target", "autocomplete-caret-focus");
    let _ = session.press_key("home", "autocomplete-caret-home");
    let _ = session.press_key("delete", "autocomplete-caret-delete-first");
    let _ = session.press_key("delete", "autocomplete-caret-delete-second");
    let unmatched = session.insert_text("component-qa-target", "新", "autocomplete-caret-insert");
    let target = node_by_automation_id(&unmatched, "component-qa-target");
    assert_eq!(target["state"]["value_text"], "新");
    assert_eq!(target["state"]["expanded"], true);
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-autocomplete", "autocomplete-no-data-caret.png");
    let _ = session.press_key("escape", "autocomplete-no-data-close");

    let _ = session.focus(
        "component-qa-autocomplete-empty",
        "autocomplete-empty-focus",
    );
    let empty_open = session.press_key("down", "autocomplete-empty-open");
    assert_eq!(
        node_by_automation_id(&empty_open, "component-qa-autocomplete-empty")["state"]["expanded"],
        true
    );
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-autocomplete", "autocomplete-empty.png");
    let _ = session.press_key("escape", "autocomplete-empty-close");

    let _ = session.focus("component-qa-autocomplete-long", "autocomplete-long-focus");
    let _ = session.press_key("down", "autocomplete-long-open");
    let _ = session.move_pointer_to_offset(
        "component-qa-autocomplete-long",
        0.5,
        74.0,
        "autocomplete-long-hover",
    );
    session.capture("uix-autocomplete", "autocomplete-long-hover.png");
    let _ = session.press_key("escape", "autocomplete-long-close");

    let _ = session.focus(
        "component-qa-autocomplete-scroll",
        "autocomplete-scroll-focus",
    );
    let _ = session.press_key("down", "autocomplete-scroll-open");
    for index in 0..20 {
        let _ = session.press_key("down", &format!("autocomplete-scroll-down-{index}"));
    }
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-autocomplete", "autocomplete-scrolled.png");
    let scrolled_commit = session.press_key("enter", "autocomplete-scroll-commit");
    let scrolled = node_by_automation_id(&scrolled_commit, "component-qa-autocomplete-scroll");
    assert_eq!(scrolled["state"]["value_text"], "候选 20");
    assert_eq!(scrolled["state"]["expanded"], false);

    let _ = session.focus(
        "component-qa-autocomplete-constrained",
        "autocomplete-constrained-focus",
    );
    let constrained_open = session.press_key("down", "autocomplete-constrained-open");
    assert_eq!(
        node_by_automation_id(&constrained_open, "component-qa-autocomplete-constrained")["state"]
            ["expanded"],
        true
    );
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-autocomplete", "autocomplete-constrained-open.png");
    let constrained_closed = session.press_key("escape", "autocomplete-constrained-close");
    assert_eq!(
        node_by_automation_id(&constrained_closed, "component-qa-autocomplete-constrained")
            ["state"]["expanded"],
        false
    );
    session.close();
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes Mentions evidence"]
fn real_demo_mentions_edits_query_at_caret_scrolls_and_clips_text() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut session = ComponentQaSession::open(36, "mentions");
    let snapshot = session.snapshot("mentions-snapshot");
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-id")["name"],
        "mentions"
    );

    let target = node_by_automation_id(&snapshot, "component-qa-target");
    assert_eq!(target["visible_bounds"]["w"].as_f64(), Some(200.0));
    assert_eq!(target["visible_bounds"]["h"].as_f64(), Some(32.0));
    assert_eq!(target["role"], "combobox");
    assert!(target["state"]["value_text"].is_null());
    assert_eq!(target["state"]["expanded"], false);
    assert!(target["actions"].as_array().is_some_and(
        |actions| actions.contains(&json!("focus")) && actions.contains(&json!("insert_text"))
    ));

    let empty = node_by_automation_id(&snapshot, "component-qa-mentions-empty");
    assert_eq!(empty["visible_bounds"]["w"].as_f64(), Some(200.0));
    assert_eq!(empty["visible_bounds"]["h"].as_f64(), Some(32.0));
    let constrained =
        &node_by_automation_id(&snapshot, "component-qa-mentions-constrained")["visible_bounds"];
    assert_eq!(constrained["w"].as_f64(), Some(80.0));
    assert_eq!(constrained["h"].as_f64(), Some(12.0));

    session.ensure_foreground();
    let _ = session.focus("component-qa-target", "mentions-focus");
    let with_suffix = session.insert_text(
        "component-qa-target",
        "你好 @Al 后文",
        "mentions-insert-with-suffix",
    );
    let target = node_by_automation_id(&with_suffix, "component-qa-target");
    assert_eq!(target["state"]["value_text"], "你好 @Al 后文");
    assert_eq!(target["state"]["expanded"], false);

    let mut query_at_caret = with_suffix;
    for index in 0..3 {
        query_at_caret = session.press_key("left", &format!("mentions-caret-left-{index}"));
    }
    let target = node_by_automation_id(&query_at_caret, "component-qa-target");
    assert_eq!(target["state"]["value_text"], "你好 @Al 后文");
    assert_eq!(target["state"]["expanded"], true);
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-mentions", "mentions-query-at-caret.png");

    let committed = session.press_key("enter", "mentions-commit-at-caret");
    let target = node_by_automation_id(&committed, "component-qa-target");
    assert_eq!(target["state"]["value_text"], "你好 @Alan 后文");
    assert_eq!(target["state"]["expanded"], false);
    let suffix = session.insert_text("component-qa-target", "X", "mentions-insert-before-suffix");
    assert_eq!(
        node_by_automation_id(&suffix, "component-qa-target")["state"]["value_text"],
        "你好 @Alan X后文"
    );
    session.capture("uix-mentions", "mentions-suffix-preserved.png");

    let long_tail = "这是用于验证单行文字水平滚动与实测光标位置的超长内容";
    let horizontal = session.insert_text(
        "component-qa-target",
        long_tail,
        "mentions-horizontal-scroll",
    );
    assert_eq!(
        node_by_automation_id(&horizontal, "component-qa-target")["state"]["value_text"],
        format!("你好 @Alan X{long_tail}后文")
    );
    session.capture("uix-mentions", "mentions-horizontal-scroll.png");

    let _ = session.focus("component-qa-mentions-empty", "mentions-empty-focus");
    let empty_open = session.insert_text("component-qa-mentions-empty", "@", "mentions-empty-open");
    assert_eq!(
        node_by_automation_id(&empty_open, "component-qa-mentions-empty")["state"]["expanded"],
        true
    );
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-mentions", "mentions-empty.png");
    let _ = session.press_key("escape", "mentions-empty-close");

    let _ = session.focus("component-qa-mentions-long", "mentions-long-focus");
    let long_open = session.insert_text("component-qa-mentions-long", "@", "mentions-long-open");
    assert_eq!(
        node_by_automation_id(&long_open, "component-qa-mentions-long")["state"]["expanded"],
        true
    );
    let _ = session.move_pointer_to_offset(
        "component-qa-mentions-long",
        0.5,
        74.0,
        "mentions-long-hover",
    );
    session.capture("uix-mentions", "mentions-long-hover.png");
    let _ = session.press_key("escape", "mentions-long-close");

    let _ = session.focus("component-qa-mentions-scroll", "mentions-scroll-focus");
    let _ = session.insert_text("component-qa-mentions-scroll", "@", "mentions-scroll-open");
    for index in 0..20 {
        let _ = session.press_key("down", &format!("mentions-scroll-down-{index}"));
    }
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-mentions", "mentions-scrolled.png");
    let scrolled_commit = session.press_key("enter", "mentions-scroll-commit");
    let scrolled = node_by_automation_id(&scrolled_commit, "component-qa-mentions-scroll");
    assert_eq!(scrolled["state"]["value_text"], "@候选 20 ");
    assert_eq!(scrolled["state"]["expanded"], false);

    let _ = session.focus(
        "component-qa-mentions-constrained",
        "mentions-constrained-focus",
    );
    let constrained_open = session.insert_text(
        "component-qa-mentions-constrained",
        "@",
        "mentions-constrained-open",
    );
    assert_eq!(
        node_by_automation_id(&constrained_open, "component-qa-mentions-constrained")["state"]
            ["expanded"],
        true
    );
    thread::sleep(Duration::from_millis(300));
    session.capture("uix-mentions", "mentions-constrained-open.png");
    let constrained_closed = session.press_key("escape", "mentions-constrained-close");
    assert_eq!(
        node_by_automation_id(&constrained_closed, "component-qa-mentions-constrained")["state"]
            ["expanded"],
        false
    );
    session.close();
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes Badge CJK evidence"]
fn real_demo_badge_matches_cjk_text_and_overflow_label_widths() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut session = ComponentQaSession::open(41, "badge-cjk");
    let snapshot = session.snapshot("badge-cjk-snapshot");
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-id")["name"],
        "badge"
    );
    let target = &node_by_automation_id(&snapshot, "component-qa-target")["visible_bounds"];
    assert_eq!(target["w"].as_f64(), Some(45.0));
    assert_eq!(target["h"].as_f64(), Some(20.0));
    let overflow =
        &node_by_automation_id(&snapshot, "component-qa-badge-overflow")["visible_bounds"];
    assert!(
        overflow["w"]
            .as_f64()
            .is_some_and(|width| (width - 30.15).abs() < 0.01),
        "overflow badge must reserve the rendered 99+ label: {overflow}"
    );
    session.capture("uix-badge-cjk", "badge-cjk-light-desktop.png");
    session.close();
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes Tooltip CJK evidence"]
fn real_demo_tooltip_uses_compact_cjk_bubble_width() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut session = ComponentQaSession::open(67, "tooltip-cjk");
    let snapshot = session.move_pointer_to("component-qa-target", "tooltip-cjk-hover");
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-id")["name"],
        "tooltip"
    );
    let target = &node_by_automation_id(&snapshot, "component-qa-target")["visible_bounds"];
    let center_x =
        target["x"].as_f64().expect("target x") + target["w"].as_f64().expect("target width") * 0.5;
    let target_capture_y = target["y"].as_f64().expect("target y");

    let path = session.capture("uix-tooltip-cjk", "tooltip-cjk-light-desktop.png");
    let image = image::open(&path)
        .expect("open tooltip evidence")
        .to_rgba8();
    let min_x = (center_x - 100.0).max(0.0).floor() as u32;
    let max_x = (center_x + 100.0).min(f64::from(image.width())).ceil() as u32;
    let min_y = (target_capture_y - 48.0).max(0.0).floor() as u32;
    let max_y = target_capture_y.min(f64::from(image.height())).ceil() as u32;
    let (bubble_row, _, bubble_width) = (min_y..max_y)
        .filter_map(|y| {
            let dark_x = (min_x..max_x)
                .filter(|x| {
                    let pixel = image.get_pixel(*x, y);
                    pixel[0] < 210 && pixel[1] < 210 && pixel[2] < 210
                })
                .collect::<Vec<_>>();
            dark_x
                .first()
                .zip(dark_x.last())
                .map(|(min, max)| (y, dark_x.len(), max - min + 1))
        })
        .max_by_key(|(_, count, _)| *count)
        .expect("visible tooltip bubble pixels");
    assert!(
        (60..=68).contains(&bubble_width),
        "four-CJK tooltip bubble should be about 64 physical pixels, got {bubble_width}; row={bubble_row}, crop_y={min_y}..{max_y}, target={target}"
    );
    session.close();
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes Tag CJK evidence"]
fn real_demo_tag_uses_compact_cjk_width_with_action_regions() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut session = ComponentQaSession::open(54, "tag-cjk");
    let snapshot = session.snapshot("tag-cjk-snapshot");
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-id")["name"],
        "tag"
    );
    let target = &node_by_automation_id(&snapshot, "component-qa-target")["visible_bounds"];
    assert_eq!(target["w"].as_f64(), Some(72.0));
    assert_eq!(target["h"].as_f64(), Some(20.0));
    session.capture("uix-tag-cjk", "tag-cjk-light-desktop.png");
    session.close();
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes Label multiline evidence"]
fn real_demo_label_multiline_keeps_following_content_below_all_lines() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut session = ComponentQaSession::open(2, "label-multiline");
    let snapshot = session.snapshot("label-multiline-snapshot");
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-id")["name"],
        "label"
    );
    assert_following_content_below(
        &snapshot,
        "component-qa-target",
        "component-qa-label-after",
        54.0,
    );
    session.capture("uix-label-multiline", "label-multiline-light-desktop.png");
    session.close();
}

#[test]
#[ignore = "requires an interactive Windows desktop and writes Typography wrap evidence"]
fn real_demo_typography_wrap_keeps_following_content_below_the_paragraph() {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut session = ComponentQaSession::open(3, "typography-wrap");
    let snapshot = session.snapshot("typography-wrap-snapshot");
    assert_eq!(
        node_by_automation_id(&snapshot, "component-qa-id")["name"],
        "typography"
    );
    let paragraph = &node_by_automation_id(&snapshot, "component-qa-target")["visible_bounds"];
    assert!(
        paragraph["w"].as_f64().is_some_and(|width| width <= 112.1),
        "Typography QA target must keep the narrow width: {paragraph}"
    );
    assert_following_content_below(
        &snapshot,
        "component-qa-target",
        "component-qa-typography-after",
        63.0,
    );
    session.capture("uix-typography-wrap", "typography-wrap-light-desktop.png");
    session.close();
}
