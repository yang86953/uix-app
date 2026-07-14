#![cfg(all(windows, feature = "agent-control"))]

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::windows::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, SetActiveWindow, SetFocus, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT,
    KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_TAB,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetForegroundWindow, GetWindowThreadProcessId, IsIconic,
    IsZoomed, PostMessageW, SetForegroundWindow, SetWindowPos, ShowWindowAsync, HWND_TOPMOST,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_SHOWWINDOW, SW_MAXIMIZE, SW_MINIMIZE, WM_CLOSE,
};

const START_TIMEOUT: Duration = Duration::from_secs(45);
const PRESENT_TIMEOUT_MS: u64 = 30_000;
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy)]
struct GraphicsExpectation {
    backend_override: Option<&'static str>,
    selected_recipe: &'static str,
    present_occlusion: &'static str,
}

const D3D11_GRAPHICS: GraphicsExpectation = GraphicsExpectation {
    backend_override: Some("d3d11"),
    selected_recipe: "backend=d3d11; raster=gpu_native; present=swapchain",
    present_occlusion: "present_status_and_test",
};
const DEFAULT_VULKAN_GRAPHICS: GraphicsExpectation = GraphicsExpectation {
    backend_override: None,
    selected_recipe: "backend=vulkan; raster=cpu; present=pixel_upload",
    present_occlusion: "unsupported",
};
static REAL_GUI_LOCK: Mutex<()> = Mutex::new(());

struct DemoProcess {
    child: Child,
    discovery_root: PathBuf,
    discovery_path: PathBuf,
    output_readers: Vec<JoinHandle<String>>,
    graphics: GraphicsExpectation,
}

