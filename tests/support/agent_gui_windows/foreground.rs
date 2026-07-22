//! 真实 Win32 前台输入、窗口定位与桌面合成像素验收。

use std::collections::HashSet;
use std::fs::File;
use std::io::BufReader;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BitBlt, ClientToScreen, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC,
    ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, CAPTUREBLT, DIB_RGB_COLORS,
    HBITMAP, HDC, HGDIOBJ, SRCCOPY,
};
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, SetActiveWindow, SetFocus, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT,
    KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN,
    MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_VIRTUALDESK, MOUSEINPUT, MOUSE_EVENT_FLAGS,
    VIRTUAL_KEY, VK_MENU,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, EnumWindows, GetCursorPos, GetForegroundWindow, GetSystemMetrics,
    GetWindowRect, GetWindowThreadProcessId, IsWindowVisible, PeekMessageW, SendMessageW,
    SetForegroundWindow, SetWindowPos, SwitchToThisWindow, HWND_TOPMOST, MSG, PM_NOREMOVE,
    SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_SHOWWINDOW, WM_SETTINGCHANGE,
};

use super::{
    assert_success, exchange, node_by_automation_id, perform_until_presentable, DemoProcess,
    PRESENT_TIMEOUT_MS,
};

#[link(name = "dwmapi")]
extern "system" {
    fn DwmFlush() -> i32;
}

const HKEY_CURRENT_USER: *mut std::ffi::c_void = 0x8000_0001usize as *mut std::ffi::c_void;
const KEY_QUERY_VALUE: u32 = 0x0001;
const KEY_SET_VALUE: u32 = 0x0002;
const REG_DWORD: u32 = 4;
const PERSONALIZE_KEY: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize";
const APPS_USE_LIGHT_THEME: &str = "AppsUseLightTheme";

#[link(name = "advapi32")]
extern "system" {
    fn RegOpenKeyExW(
        key: *mut std::ffi::c_void,
        sub_key: *const u16,
        options: u32,
        access: u32,
        result: *mut *mut std::ffi::c_void,
    ) -> i32;
    fn RegQueryValueExW(
        key: *mut std::ffi::c_void,
        value_name: *const u16,
        reserved: *mut u32,
        value_type: *mut u32,
        data: *mut u8,
        data_size: *mut u32,
    ) -> i32;
    fn RegSetValueExW(
        key: *mut std::ffi::c_void,
        value_name: *const u16,
        reserved: u32,
        value_type: u32,
        data: *const u8,
        data_size: u32,
    ) -> i32;
    fn RegCloseKey(key: *mut std::ffi::c_void) -> i32;
}

struct SystemThemePreference {
    original: u32,
    restored: bool,
}

impl SystemThemePreference {
    fn capture() -> Result<Self, String> {
        Ok(Self {
            original: read_apps_use_light_theme()?,
            restored: false,
        })
    }

    fn toggle(&self) -> Result<u32, String> {
        let next = u32::from(self.original == 0);
        write_apps_use_light_theme(next)?;
        let actual = read_apps_use_light_theme()?;
        if actual != next {
            return Err(format!(
                "AppsUseLightTheme write was not observable: expected {next}, got {actual}"
            ));
        }
        Ok(next)
    }

    fn restore(&mut self) -> Result<(), String> {
        write_apps_use_light_theme(self.original)?;
        let actual = read_apps_use_light_theme()?;
        if actual != self.original {
            return Err(format!(
                "AppsUseLightTheme restore was not observable: expected {}, got {actual}",
                self.original
            ));
        }
        self.restored = true;
        Ok(())
    }
}

impl Drop for SystemThemePreference {
    fn drop(&mut self) {
        if !self.restored {
            let _ = write_apps_use_light_theme(self.original);
        }
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn open_personalize_key(access: u32) -> Result<*mut std::ffi::c_void, String> {
    let sub_key = wide(PERSONALIZE_KEY);
    let mut key = std::ptr::null_mut();
    // SAFETY: 伪句柄和 UTF-16 字符串在同步调用期间有效，输出指针指向局部句柄。
    let status = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, sub_key.as_ptr(), 0, access, &mut key) };
    if status != 0 || key.is_null() {
        Err(format!("RegOpenKeyExW Personalize failed: {status}"))
    } else {
        Ok(key)
    }
}

