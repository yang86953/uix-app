use super::support::*;
use std::fs;
use std::path::Path;

#[test]
fn low_level_widget_loop_stays_off_prelude() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let prelude = fs::read_to_string(src.join("prelude.rs")).unwrap();

    assert!(
        !prelude.contains("run_widget_loop"),
        "prelude should route apps through App/View APIs, not the low-level WidgetTree loop"
    );
}

#[test]
fn draw_gpu_engine_has_no_legacy_shader_reexports() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let gpu_engine = fs::read_to_string(src.join("draw/gpu_engine/mod.rs")).unwrap();
    let shader_names = [
        "BLIT_FRAG",
        "BLIT_RGBA_FRAG",
        "BLUR_FRAG",
        "FULLSCREEN_VERT",
        "RECT_FRAG",
        "RECT_VERT",
    ];

    assert!(
        shader_names.iter().all(|name| !gpu_engine.contains(name)),
        "native shader sources must not be re-exported through draw::gpu_engine"
    );
}

#[test]
fn internal_widget_tree_types_stay_off_user_entrypoints() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let checked = ["prelude.rs"];
    let internal_types = [
        "WidgetTree",
        "WidgetNode",
        "BoxedWidget",
        "ViewAdapter",
        "WidgetManagers",
        "OverlayStack",
    ];
    let mut violations = Vec::new();

    for rel in checked {
        let text = fs::read_to_string(src.join(rel)).unwrap();
        for (idx, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if !(trimmed.starts_with("pub use") || trimmed.contains("assert_exported::<")) {
                continue;
            }
            for symbol in internal_types {
                if trimmed
                    .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                    .any(|token| token == symbol)
                {
                    violations.push(format!("{rel}:{} exposes {symbol}", idx + 1));
                }
            }
        }
    }

    assert!(
        violations.is_empty(),
        "internal runtime types must stay off user entrypoints: {violations:?}"
    );

    let ui_mod = fs::read_to_string(src.join("ui/mod.rs")).unwrap();
    let view_mod = fs::read_to_string(src.join("ui/view/mod.rs")).unwrap();
    assert!(
        ui_mod.contains("pub(crate) mod core;") && !ui_mod.contains("pub mod core;"),
        "ui::core must remain an internal runtime module"
    );
    assert!(
        view_mod.contains("pub(crate) mod adapter;") && !view_mod.contains("pub mod adapter;"),
        "ViewAdapter must remain an internal runtime module"
    );
}

#[test]
fn semantic_actions_stay_on_the_shared_widget_event_path() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let executor = read_source(src.join("ui/semantic_action.rs"));
    let automation = read_source(src.join("ui/automation.rs"));
    let snapshots = read_source(src.join("ui/component_snapshot/mod.rs"));

    assert!(
        executor.contains("self.dispatch_event(") && executor.contains("self.dispatch_semantic("),
        "semantic actions must reuse WidgetTree system/semantic event dispatch"
    );
    assert!(
        !executor.contains(".component_mut()") && !executor.contains("downcast_mut::<"),
        "semantic actions must not mutate component instances behind the normal event path"
    );
    assert!(
        automation.contains(".perform_semantic_action(node.id, &action)"),
        "TestApp must stay an adapter over the shared semantic action executor"
    );
    assert!(
        executor.contains(".and_then(|snapshot| snapshot.selection())")
            && executor.contains("!selection.multiple")
            && executor.contains("self.press_key(KeyCode::Down)")
            && snapshots.contains("pub struct SelectionSnapshot")
            && snapshots.contains("pub selected_indices: Vec<usize>")
            && snapshots.contains("pub disabled_indices: Vec<usize>"),
        "select must be declared from shared single-choice metadata and execute through keyboard events"
    );
}