impl DemoProcess {
    fn spawn(graphics: GraphicsExpectation) -> Self {
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

        let mut command = Command::new(env!("CARGO_BIN_EXE_uix-demo"));
        command
            .arg("--agent-control")
            .env("LOCALAPPDATA", &discovery_root)
            .env_remove("UIX_GRAPHICS_BACKEND")
            .env("RUST_LOG", "info")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(backend) = graphics.backend_override {
            command.env("UIX_GRAPHICS_BACKEND", backend);
        }
        let mut child = command.spawn().expect("launch real uix-demo process");
        let stdout = child.stdout.take().expect("capture demo stdout");
        let stderr = child.stderr.take().expect("capture demo stderr");
        let output_readers = vec![read_process_output(stdout), read_process_output(stderr)];
        let discovery_path = discovery_root
            .join("uix-agent")
            .join(format!("uix-{}.json", child.id()));
        Self {
            child,
            discovery_root,
            discovery_path,
            output_readers,
            graphics,
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
                let output = self.take_output();
                panic!("uix-demo exited before discovery publication: {status}; output={output}");
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
        let window = self.window_handle();
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

        let output = self.take_output();
        let selected = format!(
            "Graphics bootstrap: selected recipe {}; present_occlusion={}",
            self.graphics.selected_recipe, self.graphics.present_occlusion
        );
        assert!(
            output.contains(&selected),
            "demo did not select graphics recipe `{}` with present occlusion `{}`; output={output}",
            self.graphics.selected_recipe,
            self.graphics.present_occlusion
        );
        assert!(
            !output.contains("fallback=software_cpu"),
            "demo unexpectedly fell back to Software; output={output}"
        );
    }

    fn take_output(&mut self) -> String {
        std::mem::take(&mut self.output_readers)
            .into_iter()
            .map(|reader| reader.join().expect("join demo output reader"))
            .collect::<Vec<_>>()
            .join("")
    }

    fn raise_for_interaction(&self) {
        let window = self.window_handle();
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

    fn minimize(&self) {
        let window = self.window_handle();
        unsafe {
            // SAFETY: 命令只异步改变仍存活测试子进程 HWND 的显示状态。
            let _ = ShowWindowAsync(window, SW_MINIMIZE);
        }
        self.wait_for_window_state(window, "minimized", |window| unsafe {
            // SAFETY: 查询期间 HWND 属于仍存活的测试子进程。
            IsIconic(window).as_bool()
        });
    }

    fn maximize(&self) {
        let window = self.window_handle();
        unsafe {
            // SAFETY: 命令只异步改变仍存活测试子进程 HWND 的显示状态。
            let _ = ShowWindowAsync(window, SW_MAXIMIZE);
        }
        self.wait_for_window_state(window, "maximized", |window| unsafe {
            // SAFETY: 查询期间 HWND 属于仍存活的测试子进程。
            IsZoomed(window).as_bool()
        });
    }

    fn send_system_tab(&self) {
        let window = self.window_handle();
        request_foreground_focus(window);
        let inputs = [
            keyboard_input(VK_TAB, KEYBD_EVENT_FLAGS(0)),
            keyboard_input(VK_TAB, KEYEVENTF_KEYUP),
        ];
        // SAFETY: INPUT 数组在同步调用期间有效，结构尺寸与 Win32 ABI 一致。
        let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
        assert_eq!(
            sent as usize,
            inputs.len(),
            "SendInput must enqueue the complete Tab press"
        );
    }

    fn window_handle(&self) -> HWND {
        let deadline = Instant::now() + SHUTDOWN_TIMEOUT;
        loop {
            if let Some(window) = find_process_window(self.child.id()) {
                return window;
            }
            assert!(
                Instant::now() < deadline,
                "timed out locating the demo HWND"
            );
            thread::sleep(Duration::from_millis(25));
        }
    }

    fn wait_for_window_state(&self, window: HWND, state: &str, reached: impl Fn(HWND) -> bool) {
        let deadline = Instant::now() + SHUTDOWN_TIMEOUT;
        while !reached(window) {
            assert!(
                Instant::now() < deadline,
                "timed out waiting for demo window to become {state}"
            );
            thread::sleep(Duration::from_millis(25));
        }
    }
}

fn keyboard_input(vk: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn window_thread_id(window: HWND) -> u32 {
    // SAFETY: window 来自仍存活测试子进程的 EnumWindows 结果；不请求进程 ID 输出。
    unsafe { GetWindowThreadProcessId(window, None) }
}

fn foreground_window() -> HWND {
    // SAFETY: GetForegroundWindow 不接收指针，也不转移返回句柄的所有权。
    unsafe { GetForegroundWindow() }
}

fn attach_input_thread(current: u32, other: u32) -> bool {
    if other == 0 || other == current {
        return false;
    }
    // SAFETY: 两个线程 ID 均来自 Win32 查询；调用只临时合并其输入队列。
    unsafe { AttachThreadInput(current, other, true) }.as_bool()
}

fn request_foreground_focus(window: HWND) {
    // SAFETY: GetCurrentThreadId 不接收指针，返回值只用于本次输入队列操作。
    let current_thread = unsafe { GetCurrentThreadId() };
    let target_thread = window_thread_id(window);
    assert_ne!(target_thread, 0, "demo HWND must belong to a live thread");
    let foreground = foreground_window();
    let foreground_thread = if foreground.0.is_null() {
        0
    } else {
        window_thread_id(foreground)
    };
    let attached_foreground = attach_input_thread(current_thread, foreground_thread);
    let attached_target =
        target_thread != foreground_thread && attach_input_thread(current_thread, target_thread);

    unsafe {
        // SAFETY: window 属于仍存活的测试子进程；临时合并输入队列后仅请求激活与键盘焦点。
        let _ = BringWindowToTop(window);
        let _ = SetActiveWindow(window);
        let _ = SetForegroundWindow(window);
        let _ = SetFocus(Some(window));
        if attached_target {
            let _ = AttachThreadInput(current_thread, target_thread, false);
        }
        if attached_foreground {
            let _ = AttachThreadInput(current_thread, foreground_thread, false);
        }
    }

    let deadline = Instant::now() + Duration::from_secs(2);
    while foreground_window() != window {
        assert!(
            Instant::now() < deadline,
            "demo HWND did not become the foreground window"
        );
        thread::sleep(Duration::from_millis(25));
    }
}

impl Drop for DemoProcess {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        for reader in std::mem::take(&mut self.output_readers) {
            let _ = reader.join();
        }
        let _ = fs::remove_dir_all(&self.discovery_root);
    }
}

fn read_process_output(mut stream: impl Read + Send + 'static) -> JoinHandle<String> {
    thread::spawn(move || {
        let mut output = String::new();
        stream
            .read_to_string(&mut output)
            .expect("read demo process output");
        output
    })
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

fn visible_center(node: &Value) -> (f64, f64) {
    let bounds = &node["visible_bounds"];
    let x = bounds["x"].as_f64().expect("visible bounds x");
    let y = bounds["y"].as_f64().expect("visible bounds y");
    let width = bounds["w"].as_f64().expect("visible bounds width");
    let height = bounds["h"].as_f64().expect("visible bounds height");
    (x + width * 0.5, y + height * 0.5)
}

fn wait_until_presentable(
    connection: &mut BufReader<File>,
    window_id: u64,
    expected: bool,
    timeout: Duration,
) {
    let deadline = Instant::now() + timeout;
    let mut attempt = 0u32;
    loop {
        let request_id = format!("presentable-{expected}-{attempt}");
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
        if presentable == expected {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "demo window presentable state did not become {expected}"
        );
        attempt = attempt.wrapping_add(1);
        thread::sleep(Duration::from_millis(50));
    }
}

fn wait_for_focused_node(
    connection: &mut BufReader<File>,
    window_id: u64,
    automation_id: &str,
    timeout: Duration,
) {
    let deadline = Instant::now() + timeout;
    let mut attempt = 0u32;
    loop {
        let request_id = format!("focused-{automation_id}-{attempt}");
        let snapshot = exchange(
            connection,
            json!({
                "schema": "uix.agent.v1",
                "request_id": request_id,
                "type": "snapshot",
                "window_id": window_id,
            }),
        );
        assert_success(&snapshot, &request_id);
        if node_by_automation_id(&snapshot["snapshot"], automation_id)["focused"] == true {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "demo focus did not move to {automation_id}"
        );
        attempt = attempt.wrapping_add(1);
        thread::sleep(Duration::from_millis(50));
    }
}

fn perform_until_presentable(
    demo: &DemoProcess,
    connection: &mut BufReader<File>,
    window_id: u64,
    generation: u64,
    request_prefix: &str,
    target: Option<Value>,
    action: Value,
) -> Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut attempt = 0u32;
    loop {
        demo.raise_for_interaction();
        wait_until_presentable(connection, window_id, true, Duration::from_secs(10));
        let request_id = format!("{request_prefix}-{attempt}");
        let mut request = json!({
            "schema": "uix.agent.v1",
            "request_id": request_id.clone(),
            "type": "perform",
            "window_id": window_id,
            "generation": generation,
            "action": action.clone(),
        });
        if let Some(target) = target.clone() {
            request["target"] = target;
        }
        let response = exchange(connection, request);
        if response["ok"] == true {
            assert_success(&response, &request_id);
            return response;
        }
        assert_eq!(
            response["error"]["code"], "not_presentable",
            "unexpected protocol error: {response}"
        );
        assert!(
            Instant::now() < deadline,
            "window stayed non-presentable while performing {request_prefix}"
        );
        attempt = attempt.wrapping_add(1);
        thread::sleep(Duration::from_millis(50));
    }
}

fn invoke_until_presentable(
    demo: &DemoProcess,
    connection: &mut BufReader<File>,
    window_id: u64,
    generation: u64,
    automation_id: &str,
) -> Value {
    perform_until_presentable(
        demo,
        connection,
        window_id,
        generation,
        "invoke",
        Some(json!({ "automation_id": automation_id })),
        json!({ "kind": "invoke" }),
    )
}

#[test]
#[ignore = "requires an interactive Windows desktop"]
fn real_d3d11_gui_process_authenticates_performs_and_cleans_up() {
    run_real_gui_scenario(D3D11_GRAPHICS);
}

#[test]
#[ignore = "requires an interactive Windows desktop and Vulkan driver"]
fn real_default_vulkan_gui_presents_and_recovers_from_minimize() {
    run_real_gui_scenario(DEFAULT_VULKAN_GRAPHICS);
}

fn run_real_gui_scenario(graphics: GraphicsExpectation) {
    let _guard = REAL_GUI_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let mut demo = DemoProcess::spawn(graphics);
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
    wait_until_presentable(&mut connection, window_id, true, Duration::from_secs(10));

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

    let performed = invoke_until_presentable(
        &demo,
        &mut connection,
        window_id,
        generation,
        "home-count-increment",
    );
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

    let focused = perform_until_presentable(
        &demo,
        &mut connection,
        window_id,
        generation,
        "focus-increment",
        Some(json!({ "automation_id": "home-count-increment" })),
        json!({ "kind": "focus" }),
    );
    assert_eq!(focused["settled"], true);
    demo.send_system_tab();
    wait_for_focused_node(
        &mut connection,
        window_id,
        "home-count-decrement",
        Duration::from_secs(10),
    );
    let native_tab_reset = perform_until_presentable(
        &demo,
        &mut connection,
        window_id,
        generation,
        "native-tab-reset",
        None,
        json!({ "kind": "press_key", "key": "tab", "modifiers": ["shift"] }),
    );
    assert_eq!(native_tab_reset["settled"], true);
    let tabbed = perform_until_presentable(
        &demo,
        &mut connection,
        window_id,
        generation,
        "tab-forward",
        None,
        json!({ "kind": "press_key", "key": "tab", "modifiers": [] }),
    );
    assert_eq!(tabbed["settled"], true);
    let after_tab = exchange(
        &mut connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "snapshot-after-tab",
            "type": "snapshot",
            "window_id": window_id,
        }),
    );
    assert_success(&after_tab, "snapshot-after-tab");
    assert_eq!(
        node_by_automation_id(&after_tab["snapshot"], "home-count-decrement")["focused"],
        true
    );

