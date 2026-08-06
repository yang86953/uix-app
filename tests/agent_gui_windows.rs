#![cfg(all(windows, feature = "agent-control"))]
#![allow(
    clippy::expect_used,
    clippy::too_many_arguments,
    reason = "this assertion-heavy GUI test crate fails fast with scenario-specific diagnostics"
)]

#[path = "support/agent_gui_windows/accessibility.rs"]
mod accessibility;
#[path = "support/agent_gui_windows/component_visual.rs"]
mod component_visual;
#[path = "../demo/gui-demo/src/demos/component_qa/manifest.rs"]
mod component_visual_manifest;
#[path = "support/agent_gui_windows/foreground.rs"]
mod foreground;
#[path = "support/agent_gui_windows/framework.rs"]
mod framework;
#[path = "support/agent_gui_windows/graphics_recovery.rs"]
mod graphics_recovery;
#[path = "support/agent_gui_windows/multi_window.rs"]
mod multi_window;
// 单独运行大字号旋转水印，验证 MSDF 真窗 present 边界。
#[path = "support/agent_gui_windows/msdf_visual.rs"]
mod msdf_visual;
#[path = "support/agent_gui_windows/overlay_performance.rs"]
mod overlay_performance;
#[path = "support/agent_gui_windows/system_theme.rs"]
mod system_theme;
#[path = "support/agent_gui_windows/text_components.rs"]
mod text_components;
#[path = "support/agent_gui_windows/visual.rs"]
mod visual;

use std::cell::Cell;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::windows::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use windows::Win32::Foundation::{HWND, LPARAM, RECT, WPARAM};
use windows::Win32::UI::HiDpi::{
    GetDpiForWindow, SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, VK_TAB,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetClientRect, GetWindowLongW, IsIconic, IsWindowVisible, IsZoomed, PostMessageW, SetWindowPos,
    ShowWindowAsync, GWL_STYLE, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SWP_SHOWWINDOW, SW_HIDE, SW_MAXIMIZE, SW_MINIMIZE, SW_SHOW, WM_CLOSE, WS_CAPTION,
    WS_THICKFRAME,
};

use foreground::{find_process_window, keyboard_input, request_foreground_focus};

const START_TIMEOUT: Duration = Duration::from_secs(45);
const PRESENT_TIMEOUT_MS: u64 = 30_000;
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(10);
const COMPONENT_VISUAL_CASE_COUNT: usize = component_visual_manifest::COMPONENT_VISUAL_CASE_COUNT;
const EXPECT_DPI_ENV: &str = "UIX_EXPECT_DPI";
const GUI_EVIDENCE_DIR_ENV: &str = "UIX_GUI_EVIDENCE_DIR";

#[derive(Debug, Clone, Copy)]
struct GraphicsExpectation {
    evidence_label: &'static str,
    backend_override: Option<&'static str>,
    selected_recipe: Option<&'static str>,
    present_occlusion: Option<&'static str>,
    // 记录当前原生 adapter 在启动日志中的稳定识别标记。
    adapter_marker: Option<&'static str>,
    software_fallback_request: Option<&'static str>,
    run_foreground_visual_oracles: bool,
}

// 描述 Windows 默认生产 D3D11 路径的真窗验证期望。
const DEFAULT_D3D11_GRAPHICS: GraphicsExpectation = GraphicsExpectation {
    evidence_label: "auto-d3d11",
    backend_override: None,
    selected_recipe: Some("backend=d3d11; raster=gpu_native; present=swapchain"),
    present_occlusion: Some("present_status_and_test"),
    adapter_marker: Some("D3d11Context: created"),
    software_fallback_request: None,
    run_foreground_visual_oracles: true,
};

// 描述显式请求 Windows 原生 D3D11 的真窗验证期望。
const FORCED_D3D11_GRAPHICS: GraphicsExpectation = GraphicsExpectation {
    evidence_label: "forced-d3d11",
    backend_override: Some("d3d11"),
    selected_recipe: Some("backend=d3d11; raster=gpu_native; present=swapchain"),
    present_occlusion: Some("present_status_and_test"),
    adapter_marker: Some("D3d11Context: created"),
    software_fallback_request: None,
    run_foreground_visual_oracles: false,
};