#[test]
fn agent_semantics_share_one_snapshot_and_successful_present_boundary() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = root.join("src");
    let cargo = read_source(root.join("Cargo.toml"));
    let semantic_snapshot = read_source(src.join("ui/semantic_snapshot.rs"));
    let automation = read_source(src.join("ui/automation.rs"));
    let window_semantics = read_source(src.join("app/window_semantics.rs"));
    let window_driver = read_source(src.join("app/window_driver.rs"));

    assert!(
        cargo.contains("agent-control = [") && cargo.contains("\"dep:serde_json\","),
        "the opt-in Agent Bridge must keep an explicit Cargo feature gate"
    );
    assert!(
        automation.contains("nodes: self.semantic_snapshot_body().nodes")
            && !automation.contains("ComponentConfigSnapshot::from_component"),
        "TestApp and recorders must reuse the transport-neutral semantic snapshot"
    );
    assert!(
        !semantic_snapshot.contains("crate::app"),
        "the UI semantic model must not depend on app/session transport state"
    );
    assert!(
        window_semantics.contains("if !self.enabled || self.snapshot.closed")
            && window_semantics.contains("tree.semantic_snapshot_body()"),
        "disabled semantic tracking must return before traversing the widget tree"
    );
    assert!(
        window_driver
            .contains("if frame_committed {\n            semantic_state.mark_presented();"),
        "presented_revision may advance only inside the successful frame commit boundary"
    );
}

#[test]
fn agent_commands_stay_bounded_targeted_and_ui_thread_owned() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let commands = read_source(src.join("app/agent_control.rs"));
    let runtime = read_source(src.join("app/session_runtime.rs"));
    let session = read_source(src.join("app/window_session.rs"));
    let driver = read_source(src.join("app/window_driver.rs"));
    let protocol = read_source(src.join("app/agent_protocol.rs"));

    assert!(
        commands.contains("DEFAULT_AGENT_COMMAND_QUEUE_CAPACITY: usize = 64")
            && commands.contains("MAX_AGENT_SETTLE_PASSES: usize = 32")
            && commands.contains("if inner.pending.len() >= inner.capacity"),
        "Agent ingress and synchronous settle must retain explicit hard bounds"
    );
    assert!(
        runtime.contains("session.agent_commands.submit(request)?")
            && runtime.contains("if should_wake {")
            && runtime.contains("self.wake_event_loop();"),
        "AppRuntime must wake only after the target window queue accepts its first command"
    );
    assert!(
        session.contains("agent_commands: WindowAgentState")
            && session.contains("self.agent_commands.close();"),
        "WindowSession must own and terminate its UI-side command state"
    );
    assert!(
        !runtime.contains("WidgetTree")
            && commands.contains("tree.perform_semantic_action(node_id, action)"),
        "transport/runtime ingress must not touch WidgetTree; only UI-side command drain may act"
    );
    assert!(
        commands.contains("AgentWindowAction::PressKey")
            && commands.contains("tree.dispatch_event(&SystemEvent::KeyDown")
            && commands.contains("AgentWindowAction::ClickAt")
            && commands.contains("tree.dispatch_event(&SystemEvent::PointerDown")
            && protocol
                .contains("const AGENT_WINDOW_ACTIONS: &[&str] = &[\"press_key\", \"click_at\"]")
            && protocol.contains("window action must not include a target"),
        "the two window-level fallbacks must share normal UI events and remain target-free"
    );

    let main_queue = driver
        .find("main_thread_queue.drain(&mut main_thread_context)")
        .expect("main-thread queue drain");
    let agent_queue = driver
        .find("agent_commands.drain_ready(")
        .expect("Agent command drain");
    let app_state = driver
        .find("tree.drain_app_state_semantic_events()")
        .expect("AppState semantic drain");
    let settle = driver
        .find("observe_agent_settle(")
        .expect("bounded settle observation");
    let present = driver.find("match outcome {").expect("present boundary");
    assert!(
        main_queue < agent_queue && agent_queue < app_state,
        "commands must drain in the target UI turn between main-thread work and AppState effects"
    );
    assert!(
        settle < present,
        "settled responses must not wait for paint or present"
    );
}

