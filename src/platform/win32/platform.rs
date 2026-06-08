// ============================================================================
// uix-platform/src/win32/platform.rs — Win32 Platform trait implementation
// ============================================================================
//
// 包含 Platform 主 trait 实现以及所有需要 HWND 的子接口：
//   IClipboard, ICursor, IDisplay, IFileDialog, IKeyboard,
//   ITextInput, ITimer, INotification
// 委托接口 (accessors)：
//   IConsole → Win32Console
//   IFileSystem → Win32FileSystem
//   ISystemInfo → Win32SystemInfo
// ============================================================================

#![cfg(windows)]

use crate::platform::event::*;
use crate::platform::*;
use crate::platform::types::*;
use crate::platform::win32::console::Win32Console;
use crate::platform::win32::filesystem::Win32FileSystem;
use crate::platform::win32::system_info::Win32SystemInfo;
use crate::platform::win32::util::{to_utf8, to_wide, win32_diag};
use crate::diag::{Collector, Errc};

use std::collections::VecDeque;
use std::ptr;

// ════════════════════════════════════════════════════════════════════════════
// Win32Platform — 主平台结构体
// ════════════════════════════════════════════════════════════════════════════

pub struct Win32Platform {
    // ── 窗口句柄与状态 ──────────────────────────────────────────────
    hwnd: *mut std::ffi::c_void,      // HWND
    hinstance: *mut std::ffi::c_void, // HINSTANCE
    class_atom: u16,                  // 注册的窗口类 atom

    width: i32,
    height: i32,
    pos_x: i32,
    pos_y: i32,

    min_w: i32,
    min_h: i32,
    max_w: i32,
    max_h: i32,

    visible: bool,
    resizable: bool,
    borderless: bool,
    fullscreen: bool,
    always_on_top: bool,
    minimized: bool,
    maximized: bool,
    opacity: f32,
    file_drop_enabled: bool,
    text_input_active: bool,

    // ── 事件队列 ──────────────────────────────────────────────────
    event_queue: VecDeque<UiEvent>,

    // ── 计时器 ────────────────────────────────────────────────────
    next_timer_id: u32,

    // ── 通知图标 ──────────────────────────────────────────────────
    notification_active: bool,

    // ── 组合子系统 ────────────────────────────────────────────────
    console: Win32Console,
    file_system: Win32FileSystem,
    system_info: Win32SystemInfo,
}

// ════════════════════════════════════════════════════════════════════════════
// 内部常量
// ════════════════════════════════════════════════════════════════════════════

const DEFAULT_CLASS_STYLE: u32 = 0x0008 | 0x0002 | 0x0001; // CS_HREDRAW | CS_VREDRAW | CS_DBLCLKS
const GWLP_USERDATA: i32 = -21;
const WM_QUIT: u32 = 0x0012;

// ════════════════════════════════════════════════════════════════════════════
// 构造
// ════════════════════════════════════════════════════════════════════════════

impl Win32Platform {
    pub fn new() -> Self {
        Self {
            hwnd: ptr::null_mut(),
            hinstance: ptr::null_mut(),
            class_atom: 0,
            width: 0,
            height: 0,
            pos_x: 0,
            pos_y: 0,
            min_w: 0,
            min_h: 0,
            max_w: 0,
            max_h: 0,
            visible: false,
            resizable: true,
            borderless: false,
            fullscreen: false,
            always_on_top: false,
            minimized: false,
            maximized: false,
            opacity: 1.0,
            file_drop_enabled: false,
            text_input_active: false,
            event_queue: VecDeque::new(),
            next_timer_id: 1,
            notification_active: false,
            console: Win32Console::new(),
            file_system: Win32FileSystem::new(),
            system_info: Win32SystemInfo::new(),
        }
    }
}