fn read_apps_use_light_theme() -> Result<u32, String> {
    let key = open_personalize_key(KEY_QUERY_VALUE)?;
    let value_name = wide(APPS_USE_LIGHT_THEME);
    let mut value = 0u32;
    let mut value_type = 0u32;
    let mut data_size = std::mem::size_of::<u32>() as u32;
    // SAFETY: key 在本函数持有，所有输出缓冲在同步调用期间有效且长度正确。
    let status = unsafe {
        RegQueryValueExW(
            key,
            value_name.as_ptr(),
            std::ptr::null_mut(),
            &mut value_type,
            (&mut value as *mut u32).cast::<u8>(),
            &mut data_size,
        )
    };
    // SAFETY: key 由本函数成功打开且尚未关闭。
    let _ = unsafe { RegCloseKey(key) };
    if status != 0 || value_type != REG_DWORD || data_size != 4 {
        Err(format!(
            "RegQueryValueExW AppsUseLightTheme failed: status={status}, type={value_type}, size={data_size}"
        ))
    } else {
        Ok(value)
    }
}

fn write_apps_use_light_theme(value: u32) -> Result<(), String> {
    let key = open_personalize_key(KEY_SET_VALUE)?;
    let value_name = wide(APPS_USE_LIGHT_THEME);
    // SAFETY: key 在本函数持有，value 的四字节表示在同步调用期间有效。
    let status = unsafe {
        RegSetValueExW(
            key,
            value_name.as_ptr(),
            0,
            REG_DWORD,
            (&value as *const u32).cast::<u8>(),
            std::mem::size_of::<u32>() as u32,
        )
    };
    // SAFETY: key 由本函数成功打开且尚未关闭。
    let _ = unsafe { RegCloseKey(key) };
    if status == 0 {
        Ok(())
    } else {
        Err(format!("RegSetValueExW AppsUseLightTheme failed: {status}"))
    }
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
    // AttachThreadInput fails when either thread has no message queue. Cargo's
    // test worker is not otherwise a GUI thread, so create its queue first.
    let mut message = MSG::default();
    // SAFETY: the out parameter is valid and PM_NOREMOVE leaves any message queued.
    unsafe {
        let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
    }
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
        SwitchToThisWindow(window, true);
    }

    let fallback_at = Instant::now() + Duration::from_millis(250);
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut alt_unlock_sent = false;
    let mut saved_cursor = POINT::default();
    // SAFETY: the out parameter is valid for the synchronous cursor query.
    let restore_cursor = unsafe { GetCursorPos(&mut saved_cursor) }.is_ok();
    // SAFETY: GetSystemMetrics has no pointer arguments or ownership transfer.
    let (virtual_left, virtual_top, virtual_width, virtual_height) = unsafe {
        (
            GetSystemMetrics(SM_XVIRTUALSCREEN),
            GetSystemMetrics(SM_YVIRTUALSCREEN),
            GetSystemMetrics(SM_CXVIRTUALSCREEN),
            GetSystemMetrics(SM_CYVIRTUALSCREEN),
        )
    };
    assert!(
        virtual_width > 1 && virtual_height > 1,
        "foreground handshake requires a non-empty virtual desktop"
    );
    let absolute_move = MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE | MOUSEEVENTF_VIRTUALDESK;
    while foreground_window() != window && Instant::now() < deadline {
        // The Codex host may reclaim activation while streaming tool output.
        // Keep requesting the explicit task switch throughout this bounded
        // handshake instead of assuming one successful call remains active.
        unsafe {
            let _ = BringWindowToTop(window);
            let _ = SetForegroundWindow(window);
            SwitchToThisWindow(window, true);
        }
        if !alt_unlock_sent && Instant::now() >= fallback_at {
            // Windows permits the next SetForegroundWindow after the user
            // presses Alt. SendInput is real desktop input from this test
            // process, so this is the documented fallback when the current
            // foreground process has not granted activation to Cargo.
            let inputs = [
                keyboard_input(VK_MENU, KEYBD_EVENT_FLAGS(0)),
                keyboard_input(VK_MENU, KEYEVENTF_KEYUP),
            ];
            // SAFETY: INPUT array is valid for the duration of the synchronous call.
            let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
            assert_eq!(sent as usize, inputs.len(), "send Alt activation handshake");
            let mut window_rect = RECT::default();
            // SAFETY: window is a live top-level HWND and the out parameter is valid.
            unsafe { GetWindowRect(window, &mut window_rect) }
                .expect("query demo bounds for foreground click handshake");
            let click_x = window_rect.left.saturating_add(160);
            let click_y = window_rect.top.saturating_add(20);
            let click = [
                mouse_input(
                    normalized_virtual_pointer(click_x, virtual_left, virtual_width),
                    normalized_virtual_pointer(click_y, virtual_top, virtual_height),
                    absolute_move,
                ),
                mouse_input(0, 0, MOUSEEVENTF_LEFTDOWN),
                mouse_input(0, 0, MOUSEEVENTF_LEFTUP),
            ];
            // SAFETY: INPUT array is valid for the duration of the synchronous call.
            let clicked = unsafe { SendInput(&click, std::mem::size_of::<INPUT>() as i32) };
            assert_eq!(
                clicked as usize,
                click.len(),
                "send foreground click handshake"
            );
            // SAFETY: target HWND remains live and input queues are still attached.
            unsafe {
                let _ = BringWindowToTop(window);
                let _ = SetForegroundWindow(window);
                let _ = SetFocus(Some(window));
                SwitchToThisWindow(window, true);
            }
            alt_unlock_sent = true;
        }
        thread::sleep(Duration::from_millis(25));
    }

    unsafe {
        // SAFETY: detach exactly the successful input-queue attachments above.
        if attached_target {
            let _ = AttachThreadInput(current_thread, target_thread, false);
        }
        if attached_foreground {
            let _ = AttachThreadInput(current_thread, foreground_thread, false);
        }
        if restore_cursor {
            let restore = [mouse_input(
                normalized_virtual_pointer(saved_cursor.x, virtual_left, virtual_width),
                normalized_virtual_pointer(saved_cursor.y, virtual_top, virtual_height),
                absolute_move,
            )];
            let restored = SendInput(&restore, std::mem::size_of::<INPUT>() as i32);
            assert_eq!(
                restored as usize,
                restore.len(),
                "restore cursor after foreground click handshake"
            );
        }
    }
    assert_eq!(
        foreground_window(),
        window,
        "demo HWND did not become the foreground window"
    );
}