#[test]
fn agent_bridge_is_explicit_process_scoped_and_transport_neutral() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let application = read_source(src.join("app/shell/application.rs"));
    let bridge = read_source(src.join("app/agent_bridge.rs"));
    let runtime = read_source(src.join("app/session_runtime.rs"));
    let semantics = read_source(src.join("app/window_semantics.rs"));

    assert!(
        application
            .contains("#[cfg(feature = \"agent-control\")]\n    pub fn enable_agent_control")
            && application.contains("if self.agent_control_enabled {")
            && application.contains("self.runtime.enable_agent_control()"),
        "Agent control must require both its Cargo feature and an explicit App runtime switch"
    );
    assert!(
        bridge.contains("pub(crate) struct AgentProcessBridge")
            && bridge.contains("pub(crate) fn list_windows")
            && bridge.contains("self.submit_for_live_window(")
            && bridge.contains("contains_live_agent_window(window_id)"),
        "one process Bridge must list live windows and route commands only to registered targets"
    );
    assert!(
        !bridge.contains("use crate::ui::WidgetTree")
            && !bridge.contains("&mut WidgetTree")
            && !bridge.contains("native::backends")
            && !bridge.contains("TcpListener")
            && !bridge.contains("UdpSocket"),
        "the process adapter must remain transport-neutral and unable to touch widget trees"
    );
    assert!(
        bridge.contains("MAX_AGENT_WAIT_TIMEOUT: Duration = Duration::from_secs(30)")
            && bridge.contains("waiters: BTreeMap<WindowId, Arc<Condvar>>")
            && bridge.contains(".wait_timeout(state, remaining)")
            && bridge.contains("state.waiters.get(&window_id).cloned()")
            && bridge.contains("waiter.notify_all()")
            && bridge.contains("AgentWaitCondition::PresentedAtLeast"),
        "wait must be bounded and block only target-window Bridge workers on directory notifications"
    );
    assert!(
        !bridge.contains("wake_event_loop")
            && !bridge.contains("post_to_ui")
            && !bridge.contains("run_interval")
            && !bridge.contains("sleep("),
        "wait must not poll, inject UI work, or wake the native event loop"
    );
    assert!(
        runtime.contains("agent_bridge: AgentBridgeDirectory")
            && runtime.contains("self.agent_bridge.close_window(window_id)")
            && runtime.contains("self.agent_bridge.close_all()"),
        "Bridge window and app lifecycle must be owned by AppRuntime"
    );
    assert!(
        semantics.contains("agent_window: Option<AgentWindowRegistration>")
            && semantics.contains("agent_window.publish_semantics(&self.snapshot)"),
        "only the UI-owned semantic state may publish revision metadata to discovery"
    );
}

#[test]
fn widget_id_stays_off_public_widget_boundary_signatures() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let checked = [
        "ui/core/widget/mod.rs",
        "ui/core/widget/tree_core.rs",
        "ui/core/widget/tree_dirty.rs",
        "ui/core/widget/tree_events.rs",
        "ui/core/widget/tree_layout.rs",
    ];
    let mut violations = Vec::new();

    for rel in checked {
        let text = fs::read_to_string(src.join(rel)).unwrap();
        if text.contains("pub type WidgetId") {
            violations.push(format!("{rel} exposes WidgetId as a public alias"));
        }
        let mut in_public_fn = false;
        let mut in_widget_core = false;
        for (idx, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("pub trait WidgetCore") {
                in_widget_core = true;
            }
            if trimmed.starts_with("pub fn ") {
                in_public_fn = true;
            }

            if (in_public_fn || in_widget_core) && trimmed.contains("WidgetId") {
                violations.push(format!("{rel}:{} exposes WidgetId in {trimmed}", idx + 1));
            }

            if in_public_fn && trimmed.contains('{') {
                in_public_fn = false;
            }
            if in_widget_core && trimmed == "}" {
                in_widget_core = false;
            }
        }
    }

    assert!(
        violations.is_empty(),
        "public widget boundaries use ComponentId; WidgetId stays an internal WidgetTree alias: {violations:?}"
    );
}

#[test]
fn widget_core_public_boundary_uses_invalidation_not_dirty_bit() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let text = fs::read_to_string(src.join("ui/core/widget/mod.rs")).unwrap();
    let mut in_widget_core = false;
    let mut violations = Vec::new();

    for (idx, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("pub trait WidgetCore") {
            in_widget_core = true;
        }
        if in_widget_core
            && (trimmed.starts_with("fn dirty(") || trimmed.starts_with("fn set_dirty("))
        {
            violations.push(format!("ui/core/widget/mod.rs:{} has {trimmed}", idx + 1));
        }
        if in_widget_core && trimmed == "}" {
            in_widget_core = false;
        }
    }

    assert!(
        violations.is_empty(),
        "WidgetCore must expose invalidation/paint behavior through WidgetTree, not a dirty bit: {violations:?}"
    );
}

