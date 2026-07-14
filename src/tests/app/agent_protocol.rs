use std::cell::Cell;
use std::fs;
use std::io::{BufRead, BufReader, Cursor, Write};
use std::rc::Rc;
use std::sync::mpsc::sync_channel;
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};

use crate::app::agent_bridge::MAX_AGENT_WAIT_TIMEOUT;
use crate::app::agent_control::{DEFAULT_AGENT_COMMAND_QUEUE_CAPACITY, MAX_AGENT_SETTLE_PASSES};
use crate::app::agent_protocol::{
    encode_session_token, AgentProtocolReply, AgentProtocolSession, AGENT_PROTOCOL_SCHEMA,
    MAX_AGENT_CONNECTIONS, MAX_AGENT_MESSAGE_BYTES, MAX_AGENT_TEXT_BYTES,
};
use crate::app::agent_transport::{read_bounded_line, BoundedLine};
use crate::app::app_timer::AppTimerQueue;
use crate::app::main_thread_queue::MainThreadQueue;
use crate::app::session_runtime::AppRuntime;
use crate::app::window_session::WindowSession;
use crate::core::WindowId;
use crate::native::agent_transport::connect_for_test;
use crate::native::traits::event::EventLoopWaker;
use crate::tests::common::NullEngine;
use crate::ui::view::combinators::button;
use crate::ui::view::StyleExt;

fn register_runtime_session(runtime: &AppRuntime, window_id: WindowId) {
    runtime.register_session(
        window_id,
        AppTimerQueue::new(),
        MainThreadQueue::new(),
        std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true)),
    );
}

fn reply_json(reply: &AgentProtocolReply) -> Value {
    serde_json::from_slice(reply.bytes()).expect("valid JSON protocol reply")
}

fn request(value: Value) -> Vec<u8> {
    serde_json::to_vec(&value).expect("serializable request")
}

fn authenticate(protocol: &mut AgentProtocolSession, token: &[u8; 32]) {
    let reply = protocol.handle_line(&request(json!({
        "schema": AGENT_PROTOCOL_SCHEMA,
        "request_id": "hello",
        "type": "hello",
        "token": encode_session_token(token),
    })));
    let value = reply_json(&reply);
    assert_eq!(value["ok"], true);
    assert!(!reply.close_connection());
}

