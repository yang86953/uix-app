//! 《Agent 独立后台操作面》的公开消费者；生产后台与用户输入侧互不借用组件树。
#![cfg(all(feature = "agent-control", feature = "test-harness"))]

use serde_json::{Value, json};
use std::sync::{Mutex, mpsc};
use std::time::Duration;
use uix::app::agent_client::{AgentBridgeClient, AgentWindowEntry};
use uix::app::agent_workspace::AgentWorkspace;
use uix::prelude::*;
use uix::ui::test_harness::TestApp;

// 文档规定每进程一个操作面，消费者串行持有该公开租约，不重试重复启动。
static WORKSPACE: Mutex<()> = Mutex::new(());

fn action(client: &mut AgentBridgeClient, view: AgentWindowEntry, action: Value) -> Value {
    client
        .request(json!({"type":"perform", "window_id":view.window_id,
        "generation":view.generation, "action":action}))
        .unwrap()
}
fn assert_ok(reply: &Value) {
    assert_eq!(reply["ok"], true, "{reply}");
}
fn editor(text: &State<String>) -> ViewNode {
    input().value(text).build().automation_id("editor")
}
fn screenshot(client: &mut AgentBridgeClient, window: u64) -> Value {
    let reply = client
        .request(json!({"type":"screenshot", "window_id":window}))
        .unwrap();
    assert_ok(&reply);
    // PNG 文件签名的 base64 前缀来自真实响应，不接受伪造的空像素成功。
    assert!(
        reply["data_base64"]
            .as_str()
            .unwrap()
            .starts_with("iVBORw0KGgo")
    );
    assert_eq!(reply["format"], "png");
    reply
}

#[test]
fn background_input_navigation_clipboard_and_pixels_do_not_mutate_user_tree() {
    let _serial = WORKSPACE.lock().unwrap_or_else(|e| e.into_inner());
    let user_text = State::new("用户正在编辑".to_owned());
    let user_view = user_text.clone();
    let mut user = TestApp::new((400.0, 240.0), move || editor(&user_view));
    user.focus("editor").unwrap();
    user.press_key(KeyCode::A, KeyMod::CTRL).unwrap();
    let user_before = user.snapshot().nodes;

    let agent_text = State::new("agent draft".to_owned());
    let page = State::new(false);
    let business = State::new("未提交".to_owned());
    let (draft, navigation, saved) = (agent_text.clone(), page.clone(), business.clone());
    let workspace = AgentWorkspace::new(400, 240, move || {
        let save_draft = draft.clone();
        let saved = saved.clone();
        let next_page = navigation.clone();
        column_fit((
            if navigation.get() {
                label("独立后台页面").automation_id("agent-page")
            } else {
                editor(&draft)
            },
            button("提交业务")
                .on_click(&saved, move |value| {
                    value.set(save_draft.get());
                })
                .build()
                .automation_id("save"),
            button("切换后台页面")
                .on_click(&next_page, |value| {
                    value.set(!value.get());
                })
                .build()
                .automation_id("next"),
        ))
    })
    .spawn()
    .unwrap();
    let mut client = workspace.client().unwrap();
    assert_eq!(
        client.capabilities()["background_control"]["isolated_workspace"],
        true
    );
    assert_eq!(
        client.capabilities()["background_control"]["native_window_access"],
        false
    );
    let windows = client.request(json!({"type":"list_windows"})).unwrap();
    assert_eq!(windows["windows"].as_array().unwrap().len(), 1);
    assert_eq!(windows["windows"][0]["visible"], false);
    assert_eq!(windows["windows"][0]["focused"], false);
    let view = client.list_windows().unwrap()[0];
    client
        .perform_set_value(view.window_id, view.generation, "editor", "AI 私有正文")
        .unwrap();
    client
        .perform_raw(
            view.window_id,
            view.generation,
            "editor",
            json!({"kind":"focus"}),
        )
        .unwrap();
    for key in ["a", "c"] {
        assert_ok(&action(
            &mut client,
            view,
            json!({"kind":"press_key","key":key,"modifiers":["ctrl"]}),
        ));
    }
    client
        .perform_set_value(view.window_id, view.generation, "editor", "")
        .unwrap();
    assert_ok(&action(
        &mut client,
        view,
        json!({"kind":"press_key","key":"v","modifiers":["ctrl"]}),
    ));
    assert_eq!(
        agent_text.get(),
        "AI 私有正文",
        "复制与粘贴应使用后台私有剪贴板"
    );
    let first = screenshot(&mut client, view.window_id);
    assert_eq!(
        (first["width"].as_i64(), first["height"].as_i64()),
        (Some(400), Some(240))
    );
    client
        .perform_invoke(view.window_id, view.generation, "save")
        .unwrap();
    assert_eq!(
        business.get(),
        "AI 私有正文",
        "只有明确的业务提交才共享结果"
    );
    client
        .perform_invoke(view.window_id, view.generation, "next")
        .unwrap();
    assert!(page.get());
    let second = screenshot(&mut client, view.window_id);
    assert_ne!(
        first["data_base64"], second["data_base64"],
        "导航后必须绘制新帧"
    );
    assert_eq!(user_text.get(), "用户正在编辑");
    assert_eq!(
        user.snapshot().nodes,
        user_before,
        "AI 不得改写用户焦点、选区或节点状态"
    );
    // 用户此前的全选状态仍有效，而不是只检查文本刚好没变。
    user.insert_text("editor", "用户继续输入").unwrap();
    assert_eq!(user_text.get(), "用户继续输入");
    workspace.close().unwrap();
}