#[test]
fn state_public_boundary_uses_reconcile_invalidation_not_dirty_callback() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let text = fs::read_to_string(src.join("ui/foundation/state.rs")).unwrap();
    let forbidden = [
        "set_dirty_fn",
        "set_current_view_dirty_fn",
        "clear_current_view_dirty_fn",
        "CURRENT_VIEW_DIRTY_FN",
    ];
    let violations: Vec<_> = forbidden
        .into_iter()
        .filter(|term| text.contains(term))
        .collect();

    assert!(
        violations.is_empty(),
        "State must expose reconcile invalidation naming instead of dirty callbacks: {violations:?}"
    );
}

#[test]
fn demo_default_entrypoint_stays_on_prelude_app_path() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let main = fs::read_to_string(root.join("demo/src/main.rs")).unwrap();

    assert!(
        main.contains("gui::run(options.agent_control)")
            && main.contains("--cli")
            && main.contains("--agent-control"),
        "demo main should keep GUI as the default and expose only explicit CLI/agent-control modes"
    );
    assert!(
        !main.contains("--dashboard"),
        "demo should not expose legacy --dashboard alias"
    );
}

#[test]
fn component_patch_downcast_path_has_no_hard_assertions() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let text = fs::read_to_string(src.join("ui/component_patch.rs")).unwrap();
    let forbidden = [".expect(", ".unwrap("];
    let violations: Vec<_> = forbidden
        .into_iter()
        .filter(|term| text.contains(term))
        .collect();

    assert!(
        violations.is_empty(),
        "component patching is a hot reconcile path and must not hard-assert downcasts: {violations:?}"
    );
}

#[test]
fn business_event_callbacks_stay_out_of_widget_fields() {
    let widgets = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/ui/widgets");
    let mut violations = Vec::new();

    for file in rust_files_under(&widgets) {
        let rel = relative_src_path(&file);
        let text = fs::read_to_string(&file).unwrap();
        for (idx, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("on_") && trimmed.contains(':') && !trimmed.contains("=>") {
                violations.push(format!("{rel}:{} contains {trimmed}", idx + 1));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "business event handlers belong in HandlerTable/View bindings, not widget struct fields: {violations:?}"
    );
}

#[test]
fn authored_widget_callbacks_respect_component_ownership_boundaries() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let form = fs::read_to_string(root.join("src/ui/widgets/input/form.rs")).unwrap();
    let form_validation =
        fs::read_to_string(root.join("src/ui/widgets/input/form_validation.rs")).unwrap();
    let table = fs::read_to_string(root.join("src/ui/widgets/display/table.rs")).unwrap();
    let virtual_scroll =
        fs::read_to_string(root.join("src/ui/foundation/virtual_scroll.rs")).unwrap();
    let render_handlers = fs::read_to_string(root.join("src/ui/render_handler.rs")).unwrap();

    let form_component = form
        .split("pub struct Form {")
        .nth(1)
        .and_then(|tail| tail.split("measure =>").next())
        .expect("Form component declaration");
    let table_component = table
        .split("pub struct Table {")
        .nth(1)
        .and_then(|tail| tail.split("measure =>").next())
        .expect("Table component declaration");
    let virtual_scroll_component = virtual_scroll
        .split("pub struct VirtualScroll {")
        .nth(1)
        .and_then(|tail| tail.split("measure =>").next())
        .expect("VirtualScroll component declaration");

    assert!(
        !form_component.contains("Box<dyn Fn") && !form_component.contains("validator:"),
        "Form component data must not own custom validator closures"
    );
    assert!(
        form_validation.contains("pub struct FormModel")
            && form_validation.contains("type CustomValidator")
            && form_validation.contains("Custom(CustomValidator)")
            && !form.contains("FormValidatorTable")
            && !form.contains("FormValidatorKey"),
        "inline Form validators must stay in the application-owned FormModel"
    );
    assert!(
        !table_component.contains("expand_renderer"),
        "Table component data must not own expand renderer closures"
    );
    assert!(
        render_handlers.contains("HashMap<ComponentId, ExpandRenderer>")
            && render_handlers.contains("render_table_expand_view")
            && render_handlers.contains("clear_component"),
        "table expand View factories must be keyed by ComponentId and cleared with node lifecycle"
    );
    assert!(
        !virtual_scroll_component.contains("Box<dyn Fn")
            && !virtual_scroll_component.contains("renderer:"),
        "VirtualScroll component data must not own item renderer closures"
    );
    assert!(
        render_handlers.contains("HashMap<ComponentId, VirtualScrollRenderer>"),
        "virtual scroll item renderers must be keyed by ComponentId"
    );
}

#[test]
fn architecture_document_stays_in_the_qualified_docs_tree() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let architecture_doc = root.join("docs/架构.md");
    let domain_dir = root.join("docs/领域");
    let forbidden = ["ARCHITECTURE.md", "architecture.md"];
    let present: Vec<_> = forbidden
        .iter()
        .copied()
        .filter(|name| root.join(name).exists())
        .collect();

    let required_generic = ["产品.md", "架构.md", "进度.md", "使用.md"];
    for name in required_generic {
        let path = root.join("docs").join(name);
        assert!(
            path.is_file(),
            "generic docs skeleton missing: {}",
            path.display()
        );
    }

    assert!(
        architecture_doc.is_file(),
        "qualified architecture navigation is missing: {}",
        architecture_doc.display()
    );
    assert!(
        !domain_dir.exists(),
        "docs/领域/ must not exist; project depth lives in 产品/架构/进度/使用: {}",
        domain_dir.display()
    );

    let forbidden_root_domain = [
        "按需驱动.md",
        "公开API.md",
        "ui.md",
        "界面.md",
        "运行时.md",
        "渲染.md",
        "缺陷.md",
    ];
    let leaked: Vec<_> = forbidden_root_domain
        .iter()
        .copied()
        .filter(|name| root.join("docs").join(name).exists())
        .collect();
    assert!(
        leaked.is_empty(),
        "removed domain encyclopedias must not reappear under docs/: {leaked:?}"
    );

    assert!(
        root.join("AGENTS.md").is_file(),
        "AGENTS.md AI guidance contract is missing"
    );

    assert!(
        present.is_empty(),
        "docs/架构.md is the project architecture navigation; do not add standalone root architecture files: {present:?}"
    );
}