#[test]
fn protocol_requires_first_message_auth_and_returns_stable_typed_errors() {
    let runtime = AppRuntime::new();
    assert!(runtime.enable_agent_control());
    let bridge = runtime.agent_bridge().expect("enabled bridge");
    let token = std::sync::Arc::new([0x35; 32]);

    let mut unauthenticated = AgentProtocolSession::new(bridge.clone(), token.clone());
    let reply = unauthenticated.handle_line(&request(json!({
        "schema": AGENT_PROTOCOL_SCHEMA,
        "request_id": "before-hello",
        "type": "list_windows",
    })));
    assert!(reply.close_connection());
    assert_eq!(reply_json(&reply)["error"]["code"], "unauthorized");

    let mut wrong_token = AgentProtocolSession::new(bridge.clone(), token.clone());
    let reply = wrong_token.handle_line(&request(json!({
        "schema": AGENT_PROTOCOL_SCHEMA,
        "request_id": "wrong-token",
        "type": "hello",
        "token": "00".repeat(32),
    })));
    assert!(reply.close_connection());
    let body = String::from_utf8(reply.bytes().to_vec()).unwrap();
    assert!(body.contains("unauthorized"));
    assert!(!body.contains(&encode_session_token(token.as_ref())));

    let mut protocol = AgentProtocolSession::new(bridge, token.clone());
    let reply = protocol.handle_line(&request(json!({
        "schema": AGENT_PROTOCOL_SCHEMA,
        "request_id": "uppercase-token",
        "type": "hello",
        "token": encode_session_token(token.as_ref()).to_uppercase(),
    })));
    let value = reply_json(&reply);
    assert_eq!(value["ok"], true);
    assert_eq!(
        value["capabilities"]["window_actions"],
        json!(["press_key", "click_at"])
    );
    assert!(value["capabilities"]["key_names"]
        .as_array()
        .is_some_and(|keys| keys.contains(&json!("enter")) && keys.contains(&json!("f12"))));
    assert_eq!(
        value["capabilities"]["key_modifiers"],
        json!(["shift", "ctrl", "alt", "super"])
    );
    assert_eq!(
        value["limits"]["max_message_bytes"],
        MAX_AGENT_MESSAGE_BYTES
    );
    assert_eq!(value["limits"]["max_text_bytes"], MAX_AGENT_TEXT_BYTES);
    assert_eq!(value["limits"]["max_connections"], MAX_AGENT_CONNECTIONS);
    assert_eq!(
        value["limits"]["window_queue_capacity"],
        DEFAULT_AGENT_COMMAND_QUEUE_CAPACITY
    );
    assert_eq!(
        value["limits"]["max_settle_passes"],
        MAX_AGENT_SETTLE_PASSES
    );
    assert_eq!(
        value["limits"]["max_wait_ms"],
        MAX_AGENT_WAIT_TIMEOUT.as_millis() as u64
    );
    let reply = protocol.handle_line(&request(json!({
        "schema": AGENT_PROTOCOL_SCHEMA,
        "request_id": "list",
        "type": "list_windows",
        "future_field": true,
    })));
    let value = reply_json(&reply);
    assert_eq!(value["ok"], true);
    assert_eq!(value["windows"], json!([]));

    let reply = protocol.handle_line(&request(json!({
        "schema": "uix.agent.v2",
        "request_id": "schema",
        "type": "list_windows",
    })));
    assert_eq!(reply_json(&reply)["error"]["code"], "unsupported_schema");
    assert!(!reply.close_connection());

    for invalid in [
        json!({
            "schema": AGENT_PROTOCOL_SCHEMA,
            "request_id": "unknown-key",
            "type": "perform",
            "window_id": 99,
            "generation": 1,
            "action": { "kind": "press_key", "key": "return" },
        }),
        json!({
            "schema": AGENT_PROTOCOL_SCHEMA,
            "request_id": "window-target",
            "type": "perform",
            "window_id": 99,
            "generation": 1,
            "target": { "automation_id": "run" },
            "action": { "kind": "press_key", "key": "enter" },
        }),
        json!({
            "schema": AGENT_PROTOCOL_SCHEMA,
            "request_id": "semantic-without-target",
            "type": "perform",
            "window_id": 99,
            "generation": 1,
            "action": { "kind": "focus" },
        }),
        json!({
            "schema": AGENT_PROTOCOL_SCHEMA,
            "request_id": "coordinate-range",
            "type": "perform",
            "window_id": 99,
            "generation": 1,
            "action": { "kind": "click_at", "x": 1e100, "y": 0 },
        }),
    ] {
        let reply = protocol.handle_line(&request(invalid));
        assert_eq!(reply_json(&reply)["error"]["code"], "invalid_request");
        assert!(!reply.close_connection());
    }
}

#[test]
fn bounded_jsonl_reader_rejects_oversize_and_unterminated_messages() {
    let mut line = Vec::new();
    let mut reader = BufReader::new(Cursor::new(b"{}\n"));
    assert_eq!(
        read_bounded_line(&mut reader, &mut line).unwrap(),
        BoundedLine::Line
    );
    assert_eq!(line, b"{}");
    assert_eq!(
        read_bounded_line(&mut reader, &mut line).unwrap(),
        BoundedLine::Eof
    );

    let mut reader = BufReader::new(Cursor::new(b"{}"));
    assert_eq!(
        read_bounded_line(&mut reader, &mut line).unwrap(),
        BoundedLine::Unterminated
    );

    let oversized = vec![b'x'; MAX_AGENT_MESSAGE_BYTES + 1];
    let mut reader = BufReader::new(Cursor::new(oversized));
    assert_eq!(
        read_bounded_line(&mut reader, &mut line).unwrap(),
        BoundedLine::TooLarge
    );
}

