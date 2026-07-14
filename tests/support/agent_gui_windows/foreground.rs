//! 真实 Win32 前台输入、窗口定位与桌面合成像素验收。

use std::collections::HashSet;
use std::fs::File;
use std::io::BufReader;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::json;
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::{
    BitBlt, ClientToScreen, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT, DIB_RGB_COLORS,
    HBITMAP, HDC, HGDIOBJ, SRCCOPY,
};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SetActiveWindow, SetFocus, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
    VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetForegroundWindow, GetWindowRect, GetWindowThreadProcessId,
    SetForegroundWindow, SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_SHOWWINDOW,
};

use super::{assert_success, exchange, perform_until_presentable, DemoProcess, PRESENT_TIMEOUT_MS};

#[link(name = "dwmapi")]
extern "system" {
    fn DwmFlush() -> i32;
}

pub(super) fn keyboard_input(vk: VIRTUAL_KEY, flags: KEYBD_EVENT_FLAGS) -> INPUT {
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

pub(super) fn request_foreground_focus(window: HWND) {
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

struct WindowSearch {
    process_id: u32,
    window: Option<HWND>,
}

unsafe extern "system" fn find_window_callback(window: HWND, context: LPARAM) -> BOOL {
    let search = unsafe {
        // SAFETY: find_process_window 在同步 EnumWindows 调用期间保留该栈对象。
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

pub(super) fn find_process_window(process_id: u32) -> Option<HWND> {
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

struct CaptureResources {
    screen: HDC,
    memory: HDC,
    bitmap: Option<HBITMAP>,
    previous: Option<HGDIOBJ>,
}

impl CaptureResources {
    fn new() -> Self {
        // SAFETY: None 请求桌面 DC；句柄在本对象 Drop 前保持有效。
        let screen = unsafe { GetDC(None) };
        assert!(!screen.is_invalid(), "GetDC desktop failed");
        // SAFETY: screen 是当前线程刚获取的有效桌面 DC。
        let memory = unsafe { CreateCompatibleDC(Some(screen)) };
        if memory.is_invalid() {
            // SAFETY: screen 来自本函数成功的 GetDC(None)，尚未释放。
            let _ = unsafe { ReleaseDC(None, screen) };
            panic!("CreateCompatibleDC failed");
        }
        Self {
            screen,
            memory,
            bitmap: None,
            previous: None,
        }
    }
}

impl Drop for CaptureResources {
    fn drop(&mut self) {
        unsafe {
            // SAFETY: 句柄均由本对象创建；先恢复选入对象，再按逆序释放且只释放一次。
            if let Some(previous) = self.previous.take() {
                let _ = SelectObject(self.memory, previous);
            }
            if let Some(bitmap) = self.bitmap.take() {
                let _ = DeleteObject(bitmap.into());
            }
            let _ = DeleteDC(self.memory);
            let _ = ReleaseDC(None, self.screen);
        }
    }
}

struct ClientCapture {
    width: i32,
    height: i32,
    pixels: Vec<u32>,
}

fn capture_client(window: HWND) -> ClientCapture {
    let mut client = RECT::default();
    // SAFETY: window 属于仍存活的测试子进程，输出指针指向有效 RECT。
    unsafe { windows::Win32::UI::WindowsAndMessaging::GetClientRect(window, &mut client) }
        .expect("GetClientRect");
    let width = (client.right - client.left).max(1);
    let height = (client.bottom - client.top).max(1);
    let mut origin = POINT::default();
    // SAFETY: window 有效，origin 是可写的局部 POINT。
    assert!(unsafe { ClientToScreen(window, &mut origin) }.as_bool());

    let mut resources = CaptureResources::new();
    let mut bits = std::ptr::null_mut();
    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width,
            biHeight: -height,
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    // SAFETY: info 在调用期间有效，bits 接收由 bitmap 拥有且在资源释放前有效的映射地址。
    let bitmap = unsafe {
        CreateDIBSection(
            Some(resources.screen),
            &info,
            DIB_RGB_COLORS,
            &mut bits,
            None,
            0,
        )
    }
    .expect("CreateDIBSection");
    assert!(!bits.is_null(), "CreateDIBSection returned no bits");
    resources.bitmap = Some(bitmap);
    // SAFETY: memory DC 与 bitmap 均由当前资源对象持有。
    let previous = unsafe { SelectObject(resources.memory, bitmap.into()) };
    assert!(!previous.is_invalid(), "SelectObject bitmap failed");
    resources.previous = Some(previous);
    // SAFETY: 两个 DC、坐标、尺寸与 DIB 映射在同步复制期间均有效。
    unsafe {
        BitBlt(
            resources.memory,
            0,
            0,
            width,
            height,
            Some(resources.screen),
            origin.x,
            origin.y,
            SRCCOPY | CAPTUREBLT,
        )
    }
    .expect("BitBlt desktop capture");
    let pixel_count = (width as usize).saturating_mul(height as usize);
    // SAFETY: top-down 32-bit DIB 暴露至少 width*height 个连续 u32，bitmap 此时仍存活。
    let pixels = unsafe { std::slice::from_raw_parts(bits.cast::<u32>(), pixel_count) }.to_vec();
    ClientCapture {
        width,
        height,
        pixels,
    }
}

fn flush_desktop_composition() {
    // SAFETY: DwmFlush 无参数；只阻塞到当前 DWM 提交完成。
    let result = unsafe { DwmFlush() };
    assert!(result >= 0, "DwmFlush failed with HRESULT {result:#x}");
}

fn assert_meaningful_capture(capture: &ClientCapture, label: &str) {
    assert!(
        capture.width >= 320 && capture.height >= 240,
        "{label} capture too small: {}x{}",
        capture.width,
        capture.height
    );
    let step = (capture.pixels.len() / 4096).max(1);
    let unique = capture
        .pixels
        .iter()
        .step_by(step)
        .map(|pixel| pixel & 0x00FF_FFFF)
        .collect::<HashSet<_>>();
    assert!(
        unique.len() >= 8,
        "{label} capture lacks visible UI variation: {} sampled colors",
        unique.len()
    );
}

fn wait_for_presented(
    connection: &mut BufReader<File>,
    window_id: u64,
    generation: u64,
    revision: u64,
    request_id: &str,
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

fn resize_window(window: HWND) {
    let mut bounds = RECT::default();
    // SAFETY: window 有效，bounds 是可写的局部 RECT。
    unsafe { GetWindowRect(window, &mut bounds) }.expect("GetWindowRect");
    let current_width = (bounds.right - bounds.left).max(1);
    let current_height = (bounds.bottom - bounds.top).max(1);
    let width = if current_width > 900 {
        current_width - 160
    } else {
        current_width + 160
    };
    let height = if current_height > 700 {
        current_height - 120
    } else {
        current_height + 120
    };
    // SAFETY: 仅调整仍存活测试 HWND 的尺寸并保持其前台 topmost 状态。
    unsafe {
        SetWindowPos(
            window,
            Some(HWND_TOPMOST),
            0,
            0,
            width,
            height,
            SWP_NOMOVE | SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )
    }
    .expect("resize foreground demo window");
}

pub(super) fn verify_theme_and_resize_capture(
    demo: &DemoProcess,
    connection: &mut BufReader<File>,
    window_id: u64,
    generation: u64,
) {
    let window = demo.window_handle();
    demo.raise_for_interaction();
    request_foreground_focus(window);
    flush_desktop_composition();
    let before = capture_client(window);
    assert_meaningful_capture(&before, "before theme");

    let toggled = perform_until_presentable(
        demo,
        connection,
        window_id,
        generation,
        "capture-theme-toggle",
        Some(json!({ "automation_id": "theme-toggle" })),
        json!({ "kind": "invoke" }),
    );
    let theme_revision = toggled["revision"].as_u64().expect("theme revision");
    wait_for_presented(
        connection,
        window_id,
        generation,
        theme_revision,
        "capture-theme-presented",
    );
    demo.raise_for_interaction();
    request_foreground_focus(window);
    flush_desktop_composition();
    let themed = capture_client(window);
    assert_meaningful_capture(&themed, "after theme");
    assert_eq!((themed.width, themed.height), (before.width, before.height));
    let changed_pixels = before
        .pixels
        .iter()
        .zip(&themed.pixels)
        .filter(|(left, right)| (*left ^ *right) & 0x00FF_FFFF != 0)
        .count();
    let changed_ratio = changed_pixels as f64 / before.pixels.len().max(1) as f64;
    assert!(
        changed_ratio >= 0.10,
        "theme toggle changed only {:.2}% of foreground pixels",
        changed_ratio * 100.0
    );

    let snapshot = exchange(
        connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "capture-before-resize",
            "type": "snapshot",
            "window_id": window_id,
        }),
    );
    assert_success(&snapshot, "capture-before-resize");
    let presented_revision = snapshot["snapshot"]["presented_revision"]
        .as_u64()
        .expect("presented revision");
    resize_window(window);
    wait_for_presented(
        connection,
        window_id,
        generation,
        presented_revision + 1,
        "capture-resize-presented",
    );
    demo.raise_for_interaction();
    request_foreground_focus(window);
    flush_desktop_composition();
    let resized = capture_client(window);
    assert_meaningful_capture(&resized, "after resize");
    assert_ne!(
        (resized.width, resized.height),
        (themed.width, themed.height)
    );
    println!(
        "foreground capture: {}x{} -> theme delta {:.2}% -> {}x{}",
        before.width,
        before.height,
        changed_ratio * 100.0,
        resized.width,
        resized.height
    );
}