#[test]
fn offscreen_viewport_revision_guards_and_lifecycle_are_real() {
    let _serial = WORKSPACE.lock().unwrap_or_else(|e| e.into_inner());
    let text = State::new("old".to_owned());
    let workspace = AgentWorkspace::new(320, 200, move || editor(&text))
        .spawn()
        .unwrap();
    let mut client = workspace.client().unwrap();
    // 通过官方发现入口核对鉴权失败；不把端点或 token 输出到断言信息。
    let discovery: Value = serde_json::from_reader(
        std::fs::File::open(AgentBridgeClient::discovery_file(std::process::id()).unwrap())
            .unwrap(),
    )
    .unwrap();
    let unauthorized =
        AgentBridgeClient::connect(discovery["endpoint"].as_str().unwrap(), "invalid-token")
            .err()
            .unwrap();
    assert!(unauthorized.to_string().contains("unauthorized"));
    assert!(client.request(json!([])).is_err());
    let view = client.list_windows().unwrap()[0];
    let old = client.snapshot(view.window_id).unwrap()["revision"]
        .as_u64()
        .unwrap();
    let revision = client
        .perform_set_value(view.window_id, view.generation, "editor", "new")
        .unwrap();
    client
        .wait_until_presented(
            view.window_id,
            view.generation,
            revision,
            Duration::from_secs(2),
        )
        .unwrap();
    let stale = client
        .request(json!({"type":"perform", "window_id":view.window_id,
        "generation":view.generation,"expected_revision":old,"target":{"automation_id":"editor"},
        "action":{"kind":"set_value","value":"must not apply"}}))
        .unwrap();
    assert_eq!(stale["error"]["code"], "stale_revision");
    for kind in [
        "activate_window",
        "move_window",
        "maximize_window",
        "minimize_window",
        "restore_window",
    ] {
        let reply = action(&mut client, view, json!({"kind":kind,"x":0,"y":0}));
        assert_eq!(
            reply["ok"], false,
            "{kind} must not borrow desktop access: {reply}"
        );
    }
    assert_ok(&action(
        &mut client,
        view,
        json!({"kind":"resize_window","width":480,"height":280}),
    ));
    let png = screenshot(&mut client, view.window_id);
    assert_eq!(
        (png["width"].as_i64(), png["height"].as_i64()),
        (Some(480), Some(280))
    );
    let invalid = action(
        &mut client,
        view,
        json!({"kind":"resize_window","width":8192,"height":8192}),
    );
    assert_eq!(invalid["ok"], false);
    assert_eq!(screenshot(&mut client, view.window_id)["width"], 480);
    assert!(
        AgentWorkspace::new(1, 1, || label("second"))
            .spawn()
            .is_err()
    );
    assert_ok(&action(&mut client, view, json!({"kind":"close_window"})));
    workspace.close().unwrap();
    assert!(
        client
            .request(json!({"type":"snapshot", "window_id":view.window_id}))
            .is_err()
    );
    let replacement = AgentWorkspace::new(64, 64, || label("新代际"))
        .spawn()
        .unwrap();
    assert!(replacement.client().is_ok());
    replacement.close().unwrap();
}