#[test]
fn protocol_routes_snapshot_perform_and_wait_through_the_window_ui_turn() {
    let runtime = AppRuntime::new();
    assert!(runtime.enable_agent_control());
    let window_id = WindowId::new(81);
    register_runtime_session(&runtime, window_id);
    let registration = runtime
        .register_agent_window(window_id, "protocol".to_owned(), true, true)
        .expect("live registration");
    let invoked = Rc::new(Cell::new(0usize));
    let invoked_for_handler = invoked.clone();
    let mut window = WindowSession::from_root_for_window(
        window_id,
        button("Run")
            .on_click_fn(move || invoked_for_handler.set(invoked_for_handler.get() + 1))
            .automation_id("run"),
        Box::new(NullEngine::new()),
        320,
        160,
    );
    window.set_agent_command_queue(runtime.agent_command_queue(window_id).unwrap());
    assert!(window.bind_agent_window(registration));

    let (wake_tx, wake_rx) = sync_channel(4);
    runtime.set_event_loop_waker(EventLoopWaker::new(move || {
        let _ = wake_tx.send(());
    }));
    let token = [0x42; 32];
    let mut protocol = AgentProtocolSession::new(
        runtime.agent_bridge().expect("enabled bridge"),
        std::sync::Arc::new(token),
    );
    authenticate(&mut protocol, &token);

    let snapshot_request = request(json!({
        "schema": AGENT_PROTOCOL_SCHEMA,
        "request_id": "snapshot",
        "type": "snapshot",
        "window_id": window_id.raw(),
    }));
    let snapshot_worker = thread::spawn(move || {
        let reply = protocol.handle_line(&snapshot_request);
        (protocol, reply)
    });
    wake_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("snapshot wakes its target window");
    {
        let parts = window.parts_mut();
        assert!(parts
            .agent_commands
            .drain_ready(parts.tree, parts.semantic_state, true));
    }
    let (mut protocol, reply) = snapshot_worker.join().unwrap();
    let value = reply_json(&reply);
    assert_eq!(value["ok"], true);
    assert_eq!(value["snapshot"]["generation"], 1);
    assert_eq!(value["snapshot"]["revision"], 1);
    assert_eq!(value["snapshot"]["nodes"][0]["automation_id"], "run");
    let frame = &value["snapshot"]["nodes"][0]["frame"];
    let click_x = frame["x"].as_f64().unwrap() + frame["w"].as_f64().unwrap() * 0.5;
    let click_y = frame["y"].as_f64().unwrap() + frame["h"].as_f64().unwrap() * 0.5;

    let perform_request = request(json!({
        "schema": AGENT_PROTOCOL_SCHEMA,
        "request_id": "perform",
        "type": "perform",
        "window_id": window_id.raw(),
        "generation": 1,
        "expected_revision": 1,
        "target": { "automation_id": "run" },
        "action": { "kind": "focus" },
    }));
    let perform_worker = thread::spawn(move || {
        let reply = protocol.handle_line(&perform_request);
        (protocol, reply)
    });
    wake_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("perform wakes its target window");
    {
        let parts = window.parts_mut();
        assert!(parts
            .agent_commands
            .drain_ready(parts.tree, parts.semantic_state, true));
        assert!(parts.semantic_state.refresh(parts.tree));
        assert!(parts
            .agent_commands
            .finish_or_defer(parts.semantic_state, false));
    }
    let (mut protocol, reply) = perform_worker.join().unwrap();
    let value = reply_json(&reply);
    assert_eq!(value["ok"], true);
    assert_eq!(value["revision"], 2);
    assert_eq!(value["settled"], true);

    let key_request = request(json!({
        "schema": AGENT_PROTOCOL_SCHEMA,
        "request_id": "press-key",
        "type": "perform",
        "window_id": window_id.raw(),
        "generation": 1,
        "expected_revision": 2,
        "action": { "kind": "press_key", "key": "enter", "modifiers": [] },
    }));
    let key_worker = thread::spawn(move || {
        let reply = protocol.handle_line(&key_request);
        (protocol, reply)
    });
    wake_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("press_key wakes its target window");
    {
        let parts = window.parts_mut();
        assert!(parts
            .agent_commands
            .drain_ready(parts.tree, parts.semantic_state, true));
        let _ = parts.semantic_state.refresh(parts.tree);
        assert!(parts
            .agent_commands
            .finish_or_defer(parts.semantic_state, false));
    }
    assert_eq!(invoked.get(), 1, "press_key follows the focused key path");
    let (mut protocol, reply) = key_worker.join().unwrap();
    let value = reply_json(&reply);
    assert_eq!(value["ok"], true);
    assert_eq!(value["revision"], 2);
    assert_eq!(value["settled"], true);

    let click_request = request(json!({
        "schema": AGENT_PROTOCOL_SCHEMA,
        "request_id": "click-at",
        "type": "perform",
        "window_id": window_id.raw(),
        "generation": 1,
        "expected_revision": 2,
        "action": { "kind": "click_at", "x": click_x, "y": click_y },
    }));
    let click_worker = thread::spawn(move || {
        let reply = protocol.handle_line(&click_request);
        (protocol, reply)
    });
    wake_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("click_at wakes its target window");
    {
        let parts = window.parts_mut();
        assert!(parts
            .agent_commands
            .drain_ready(parts.tree, parts.semantic_state, true));
        let _ = parts.semantic_state.refresh(parts.tree);
        assert!(parts
            .agent_commands
            .finish_or_defer(parts.semantic_state, false));
    }
    assert_eq!(invoked.get(), 2, "click_at follows logical hit testing");
    let (mut protocol, reply) = click_worker.join().unwrap();
    let value = reply_json(&reply);
    assert_eq!(value["ok"], true);
    assert_eq!(value["revision"], 2);
    assert_eq!(value["settled"], true);

    let reply = protocol.handle_line(&request(json!({
        "schema": AGENT_PROTOCOL_SCHEMA,
        "request_id": "wait",
        "type": "wait",
        "window_id": window_id.raw(),
        "generation": 1,
        "after_revision": 1,
        "timeout_ms": 0,
    })));
    let value = reply_json(&reply);
    assert_eq!(value["ok"], true);
    assert_eq!(value["outcome"], "changed");
    assert_eq!(value["window"]["revision"], 2);
}