// 描述显式请求 Windows OpenGL ES 的真窗验证期望。
#[cfg(feature = "opengles")]
const FORCED_OPENGLES_GRAPHICS: GraphicsExpectation = GraphicsExpectation {
    // OpenGL 验收日志需要与 D3D11 路径分开保存。
    evidence_label: "forced-opengles",
    // 通过环境变量强制选择 OpenGL ES 原生适配器。
    backend_override: Some("opengles"),
    // OpenGL ES 使用共享的 GPU-native swapchain recipe。
    selected_recipe: Some("backend=opengles; raster=gpu_native; present=swapchain"),
    // WGL/EGL 当前没有可靠的逐窗 PresentStatusAndTest 能力。
    present_occlusion: Some("unsupported"),
    // WGL 创建日志提供稳定的适配器识别标记。
    adapter_marker: Some("WglContext: OpenGL ES context created"),
    // 该场景必须保持在 OpenGL ES 原生路径，不能请求 CPU fallback。
    software_fallback_request: None,
    // 故障恢复测试通过 agent-control 交互，不重复前台视觉断言。
    run_foreground_visual_oracles: false,
};
// Production intentionally has no direct `software` backend override. A Metal
// request is valid configuration but has no Windows registry row, so it drives
// the shipping GPU-probe-exhausted -> Renderer::cpu fallback boundary.
const SOFTWARE_FALLBACK_GRAPHICS: GraphicsExpectation = GraphicsExpectation {
    evidence_label: "software-fallback",
    backend_override: Some("metal"),
    selected_recipe: None,
    present_occlusion: None,
    adapter_marker: None,
    software_fallback_request: Some("metal"),
    run_foreground_visual_oracles: false,
};
static REAL_GUI_LOCK: Mutex<()> = Mutex::new(());

fn parse_expected_dpi(value: &str) -> Result<u32, String> {
    let dpi = value
        .parse::<u32>()
        .map_err(|_| format!("{EXPECT_DPI_ENV} must be one of 96, 144, or 192; got {value:?}"))?;
    if matches!(dpi, 96 | 144 | 192) {
        Ok(dpi)
    } else {
        Err(format!(
            "{EXPECT_DPI_ENV} must be one of 96, 144, or 192; got {dpi}"
        ))
    }
}

fn expected_dpi_from_env() -> Option<u32> {
    let value = std::env::var_os(EXPECT_DPI_ENV)?;
    let value = match value.into_string() {
        Ok(value) => value,
        Err(_) => panic!("{EXPECT_DPI_ENV} must contain Unicode decimal digits"),
    };
    match parse_expected_dpi(&value) {
        Ok(dpi) => Some(dpi),
        Err(message) => panic!("{message}"),
    }
}

struct DemoProcess {
    child: Child,
    window: Cell<Option<HWND>>,
    discovery_root: PathBuf,
    discovery_path: PathBuf,
    output_readers: Vec<JoinHandle<String>>,
    graphics: GraphicsExpectation,
}