    let reverse_tabbed = perform_until_presentable(
        &demo,
        &mut connection,
        window_id,
        generation,
        "tab-backward",
        None,
        json!({ "kind": "press_key", "key": "tab", "modifiers": ["shift"] }),
    );
    assert_eq!(reverse_tabbed["settled"], true);
    let after_reverse_tab = exchange(
        &mut connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "snapshot-after-reverse-tab",
            "type": "snapshot",
            "window_id": window_id,
        }),
    );
    assert_success(&after_reverse_tab, "snapshot-after-reverse-tab");
    assert_eq!(
        node_by_automation_id(&after_reverse_tab["snapshot"], "home-count-increment")["focused"],
        true
    );
    let (increment_x, increment_y) = visible_center(node_by_automation_id(
        &after_reverse_tab["snapshot"],
        "home-count-increment",
    ));
    let clicked = perform_until_presentable(
        &demo,
        &mut connection,
        window_id,
        generation,
        "click-increment",
        None,
        json!({ "kind": "click_at", "x": increment_x, "y": increment_y }),
    );
    assert_eq!(clicked["settled"], true);
    let after_click = exchange(
        &mut connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "snapshot-after-click",
            "type": "snapshot",
            "window_id": window_id,
        }),
    );
    assert_success(&after_click, "snapshot-after-click");
    assert_eq!(
        node_by_automation_id(&after_click["snapshot"], "home-count-value")["name"],
        "计数: 2"
    );
    demo.minimize();
    wait_until_presentable(&mut connection, window_id, false, Duration::from_secs(10));
    let minimized = exchange(
        &mut connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "snapshot-minimized",
            "type": "snapshot",
            "window_id": window_id,
        }),
    );
    assert_success(&minimized, "snapshot-minimized");
    let before_minimize_revision = minimized["snapshot"]["revision"]
        .as_u64()
        .expect("revision while minimized");
    let minimized_presented_revision = minimized["snapshot"]["presented_revision"]
        .as_u64()
        .expect("presented revision while minimized");
    let premature_present = exchange(
        &mut connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "minimized-present",
            "type": "wait",
            "window_id": window_id,
            "generation": generation,
            "presented_revision": minimized_presented_revision + 1,
            "timeout_ms": 250,
        }),
    );
    assert_eq!(premature_present["ok"], false);
    assert_eq!(premature_present["error"]["code"], "timeout");

    demo.maximize();
    wait_until_presentable(&mut connection, window_id, true, Duration::from_secs(10));
    let recovered_change = exchange(
        &mut connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "maximized-change",
            "type": "wait",
            "window_id": window_id,
            "generation": generation,
            "after_revision": before_minimize_revision,
            "timeout_ms": PRESENT_TIMEOUT_MS,
        }),
    );
    assert_success(&recovered_change, "maximized-change");
    assert_eq!(recovered_change["outcome"], "changed");
    let recovered_revision = recovered_change["window"]["revision"]
        .as_u64()
        .expect("maximized revision");
    assert!(recovered_revision > before_minimize_revision);

    let recovered_present = exchange(
        &mut connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "maximized-present",
            "type": "wait",
            "window_id": window_id,
            "generation": generation,
            "presented_revision": recovered_revision,
            "timeout_ms": PRESENT_TIMEOUT_MS,
        }),
    );
    assert_success(&recovered_present, "maximized-present");
    assert_eq!(recovered_present["outcome"], "presented");

    let recovered = exchange(
        &mut connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "snapshot-recovered",
            "type": "snapshot",
            "window_id": window_id,
        }),
    );
    assert_success(&recovered, "snapshot-recovered");
    assert_eq!(
        node_by_automation_id(&recovered["snapshot"], "home-count-value")["name"],
        "计数: 2"
    );

    drop(connection);
    demo.close_and_wait();
}