#[test]
fn read_only_protection_and_confirmations_remain_enforced() {
    let _serial = WORKSPACE.lock().unwrap_or_else(|e| e.into_inner());
    let text = State::new("protected".to_owned());
    let readonly = AgentWorkspace::new(160, 100, move || editor(&text))
        .read_only()
        .spawn()
        .unwrap();
    let mut client = readonly.client().unwrap();
    let view = client.list_windows().unwrap()[0];
    assert!(
        client
            .perform_set_value(view.window_id, view.generation, "editor", "bad")
            .is_err()
    );
    assert_eq!(
        action(
            &mut client,
            view,
            json!({"kind":"press_key","key":"a","modifiers":[]})
        )["error"]["code"],
        "forbidden"
    );
    screenshot(&mut client, view.window_id);
    readonly.close().unwrap();

    let text = State::new("protected".to_owned());
    let protected = AgentWorkspace::new(160, 100, move || editor(&text))
        .protect("editor")
        .require_confirm("editor")
        .spawn()
        .unwrap();
    let mut client = protected.client().unwrap();
    let view = client.list_windows().unwrap()[0];
    let reply = client
        .request(
            json!({"type":"perform", "window_id":view.window_id,"generation":view.generation,
        "target":{"automation_id":"editor"},"action":{"kind":"set_value","value":"forbidden"}}),
        )
        .unwrap();
    assert_eq!(
        reply["error"]["code"], "forbidden",
        "保护规则不能被确认覆盖"
    );
    protected.close().unwrap();

    let (tx, rx) = mpsc::sync_channel(1);
    let count = State::new(0);
    let calls = count.clone();
    let workspace = AgentWorkspace::new(180, 120, move || {
        let calls = calls.clone();
        button(format!("确认提交 {}", calls.get()))
            .on_click(&calls, |value| {
                value.set(value.get() + 1);
            })
            .build()
            .automation_id("submit")
    })
    .require_confirm("submit")
    .confirm_with(move |request| {
        tx.send(request).unwrap();
    })
    .spawn()
    .unwrap();
    for (allow, make_stale) in [(false, false), (true, false), (true, true)] {
        let mut client = workspace.client().unwrap();
        let view = client.list_windows().unwrap()[0];
        let before = count.get();
        let revision = client.snapshot(view.window_id).unwrap()["revision"].clone();
        let pending = client.request(json!({"type":"perform", "window_id":view.window_id,"generation":view.generation,
            "expected_revision":revision,"target":{"automation_id":"submit"},"action":{"kind":"invoke"}})).unwrap();
        assert_eq!(pending["error"]["code"], "requires_confirmation");
        let confirm_id = pending["error"]["confirm_id"].as_u64().unwrap();
        let confirmer = std::thread::spawn(move || {
            client
                .request(
                    json!({"type":"confirm", "window_id":view.window_id, "confirm_id":confirm_id}),
                )
                .unwrap()
        });
        let request = rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(count.get(), before, "等待决定时不得先执行");
        if make_stale {
            let counter = count.clone();
            workspace.post_to_ui(move || counter.set(counter.get() + 10));
            workspace
                .client()
                .unwrap()
                .snapshot(view.window_id)
                .unwrap();
            assert_eq!(count.get(), before + 10);
        }
        workspace
            .resolve_confirmation(request.window_id, request.confirm_id, allow)
            .unwrap();
        let reply = confirmer.join().unwrap();
        if make_stale {
            assert_eq!(reply["error"]["code"], "stale_revision");
            assert_eq!(count.get(), before + 10);
        } else if allow {
            assert_ok(&reply);
            assert_eq!(
                count.get(),
                before + 1,
                "一次批准仅执行一次且不循环要求确认"
            );
        } else {
            assert_eq!(reply["error"]["code"], "confirmation_rejected");
            assert_eq!(count.get(), before);
        }
    }
    workspace.close().unwrap();
}

#[test]
fn invalid_configuration_fails_before_any_native_window_is_created() {
    let _serial = WORKSPACE.lock().unwrap_or_else(|e| e.into_inner());
    assert!(
        AgentWorkspace::new(0, 100, || label("invalid"))
            .spawn()
            .is_err()
    );
    assert_eq!(
        App::new()
            .enable_agent_control()
            .root(|| label("must not show"))
            .run(),
        1
    );
    // 模拟不受信任的宿主根工厂失败；失败不回退到前台，租约必须归还。
    assert!(
        AgentWorkspace::new(64, 64, || panic!("fixture root failure"))
            .spawn()
            .is_err()
    );
    AgentWorkspace::new(64, 64, || label("recovered lease"))
        .spawn()
        .unwrap()
        .close()
        .unwrap();
}

#[test]
fn native_chrome_never_acknowledges_desktop_side_effects() {
    let _serial = WORKSPACE.lock().unwrap_or_else(|e| e.into_inner());
    let workspace = AgentWorkspace::new(160, 100, || {
        window_control(WindowControl::Close, label("关闭桌面")).automation_id("native-close")
    })
    .spawn()
    .unwrap();
    let mut client = workspace.client().unwrap();
    let view = client.list_windows().unwrap()[0];
    let invoke = client
        .request(
            json!({"type":"perform","window_id":view.window_id,"generation":view.generation,
        "target":{"automation_id":"native-close"},"action":{"kind":"invoke"}}),
        )
        .unwrap();
    assert_eq!(invoke["error"]["code"], "unsupported_action");
    client
        .perform_raw(
            view.window_id,
            view.generation,
            "native-close",
            json!({"kind":"focus"}),
        )
        .unwrap();
    let press = action(
        &mut client,
        view,
        json!({"kind":"press_key","key":"enter","modifiers":[]}),
    );
    assert_eq!(press["error"]["code"], "window_operation_failed");
    screenshot(&mut client, view.window_id);
    workspace.close().unwrap();
}