impl Default for Win32Platform {
    fn default() -> Self {
        Self::new()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 内部辅助方法
// ════════════════════════════════════════════════════════════════════════════

impl Win32Platform {
    fn class_name(&self) -> Vec<u16> {
        let name = format!("UIX_Win32Platform_{:p}", self.hinstance);
        to_wide(&name)
    }

    fn register_class(&mut self) -> bool {
        unsafe {
            let class_name = self.class_name();

            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: DEFAULT_CLASS_STYLE,
                lpfnWndProc: Some(wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: 0,
                hInstance: self.hinstance,
                hIcon: ptr::null_mut(),
                hCursor: ptr::null_mut(),
                hbrBackground: ptr::null_mut(), // NULL → no automatic background erase
                lpszMenuName: ptr::null(),
                lpszClassName: class_name.as_ptr(),
                hIconSm: ptr::null_mut(),
            };

            let atom = RegisterClassExW(&wc);
            if atom == 0 {
                let err = win32_diag(Errc::ClassRegistrationFailed, "RegisterClassExW failed");
                Collector::instance().collect(err);
                return false;
            }
            self.class_atom = atom;
            true
        }
    }

    fn get_modifier_state() -> KeyMod {
        let mut mods = KeyMod::NONE;
        if is_key_down(VK_SHIFT) {
            mods |= KeyMod::SHIFT;
        }
        if is_key_down(VK_CONTROL) {
            mods |= KeyMod::CTRL;
        }
        if is_key_down(VK_MENU) {
            mods |= KeyMod::ALT;
        }
        mods
    }

    fn vk_to_keycode(vk: u32) -> KeyCode {
        match vk {
            0x41..=0x5A => {
                let idx = (vk - 0x41) as usize;
                const KEYS: [KeyCode; 26] = [
                    KeyCode::A,
                    KeyCode::B,
                    KeyCode::C,
                    KeyCode::D,
                    KeyCode::E,
                    KeyCode::F,
                    KeyCode::G,
                    KeyCode::H,
                    KeyCode::I,
                    KeyCode::J,
                    KeyCode::K,
                    KeyCode::L,
                    KeyCode::M,
                    KeyCode::N,
                    KeyCode::O,
                    KeyCode::P,
                    KeyCode::Q,
                    KeyCode::R,
                    KeyCode::S,
                    KeyCode::T,
                    KeyCode::U,
                    KeyCode::V,
                    KeyCode::W,
                    KeyCode::X,
                    KeyCode::Y,
                    KeyCode::Z,
                ];
                KEYS[idx]
            }
            0x30..=0x39 => {
                let idx = (vk - 0x30) as usize;
                const KEYS: [KeyCode; 10] = [
                    KeyCode::Num0,
                    KeyCode::Num1,
                    KeyCode::Num2,
                    KeyCode::Num3,
                    KeyCode::Num4,
                    KeyCode::Num5,
                    KeyCode::Num6,
                    KeyCode::Num7,
                    KeyCode::Num8,
                    KeyCode::Num9,
                ];
                KEYS[idx]
            }
            0x70..=0x7B => {
                let idx = (vk - 0x70) as usize;
                const KEYS: [KeyCode; 12] = [
                    KeyCode::F1,
                    KeyCode::F2,
                    KeyCode::F3,
                    KeyCode::F4,
                    KeyCode::F5,
                    KeyCode::F6,
                    KeyCode::F7,
                    KeyCode::F8,
                    KeyCode::F9,
                    KeyCode::F10,
                    KeyCode::F11,
                    KeyCode::F12,
                ];
                KEYS[idx]
            }
            VK_LEFT => KeyCode::Left,
            VK_RIGHT => KeyCode::Right,
            VK_UP => KeyCode::Up,
            VK_DOWN => KeyCode::Down,
            VK_RETURN => KeyCode::Enter,
            VK_ESCAPE => KeyCode::Escape,
            VK_BACK => KeyCode::Backspace,
            VK_DELETE => KeyCode::Delete,
            VK_TAB => KeyCode::Tab,
            VK_SPACE => KeyCode::Space,
            VK_INSERT => KeyCode::Insert,
            VK_HOME => KeyCode::Home,
            VK_END => KeyCode::End,
            VK_PRIOR => KeyCode::PageUp,
            VK_NEXT => KeyCode::PageDown,
            VK_SHIFT => KeyCode::Shift,
            VK_CONTROL => KeyCode::Ctrl,
            VK_MENU => KeyCode::Alt,
            VK_LWIN | VK_RWIN => KeyCode::Super,
            _ => KeyCode::Unknown,
        }
    }

    fn hiword(lparam: isize) -> u16 {
        ((lparam >> 16) & 0xFFFF) as u16
    }

    fn loword(lparam: isize) -> u16 {
        (lparam & 0xFFFF) as u16
    }

    fn hiword_usize(wparam: usize) -> u16 {
        ((wparam >> 16) & 0xFFFF) as u16
    }

    fn push_event(&mut self, event: UiEvent) {
        self.event_queue.push_back(event);
    }

    // ════════════════════════════════════════════════════════════════════════════
    // 窗口过程消息处理
    // ════════════════════════════════════════════════════════════════════════════

    fn handle_message(&mut self, msg: u32, wparam: usize, lparam: isize) -> isize {
        match msg {
            WM_CLOSE => {
                self.push_event(UiEvent::close());
                0
            }

            WM_DESTROY => {
                unsafe {
                    PostQuitMessage(0);
                }
                0
            }

            WM_SIZE => {
                let w = Self::loword(lparam) as i32;
                let h = Self::hiword(lparam) as i32;
                self.width = w;
                self.height = h;

                match wparam {
                    SIZE_MINIMIZED => {
                        self.minimized = true;
                        self.push_event(UiEvent {
                            type_: UiEventType::WindowMinimize,
                            payload: UiEventPayload::None,
                        });
                    }
                    SIZE_MAXIMIZED => {
                        self.minimized = false;
                        self.maximized = true;
                        self.push_event(UiEvent {
                            type_: UiEventType::WindowMaximize,
                            payload: UiEventPayload::None,
                        });
                    }
                    SIZE_RESTORED => {
                        let was_min = self.minimized;
                        let was_max = self.maximized;
                        self.minimized = false;
                        self.maximized = false;
                        if was_min || was_max {
                            self.push_event(UiEvent {
                                type_: UiEventType::WindowRestore,
                                payload: UiEventPayload::None,
                            });
                        }
                        self.push_event(UiEvent::resize(w, h));
                    }
                    _ => {
                        self.push_event(UiEvent::resize(w, h));
                    }
                }
                0
            }

            WM_MOVE => {
                self.pos_x = Self::loword(lparam) as i32;
                self.pos_y = Self::hiword(lparam) as i32;
                0
            }

            WM_SETFOCUS => {
                self.push_event(UiEvent {
                    type_: UiEventType::WindowFocus,
                    payload: UiEventPayload::None,
                });
                0
            }

            WM_KILLFOCUS => {
                self.push_event(UiEvent {
                    type_: UiEventType::WindowBlur,
                    payload: UiEventPayload::None,
                });
                0
            }

            WM_KEYDOWN | WM_SYSKEYDOWN => {
                let vk = wparam as u32;
                let key = Self::vk_to_keycode(vk);
                let mods = Self::get_modifier_state();
                self.push_event(UiEvent::key_down(key, mods));
                0
            }

            WM_KEYUP | WM_SYSKEYUP => {
                let vk = wparam as u32;
                let key = Self::vk_to_keycode(vk);
                let mods = Self::get_modifier_state();
                self.push_event(UiEvent::key_up(key, mods));
                0
            }

            WM_CHAR => {
                let ch = wparam as u32 as u8 as char;
                let text = ch.to_string();
                self.push_event(UiEvent::key_press(text));
                0
            }

            WM_LBUTTONDOWN => {
                let pos = self.mouse_pos_from_lparam(lparam);
                let mods = Self::get_modifier_state();
                let mut ev = UiEvent::mouse_down(pos, MouseButton::Left);
                if let UiEventPayload::MouseButton(ref mut data) = ev.payload {
                    data.mods = mods;
                }
                self.push_event(ev);
                unsafe {
                    SetCapture(self.hwnd);
                }
                0
            }

            WM_LBUTTONUP => {
                let pos = self.mouse_pos_from_lparam(lparam);
                let mods = Self::get_modifier_state();
                let mut ev = UiEvent::mouse_up(pos, MouseButton::Left);
                if let UiEventPayload::MouseButton(ref mut data) = ev.payload {
                    data.mods = mods;
                }
                self.push_event(ev);
                unsafe {
                    ReleaseCapture();
                }
                0
            }

            WM_RBUTTONDOWN => {
                let pos = self.mouse_pos_from_lparam(lparam);
                let mods = Self::get_modifier_state();
                let mut ev = UiEvent::mouse_down(pos, MouseButton::Right);
                if let UiEventPayload::MouseButton(ref mut data) = ev.payload {
                    data.mods = mods;
                }
                self.push_event(ev);
                unsafe {
                    SetCapture(self.hwnd);
                }
                0
            }

            WM_RBUTTONUP => {
                let pos = self.mouse_pos_from_lparam(lparam);
                let mods = Self::get_modifier_state();
                let mut ev = UiEvent::mouse_up(pos, MouseButton::Right);
                if let UiEventPayload::MouseButton(ref mut data) = ev.payload {
                    data.mods = mods;
                }
                self.push_event(ev);
                unsafe {
                    ReleaseCapture();
                }
                0
            }

            WM_MBUTTONDOWN => {
                let pos = self.mouse_pos_from_lparam(lparam);
                let mods = Self::get_modifier_state();
                let mut ev = UiEvent::mouse_down(pos, MouseButton::Middle);
                if let UiEventPayload::MouseButton(ref mut data) = ev.payload {
                    data.mods = mods;
                }
                self.push_event(ev);
                unsafe {
                    SetCapture(self.hwnd);
                }
                0
            }

            WM_MBUTTONUP => {
                let pos = self.mouse_pos_from_lparam(lparam);
                let mods = Self::get_modifier_state();
                let mut ev = UiEvent::mouse_up(pos, MouseButton::Middle);
                if let UiEventPayload::MouseButton(ref mut data) = ev.payload {
                    data.mods = mods;
                }
                self.push_event(ev);
                unsafe {
                    ReleaseCapture();
                }
                0
            }

            WM_MOUSEMOVE => {
                let pos = self.mouse_pos_from_lparam(lparam);
                let _mods = Self::get_modifier_state();
                self.push_event(UiEvent::mouse_move(pos));
                0
            }

            WM_MOUSEWHEEL => {
                let screen_pt = POINT {
                    x: Self::loword(lparam) as i32,
                    y: Self::hiword(lparam) as i32,
                };
                let mut client_pt = screen_pt;
                unsafe {
                    ScreenToClient(self.hwnd, &mut client_pt);
                }
                let pos = Point::new(client_pt.x as f32, client_pt.y as f32);
                let delta = (Self::hiword_usize(wparam) as i16) as i32;
                let delta_y = delta as f32 / 120.0;
                let mods = Self::get_modifier_state();
                self.push_event(UiEvent::mouse_wheel(pos, 0.0, delta_y, mods));
                0
            }

            WM_TIMER => {
                let timer_id = wparam as u32;
                self.push_event(UiEvent::timer(timer_id));
                0
            }

            WM_DROPFILES => {
                let hdrop = lparam as *mut std::ffi::c_void;
                self.handle_file_drop(hdrop);
                0
            }

            WM_SETCURSOR => {
                        let hit = Self::loword(lparam) as u32;
                        if hit == HTCLIENT {
                            unsafe {
                                let cursor = LoadCursorW(ptr::null_mut(), IDC_ARROW as *const u16);
                                SetCursor(cursor);
                            }
                            return 1; // TRUE — cursor set
                        }
                        // Fall through to DefWindowProc
                        self.def_window_proc(msg, wparam, lparam)
                    }

            _ => self.def_window_proc(msg, wparam, lparam),
        }
    }

    fn mouse_pos_from_lparam(&self, lparam: isize) -> Point {
        let x = Self::loword(lparam) as i32 as f32;
        let y = Self::hiword(lparam) as i32 as f32;
        Point::new(x, y)
    }

    fn handle_file_drop(&mut self, hdrop: *mut std::ffi::c_void) {
        unsafe {
            let count = DragQueryFileW(hdrop, 0xFFFFFFFF, ptr::null_mut(), 0);
            let mut files = Vec::with_capacity(count as usize);

            // Get cursor position at drop time
            let mut pt = POINT { x: 0, y: 0 };
            DragQueryPoint(hdrop, &mut pt);

            for i in 0..count {
                let len = DragQueryFileW(hdrop, i, ptr::null_mut(), 0);
                if len > 0 {
                    let mut buf = vec![0u16; (len + 1) as usize];
                    DragQueryFileW(hdrop, i, buf.as_mut_ptr(), len + 1);
                    files.push(to_utf8(&buf));
                }
            }

            DragFinish(hdrop);

            let position = Point::new(pt.x as f32, pt.y as f32);
            self.push_event(UiEvent::file_drop(files, position));
        }
    }

    fn def_window_proc(&self, msg: u32, wparam: usize, lparam: isize) -> isize {
        unsafe { DefWindowProcW(self.hwnd, msg, wparam, lparam) }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 窗口过程（extern "system" 回调）
// ════════════════════════════════════════════════════════════════════════════

unsafe extern "system" fn wnd_proc(
    hwnd: *mut std::ffi::c_void,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    if msg == WM_NCCREATE {
        // Extract the Win32Platform pointer from CREATESTRUCTW.lpCreateParams
        let cs = lparam as *const CREATESTRUCTW;
        let this_ptr = (*cs).lpCreateParams;
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, this_ptr as isize);
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }

    // Retrieve the platform pointer
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
    if ptr == 0 {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }

    let platform = &mut *(ptr as *mut Win32Platform);
    platform.handle_message(msg, wparam, lparam)
}

// ════════════════════════════════════════════════════════════════════════════
// Platform trait 实现
// ════════════════════════════════════════════════════════════════════════════

impl Platform for Win32Platform {
    // ── 窗口管理 ──────────────────────────────────────────────────

    fn create_window(&mut self, title: &str, width: i32, height: i32) -> bool {
        unsafe {
            self.hinstance = GetModuleHandleW(ptr::null());
            if self.hinstance.is_null() {
                let err = win32_diag(Errc::PlatformError, "GetModuleHandleW failed");
                Collector::instance().collect(err);
                return false;
            }

            if !self.register_class() {
                // register_class already reports to Collector
                return false;
            }

            self.width = width;
            self.height = height;

            let title_wide = to_wide(title);
            let class_name = self.class_name();

            let style = WS_OVERLAPPEDWINDOW;
            let ex_style = WS_EX_APPWINDOW;

            // Adjust window rect so client area matches requested width/height
            let mut rect = RECT { left: 0, top: 0, right: width, bottom: height };
            AdjustWindowRectEx(&mut rect, style, 0, ex_style);
            let win_w = rect.right - rect.left;
            let win_h = rect.bottom - rect.top;

            let hwnd = CreateWindowExW(
                ex_style,
                class_name.as_ptr(),
                title_wide.as_ptr(),
                style,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                win_w,
                win_h,
                ptr::null_mut(), // no parent
                ptr::null_mut(), // no menu
                self.hinstance,
                self as *mut Win32Platform as *mut std::ffi::c_void,
            );

            if hwnd.is_null() {
                let err = win32_diag(Errc::WindowCreationFailed, "CreateWindowExW failed");
                Collector::instance().collect(err);
                return false;
            }

            self.hwnd = hwnd;
            self.visible = false;
            true
        }
    }

    fn destroy_window(&mut self) {
        if !self.hwnd.is_null() {
            unsafe {
                DestroyWindow(self.hwnd);
            }
            self.hwnd = ptr::null_mut();
        }
        // Unregister class
        if self.class_atom != 0 && !self.hinstance.is_null() {
            unsafe {
                let class_name = self.class_name();
                UnregisterClassW(class_name.as_ptr(), self.hinstance);
            }
            self.class_atom = 0;
        }
    }

    fn native_window(&self) -> *mut std::ffi::c_void {
        self.hwnd
    }

    fn width(&self) -> i32 {
        self.width
    }

    fn height(&self) -> i32 {
        self.height
    }

    fn set_title(&mut self, title: &str) {
        let wide = to_wide(title);
        unsafe {
            SetWindowTextW(self.hwnd, wide.as_ptr());
        }
    }

    fn set_resizable(&mut self, resizable: bool) {
        self.resizable = resizable;
        let style = if resizable {
            WS_OVERLAPPEDWINDOW
        } else {
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX
        };
        unsafe {
            SetWindowLongW(self.hwnd, GWL_STYLE, style);
            SetWindowPos(
                self.hwnd,
                ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
            );
        }
    }

    fn maximize(&mut self) {
        unsafe {
            ShowWindow(self.hwnd, SW_MAXIMIZE);
        }
    }

    fn minimize(&mut self) {
        unsafe {
            ShowWindow(self.hwnd, SW_MINIMIZE);
        }
    }

    fn restore(&mut self) {
        unsafe {
            ShowWindow(self.hwnd, SW_RESTORE);
        }
    }

    fn is_maximized(&self) -> bool {
        self.maximized
    }

    fn is_minimized(&self) -> bool {
        self.minimized
    }

    fn set_position(&mut self, x: i32, y: i32) {
        self.pos_x = x;
        self.pos_y = y;
        unsafe {
            SetWindowPos(
                self.hwnd,
                ptr::null_mut(),
                x,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER,
            );
        }
    }

    fn position(&self) -> Point {
        Point::new(self.pos_x as f32, self.pos_y as f32)
    }

    fn set_size(&mut self, w: i32, h: i32) {
        self.width = w;
        self.height = h;
        unsafe {
            SetWindowPos(
                self.hwnd,
                ptr::null_mut(),
                0,
                0,
                w,
                h,
                SWP_NOMOVE | SWP_NOZORDER,
            );
        }
    }

    fn set_minimum_size(&mut self, w: i32, h: i32) {
        self.min_w = w;
        self.min_h = h;
    }

    fn set_maximum_size(&mut self, w: i32, h: i32) {
        self.max_w = w;
        self.max_h = h;
    }

    fn center_on_screen(&mut self) {
        unsafe {
            let screen_w = GetSystemMetrics(SM_CXSCREEN);
            let screen_h = GetSystemMetrics(SM_CYSCREEN);
            let x = (screen_w - self.width) / 2;
            let y = (screen_h - self.height) / 2;
            self.set_position(x.max(0), y.max(0));
        }
    }

    fn show(&mut self) {
        self.visible = true;
        unsafe {
            ShowWindow(self.hwnd, SW_SHOW);
        }
    }

    fn hide(&mut self) {
        self.visible = false;
        unsafe {
            ShowWindow(self.hwnd, SW_HIDE);
        }
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn set_always_on_top(&mut self, on: bool) {
        self.always_on_top = on;
        let insert_after = if on { HWND_TOPMOST } else { HWND_NOTOPMOST } as *mut std::ffi::c_void;
        unsafe {
            SetWindowPos(self.hwnd, insert_after, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
        }
    }

    fn set_borderless(&mut self, borderless: bool) {
        self.borderless = borderless;
        let style = if borderless {
            WS_POPUP
        } else if self.resizable {
            WS_OVERLAPPEDWINDOW
        } else {
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX
        };
        unsafe {
            SetWindowLongW(self.hwnd, GWL_STYLE, style);
            SetWindowPos(
                self.hwnd,
                ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
            );
            // Refresh
            ShowWindow(self.hwnd, SW_SHOW);
        }
    }

    fn set_fullscreen(&mut self, fullscreen: bool) {
        self.fullscreen = fullscreen;
        if fullscreen {
            // Save current position/size and switch to fullscreen
            unsafe {
                let style = GetWindowLongW(self.hwnd, GWL_STYLE);
                SetWindowLongW(self.hwnd, GWL_STYLE, style & !WS_OVERLAPPEDWINDOW);
                let screen_w = GetSystemMetrics(SM_CXSCREEN);
                let screen_h = GetSystemMetrics(SM_CYSCREEN);
                SetWindowPos(
                    self.hwnd,
                    HWND_TOPMOST as *mut std::ffi::c_void,
                    0,
                    0,
                    screen_w,
                    screen_h,
                    SWP_FRAMECHANGED,
                );
                ShowWindow(self.hwnd, SW_SHOW);
            }
        } else {
            // Restore
            let style = if self.resizable {
                WS_OVERLAPPEDWINDOW
            } else {
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX
            };
            unsafe {
                SetWindowLongW(self.hwnd, GWL_STYLE, style);
                SetWindowPos(
                    self.hwnd,
                    HWND_NOTOPMOST as *mut std::ffi::c_void,
                    self.pos_x,
                    self.pos_y,
                    self.width,
                    self.height,
                    SWP_FRAMECHANGED,
                );
                ShowWindow(self.hwnd, SW_SHOW);
            }
        }
    }

    fn is_fullscreen(&self) -> bool {
        self.fullscreen
    }

    fn set_window_opacity(&mut self, opacity: f32) {
        self.opacity = opacity;
        unsafe {
            let ex_style = GetWindowLongW(self.hwnd, GWL_EXSTYLE);
            if opacity < 1.0 {
                SetWindowLongW(self.hwnd, GWL_EXSTYLE, ex_style | WS_EX_LAYERED);
                SetLayeredWindowAttributes(self.hwnd, 0, (opacity * 255.0) as u8, LWA_ALPHA);
            } else {
                SetWindowLongW(self.hwnd, GWL_EXSTYLE, ex_style & !WS_EX_LAYERED);
            }
        }
    }

    fn flash_window(&mut self) {
        unsafe {
            let fi = FLASHWINFO {
                cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
                hwnd: self.hwnd,
                dwFlags: FLASHW_ALL | FLASHW_TIMERNOFG,
                uCount: 0,
                dwTimeout: 0,
            };
            FlashWindowEx(&fi);
        }
    }

    fn raise(&mut self) {
        unsafe {
            SetWindowPos(
                self.hwnd,
                HWND_TOP as *mut std::ffi::c_void,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE,
            );
        }
    }

    fn lower(&mut self) {
        unsafe {
            SetWindowPos(
                self.hwnd,
                HWND_BOTTOM as *mut std::ffi::c_void,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE,
            );
        }
    }

    fn set_window_icon(&mut self, icon_path: &str) {
        let wide = to_wide(icon_path);
        unsafe {
            let hicon = LoadImageW(
                ptr::null_mut(), // Load from file
                wide.as_ptr(),
                IMAGE_ICON,
                0,
                0, // Default size
                LR_LOADFROMFILE | LR_DEFAULTSIZE,
            ) as *mut std::ffi::c_void;
            if !hicon.is_null() {
                // Send WM_SETICON for both big and small icons
                SendMessageW(self.hwnd, WM_SETICON, ICON_BIG as usize, hicon as isize);
                SendMessageW(self.hwnd, WM_SETICON, ICON_SMALL as usize, hicon as isize);
            }
        }
    }

    // ── 事件循环 ──────────────────────────────────────────────────

    fn poll_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool {
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_QUIT {
                    return false;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&mut msg);
            }
        }

        // Drain the event queue
        while let Some(event) = self.event_queue.pop_front() {
            if !callback(&event) {
                return false;
            }
        }
        true
    }

    fn wait_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool {
        unsafe {
            let mut msg = MSG::default();

            // First drain any existing events
            while PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_QUIT {
                    return false;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&mut msg);
            }

            // Drain queue
            while let Some(event) = self.event_queue.pop_front() {
                if !callback(&event) {
                    return false;
                }
            }

            // Now wait for a new message
            let ret = GetMessageW(&mut msg, ptr::null_mut(), 0, 0);
            if ret < 0 {
                let err = win32_diag(Errc::PlatformError, "GetMessageW failed");
                Collector::instance().collect(err);
                return false;
            }
            if ret == 0 {
                return false; // WM_QUIT
            }
            TranslateMessage(&msg);
            DispatchMessageW(&mut msg);

            // Drain resulting events
            while let Some(event) = self.event_queue.pop_front() {
                if !callback(&event) {
                    return false;
                }
            }
        }
        true
    }

    // ── 输入控制 ──────────────────────────────────────────────────

    fn start_text_input(&mut self) {
        self.text_input_active = true;
        unsafe {
            ImmAssociateContextEx(self.hwnd, ptr::null_mut(), IACE_DEFAULT);
        }
    }

    fn stop_text_input(&mut self) {
        self.text_input_active = false;
        unsafe {
            ImmAssociateContextEx(self.hwnd, ptr::null_mut(), 0);
        }
    }

    // ── 文件拖放 ──────────────────────────────────────────────────

    fn enable_file_drop(&mut self, enable: bool) {
        self.file_drop_enabled = enable;
        unsafe {
            DragAcceptFiles(self.hwnd, if enable { TRUE } else { FALSE });
        }
    }

    // ── 子系统访问器 ──────────────────────────────────────────────

    fn clipboard(&mut self) -> &mut dyn IClipboard {
        self
    }

    fn cursor(&mut self) -> &mut dyn ICursor {
        self
    }

    fn display(&self) -> &dyn IDisplay {
        self
    }

    fn file_dialog(&mut self) -> &mut dyn IFileDialog {
        self
    }

    fn keyboard(&self) -> &dyn IKeyboard {
        self
    }

    fn text_input(&mut self) -> &mut dyn ITextInput {
        self
    }

    fn timer(&mut self) -> &mut dyn ITimer {
        self
    }

    fn notification(&mut self) -> &mut dyn INotification {
        self
    }

    fn console(&mut self) -> &mut dyn IConsole {
        &mut self.console
    }

    fn file_system(&self) -> &dyn IFileSystem {
        &self.file_system
    }

    fn system_info(&self) -> &dyn ISystemInfo {
        &self.system_info
    }
}

// ════════════════════════════════════════════════════════════════════════════
// IClipboard 实现
// ════════════════════════════════════════════════════════════════════════════

impl IClipboard for Win32Platform {
    fn text(&self) -> String {
        unsafe {
            if OpenClipboard(self.hwnd) == 0 {
                return String::new();
            }

            let handle = GetClipboardData(CF_UNICODETEXT);
            if handle.is_null() {
                CloseClipboard();
                return String::new();
            }

            // Lock the handle to get the wide string
            let ptr = GlobalLock(handle) as *const u16;
            if ptr.is_null() {
                CloseClipboard();
                return String::new();
            }

            let mut len = 0;
            while *ptr.add(len) != 0 {
                len += 1;
            }
            let result = to_utf8(std::slice::from_raw_parts(ptr, len));

            GlobalUnlock(handle);
            CloseClipboard();
            result
        }
    }

    fn set_text(&mut self, text: &str) {
        unsafe {
            if OpenClipboard(self.hwnd) == 0 {
                return;
            }
            EmptyClipboard();

            let wide = to_wide(text);
            let size = wide.len() * 2; // bytes
            let hglobal = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, size);
            if hglobal.is_null() {
                CloseClipboard();
                return;
            }

            let dest = GlobalLock(hglobal) as *mut u16;
            if !dest.is_null() {
                ptr::copy_nonoverlapping(wide.as_ptr(), dest, wide.len());
                GlobalUnlock(hglobal);
            }

            SetClipboardData(CF_UNICODETEXT, hglobal);
            CloseClipboard();
        }
    }

    fn has_text(&self) -> bool {
        unsafe {
            if OpenClipboard(self.hwnd) == 0 {
                Collector::instance().report(Errc::AccessDenied, "OpenClipboard failed");
                return false;
            }
            let formats = [CF_UNICODETEXT, CF_TEXT];
            let fmt = GetPriorityClipboardFormat(formats.as_ptr(), 2);
            CloseClipboard();
            fmt != -1
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// ICursor 实现
// ════════════════════════════════════════════════════════════════════════════

impl ICursor for Win32Platform {
    fn set_cursor(&mut self, cursor: CursorType) {
        let id = match cursor {
            CursorType::Arrow => IDC_ARROW,
            CursorType::IBeam => IDC_IBEAM,
            CursorType::Crosshair => IDC_CROSS,
            CursorType::Hand => IDC_HAND,
            CursorType::ResizeH => IDC_SIZEWE,
            CursorType::ResizeV => IDC_SIZENS,
            CursorType::ResizeNE => IDC_SIZENESW,
            CursorType::ResizeNW => IDC_SIZENWSE,
            CursorType::Move => IDC_SIZEALL,
            CursorType::Wait => IDC_WAIT,
            CursorType::NotAllowed => IDC_NO,
            CursorType::Custom => return, // Not supported via simple API
        };
        unsafe {
            let hcursor = LoadCursorW(ptr::null_mut(), id as *const u16);
            if !hcursor.is_null() {
                SetCursor(hcursor);
            }
        }
    }

    fn show_cursor(&mut self, visible: bool) {
        unsafe {
            ShowCursor(if visible { TRUE } else { FALSE });
        }
    }

    fn cursor_position(&self) -> Point {
        unsafe {
            let mut pt = POINT { x: 0, y: 0 };
            if GetCursorPos(&mut pt) != 0 {
                Point::new(pt.x as f32, pt.y as f32)
            } else {
                Point::default()
            }
        }
    }

    fn set_cursor_position(&mut self, x: i32, y: i32) {
        unsafe {
            SetCursorPos(x, y);
        }
    }

    fn confine_cursor(&mut self, confine: bool) {
        if confine {
            unsafe {
                let mut rect = RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                };
                if GetWindowRect(self.hwnd, &mut rect) != 0 {
                    ClipCursor(&rect);
                }
            }
        } else {
            unsafe {
                ClipCursor(ptr::null());
            }
        }
    }

    fn capture_mouse(&mut self) {
        unsafe {
            SetCapture(self.hwnd);
        }
    }

    fn release_mouse(&mut self) {
        unsafe {
            ReleaseCapture();
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// IDisplay 实现
// ════════════════════════════════════════════════════════════════════════════

impl IDisplay for Win32Platform {
    fn dpi_scale(&self) -> f32 {
        unsafe {
            // Use GetDeviceCaps for broad compatibility
            let hdc = GetDC(ptr::null_mut());
            if hdc.is_null() {
                return 1.0;
            }
            let dpi = GetDeviceCaps(hdc, LOGPIXELSX);
            ReleaseDC(ptr::null_mut(), hdc);
            dpi as f32 / 96.0
        }
    }

    fn is_dark_mode(&self) -> bool {
        // Simple check: on Windows 10+, check if title bar theme is dark
        // For now, return false (light mode) as default
        false
    }

    fn count(&self) -> i32 {
        1 // Primary monitor only
    }

    fn info(&self, _index: i32) -> DisplayInfo {
        unsafe {
            let w = GetSystemMetrics(SM_CXSCREEN);
            let h = GetSystemMetrics(SM_CYSCREEN);
            let dpi_scale = self.dpi_scale();
            DisplayInfo {
                bounds: Rect::new(0.0, 0.0, w as f32, h as f32),
                dpi_scale,
                is_primary: true,
            }
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// IFileDialog 实现（使用 GetOpenFileNameW / GetSaveFileNameW）
// ════════════════════════════════════════════════════════════════════════════

impl IFileDialog for Win32Platform {
    fn open(&mut self, title: &str, filters: &str) -> Vec<String> {
        let wide_filters = to_wide(filters);
        let mut buf = [0u16; 4096];
        let wide_title = to_wide(title);

        unsafe {
            let mut ofn = OPENFILENAMEW {
                lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
                hwndOwner: self.hwnd,
                hInstance: ptr::null_mut(),
                lpstrFilter: wide_filters.as_ptr(),
                lpstrCustomFilter: ptr::null_mut(),
                nMaxCustFilter: 0,
                nFilterIndex: 1,
                lpstrFile: buf.as_mut_ptr(),
                nMaxFile: 4096,
                lpstrFileTitle: ptr::null_mut(),
                nMaxFileTitle: 0,
                lpstrInitialDir: ptr::null(),
                lpstrTitle: wide_title.as_ptr(),
                Flags: OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_HIDEREADONLY | OFN_ALLOWMULTISELECT,
                nFileOffset: 0,
                nFileExtension: 0,
                lpstrDefExt: ptr::null(),
                lCustData: 0,
                lpfnHook: ptr::null_mut(),
                lpTemplateName: ptr::null(),
                pvReserved: ptr::null_mut(),
                dwReserved: 0,
                FlagsEx: 0,
            };

            let result = GetOpenFileNameW(&mut ofn);
            if result == 0 {
                // Check if user cancelled
                let err = CommDlgExtendedError();
                if err != 0 {
                    // An error occurred
                }
                return Vec::new();
            }

            // Parse the result
            // First part is the directory, then files
            let wide_str = &buf[..];
            let null_pos = wide_str.iter().position(|&c| c == 0).unwrap_or(0);
            if null_pos == 0 {
                return Vec::new();
            }

            // Check if there are multiple files (directory followed by files)
            let dir = to_utf8(&wide_str[..null_pos]);
            if dir.is_empty() {
                return Vec::new();
            }

            let remaining = &wide_str[(null_pos + 1)..];
            let second_null = remaining.iter().position(|&c| c == 0).unwrap_or(0);

            if second_null == 0 || remaining[0] == 0 {
                // Single file
                vec![dir]
            } else {
                // Multiple files
                let mut result = Vec::new();
                let mut pos = 0;
                loop {
                    if pos >= remaining.len() || remaining[pos] == 0 {
                        break;
                    }
                    let end = remaining[pos..]
                        .iter()
                        .position(|&c| c == 0)
                        .unwrap_or(remaining.len() - pos);
                    let file_name = to_utf8(&remaining[pos..pos + end]);
                    if file_name.is_empty() {
                        break;
                    }
                    result.push(format!("{}\\{}", dir, file_name));
                    pos = pos + end + 1;
                }
                result
            }
        }
    }

    fn save(&mut self, title: &str, filters: &str) -> String {
        let wide_filters = to_wide(filters);
        let mut buf = [0u16; 4096];
        let wide_title = to_wide(title);

        unsafe {
            let mut ofn = OPENFILENAMEW {
                lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
                hwndOwner: self.hwnd,
                hInstance: ptr::null_mut(),
                lpstrFilter: wide_filters.as_ptr(),
                lpstrCustomFilter: ptr::null_mut(),
                nMaxCustFilter: 0,
                nFilterIndex: 1,
                lpstrFile: buf.as_mut_ptr(),
                nMaxFile: 4096,
                lpstrFileTitle: ptr::null_mut(),
                nMaxFileTitle: 0,
                lpstrInitialDir: ptr::null(),
                lpstrTitle: wide_title.as_ptr(),
                Flags: OFN_EXPLORER | OFN_PATHMUSTEXIST | OFN_HIDEREADONLY | OFN_OVERWRITEPROMPT,
                nFileOffset: 0,
                nFileExtension: 0,
                lpstrDefExt: ptr::null(),
                lCustData: 0,
                lpfnHook: ptr::null_mut(),
                lpTemplateName: ptr::null(),
                pvReserved: ptr::null_mut(),
                dwReserved: 0,
                FlagsEx: 0,
            };

            let result = GetSaveFileNameW(&mut ofn);
            if result == 0 {
                return String::new();
            }

            to_utf8(&buf)
        }
    }

    fn open_folder(&mut self, title: &str) -> String {
        // Fall back to a simple file dialog approach; for a proper folder picker
        // we'd use IFileOpenDialog with FOS_PICKFOLDERS, but that requires COM.
        // For now, return empty string (callers should fall back to a cross-platform dialog).
        let _ = title;
        String::new()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// IKeyboard 实现
// ════════════════════════════════════════════════════════════════════════════

impl IKeyboard for Win32Platform {
    fn is_down(&self, key: KeyCode) -> bool {
        let disc = key as u32;
        let vk = if disc == KeyCode::Shift as u32 {
            VK_SHIFT
        } else if disc == KeyCode::Ctrl as u32 {
            VK_CONTROL
        } else if disc == KeyCode::Alt as u32 {
            VK_MENU
        } else if disc == KeyCode::Super as u32 {
            VK_LWIN
        } else if (KeyCode::A as u32..=KeyCode::Z as u32).contains(&disc) {
            0x41 + (disc - KeyCode::A as u32)
        } else if (KeyCode::Num0 as u32..=KeyCode::Num9 as u32).contains(&disc) {
            0x30 + (disc - KeyCode::Num0 as u32)
        } else if (KeyCode::F1 as u32..=KeyCode::F12 as u32).contains(&disc) {
            0x70 + (disc - KeyCode::F1 as u32)
        } else if disc == KeyCode::Left as u32 {
            VK_LEFT
        } else if disc == KeyCode::Right as u32 {
            VK_RIGHT
        } else if disc == KeyCode::Up as u32 {
            VK_UP
        } else if disc == KeyCode::Down as u32 {
            VK_DOWN
        } else if disc == KeyCode::Home as u32 {
            VK_HOME
        } else if disc == KeyCode::End as u32 {
            VK_END
        } else if disc == KeyCode::PageUp as u32 {
            VK_PRIOR
        } else if disc == KeyCode::PageDown as u32 {
            VK_NEXT
        } else if disc == KeyCode::Enter as u32 {
            VK_RETURN
        } else if disc == KeyCode::Escape as u32 {
            VK_ESCAPE
        } else if disc == KeyCode::Backspace as u32 {
            VK_BACK
        } else if disc == KeyCode::Delete as u32 {
            VK_DELETE
        } else if disc == KeyCode::Tab as u32 {
            VK_TAB
        } else if disc == KeyCode::Space as u32 {
            VK_SPACE
        } else if disc == KeyCode::Insert as u32 {
            VK_INSERT
        } else {
            return false;
        };
        is_key_down(vk)
    }

    fn idle_ms(&self) -> u32 {
        unsafe {
            let mut info = LASTINPUTINFO {
                cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
                dwTime: 0,
            };
            if GetLastInputInfo(&mut info) != 0 {
                let tick = GetTickCount();
                if tick >= info.dwTime {
                    tick - info.dwTime
                } else {
                    0
                }
            } else {
                0
            }
        }
    }

    fn double_click_ms(&self) -> u32 {
        double_click_time()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// ITextInput 实现
// ════════════════════════════════════════════════════════════════════════════

impl ITextInput for Win32Platform {
    fn start(&mut self) {
        self.start_text_input();
    }

    fn stop(&mut self) {
        self.stop_text_input();
    }
}

// ════════════════════════════════════════════════════════════════════════════
// ITimer 实现
// ════════════════════════════════════════════════════════════════════════════

impl ITimer for Win32Platform {
    fn set(&mut self, interval_ms: u32, repeating: bool) -> u32 {
        let id = self.next_timer_id;
        self.next_timer_id = self.next_timer_id.wrapping_add(1);
        unsafe {
            SetTimer(self.hwnd, id, interval_ms, None);
        }
        if !repeating {
            // For one-shot timers, we can't easily use SetTimer which is inherently repeating.
            // We just leave it and the user should call clear() after receiving the event.
        }
        id
    }

    fn clear(&mut self, id: u32) {
        unsafe {
            KillTimer(self.hwnd, id);
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// INotification 实现
// ════════════════════════════════════════════════════════════════════════════

impl INotification for Win32Platform {
    fn show(&mut self, title: &str, message: &str) {
        unsafe {
            let wide_title = to_wide(title);
            let wide_msg = to_wide(message);

            let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = self.hwnd;
            nid.uID = 1;
            nid.uFlags = NIF_INFO | NIF_ICON | NIF_TIP;
            nid.uCallbackMessage = WM_APP_NOTIFY;

            // Copy title (up to 64 chars including null)
            let title_len = wide_title.len().min(64);
            for i in 0..title_len {
                nid.szInfoTitle[i] = wide_title[i];
            }
            if title_len < 64 {
                nid.szInfoTitle[title_len] = 0;
            }

            // Copy message (up to 256 chars including null)
            let msg_len = wide_msg.len().min(256);
            for i in 0..msg_len {
                nid.szInfo[i] = wide_msg[i];
            }
            if msg_len < 256 {
                nid.szInfo[msg_len] = 0;
            }

            nid.dwInfoFlags = NIIF_INFO;

            // Use standard application icon
            nid.hIcon = LoadIconW(ptr::null_mut(), IDI_APPLICATION as *const u16);

            if self.notification_active {
                Shell_NotifyIconW(NIM_MODIFY, &mut nid);
            } else {
                Shell_NotifyIconW(NIM_ADD, &mut nid);
                self.notification_active = true;
            }
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Drop — 清理资源
// ════════════════════════════════════════════════════════════════════════════

impl Drop for Win32Platform {
    fn drop(&mut self) {
        // Remove notification icon if active
        if self.notification_active && !self.hwnd.is_null() {
            unsafe {
                let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
                nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
                nid.hWnd = self.hwnd;
                nid.uID = 1;
                Shell_NotifyIconW(NIM_DELETE, &mut nid);
            }
        }
        self.destroy_window();
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Win32 窗口消息常量
// ════════════════════════════════════════════════════════════════════════════

const WM_NCCREATE: u32 = 0x0081;
const WM_DESTROY: u32 = 0x0002;
const WM_CLOSE: u32 = 0x0010;
const WM_SIZE: u32 = 0x0005;
const WM_MOVE: u32 = 0x0003;
const WM_SETFOCUS: u32 = 0x0007;
const WM_KILLFOCUS: u32 = 0x0008;
const WM_KEYDOWN: u32 = 0x0100;
const WM_KEYUP: u32 = 0x0101;
const WM_CHAR: u32 = 0x0102;
const WM_SYSKEYDOWN: u32 = 0x0104;
const WM_SYSKEYUP: u32 = 0x0105;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_RBUTTONDOWN: u32 = 0x0204;
const WM_RBUTTONUP: u32 = 0x0205;
const WM_MBUTTONDOWN: u32 = 0x0207;
const WM_MBUTTONUP: u32 = 0x0208;
const WM_MOUSEMOVE: u32 = 0x0200;
const WM_MOUSEWHEEL: u32 = 0x020A;
const WM_TIMER: u32 = 0x0113;
const WM_DROPFILES: u32 = 0x0233;
const WM_SETCURSOR: u32 = 0x0020;
const WM_SETICON: u32 = 0x0080;
const WM_APP_NOTIFY: u32 = 0x8000;

const SIZE_RESTORED: usize = 0;
const SIZE_MINIMIZED: usize = 1;
const SIZE_MAXIMIZED: usize = 2;

const HTCLIENT: u32 = 1;

const ICON_BIG: u32 = 1;
const ICON_SMALL: u32 = 0;

const SW_HIDE: i32 = 0;
const SW_SHOW: i32 = 5;
const SW_RESTORE: i32 = 9;
const SW_MINIMIZE: i32 = 6;
const SW_MAXIMIZE: i32 = 3;

const PM_REMOVE: u32 = 1;

const CW_USEDEFAULT: i32 = 0x8000_0000u32 as i32;

const SWP_NOMOVE: u32 = 0x0002;
const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_FRAMECHANGED: u32 = 0x0020;

const HWND_TOP: isize = 0;
const HWND_BOTTOM: isize = 1;
const HWND_TOPMOST: isize = -1;
const HWND_NOTOPMOST: isize = -2;

const GWL_STYLE: i32 = -16;
const GWL_EXSTYLE: i32 = -20;

const WS_OVERLAPPED: u32 = 0x0000_0000;
const WS_POPUP: u32 = 0x8000_0000;
const WS_CAPTION: u32 = 0x00C0_0000;
const WS_SYSMENU: u32 = 0x0008_0000;
const WS_MINIMIZEBOX: u32 = 0x0002_0000;
const WS_MAXIMIZEBOX: u32 = 0x0001_0000;
const WS_OVERLAPPEDWINDOW: u32 =
    WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX;
const WS_EX_APPWINDOW: u32 = 0x0004_0000;
const WS_EX_LAYERED: u32 = 0x0008_0000;

const LWA_ALPHA: u32 = 0x0000_0002;

const SM_CXSCREEN: i32 = 0;
const SM_CYSCREEN: i32 = 1;

const LR_LOADFROMFILE: u32 = 0x0000_0010;
const LR_DEFAULTSIZE: u32 = 0x0000_0040;
const IMAGE_ICON: u32 = 1;

// Virtual key codes
const VK_SHIFT: u32 = 0x10;
const VK_CONTROL: u32 = 0x11;
const VK_MENU: u32 = 0x12;
const VK_LWIN: u32 = 0x5B;
const VK_RWIN: u32 = 0x5C;
const VK_LEFT: u32 = 0x25;
const VK_UP: u32 = 0x26;
const VK_RIGHT: u32 = 0x27;
const VK_DOWN: u32 = 0x28;
const VK_RETURN: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;
const VK_BACK: u32 = 0x08;
const VK_DELETE: u32 = 0x2E;
const VK_TAB: u32 = 0x09;
const VK_SPACE: u32 = 0x20;
const VK_INSERT: u32 = 0x2D;
const VK_HOME: u32 = 0x24;
const VK_END: u32 = 0x23;
const VK_PRIOR: u32 = 0x21; // Page Up
const VK_NEXT: u32 = 0x22; // Page Down

// Cursor IDs (MAKEINTRESOURCE values)
const IDC_ARROW: u32 = 32512;
const IDC_IBEAM: u32 = 32513;
const IDC_WAIT: u32 = 32514;
const IDC_CROSS: u32 = 32515;
const IDC_SIZEALL: u32 = 32518;
const IDC_NO: u32 = 32520;
const IDC_HAND: u32 = 32522;
const IDC_SIZENS: u32 = 32523;
const IDC_SIZEWE: u32 = 32524;
const IDC_SIZENWSE: u32 = 32525;
const IDC_SIZENESW: u32 = 32526;

// Clipboard formats
const CF_UNICODETEXT: u32 = 13;
const CF_TEXT: u32 = 1;

// Global memory flags
const GMEM_MOVEABLE: u32 = 0x0002;
const GMEM_ZEROINIT: u32 = 0x0040;

// IME
const IACE_DEFAULT: usize = 1;

// DPI / GDI
const LOGPIXELSX: i32 = 88;

// Notification icon constants
const NIM_ADD: u32 = 0;
const NIM_MODIFY: u32 = 1;
const NIM_DELETE: u32 = 2;
const NIF_ICON: u32 = 0x0002;
const NIF_INFO: u32 = 0x0010;
const NIF_TIP: u32 = 0x0004;
const NIIF_INFO: u32 = 1;

const IDI_APPLICATION: u32 = 32512;

// File dialog flags
const OFN_EXPLORER: u32 = 0x0008_0000;
const OFN_FILEMUSTEXIST: u32 = 0x0000_1000;
const OFN_HIDEREADONLY: u32 = 0x0000_0004;
const OFN_ALLOWMULTISELECT: u32 = 0x0000_0200;
const OFN_PATHMUSTEXIST: u32 = 0x0000_0800;
const OFN_OVERWRITEPROMPT: u32 = 0x0000_0002;

// Flash window
const FLASHW_ALL: u32 = 0x0003;
const FLASHW_TIMERNOFG: u32 = 0x000C;

// Drag & drop
const TRUE: i32 = 1;
const FALSE: i32 = 0;

// ════════════════════════════════════════════════════════════════════════════
// Win32 数据结构定义
// ════════════════════════════════════════════════════════════════════════════

#[repr(C)]
#[derive(Default)]
struct MSG {
    hwnd: *mut std::ffi::c_void,
    message: u32,
    wParam: usize,
    lParam: isize,
    time: u32,
    pt: POINT,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct POINT {
    x: i32,
    y: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct RECT {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct WNDCLASSEXW {
    cbSize: u32,
    style: u32,
    lpfnWndProc:
        Option<unsafe extern "system" fn(*mut std::ffi::c_void, u32, usize, isize) -> isize>,
    cbClsExtra: i32,
    cbWndExtra: i32,
    hInstance: *mut std::ffi::c_void,
    hIcon: *mut std::ffi::c_void,
    hCursor: *mut std::ffi::c_void,
    hbrBackground: *mut std::ffi::c_void,
    lpszMenuName: *const u16,
    lpszClassName: *const u16,
    hIconSm: *mut std::ffi::c_void,
}

#[repr(C)]
struct CREATESTRUCTW {
    lpCreateParams: *mut std::ffi::c_void,
    hInstance: *mut std::ffi::c_void,
    hMenu: *mut std::ffi::c_void,
    hwndParent: *mut std::ffi::c_void,
    cy: i32,
    cx: i32,
    y: i32,
    x: i32,
    style: u32,
    lpszName: *const u16,
    lpszClass: *const u16,
    dwExStyle: u32,
}

#[repr(C)]
struct OPENFILENAMEW {
    lStructSize: u32,
    hwndOwner: *mut std::ffi::c_void,
    hInstance: *mut std::ffi::c_void,
    lpstrFilter: *const u16,
    lpstrCustomFilter: *mut u16,
    nMaxCustFilter: u32,
    nFilterIndex: u32,
    lpstrFile: *mut u16,
    nMaxFile: u32,
    lpstrFileTitle: *mut u16,
    nMaxFileTitle: u32,
    lpstrInitialDir: *const u16,
    lpstrTitle: *const u16,
    Flags: u32,
    nFileOffset: i16,
    nFileExtension: i16,
    lpstrDefExt: *const u16,
    lCustData: isize,
    lpfnHook: *mut std::ffi::c_void,
    lpTemplateName: *const u16,
    pvReserved: *mut std::ffi::c_void,
    dwReserved: u32,
    FlagsEx: u32,
}

#[repr(C)]
struct FLASHWINFO {
    cbSize: u32,
    hwnd: *mut std::ffi::c_void,
    dwFlags: u32,
    uCount: u32,
    dwTimeout: u32,
}

#[repr(C)]
struct LASTINPUTINFO {
    cbSize: u32,
    dwTime: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct NOTIFYICONDATAW {
    cbSize: u32,
    hWnd: *mut std::ffi::c_void,
    uID: u32,
    uFlags: u32,
    uCallbackMessage: u32,
    hIcon: *mut std::ffi::c_void,
    szTip: [u16; 128],
    dwState: u32,
    dwStateMask: u32,
    szInfo: [u16; 256],
    uVersion: u32,
    szInfoTitle: [u16; 64],
    dwInfoFlags: u32,
    guidItem: [u8; 16], // GUID placeholder
    hBalloonIcon: *mut std::ffi::c_void,
}

// ════════════════════════════════════════════════════════════════════════════
// Raw FFI — user32.dll / kernel32.dll / gdi32.dll / comdlg32.dll / imm32.dll / shell32.dll
// ════════════════════════════════════════════════════════════════════════════

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(lpModuleName: *const u16) -> *mut std::ffi::c_void;
    fn GetTickCount() -> u32;
}

#[link(name = "user32")]
extern "system" {
    fn RegisterClassExW(lpwcx: *const WNDCLASSEXW) -> u16;
    fn UnregisterClassW(lpClassName: *const u16, hInstance: *mut std::ffi::c_void) -> i32;
    fn CreateWindowExW(
        dwExStyle: u32,
        lpClassName: *const u16,
        lpWindowName: *const u16,
        dwStyle: u32,
        x: i32,
        y: i32,
        nWidth: i32,
        nHeight: i32,
        hWndParent: *mut std::ffi::c_void,
        hMenu: *mut std::ffi::c_void,
        hInstance: *mut std::ffi::c_void,
        lpParam: *mut std::ffi::c_void,
    ) -> *mut std::ffi::c_void;
    fn DestroyWindow(hWnd: *mut std::ffi::c_void) -> i32;
    fn AdjustWindowRectEx(lpRect: *mut RECT, dwStyle: u32, bMenu: i32, dwExStyle: u32) -> i32;
    fn DefWindowProcW(hWnd: *mut std::ffi::c_void, msg: u32, wParam: usize, lParam: isize)
        -> isize;
    fn GetMessageW(
        lpMsg: *mut MSG,
        hWnd: *mut std::ffi::c_void,
        wMsgFilterMin: u32,
        wMsgFilterMax: u32,
    ) -> i32;
    fn PeekMessageW(
        lpMsg: *mut MSG,
        hWnd: *mut std::ffi::c_void,
        wMsgFilterMin: u32,
        wMsgFilterMax: u32,
        wRemoveMsg: u32,
    ) -> i32;
    fn TranslateMessage(lpMsg: *const MSG) -> i32;
    fn DispatchMessageW(lpMsg: *const MSG) -> isize;
    fn PostQuitMessage(nExitCode: i32);
    fn ShowWindow(hWnd: *mut std::ffi::c_void, nCmdShow: i32) -> i32;
    fn SetWindowTextW(hWnd: *mut std::ffi::c_void, lpString: *const u16) -> i32;
    fn SetWindowPos(
        hWnd: *mut std::ffi::c_void,
        hWndInsertAfter: *mut std::ffi::c_void,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        uFlags: u32,
    ) -> i32;
    fn GetWindowRect(hWnd: *mut std::ffi::c_void, lpRect: *mut RECT) -> i32;
    fn SetWindowLongW(hWnd: *mut std::ffi::c_void, nIndex: i32, dwNewLong: u32) -> u32;
    fn GetWindowLongW(hWnd: *mut std::ffi::c_void, nIndex: i32) -> u32;
    fn SetWindowLongPtrW(hWnd: *mut std::ffi::c_void, nIndex: i32, dwNewLong: isize) -> isize;
    fn GetWindowLongPtrW(hWnd: *mut std::ffi::c_void, nIndex: i32) -> isize;
    fn SetLayeredWindowAttributes(
        hWnd: *mut std::ffi::c_void,
        crKey: u32,
        bAlpha: u8,
        dwFlags: u32,
    ) -> i32;
    fn FlashWindowEx(pfwi: *const FLASHWINFO) -> i32;
    fn GetSystemMetrics(nIndex: i32) -> i32;
    fn LoadImageW(
        hInst: *mut std::ffi::c_void,
        name: *const u16,
        typ: u32,
        cx: i32,
        cy: i32,
        fuLoad: u32,
    ) -> *mut std::ffi::c_void;
    fn LoadIconW(hInstance: *mut std::ffi::c_void, lpIconName: *const u16)
        -> *mut std::ffi::c_void;
    fn LoadCursorW(
        hInstance: *mut std::ffi::c_void,
        lpCursorName: *const u16,
    ) -> *mut std::ffi::c_void;
    fn SetCursor(hCursor: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn ShowCursor(bShow: i32) -> i32;
    fn GetCursorPos(lpPoint: *mut POINT) -> i32;
    fn SetCursorPos(x: i32, y: i32) -> i32;
    fn ClipCursor(lpRect: *const RECT) -> i32;
    fn SetCapture(hWnd: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn ReleaseCapture() -> i32;
    fn SetTimer(
        hWnd: *mut std::ffi::c_void,
        nIDEvent: u32,
        uElapse: u32,
        lpTimerFunc: Option<
            unsafe extern "system" fn(*mut std::ffi::c_void, u32, usize, isize) -> (),
        >,
    ) -> usize;
    fn KillTimer(hWnd: *mut std::ffi::c_void, uIDEvent: u32) -> i32;
    fn SendMessageW(hWnd: *mut std::ffi::c_void, msg: u32, wParam: usize, lParam: isize) -> isize;
    fn ScreenToClient(hWnd: *mut std::ffi::c_void, lpPoint: *mut POINT) -> i32;
    fn GetAsyncKeyState(vKey: u32) -> i16;
    fn GetLastInputInfo(plii: *mut LASTINPUTINFO) -> i32;
    fn GetDoubleClickTime() -> u32;

    // Clipboard
    fn OpenClipboard(hWndNewOwner: *mut std::ffi::c_void) -> i32;
    fn CloseClipboard() -> i32;
    fn EmptyClipboard() -> i32;
    fn GetClipboardData(uFormat: u32) -> *mut std::ffi::c_void;
    fn SetClipboardData(uFormat: u32, hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn GetPriorityClipboardFormat(paFormatPriorityList: *const u32, cFormats: i32) -> i32;

    // Drag & drop
    fn DragAcceptFiles(hWnd: *mut std::ffi::c_void, fAccept: i32);
    fn DragQueryFileW(
        hDrop: *mut std::ffi::c_void,
        iFile: u32,
        lpszFile: *mut u16,
        cch: u32,
    ) -> u32;
    fn DragQueryPoint(hDrop: *mut std::ffi::c_void, lppt: *mut POINT) -> i32;
    fn DragFinish(hDrop: *mut std::ffi::c_void);
}

#[link(name = "imm32")]
extern "system" {
    fn ImmAssociateContextEx(
        hWnd: *mut std::ffi::c_void,
        hIMC: *mut std::ffi::c_void,
        dwFlags: usize,
    ) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    fn GetDC(hWnd: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn ReleaseDC(hWnd: *mut std::ffi::c_void, hDC: *mut std::ffi::c_void) -> i32;
    fn GetDeviceCaps(hdc: *mut std::ffi::c_void, nIndex: i32) -> i32;
}

#[link(name = "comdlg32")]
extern "system" {
    fn GetOpenFileNameW(lpofn: *mut OPENFILENAMEW) -> i32;
    fn GetSaveFileNameW(lpofn: *mut OPENFILENAMEW) -> i32;
    fn CommDlgExtendedError() -> u32;
}

#[link(name = "shell32")]
extern "system" {
    fn Shell_NotifyIconW(dwMessage: u32, lpData: *mut NOTIFYICONDATAW) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> *mut std::ffi::c_void;
    fn GlobalLock(hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn GlobalUnlock(hMem: *mut std::ffi::c_void) -> i32;
}

// ════════════════════════════════════════════════════════════════════════════
// 安全封装 — 高频 Win32 查询
// ════════════════════════════════════════════════════════════════════════════

/// Returns true if the given virtual key is currently pressed.
fn is_key_down(vk: u32) -> bool {
    // SAFETY: GetAsyncKeyState is always safe to call from any thread;
    // it reads global keyboard state with no side effects.
    unsafe { (GetAsyncKeyState(vk) as i32 & 0x8000) != 0 }
}

/// Returns the system double-click time in milliseconds.
fn double_click_time() -> u32 {
    // SAFETY: GetDoubleClickTime is always safe to call.
    unsafe { GetDoubleClickTime() }
}