fn mouse_input(dx: i32, dy: i32, flags: MOUSE_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: 0,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn normalized_virtual_pointer(position: i32, origin: i32, extent: i32) -> i32 {
    let span = extent.saturating_sub(1).max(1);
    let offset = position.saturating_sub(origin).clamp(0, span);
    ((i64::from(offset) * 65_535) / i64::from(span)) as i32
}

struct WindowSearch {
    process_id: u32,
    window: Option<HWND>,
    score: i64,
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
    let mut bounds = RECT::default();
    // SAFETY: HWND is supplied by EnumWindows and `bounds` is a valid out parameter.
    let has_window_bounds = unsafe { GetWindowRect(window, &mut bounds) }.is_ok();
    // A process may own hidden helper windows. Prefer its visible drawable
    // main window, then retain the largest drawable window while it is hidden.
    let width = bounds.right.saturating_sub(bounds.left);
    let height = bounds.bottom.saturating_sub(bounds.top);
    if process_id == search.process_id && has_window_bounds && width >= 100 && height >= 100 {
        let area = i64::from(width) * i64::from(height);
        let visible_priority = i64::from(unsafe { IsWindowVisible(window) }.as_bool()) << 48;
        let score = visible_priority.saturating_add(area);
        if score > search.score {
            search.window = Some(window);
            search.score = score;
        }
    }
    BOOL(1)
}

pub(super) fn find_process_window(process_id: u32) -> Option<HWND> {
    let mut search = WindowSearch {
        process_id,
        window: None,
        score: -1,
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

pub(super) fn capture_demo_client_png(demo: &DemoProcess, path: &Path) {
    let window = demo.window_handle();
    // Screenshot scenarios are driven through the agent protocol. Raising the
    // real HWND above the desktop and flushing DWM is sufficient for a screen
    // pixel oracle; exact keyboard foreground ownership is separately proved
    // by the default Vulkan interaction test.
    demo.raise_for_interaction();
    flush_desktop_composition();
    let capture = capture_client(window);

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create GUI evidence directory");
    }
    let mut rgba = Vec::with_capacity(capture.pixels.len().saturating_mul(4));
    for &pixel in &capture.pixels {
        let [blue, green, red, _] = pixel.to_le_bytes();
        rgba.extend_from_slice(&[red, green, blue, 255]);
    }
    image::save_buffer_with_format(
        path,
        &rgba,
        capture.width as u32,
        capture.height as u32,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    )
    .expect("save GUI evidence PNG");
    assert_meaningful_capture(&capture, "demo visual evidence");
}

pub(super) fn resize_demo_window(demo: &DemoProcess, width: i32, height: i32) {
    let window = demo.window_handle();
    demo.raise_for_interaction();
    // SAFETY: 仅调整仍存活测试窗口的外框尺寸，并保持现有前景测试约束。
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
    .expect("resize demo window for visual acceptance");
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

fn changed_pixel_ratio(left: &ClientCapture, right: &ClientCapture) -> f64 {
    assert_eq!(
        (left.width, left.height),
        (right.width, right.height),
        "pixel captures must have the same client extent"
    );
    let changed = left
        .pixels
        .iter()
        .zip(&right.pixels)
        .filter(|(left, right)| (*left ^ *right) & 0x00FF_FFFF != 0)
        .count();
    changed as f64 / left.pixels.len().max(1) as f64
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

fn wait_for_changed_and_presented(
    connection: &mut BufReader<File>,
    window_id: u64,
    generation: u64,
    after_revision: u64,
    request_prefix: &str,
) -> u64 {
    let changed_id = format!("{request_prefix}-changed");
    let changed = exchange(
        connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": changed_id,
            "type": "wait",
            "window_id": window_id,
            "generation": generation,
            "after_revision": after_revision,
            "timeout_ms": PRESENT_TIMEOUT_MS,
        }),
    );
    assert_success(&changed, &changed_id);
    assert_eq!(changed["outcome"], "changed");
    let revision = changed["window"]["revision"]
        .as_u64()
        .expect("changed revision");
    wait_for_presented(
        connection,
        window_id,
        generation,
        revision,
        &format!("{request_prefix}-presented"),
    );
    revision
}

fn current_revision(connection: &mut BufReader<File>, window_id: u64, request_id: &str) -> u64 {
    let snapshot = exchange(
        connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": request_id,
            "type": "snapshot",
            "window_id": window_id,
        }),
    );
    assert_success(&snapshot, request_id);
    snapshot["snapshot"]["revision"]
        .as_u64()
        .expect("snapshot revision")
}