#[test]
fn user_and_agent_can_edit_concurrently_without_crossing_drafts() {
    let _serial = WORKSPACE.lock().unwrap_or_else(|e| e.into_inner());
    let user_text = State::new(String::new());
    let user_view = user_text.clone();
    let mut user = TestApp::new((320.0, 200.0), move || editor(&user_view));
    user.focus("editor").unwrap();
    let agent_text = State::new(String::new());
    let agent_view = agent_text.clone();
    let workspace = AgentWorkspace::new(320, 200, move || editor(&agent_view))
        .spawn()
        .unwrap();
    let mut client = workspace.client().unwrap();
    let view = client.list_windows().unwrap()[0];
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let ai_start = barrier.clone();
    let ai = std::thread::spawn(move || {
        ai_start.wait();
        for index in 0..20 {
            client
                .perform_set_value(
                    view.window_id,
                    view.generation,
                    "editor",
                    &format!("AI {index}"),
                )
                .unwrap();
        }
    });
    barrier.wait();
    for index in 0..20 {
        user.press_key(KeyCode::A, KeyMod::CTRL).unwrap();
        user.insert_text("editor", &format!("User {index}"))
            .unwrap();
    }
    ai.join().unwrap();
    assert_eq!(user_text.get(), "User 19");
    assert_eq!(agent_text.get(), "AI 19");
    workspace.close().unwrap();
}

#[cfg(unix)]
#[test]
fn mcp_public_catalog_accepts_root_zero_and_numeric_confirmation_ids() {
    use std::io::{BufRead, BufReader, Write};
    use std::process::{Command, Stdio};
    // 只通过公开 STDIO MCP 协议检验目录，不导入 Python 内部函数或访问用户应用。
    let mut child = Command::new("python3")
        .arg("scripts/uix_agent_mcp.py")
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    writeln!(
        input,
        "{}",
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}})
    )
    .unwrap();
    output.read_line(&mut line).unwrap();
    let initialized: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(initialized["id"], 1);
    assert!(
        initialized["result"]["instructions"]
            .as_str()
            .unwrap()
            .contains("独立后台")
    );
    line.clear();
    writeln!(
        input,
        "{}",
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}})
    )
    .unwrap();
    output.read_line(&mut line).unwrap();
    let reply: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(reply["id"], 2);
    let tools = reply["result"]["tools"].as_array().unwrap();
    for tool in tools {
        if let Some(window) = tool["inputSchema"]["properties"].get("window_id") {
            assert_eq!(
                window["minimum"], 0,
                "MCP 必须接纳 list_windows 返回的根视口 0"
            );
        }
    }
    let confirm = tools
        .iter()
        .find(|tool| tool["name"] == "uix_confirm")
        .unwrap();
    assert_eq!(
        confirm["inputSchema"]["properties"]["confirm_id"]["type"],
        "integer"
    );
    drop(input);
    assert!(child.wait().unwrap().success());
}

#[test]
fn default_resources_render_real_text_pixels_without_desktop_fonts() {
    let _serial = WORKSPACE.lock().unwrap_or_else(|e| e.into_inner());
    let workspace = AgentWorkspace::new(180, 80, || label("Visible text 文字"))
        .spawn()
        .unwrap();
    let mut client = workspace.client().unwrap();
    let window = client.list_windows().unwrap()[0].window_id;
    let reply = screenshot(&mut client, window);
    // 公开 PNG 协议消费者：解码真实响应；仅有正确签名但没有字形的空白图也必须失败。
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut encoded = 0_u32;
    let mut bits = 0_u32;
    let mut png_bytes = Vec::new();
    for byte in reply["data_base64"]
        .as_str()
        .unwrap()
        .bytes()
        .take_while(|b| *b != b'=')
    {
        let value = alphabet.iter().position(|b| *b == byte).unwrap() as u32;
        encoded = (encoded << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            png_bytes.push((encoded >> bits) as u8);
        }
    }
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png_bytes))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
    let frame = decoder.next_frame(&mut pixels).unwrap();
    assert_eq!(frame.color_type, png::ColorType::Rgb);
    let pixels = &pixels[..frame.buffer_size()];
    assert!(
        pixels.chunks_exact(3).any(|pixel| pixel != &pixels[..3]),
        "单个 Text 节点必须产生字形像素，不允许语义有文字而画面全空白"
    );
    workspace.close().unwrap();
}
