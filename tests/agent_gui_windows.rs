#![cfg(all(windows, feature = "agent-control"))]

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::windows::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowThreadProcessId, PostMessageW, SetWindowPos, HWND_TOPMOST,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, WM_CLOSE,
};

const START_TIMEOUT: Duration = Duration::from_secs(45);
const PRESENT_TIMEOUT_MS: u64 = 30_000;
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);

struct DemoProcess {
    child: Child,
    discovery_root: PathBuf,
    discovery_path: PathBuf,
}

impl DemoProcess {
    fn spawn() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after Unix epoch")
            .as_nanos();
        let discovery_root = std::env::temp_dir().join(format!(
            "uix-agent-gui-test-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&discovery_root).expect("create isolated LOCALAPPDATA");
        fs::create_dir(discovery_root.join("uix-agent"))
            .expect("create pre-existing discovery directory");

        let child = Command::new(env!("CARGO_BIN_EXE_uix-demo"))
            .arg("--agent-control")
            .env("LOCALAPPDATA", &discovery_root)
            .env("UIX_GRAPHICS_BACKEND", "d3d11")
            .env("RUST_LOG", "warn")
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("launch real uix-demo process");
        let discovery_path = discovery_root
            .join("uix-agent")
            .join(format!("uix-{}.json", child.id()));
        Self {
            child,
            discovery_root,
            discovery_path,
        }
    }

    fn wait_for_descriptor(&mut self) -> Value {
        let deadline = Instant::now() + START_TIMEOUT;
        loop {
            if let Ok(bytes) = fs::read(&self.discovery_path) {
                if let Ok(descriptor) = serde_json::from_slice::<Value>(&bytes) {
                    if descriptor["state"] == "ready"
                        && descriptor["process_id"].as_u64() == Some(self.child.id() as u64)
                    {
                        return descriptor;
                    }
                }
            }
            if let Some(status) = self.child.try_wait().expect("query demo process") {
                panic!("uix-demo exited before discovery publication: {status}");
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {}",
                self.discovery_path.display()
            );
            thread::sleep(Duration::from_millis(25));
        }
    }

    fn close_and_wait(&mut self) {
        let deadline = Instant::now() + SHUTDOWN_TIMEOUT;
        let window = loop {
            if let Some(window) = find_process_window(self.child.id()) {
                break window;
            }
            assert!(
                Instant::now() < deadline,
                "timed out locating the demo HWND"
            );
            thread::sleep(Duration::from_millis(25));
        };
        unsafe {
            // SAFETY: 目标 HWND 属于仍存活的测试子进程，消息不携带借用指针。
            PostMessageW(Some(window), WM_CLOSE, WPARAM(0), LPARAM(0))
        }
        .expect("post WM_CLOSE to demo");

        loop {
            if let Some(status) = self.child.try_wait().expect("query demo shutdown") {
                assert!(status.success(), "uix-demo exited with {status}");
                break;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for graceful demo shutdown"
            );
            thread::sleep(Duration::from_millis(25));
        }

        let cleanup_deadline = Instant::now() + Duration::from_secs(2);
        while self.discovery_path.exists() && Instant::now() < cleanup_deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            !self.discovery_path.exists(),
            "graceful shutdown must remove the discovery descriptor"
        );
    }

    fn raise_for_interaction(&self) {
        let deadline = Instant::now() + SHUTDOWN_TIMEOUT;
        let window = loop {
            if let Some(window) = find_process_window(self.child.id()) {
                break window;
            }
            assert!(
                Instant::now() < deadline,
                "timed out locating the demo HWND"
            );
            thread::sleep(Duration::from_millis(25));
        };
        unsafe {
            // SAFETY: 仅调整测试子进程 HWND 的 Z-order 与显示状态，不传递借用数据。
            SetWindowPos(
                window,
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            )
        }
        .expect("raise demo window for interaction");
    }
}

impl Drop for DemoProcess {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        let _ = fs::remove_dir_all(&self.discovery_root);
    }
}

struct WindowSearch {
    process_id: u32,
    window: Option<HWND>,
}

unsafe extern "system" fn find_window_callback(window: HWND, context: LPARAM) -> BOOL {
    let search = unsafe {
        // SAFETY: `find_process_window` 在同步 EnumWindows 调用期间保留该栈对象。
        &mut *(context.0 as *mut WindowSearch)
    };
    let mut process_id = 0u32;
    unsafe {
        // SAFETY: HWND 由 EnumWindows 提供，输出指针指向有效的局部 u32。
        GetWindowThreadProcessId(window, Some(&mut process_id));
    }
    if process_id == search.process_id {
        search.window = Some(window);
        BOOL(0)
    } else {
        BOOL(1)
    }
}

fn find_process_window(process_id: u32) -> Option<HWND> {
    let mut search = WindowSearch {
        process_id,
        window: None,
    };
    unsafe {
        // SAFETY: callback 与 context 仅在同步枚举期间使用，context 指针始终有效。
        let _ = EnumWindows(
            Some(find_window_callback),
            LPARAM((&mut search as *mut WindowSearch) as isize),
        );
    }
    search.window
}