fn notify_system_theme_changed(window: HWND) {
    // SAFETY: window 属于仍存活的测试进程；消息不携带借用指针，仅要求其重新读取系统主题。
    unsafe { SendMessageW(window, WM_SETTINGCHANGE, Some(WPARAM(0)), Some(LPARAM(0))) };
}

pub(super) fn verify_system_theme_follow(
    demo: &DemoProcess,
    connection: &mut BufReader<File>,
    window_id: u64,
    generation: u64,
) {
    let window = demo.window_handle();
    demo.raise_for_interaction();
    request_foreground_focus(window);
    flush_desktop_composition();
    let baseline = capture_client(window);
    assert_meaningful_capture(&baseline, "system theme baseline");

    let mut preference = SystemThemePreference::capture().expect("read AppsUseLightTheme");
    let original = preference.original;
    let revision = current_revision(connection, window_id, "system-theme-before-toggle");
    let toggled_value = preference.toggle().expect("toggle AppsUseLightTheme");
    notify_system_theme_changed(window);
    let toggled_revision = wait_for_changed_and_presented(
        connection,
        window_id,
        generation,
        revision,
        "system-theme-toggle",
    );
    demo.raise_for_interaction();
    request_foreground_focus(window);
    flush_desktop_composition();
    let toggled = capture_client(window);
    assert_meaningful_capture(&toggled, "system theme toggled");
    let toggled_ratio = changed_pixel_ratio(&baseline, &toggled);

    preference.restore().expect("restore AppsUseLightTheme");
    notify_system_theme_changed(window);
    let _ = wait_for_changed_and_presented(
        connection,
        window_id,
        generation,
        toggled_revision,
        "system-theme-restore",
    );
    demo.raise_for_interaction();
    request_foreground_focus(window);
    flush_desktop_composition();
    let restored = capture_client(window);
    assert_meaningful_capture(&restored, "system theme restored");
    let restored_ratio = changed_pixel_ratio(&baseline, &restored);

    assert!(
        toggled_ratio >= 0.10,
        "system theme signal changed only {:.2}% of foreground pixels",
        toggled_ratio * 100.0
    );
    assert!(
        restored_ratio <= 0.05 && restored_ratio < toggled_ratio * 0.5,
        "restored system theme still differs by {:.2}% (toggle delta {:.2}%)",
        restored_ratio * 100.0,
        toggled_ratio * 100.0
    );
    println!(
        "system theme capture: AppsUseLightTheme {original} -> {toggled_value} -> {original}; delta {:.2}%, restored {:.2}%",
        toggled_ratio * 100.0,
        restored_ratio * 100.0
    );
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
    let before_layout = assert_shell_fills_snapshot(&snapshot["snapshot"], "before resize");
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
    let resized_snapshot = exchange(
        connection,
        json!({
            "schema": "uix.agent.v1",
            "request_id": "capture-after-resize",
            "type": "snapshot",
            "window_id": window_id,
        }),
    );
    assert_success(&resized_snapshot, "capture-after-resize");
    let resized_layout = assert_shell_fills_snapshot(&resized_snapshot["snapshot"], "after resize");
    assert_ne!(
        (before_layout.0, before_layout.1),
        (resized_layout.0, resized_layout.1),
        "root logical viewport must change after the native client resize"
    );
    assert_ne!(
        (before_layout.2, before_layout.3),
        (resized_layout.2, resized_layout.3),
        "page scroll viewport must reflow after the native client resize"
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

pub(super) fn verify_pointer_and_keyboard_focus_visuals(
    demo: &DemoProcess,
    connection: &mut BufReader<File>,
    window_id: u64,
    generation: u64,
    snapshot: &Value,
) {
    let window = demo.window_handle();
    let bounds = &node_by_automation_id(snapshot, "sidebar-page-0")["visible_bounds"];
    let x = bounds["x"].as_f64().expect("home nav x");
    let y = bounds["y"].as_f64().expect("home nav y");
    let w = bounds["w"].as_f64().expect("home nav width");
    let h = bounds["h"].as_f64().expect("home nav height");
    let region = (x, y, w, h);

    demo.raise_for_interaction();
    request_foreground_focus(window);
    flush_desktop_composition();
    let idle = capture_client(window);

    let pointer = perform_until_presentable(
        demo,
        connection,
        window_id,
        generation,
        "focus-visual-pointer",
        None,
        json!({ "kind": "click_at", "x": x + w * 0.5, "y": y + h * 0.5 }),
    );
    wait_for_presented(
        connection,
        window_id,
        generation,
        pointer["revision"].as_u64().expect("pointer revision"),
        "focus-visual-pointer-presented",
    );
    demo.raise_for_interaction();
    request_foreground_focus(window);
    flush_desktop_composition();
    let pointer_focused = capture_client(window);
    let pointer_delta = capture_region_change_ratio(&idle, &pointer_focused, region);
    assert!(
        pointer_delta <= 0.02,
        "pointer focus changed {:.2}% of the active navigation item; no focus ring was expected",
        pointer_delta * 100.0
    );

    let keyboard = perform_until_presentable(
        demo,
        connection,
        window_id,
        generation,
        "focus-visual-keyboard",
        Some(json!({ "automation_id": "sidebar-page-0" })),
        json!({ "kind": "focus" }),
    );
    wait_for_presented(
        connection,
        window_id,
        generation,
        keyboard["revision"].as_u64().expect("keyboard revision"),
        "focus-visual-keyboard-presented",
    );
    demo.raise_for_interaction();
    request_foreground_focus(window);
    flush_desktop_composition();
    let keyboard_focused = capture_client(window);
    let keyboard_delta = capture_region_change_ratio(&pointer_focused, &keyboard_focused, region);
    assert!(
        keyboard_delta >= 0.02,
        "keyboard focus changed only {:.2}% of the navigation item; focus ring must remain visible",
        keyboard_delta * 100.0
    );
    println!(
        "focus visual capture: pointer delta {:.2}% -> keyboard delta {:.2}%",
        pointer_delta * 100.0,
        keyboard_delta * 100.0
    );
}

fn capture_region_change_ratio(
    left: &ClientCapture,
    right: &ClientCapture,
    region: (f64, f64, f64, f64),
) -> f64 {
    assert_eq!(
        (left.width, left.height),
        (right.width, right.height),
        "regional captures must have the same client extent"
    );
    let x0 = region.0.floor().max(0.0) as usize;
    let y0 = region.1.floor().max(0.0) as usize;
    let x1 = (region.0 + region.2).ceil().max(0.0) as usize;
    let y1 = (region.1 + region.3).ceil().max(0.0) as usize;
    let width = left.width.max(1) as usize;
    let height = left.height.max(1) as usize;
    let x1 = x1.min(width);
    let y1 = y1.min(height);
    assert!(x0 < x1 && y0 < y1, "focus region must be visible");

    let mut changed = 0usize;
    let mut total = 0usize;
    for y in y0.min(height)..y1 {
        let row = y * width;
        for x in x0.min(width)..x1 {
            total += 1;
            if (left.pixels[row + x] ^ right.pixels[row + x]) & 0x00FF_FFFF != 0 {
                changed += 1;
            }
        }
    }
    changed as f64 / total.max(1) as f64
}

fn assert_shell_fills_snapshot(snapshot: &Value, label: &str) -> (f64, f64, f64, f64) {
    let nodes = snapshot["nodes"]
        .as_array()
        .expect("semantic snapshot nodes");
    let root = nodes
        .iter()
        .find(|node| node["parent"].is_null())
        .expect("semantic root node");
    let root_bounds = &root["visible_bounds"];
    let root_w = root_bounds["w"].as_f64().expect("root visible width");
    let root_h = root_bounds["h"].as_f64().expect("root visible height");
    assert!(root_w > 0.0 && root_h > 0.0, "{label}: empty root bounds");
    assert_eq!(root_bounds["x"].as_f64(), Some(0.0), "{label}: root x");
    assert_eq!(root_bounds["y"].as_f64(), Some(0.0), "{label}: root y");

    let title_bar = &node_by_automation_id(snapshot, "window-titlebar")["visible_bounds"];
    assert_eq!(title_bar["x"].as_f64(), Some(0.0), "{label}: title x");
    assert_eq!(title_bar["y"].as_f64(), Some(0.0), "{label}: title y");
    assert!(
        (title_bar["w"].as_f64().expect("title width") - root_w).abs() < 0.5,
        "{label}: title bar must span root width"
    );
    assert!(
        title_bar["h"].as_f64().expect("title height") > 0.0,
        "{label}: empty title bar"
    );

    let status = &node_by_automation_id(snapshot, "app-status-bar")["visible_bounds"];
    let status_bottom =
        status["y"].as_f64().expect("status y") + status["h"].as_f64().expect("status height");
    assert!(
        (status_bottom - root_h).abs() < 0.5,
        "{label}: status bar bottom {status_bottom} must meet root bottom {root_h}"
    );

    let page = &node_by_automation_id(snapshot, "page-scroll-0")["visible_bounds"];
    let page_w = page["w"].as_f64().expect("page visible width");
    let page_h = page["h"].as_f64().expect("page visible height");
    assert!(page_w > 0.0 && page_h > 0.0, "{label}: empty page viewport");
    (root_w, root_h, page_w, page_h)
}