impl DemoProcess {
    // uix-demo 已迁入独立 workspace（demo/gui-demo），集成测试无法使用
    // CARGO_BIN_EXE_ 编译期变量，改为定位 demo workspace 的构建产物。
    fn demo_binary_path() -> PathBuf {
        let profile = if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        };
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("demo")
            .join("target")
            .join(profile)
            .join(if cfg!(windows) { "uix-demo.exe" } else { "uix-demo" });
        if !path.is_file() {
            panic!(
                "demo 二进制不存在：{}；请先构建：\ncargo build --manifest-path demo/Cargo.toml --features agent-control --bin uix-demo",
                path.display()
            );
        }
        path
    }

    fn spawn(graphics: GraphicsExpectation) -> Self {
        Self::spawn_with_args(graphics, &[])
    }

    fn spawn_with_args(graphics: GraphicsExpectation, extra_args: &[&str]) -> Self {
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

        let mut command = Command::new(Self::demo_binary_path());
        command
            .arg("--agent-control")
            .args(extra_args)
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
            window: Cell::new(None),
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

    fn close_and_wait(&mut self) -> String {
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
        self.assert_graphics_output(&output);
        self.persist_graphics_output(&output);
        output
    }

    fn assert_graphics_output(&self, output: &str) {
        if let Some(recipe) = self.graphics.selected_recipe {
            let Some(present_occlusion) = self.graphics.present_occlusion else {
                panic!("GPU graphics expectation must declare present occlusion support");
            };
            let selected = format!(
                "Graphics bootstrap: selected recipe {recipe}; present_occlusion={present_occlusion}"
            );
            assert!(
                output.contains(&selected),
                "demo did not select graphics recipe `{recipe}` with present occlusion `{present_occlusion}`; output={output}"
            );
            let Some(adapter_marker) = self.graphics.adapter_marker else {
                panic!("GPU graphics expectation must declare its native adapter marker");
            };
            let adapter_line = output
                .lines()
                .find(|line| line.contains(adapter_marker))
                .unwrap_or_else(|| {
                    panic!("demo did not report native adapter marker `{adapter_marker}`; output={output}")
                });
            assert!(
                !output.contains("fallback=software_cpu"),
                "demo unexpectedly fell back to Software; output={output}"
            );
            eprintln!(
                "graphics acceptance evidence: path={}; recipe={recipe}; present_occlusion={present_occlusion}; {}",
                self.graphics.evidence_label,
                adapter_line.trim()
            );
            return;
        }

        let Some(request) = self.graphics.software_fallback_request else {
            panic!("non-GPU graphics expectation must declare a fallback request");
        };
        let fallback = format!(
            "GPU probe exhausted; request={request}; platform=windows; fallback=software_cpu"
        );
        assert!(
            output.contains(&fallback),
            "demo did not reach the production whole-Software fallback for request `{request}`; output={output}"
        );
        assert!(
            output.contains("CPU renderer initialized"),
            "demo did not initialize the CPU Renderer after GPU probe exhaustion; output={output}"
        );
        assert!(
            !output.contains("Graphics bootstrap: selected recipe"),
            "software fallback scenario unexpectedly selected a GPU recipe; output={output}"
        );
        assert!(
            !output.contains("D3d11Context: created"),
            "software fallback scenario unexpectedly created a D3D11 adapter; output={output}"
        );
        eprintln!(
            "graphics acceptance evidence: path={}; request={request}; fallback=software_cpu; renderer=Renderer::cpu",
            self.graphics.evidence_label
        );
    }

    fn persist_graphics_output(&self, output: &str) {
        let Some(root) = std::env::var_os(GUI_EVIDENCE_DIR_ENV) else {
            return;
        };
        let root = PathBuf::from(root);
        fs::create_dir_all(&root).expect("create GUI evidence directory");
        let path = root.join(format!("graphics-{}.log", self.graphics.evidence_label));
        fs::write(&path, output).expect("write graphics acceptance log");
        eprintln!("graphics acceptance log: {}", path.display());
    }

    fn assert_expected_dpi(&self, snapshot: &Value, scenario: &str) {
        let Some(expected_dpi) = expected_dpi_from_env() else {
            return;
        };
        let window = self.window_handle();
        // SAFETY: the HWND belongs to the live child process for the duration of this call.
        let actual_dpi = unsafe { GetDpiForWindow(window) };
        assert_ne!(actual_dpi, 0, "GetDpiForWindow returned zero");
        assert_eq!(
            actual_dpi, expected_dpi,
            "window is on a {actual_dpi} DPI monitor, but {EXPECT_DPI_ENV} requested {expected_dpi}"
        );

        let mut client = RECT::default();
        // GetClientRect must run in a Per-Monitor V2 caller context so its
        // extent is the same physical-pixel contract consumed by the surface.
        // SAFETY: this changes only the current test thread and the returned
        // context is restored below before any assertion can panic.
        let previous =
            unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        assert!(
            !previous.0.is_null(),
            "failed to enter a Per-Monitor V2 DPI context for evidence"
        );
        // SAFETY: `client` is a valid out parameter and the child HWND is still live.
        let client_result = unsafe { GetClientRect(window, &mut client) };
        // SAFETY: `previous` was returned by this thread's successful context switch.
        let restored = unsafe { SetThreadDpiAwarenessContext(previous) };
        assert!(
            !restored.0.is_null(),
            "failed to restore the test thread DPI awareness context"
        );
        if let Err(error) = client_result {
            panic!("GetClientRect for DPI evidence failed: {error}");
        }
        let physical_width = (client.right - client.left).max(0);
        let physical_height = (client.bottom - client.top).max(0);
        assert!(
            physical_width > 0 && physical_height > 0,
            "DPI evidence requires a non-empty client extent"
        );

        let root = snapshot["nodes"]
            .as_array()
            .and_then(|nodes| nodes.iter().find(|node| node["parent"].is_null()))
            .unwrap_or_else(|| panic!("DPI evidence snapshot must contain a semantic root"));
        let logical_width = root["visible_bounds"]["w"]
            .as_f64()
            .unwrap_or_else(|| panic!("DPI evidence root must have a logical width"));
        let logical_height = root["visible_bounds"]["h"]
            .as_f64()
            .unwrap_or_else(|| panic!("DPI evidence root must have a logical height"));
        assert!(
            logical_width > 0.0 && logical_height > 0.0,
            "DPI evidence requires a non-empty logical extent"
        );

        let scale = actual_dpi as f64 / 96.0;
        let expected_physical_width = logical_width * scale;
        let expected_physical_height = logical_height * scale;
        let width_delta = (physical_width as f64 - expected_physical_width).abs();
        let height_delta = (physical_height as f64 - expected_physical_height).abs();
        assert!(
            width_delta <= 2.0 && height_delta <= 2.0,
            "DPI extent mismatch: logical={logical_width:.2}x{logical_height:.2}, physical={physical_width}x{physical_height}, dpi={actual_dpi}, deltas={width_delta:.2}x{height_delta:.2}"
        );

        let evidence = json!({
            "schema": "uix.gui.dpi.v1",
            "graphics_path": self.graphics.evidence_label,
            "scenario": scenario,
            "expected_dpi": expected_dpi,
            "actual_dpi": actual_dpi,
            "logical_extent": {
                "width": logical_width,
                "height": logical_height,
            },
            "physical_extent": {
                "width": physical_width,
                "height": physical_height,
            },
        });
        eprintln!("DPI acceptance evidence: {evidence}");
        if let Some(root) = std::env::var_os(GUI_EVIDENCE_DIR_ENV) {
            let root = PathBuf::from(root);
            fs::create_dir_all(&root).expect("create GUI evidence directory");
            let path = root.join(format!(
                "dpi-{}-{scenario}.json",
                self.graphics.evidence_label
            ));
            let bytes = serde_json::to_vec_pretty(&evidence).expect("serialize DPI evidence");
            fs::write(&path, bytes).expect("write DPI acceptance evidence");
            eprintln!("DPI acceptance record: {}", path.display());
        }
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

    fn assert_custom_title_bar_style(&self) {
        let window = self.window_handle();
        let style = unsafe {
            // SAFETY: 查询期间 HWND 属于仍存活的测试子进程。
            GetWindowLongW(window, GWL_STYLE) as u32
        };
        assert_eq!(style & WS_CAPTION.0, 0, "demo must remove system caption");
        assert_ne!(
            style & WS_THICKFRAME.0,
            0,
            "demo must retain the native resize frame"
        );
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

    fn hide(&self) {
        let window = self.window_handle();
        unsafe {
            // SAFETY: 命令只异步隐藏仍存活测试子进程的 HWND。
            let _ = ShowWindowAsync(window, SW_HIDE);
        }
        self.wait_for_window_state(window, "hidden", |window| unsafe {
            // SAFETY: 查询期间 HWND 属于仍存活的测试子进程。
            !IsWindowVisible(window).as_bool()
        });
    }

    fn show(&self) {
        let window = self.window_handle();
        unsafe {
            // SAFETY: 命令只异步显示仍存活测试子进程的 HWND。
            let _ = ShowWindowAsync(window, SW_SHOW);
        }
        self.wait_for_window_state(window, "visible", |window| unsafe {
            // SAFETY: 查询期间 HWND 属于仍存活的测试子进程。
            IsWindowVisible(window).as_bool()
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
        if let Some(window) = self.window.get() {
            return window;
        }
        let deadline = Instant::now() + SHUTDOWN_TIMEOUT;
        loop {
            if let Some(window) = find_process_window(self.child.id()) {
                self.window.set(Some(window));
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

impl Drop for DemoProcess {
    fn drop(&mut self) {
        let panicking = thread::panicking();
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
        let mut failure_output = String::new();
        for reader in std::mem::take(&mut self.output_readers) {
            if let Ok(output) = reader.join() {
                failure_output.push_str(&output);
            }
        }
        if panicking && !failure_output.is_empty() {
            eprintln!("uix-demo output during failed GUI test:\n{failure_output}");
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
    let trace = std::env::var_os("UIX_AGENT_TRACE_REQUESTS").is_some();
    if trace {
        eprintln!(
            "agent request -> {}",
            request["request_id"].as_str().unwrap_or("<missing>")
        );
    }
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
    let response: Value = serde_json::from_str(&response).expect("parse protocol response");
    if trace {
        eprintln!("agent response <- {}", response["request_id"]);
    }
    response
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

fn assert_custom_title_bar_nodes(snapshot: &Value) {
    for (automation_id, accessible_name) in [
        ("window-control-minimize", "最小化窗口"),
        ("window-control-maximize-restore", "最大化或还原窗口"),
        ("window-control-close", "关闭窗口"),
    ] {
        let node = node_by_automation_id(snapshot, automation_id);
        assert_eq!(node["role"], "button");
        assert_eq!(node["name"], accessible_name);
        assert!(node["actions"]
            .as_array()
            .is_some_and(|actions| actions.contains(&json!("invoke"))));
    }
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
#[ignore = "requires an interactive Windows desktop and D3D11 driver"]
fn real_default_auto_d3d11_gui_presents_and_recovers_from_minimize() {
    run_real_gui_scenario(DEFAULT_D3D11_GRAPHICS);
}

#[test]
#[ignore = "requires an interactive Windows desktop and D3D11 driver"]
fn real_forced_d3d11_gui_presents_and_recovers_from_minimize() {
    run_real_gui_scenario(FORCED_D3D11_GRAPHICS);
}

#[test]
#[ignore = "requires an interactive Windows desktop and validates production Software fallback"]
fn real_whole_software_fallback_gui_presents_and_recovers_from_minimize() {
    run_real_gui_scenario(SOFTWARE_FALLBACK_GRAPHICS);
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
    demo.assert_custom_title_bar_style();

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
    assert_custom_title_bar_nodes(&before["snapshot"]);
    demo.assert_expected_dpi(&before["snapshot"], "main-window");
    let maximized = invoke_until_presentable(
        &demo,
        &mut connection,
        window_id,
        generation,
        "window-control-maximize-restore",
    );
    assert_eq!(maximized["settled"], true);
    let hwnd = demo.window_handle();
    demo.wait_for_window_state(hwnd, "maximized", |window| unsafe {
        IsZoomed(window).as_bool()
    });
    let restored = invoke_until_presentable(
        &demo,
        &mut connection,
        window_id,
        generation,
        "window-control-maximize-restore",
    );
    assert_eq!(restored["settled"], true);
    demo.wait_for_window_state(hwnd, "restored", |window| unsafe {
        !IsZoomed(window).as_bool()
    });
    let increment = node_by_automation_id(&before["snapshot"], "home-count-increment");
    assert!(increment["actions"]
        .as_array()
        .is_some_and(|actions| actions.contains(&json!("invoke"))));
    assert_eq!(
        node_by_automation_id(&before["snapshot"], "home-count-value")["name"],
        "计数: 0"
    );
    if graphics.run_foreground_visual_oracles {
        foreground::verify_pointer_and_keyboard_focus_visuals(
            &demo,
            &mut connection,
            window_id,
            generation,
            &before["snapshot"],
        );
        foreground::verify_theme_and_resize_capture(&demo, &mut connection, window_id, generation);
    }

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
    if graphics.run_foreground_visual_oracles {
        demo.send_system_tab();
    } else {
        let tabbed = perform_until_presentable(
            &demo,
            &mut connection,
            window_id,
            generation,
            "portable-tab-forward",
            None,
            json!({ "kind": "press_key", "key": "tab", "modifiers": [] }),
        );
        assert_eq!(tabbed["settled"], true);
    }
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

    let (feedback_x, feedback_y) = visible_center(node_by_automation_id(
        &recovered["snapshot"],
        "sidebar-page-7",
    ));
    let opened_feedback = perform_until_presentable(
        &demo,
        &mut connection,
        window_id,
        generation,
        "open-feedback-page",
        None,
        json!({ "kind": "click_at", "x": feedback_x, "y": feedback_y }),
    );
    assert_eq!(opened_feedback["settled"], true);
    let feedback = exchange(
        &mut connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "snapshot-feedback",
            "type": "snapshot",
            "window_id": window_id,
        }),
    );
    assert_success(&feedback, "snapshot-feedback");
    let (modal_x, modal_y) = visible_center(node_by_automation_id(
        &feedback["snapshot"],
        "feedback-focus-modal",
    ));
    let opened_modal = perform_until_presentable(
        &demo,
        &mut connection,
        window_id,
        generation,
        "open-focus-modal",
        None,
        json!({ "kind": "click_at", "x": modal_x, "y": modal_y }),
    );
    assert_eq!(opened_modal["settled"], true);

    if graphics.run_foreground_visual_oracles {
        demo.send_system_tab();
    } else {
        let modal_tabbed = perform_until_presentable(
            &demo,
            &mut connection,
            window_id,
            generation,
            "portable-modal-tab-forward",
            None,
            json!({ "kind": "press_key", "key": "tab", "modifiers": [] }),
        );
        assert_eq!(modal_tabbed["settled"], true);
    }
    wait_for_focused_node(
        &mut connection,
        window_id,
        "feedback-modal-cancel",
        Duration::from_secs(10),
    );
    // D3D11 reports present_occlusion=present_status_and_test. SW_HIDE validates the
    // production hidden/non-presentable lifecycle only; it is not evidence of
    // true compositor occlusion by another foreground window.
    demo.hide();
    wait_until_presentable(&mut connection, window_id, false, Duration::from_secs(10));
    let hidden_modal = exchange(
        &mut connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "snapshot-hidden-modal",
            "type": "snapshot",
            "window_id": window_id,
        }),
    );
    assert_success(&hidden_modal, "snapshot-hidden-modal");
    assert_eq!(
        node_by_automation_id(&hidden_modal["snapshot"], "feedback-modal-cancel")["focused"],
        true
    );
    let hidden_presented_revision = hidden_modal["snapshot"]["presented_revision"]
        .as_u64()
        .expect("presented revision while hidden");
    let hidden_present = exchange(
        &mut connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "hidden-modal-present",
            "type": "wait",
            "window_id": window_id,
            "generation": generation,
            "presented_revision": hidden_presented_revision + 1,
            "timeout_ms": 250,
        }),
    );
    assert_eq!(hidden_present["ok"], false);
    assert_eq!(hidden_present["error"]["code"], "timeout");

    demo.show();
    wait_until_presentable(&mut connection, window_id, true, Duration::from_secs(10));
    demo.send_system_tab();
    wait_for_focused_node(
        &mut connection,
        window_id,
        "feedback-modal-confirm",
        Duration::from_secs(10),
    );

    drop(connection);
    demo.close_and_wait();
}