#[test]
fn production_modules_do_not_embed_or_mount_tests() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut inline_body = Vec::new();
    let mut path_mount = Vec::new();
    let mut mod_tests = Vec::new();
    let mut bare_test_attr = Vec::new();

    for path in rust_files_under(&src) {
        let rel = relative_src_path(&path);
        if rel.starts_with("tests/") || rel == "lib.rs" {
            continue;
        }
        let text = read_source(&path);

        if text.contains("#[path") && text.contains("tests/") {
            path_mount.push(rel.clone());
        }
        if text.contains("mod tests;") {
            mod_tests.push(rel.clone());
        }
        if text.contains("#[test]") {
            bare_test_attr.push(rel.clone());
        }

        let mut search = text.as_str();
        let mut offset = 0usize;
        while let Some(idx) = search.find("mod ") {
            let abs = offset + idx;
            let after = &text[abs..];
            let Some(rest) = after.strip_prefix("mod ") else {
                break;
            };
            let name_end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(rest.len());
            let name = &rest[..name_end];
            let after_name = rest[name_end..].trim_start();
            if after_name.starts_with('{') {
                let body_start = abs + after.find('{').expect("brace");
                let mut depth = 0i32;
                let mut i = body_start;
                let bytes = text.as_bytes();
                while i < bytes.len() {
                    match bytes[i] as char {
                        '{' => depth += 1,
                        '}' => {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                        }
                        _ => {}
                    }
                    i += 1;
                }
                let body = &text[body_start..=i.min(text.len().saturating_sub(1))];
                if body.contains("#[test]") {
                    inline_body.push(format!("{rel}::{name}"));
                }
            }
            offset = abs + 4;
            search = &text[offset..];
        }
    }

    assert!(
        inline_body.is_empty(),
        "inline test bodies must live under src/tests: {inline_body:?}"
    );
    assert!(
        path_mount.is_empty(),
        "production modules must not #[path]-mount tests: {path_mount:?}"
    );
    assert!(
        mod_tests.is_empty(),
        "production modules must not declare mod tests: {mod_tests:?}"
    );
    assert!(
        bare_test_attr.is_empty(),
        "production modules must not contain #[test]: {bare_test_attr:?}"
    );
}