#[test]
fn native_transport_publishes_discovery_authenticates_and_cleans_up() {
    let runtime = AppRuntime::new();
    assert!(runtime.enable_agent_control());
    let window_id = WindowId::new(82);
    register_runtime_session(&runtime, window_id);
    let _registration = runtime
        .register_agent_window(window_id, "native transport".to_owned(), true, true)
        .expect("live registration");

    let info = runtime.start_agent_transport().expect("native transport");
    assert_eq!(
        runtime.start_agent_transport().unwrap(),
        info,
        "transport startup is idempotent"
    );
    assert_eq!(runtime.agent_transport_info().as_ref(), Some(&info));
    let descriptor: Value = serde_json::from_slice(
        &fs::read(&info.discovery_path).expect("published discovery descriptor"),
    )
    .expect("valid discovery JSON");
    assert_eq!(descriptor["schema"], AGENT_PROTOCOL_SCHEMA);
    assert_eq!(descriptor["process_id"], std::process::id());
    assert_eq!(descriptor["endpoint"], info.endpoint);
    let token = descriptor["token"].as_str().expect("session token");
    assert_eq!(token.len(), 64);

    let stream = connect_for_test(&info.endpoint).expect("connect to native endpoint");
    let mut stream = BufReader::new(stream);
    writeln!(
        stream.get_mut(),
        "{}",
        json!({
            "schema": AGENT_PROTOCOL_SCHEMA,
            "request_id": "hello",
            "type": "hello",
            "token": token,
        })
    )
    .unwrap();
    stream.get_mut().flush().unwrap();
    let mut line = String::new();
    stream.read_line(&mut line).unwrap();
    let hello: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(hello["ok"], true);

    writeln!(
        stream.get_mut(),
        "{}",
        json!({
            "schema": AGENT_PROTOCOL_SCHEMA,
            "request_id": "list",
            "type": "list_windows",
        })
    )
    .unwrap();
    stream.get_mut().flush().unwrap();
    line.clear();
    stream.read_line(&mut line).unwrap();
    let listed: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(listed["ok"], true);
    assert_eq!(listed["windows"][0]["window_id"], window_id.raw());
    // Shutdown must disconnect an idle authenticated client and join every
    // listener/connection worker; clients are not required to cooperate.
    runtime.shutdown_all();
    assert!(!info.discovery_path.exists());
    assert!(runtime.agent_transport_info().is_none());
    drop(stream);
}