fn connect(endpoint: &str, child: &mut Child) -> File {
    let deadline = Instant::now() + START_TIMEOUT;
    loop {
        match OpenOptions::new()
            .read(true)
            .write(true)
            .share_mode(0)
            .open(endpoint)
        {
            Ok(stream) => return stream,
            Err(error) => {
                if let Some(status) = child.try_wait().expect("query demo process") {
                    panic!("uix-demo exited before pipe connection: {status}; error={error}");
                }
                assert!(
                    Instant::now() < deadline,
                    "timed out connecting to {endpoint}: {error}"
                );
                thread::sleep(Duration::from_millis(25));
            }
        }
    }
}

fn exchange(connection: &mut BufReader<File>, request: Value) -> Value {
    let mut bytes = serde_json::to_vec(&request).expect("serialize protocol request");
    bytes.push(b'\n');
    connection
        .get_mut()
        .write_all(&bytes)
        .expect("write protocol request");
    connection.get_mut().flush().expect("flush request");

    let mut response = String::new();
    let read = connection
        .read_line(&mut response)
        .expect("read protocol response");
    assert!(read > 0, "agent connection closed without a response");
    serde_json::from_str(&response).expect("parse protocol response")
}

fn assert_success(response: &Value, request_id: &str) {
    assert_eq!(response["schema"], "uix.agent.v1");
    assert_eq!(response["request_id"], request_id);
    assert_eq!(response["ok"], true, "protocol error: {response}");
}

fn node_by_automation_id<'a>(snapshot: &'a Value, automation_id: &str) -> &'a Value {
    snapshot["nodes"]
        .as_array()
        .expect("snapshot nodes")
        .iter()
        .find(|node| node["automation_id"] == automation_id)
        .unwrap_or_else(|| panic!("missing automation node {automation_id}"))
}

fn wait_until_presentable(connection: &mut BufReader<File>, window_id: u64, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    let mut attempt = 0u32;
    loop {
        let request_id = format!("presentable-{attempt}");
        let listed = exchange(
            connection,
            json!({
                "schema": "uix.agent.v1",
                "request_id": request_id,
                "type": "list_windows",
            }),
        );
        assert_success(&listed, &request_id);
        let presentable = listed["windows"]
            .as_array()
            .and_then(|windows| {
                windows
                    .iter()
                    .find(|window| window["window_id"].as_u64() == Some(window_id))
            })
            .and_then(|window| window["presentable"].as_bool())
            .unwrap_or(false);
        if presentable {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "demo window did not recover presentability"
        );
        attempt = attempt.wrapping_add(1);
        thread::sleep(Duration::from_millis(50));
    }
}

#[test]
#[ignore = "requires an interactive Windows desktop"]
fn real_gui_process_authenticates_performs_and_cleans_up() {
    let mut demo = DemoProcess::spawn();
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
            "request_id": "hello",
            "type": "hello",
            "token": token,
            "client": { "name": "uix-agent-gui-test" },
        }),
    );
    assert_success(&hello, "hello");
    assert!(hello["capabilities"]["request_types"]
        .as_array()
        .is_some_and(|types| types.contains(&json!("wait"))));

    let listed = exchange(
        &mut connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "list",
            "type": "list_windows",
        }),
    );
    assert_success(&listed, "list");
    let windows = listed["windows"].as_array().expect("listed windows");
    assert_eq!(windows.len(), 1);
    assert_eq!(windows[0]["title"], "UIX Demo");
    let window_id = windows[0]["window_id"].as_u64().expect("window id");
    let generation = windows[0]["generation"].as_u64().expect("generation");

    let first_present = exchange(
        &mut connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "first-present",
            "type": "wait",
            "window_id": window_id,
            "generation": generation,
            "presented_revision": 1,
            "timeout_ms": PRESENT_TIMEOUT_MS,
        }),
    );
    assert_success(&first_present, "first-present");
    assert_eq!(first_present["outcome"], "presented");
    demo.raise_for_interaction();
    wait_until_presentable(&mut connection, window_id, Duration::from_secs(10));

    let before = exchange(
        &mut connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "snapshot-before",
            "type": "snapshot",
            "window_id": window_id,
        }),
    );
    assert_success(&before, "snapshot-before");
    let increment = node_by_automation_id(&before["snapshot"], "home-count-increment");
    assert!(increment["actions"]
        .as_array()
        .is_some_and(|actions| actions.contains(&json!("invoke"))));
    assert_eq!(
        node_by_automation_id(&before["snapshot"], "home-count-value")["name"],
        "计数: 0"
    );

    let performed = exchange(
        &mut connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "increment",
            "type": "perform",
            "window_id": window_id,
            "generation": generation,
            "target": { "automation_id": "home-count-increment" },
            "action": { "kind": "invoke" },
        }),
    );
    assert_success(&performed, "increment");
    assert_eq!(performed["settled"], true);
    let changed_revision = performed["revision"].as_u64().expect("changed revision");

    let presented = exchange(
        &mut connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "changed-present",
            "type": "wait",
            "window_id": window_id,
            "generation": generation,
            "presented_revision": changed_revision,
            "timeout_ms": PRESENT_TIMEOUT_MS,
        }),
    );
    assert_success(&presented, "changed-present");
    assert_eq!(presented["outcome"], "presented");

    let after = exchange(
        &mut connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "snapshot-after",
            "type": "snapshot",
            "window_id": window_id,
        }),
    );
    assert_success(&after, "snapshot-after");
    assert_eq!(
        node_by_automation_id(&after["snapshot"], "home-count-value")["name"],
        "计数: 1"
    );

    drop(connection);
    demo.close_and_wait();
}
