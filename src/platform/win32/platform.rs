// ============================================================================
// uix-platform/src/win32/platform.rs — Win32 Platform trait implementation
//
// 职责范围：
//   窗口管理 + 事件循环 — 核心职责
//   子系统通过组合模式委托给独立的 Win32* 结构体
// ============================================================================
//
// # Safety
//
// 窗口过程 `wnd_proc` 通过 `GWLP_USERDATA` 存储的 `*mut Win32Platform` 指针
// 重建 `&mut Win32Platform` 引用。安全前提：
//   1. Win32Platform 必须在创建窗口的同一线程上存活
//   2. `WM_NCCREATE` 中必须写入有效指针
//   3. 窗口过程运行时不能有其他 `&mut` 引用共存
//   4. Win32Platform 的 Drop 必须在窗口销毁后执行
//
// ============================================================================

// NOTE: Module is conditionally compiled via #[cfg(windows)] in platform/mod.rs.
// This file is Windows-only; cross-compilation is guarded at the module level.

use crate::platform::event::*;
use crate::platform::types::*;
use crate::platform::presenter::NullPresenter;
use crate::platform::win32::bindings::*;
use crate::platform::win32::clipboard::Win32Clipboard;
use crate::platform::win32::console::Win32Console;
use crate::platform::win32::cursor::Win32Cursor;
use crate::platform::win32::display::Win32Display;
use crate::platform::win32::ffi::*;
use crate::platform::win32::file_dialog::Win32FileDialog;
use crate::platform::win32::filesystem::Win32FileSystem;
use crate::platform::win32::gdi_presenter::GdiPresenter;
use crate::platform::win32::keyboard::Win32Keyboard;
use crate::platform::win32::notification::Win32Notification;
use crate::platform::win32::system_info::Win32SystemInfo;
use crate::platform::win32::text_input::Win32TextInput;
use crate::platform::win32::timer::Win32Timer;
use crate::platform::win32::util::{to_utf8, to_wide};
use crate::platform::*;

use std::collections::VecDeque;
use std::ptr;

// ════════════════════════════════════════════════════════════════════════════
// Win32Platform — 主平台结构体
// ════════════════════════════════════════════════════════════════════════════

pub struct Win32Platform {
    // ── 窗口句柄与状态 ──────────────────────────────────────────────
    hwnd: *mut std::ffi::c_void,
    hinstance: *mut std::ffi::c_void,
    class_atom: u16,

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

    // ── 事件队列（窗口过程写入，事件循环读取）───────────────────
    event_queue: VecDeque<UiEvent>,

    // ── 像素呈现器（默认 NullPresenter，窗口创建后替换为 GdiPresenter）─
    presenter: Box<dyn IPresenter>,

    // ── 组合子系统（独立 struct，通过 accessor 暴露）─────────────
    clipboard_subsys: Win32Clipboard,
    cursor_subsys: Win32Cursor,
    display_subsys: Win32Display,
    file_dialog_subsys: Win32FileDialog,
    keyboard_subsys: Win32Keyboard,
    text_input_subsys: Win32TextInput,
    timer_subsys: Win32Timer,
    notification_subsys: Win32Notification,
    console_subsys: Win32Console,
    file_system_subsys: Win32FileSystem,
    system_info_subsys: Win32SystemInfo,
}

// ════════════════════════════════════════════════════════════════════════════
// 常量
// ════════════════════════════════════════════════════════════════════════════

const DEFAULT_CLASS_STYLE: u32 = 0x0008 | 0x0002 | 0x0001;
const GWLP_USERDATA: i32 = -21;
const WM_QUIT: u32 = 0x0012;

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

const SIZE_RESTORED: usize = 0;
const SIZE_MINIMIZED: usize = 1;
const SIZE_MAXIMIZED: usize = 2;

const HTCLIENT: u32 = 1;

const ICON_BIG: usize = 1;
const ICON_SMALL: usize = 0;

const SW_HIDE: i32 = 0;
const SW_SHOWNORMAL: i32 = 1;
const SW_RESTORE: i32 = 9;
const SW_MINIMIZE: i32 = 6;
const SW_MAXIMIZE: i32 = 3;

const PM_REMOVE: u32 = 0x0001;

const CW_USEDEFAULT: i32 = -2147483648;

const IDI_APPLICATION: *const u16 = 32512 as *const u16;

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

const WS_OVERLAPPED: u32 = 0x00000000;
const WS_POPUP: u32 = 0x80000000;
const WS_CAPTION: u32 = 0x00C00000;
const WS_SYSMENU: u32 = 0x00080000;
const WS_MINIMIZEBOX: u32 = 0x00020000;
const WS_MAXIMIZEBOX: u32 = 0x00010000;
const WS_OVERLAPPEDWINDOW: u32 =
    WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_THICKFRAME | WS_MINIMIZEBOX | WS_MAXIMIZEBOX;
const WS_THICKFRAME: u32 = 0x00040000;
const WS_EX_APPWINDOW: u32 = 0x00040000;
const WS_EX_LAYERED: u32 = 0x00080000;

const LWA_ALPHA: u32 = 0x00000002;

const COLOR_APPWORKSPACE: u32 = 1;

const SM_CXSCREEN: i32 = 0;
const SM_CYSCREEN: i32 = 1;

const LR_LOADFROMFILE: u32 = 0x0010;
const LR_DEFAULTSIZE: u32 = 0x0040;

const IMAGE_ICON: u32 = 1;

const IDC_ARROW: u16 = 32512;

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
const VK_PRIOR: u32 = 0x21;
const VK_NEXT: u32 = 0x22;

const WS_VISIBLE: u32 = 0x10000000;

const TRUE: i32 = 1;
const FALSE: i32 = 0;

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
            presenter: Box::new(NullPresenter::new()),
            clipboard_subsys: Win32Clipboard::new(),
            cursor_subsys: Win32Cursor::new(),
            display_subsys: Win32Display::new(),
            file_dialog_subsys: Win32FileDialog::new(),
            keyboard_subsys: Win32Keyboard::new(),
            text_input_subsys: Win32TextInput::new(),
            timer_subsys: Win32Timer::new(),
            notification_subsys: Win32Notification::new(),
            console_subsys: Win32Console::new(),
            file_system_subsys: Win32FileSystem::new(),
            system_info_subsys: Win32SystemInfo::new(),
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
        to_wide("UIX_WindowClass")
    }

    fn register_class(&mut self) -> bool {
        let class_name = self.class_name();
        unsafe {
            let hinstance = GetModuleHandleW(ptr::null_mut());
            if hinstance.is_null() {
                return false;
            }
            self.hinstance = hinstance;

            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: DEFAULT_CLASS_STYLE,
                lpfnWndProc: Some(wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: std::mem::size_of::<*mut std::ffi::c_void>() as i32,
                hInstance: hinstance,
                hIcon: LoadIconW(ptr::null_mut(), IDI_APPLICATION),
                hCursor: ptr::null_mut(),
                hbrBackground: (COLOR_APPWORKSPACE + 1) as *mut std::ffi::c_void,
                lpszMenuName: ptr::null(),
                lpszClassName: class_name.as_ptr(),
                hIconSm: ptr::null_mut(),
            };

            let atom = RegisterClassExW(&wc);
            if atom == 0 {
                return false;
            }
            self.class_atom = atom;
            true
        }
    }

    fn get_modifier_state() -> KeyMod {
        let mut mods = KeyMod::NONE;
        unsafe {
            if GetAsyncKeyState(VK_SHIFT as i32) < 0 {
                mods |= KeyMod::SHIFT;
            }
            if GetAsyncKeyState(VK_CONTROL as i32) < 0 {
                mods |= KeyMod::CTRL;
            }
            if GetAsyncKeyState(VK_MENU as i32) < 0 {
                mods |= KeyMod::ALT;
            }
        }
        mods
    }

    fn vk_to_keycode(vk: u32) -> KeyCode {
        const KEYS_AZ: [KeyCode; 26] = [
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
        const KEYS_NUM: [KeyCode; 10] = [
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
        const KEYS_FN: [KeyCode; 12] = [
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
        match vk {
            0x41..=0x5A => KEYS_AZ[(vk - 0x41) as usize],
            0x30..=0x39 => KEYS_NUM[(vk - 0x30) as usize],
            0x70..=0x7B => KEYS_FN[(vk - 0x70) as usize],
            VK_LEFT => KeyCode::Left,
            VK_UP => KeyCode::Up,
            VK_RIGHT => KeyCode::Right,
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

    fn hiword(value: isize) -> u16 {
        ((value >> 16) & 0xFFFF) as u16
    }
    fn loword(value: isize) -> u16 {
        (value & 0xFFFF) as u16
    }
    fn hiword_usize(value: usize) -> u16 {
        ((value >> 16) & 0xFFFF) as u16
    }

    fn push_event(&mut self, event: UiEvent) {
        self.event_queue.push_back(event);
    }

    // ── 窗口过程消息处理 ──────────────────────────────────────────

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

                // 同步呈现器尺寸
                if let Err(e) = self.presenter.resize(w, h) {
                    log::debug!("Win32Platform: presenter resize failed: {}", e.short_what());
                }

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
                let key = Self::vk_to_keycode(wparam as u32);
                let mods = Self::get_modifier_state();
                self.push_event(UiEvent::key_down(key, mods));
                0
            }

            WM_KEYUP | WM_SYSKEYUP => {
                let key = Self::vk_to_keycode(wparam as u32);
                let mods = Self::get_modifier_state();
                self.push_event(UiEvent::key_up(key, mods));
                0
            }

            // SAFETY: WM_CHAR delivers valid Unicode code points (UTF-16).
            // Use char::from_u32 to safely handle the full Unicode range,
            // avoiding truncation to u8 and potential invalid char values.
            WM_CHAR => {
                if let Some(ch) = std::char::from_u32(wparam as u32) {
                    self.push_event(UiEvent::key_press(ch.to_string()));
                }
                0
            }

            WM_LBUTTONDOWN => {
                self.handle_mouse_down(lparam, MouseButton::Left);
                0
            }
            WM_LBUTTONUP => {
                self.handle_mouse_up(lparam, MouseButton::Left);
                0
            }
            WM_RBUTTONDOWN => {
                self.handle_mouse_down(lparam, MouseButton::Right);
                0
            }
            WM_RBUTTONUP => {
                self.handle_mouse_up(lparam, MouseButton::Right);
                0
            }
            WM_MBUTTONDOWN => {
                self.handle_mouse_down(lparam, MouseButton::Middle);
                0
            }
            WM_MBUTTONUP => {
                self.handle_mouse_up(lparam, MouseButton::Middle);
                0
            }

            WM_MOUSEMOVE => {
                let pos = self.mouse_pos_from_lparam(lparam);
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
                self.push_event(UiEvent::timer(wparam as u32));
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
                    return 1;
                }
                self.def_window_proc(msg, wparam, lparam)
            }

            _ => self.def_window_proc(msg, wparam, lparam),
        }
    }

    fn handle_mouse_down(&mut self, lparam: isize, btn: MouseButton) {
        let pos = self.mouse_pos_from_lparam(lparam);
        let mods = Self::get_modifier_state();
        let mut ev = UiEvent::mouse_down(pos, btn);
        if let UiEventPayload::MouseButton(ref mut data) = ev.payload {
            data.mods = mods;
        }
        self.push_event(ev);
        unsafe {
            SetCapture(self.hwnd);
        }
    }

    fn handle_mouse_up(&mut self, lparam: isize, btn: MouseButton) {
        let pos = self.mouse_pos_from_lparam(lparam);
        let mods = Self::get_modifier_state();
        let mut ev = UiEvent::mouse_up(pos, btn);
        if let UiEventPayload::MouseButton(ref mut data) = ev.payload {
            data.mods = mods;
        }
        self.push_event(ev);
        unsafe {
            ReleaseCapture();
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
//
// # Safety
//
// 1. `WM_NCCREATE` 写入 `GWLP_USERDATA` 存储 `*mut Win32Platform`
// 2. 该指针在窗口销毁前有效
// 3. 窗口过程在 Win32Platform 所属线程上调用
// 4. 调用期间没有其他 `&mut Win32Platform` 引用
// ════════════════════════════════════════════════════════════════════════════

unsafe extern "system" fn wnd_proc(
    hwnd: *mut std::ffi::c_void,
    msg: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    if msg == WM_NCCREATE {
        let cs = lparam as *const CREATESTRUCTW;
        let this_ptr = (*cs).lpCreateParams;
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, this_ptr as isize);
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }

    // 从 GWLP_USERDATA 重建 &mut Win32Platform
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
    if ptr == 0 {
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    let platform = &mut *(ptr as *mut Win32Platform);
    platform.handle_message(msg, wparam, lparam)
}

// ════════════════════════════════════════════════════════════════════════════
// IWindowManager — 窗口生命周期管理
// ════════════════════════════════════════════════════════════════════════════

impl IWindowManager for Win32Platform {
    fn create_window(&mut self, title: &str, width: i32, height: i32) -> bool {
        if self.class_atom == 0 && !self.register_class() {
            return false;
        }

        let wide_title = to_wide(title);
        let class_name = self.class_name();
        let style = WS_OVERLAPPEDWINDOW;

        unsafe {
            let mut rect = RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: height,
            };
            AdjustWindowRectEx(&mut rect, style, FALSE, WS_EX_APPWINDOW);
            let win_w = rect.right - rect.left;
            let win_h = rect.bottom - rect.top;

            let hwnd = CreateWindowExW(
                WS_EX_APPWINDOW,
                class_name.as_ptr(),
                wide_title.as_ptr(),
                style,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                win_w,
                win_h,
                ptr::null_mut(),
                ptr::null_mut(),
                self.hinstance,
                self as *mut Win32Platform as *mut std::ffi::c_void,
            );

            if hwnd.is_null() {
                return false;
            }
            self.hwnd = hwnd;
            self.width = width;
            self.height = height;
            self.visible = false;

            // 将 hwnd 同步到所有需要它的子系统中
            self.clipboard_subsys.set_hwnd(hwnd);
            self.cursor_subsys.set_hwnd(hwnd);
            self.file_dialog_subsys.set_hwnd(hwnd);
            self.text_input_subsys.set_hwnd(hwnd);
            self.timer_subsys.set_hwnd(hwnd);
            self.notification_subsys.set_hwnd(hwnd);

            // 创建 GDI 呈现器（关联窗口的 DIB section）
            match GdiPresenter::new(hwnd, width, height) {
                Ok(p) => self.presenter = Box::new(p),
                Err(e) => {
                    log::warn!(
                        "Win32Platform: GdiPresenter creation failed ({}), display disabled",
                        e.short_what()
                    );
                }
            }
        }
        true
    }

    fn destroy_window(&mut self) {
        if !self.hwnd.is_null() {
            unsafe {
                DestroyWindow(self.hwnd);
            }
            self.hwnd = ptr::null_mut();
            self.visible = false;
        }
    }

    fn set_title(&mut self, title: &str) {
        let wide = to_wide(title);
        unsafe {
            SetWindowTextW(self.hwnd, wide.as_ptr());
        }
    }

    fn show(&mut self) {
        unsafe {
            ShowWindow(self.hwnd, SW_SHOWNORMAL);
        }
        self.visible = true;
    }

    fn hide(&mut self) {
        unsafe {
            ShowWindow(self.hwnd, SW_HIDE);
        }
        self.visible = false;
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn center_on_screen(&mut self) {
        unsafe {
            let sw = GetSystemMetrics(SM_CXSCREEN);
            let sh = GetSystemMetrics(SM_CYSCREEN);
            let x = (sw - self.width) / 2;
            let y = (sh - self.height) / 2;
            self.set_position(x, y);
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
                SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED,
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

    fn flash_window(&mut self) {
        unsafe {
            let mut fi = FLASHWINFO {
                cbSize: std::mem::size_of::<FLASHWINFO>() as u32,
                hwnd: self.hwnd,
                dwFlags: FLASHW_ALL | FLASHW_TIMERNOFG,
                uCount: 0,
                dwTimeout: 0,
            };
            FlashWindowEx(&mut fi);
        }
    }

    fn set_window_icon(&mut self, icon_path: &str) {
        let wide = to_wide(icon_path);
        unsafe {
            let hicon = LoadImageW(
                self.hinstance,
                wide.as_ptr(),
                IMAGE_ICON,
                0,
                0,
                LR_LOADFROMFILE | LR_DEFAULTSIZE,
            ) as *mut std::ffi::c_void;
            if !hicon.is_null() {
                SendMessageW(self.hwnd, WM_SETICON, ICON_BIG, hicon as isize);
                SendMessageW(self.hwnd, WM_SETICON, ICON_SMALL, hicon as isize);
            }
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// IWindowProperties — 窗口属性
// ════════════════════════════════════════════════════════════════════════════

impl IWindowProperties for Win32Platform {
    fn width(&self) -> i32 {
        self.width
    }
    fn height(&self) -> i32 {
        self.height
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

    fn position(&self) -> Point {
        Point::new(self.pos_x as f32, self.pos_y as f32)
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
                SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
            );
        }
    }

    fn set_resizable(&mut self, resizable: bool) {
        self.resizable = resizable;
        unsafe {
            let style = GetWindowLongW(self.hwnd, GWL_STYLE) as u32;
            let new_style = if resizable {
                style | WS_THICKFRAME
            } else {
                style & !WS_THICKFRAME
            };
            SetWindowLongW(self.hwnd, GWL_STYLE, new_style as i32);
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

    fn is_maximized(&self) -> bool {
        self.maximized
    }
    fn is_minimized(&self) -> bool {
        self.minimized
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

    fn set_borderless(&mut self, borderless: bool) {
        self.borderless = borderless;
        unsafe {
            let style = GetWindowLongW(self.hwnd, GWL_STYLE) as u32;
            let new_style = if borderless {
                style & !WS_OVERLAPPEDWINDOW
            } else {
                style | WS_OVERLAPPEDWINDOW
            };
            SetWindowLongW(self.hwnd, GWL_STYLE, new_style as i32);
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

    fn set_fullscreen(&mut self, fullscreen: bool) {
        if fullscreen == self.fullscreen {
            return;
        }
        self.fullscreen = fullscreen;
        if fullscreen {
            unsafe {
                SetWindowLongW(self.hwnd, GWL_STYLE, (WS_POPUP | WS_VISIBLE) as i32);
                let sw = GetSystemMetrics(SM_CXSCREEN);
                let sh = GetSystemMetrics(SM_CYSCREEN);
                SetWindowPos(
                    self.hwnd,
                    HWND_TOPMOST as *mut std::ffi::c_void,
                    0,
                    0,
                    sw,
                    sh,
                    SWP_FRAMECHANGED,
                );
            }
        } else {
            unsafe {
                let mut flags = WS_OVERLAPPEDWINDOW | WS_VISIBLE;
                if !self.resizable {
                    flags &= !WS_THICKFRAME;
                }
                SetWindowLongW(self.hwnd, GWL_STYLE, flags as i32);
                SetWindowPos(
                    self.hwnd,
                    HWND_NOTOPMOST as *mut std::ffi::c_void,
                    self.pos_x,
                    self.pos_y,
                    self.width,
                    self.height,
                    SWP_FRAMECHANGED,
                );
            }
        }
    }

    fn is_fullscreen(&self) -> bool {
        self.fullscreen
    }

    fn set_always_on_top(&mut self, on: bool) {
        self.always_on_top = on;
        unsafe {
            let pos = if on { HWND_TOPMOST } else { HWND_NOTOPMOST };
            SetWindowPos(
                self.hwnd,
                pos as *mut std::ffi::c_void,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE,
            );
        }
    }

    fn set_window_opacity(&mut self, opacity: f32) {
        self.opacity = opacity;
        if opacity < 1.0 {
            unsafe {
                let ex_style = GetWindowLongW(self.hwnd, GWL_EXSTYLE) as u32;
                SetWindowLongW(self.hwnd, GWL_EXSTYLE, (ex_style | WS_EX_LAYERED) as i32);
                SetLayeredWindowAttributes(self.hwnd, 0, (opacity * 255.0) as u8, LWA_ALPHA);
            }
        }
    }

    fn start_text_input(&mut self) {
        self.text_input_active = true;
        self.text_input_subsys.start();
    }
    fn stop_text_input(&mut self) {
        self.text_input_active = false;
        self.text_input_subsys.stop();
    }

    fn enable_file_drop(&mut self, enable: bool) {
        self.file_drop_enabled = enable;
        unsafe {
            DragAcceptFiles(self.hwnd, if enable { TRUE } else { FALSE });
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// IEventLoop — 事件循环
// ════════════════════════════════════════════════════════════════════════════

impl IEventLoop for Win32Platform {
    fn poll_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool {
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_QUIT {
                    return false;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        while let Some(event) = self.event_queue.pop_front() {
            if !callback(&event) {
                return false;
            }
        }
        true
    }

    fn wait_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool {
        // Drain pending events first
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_QUIT {
                    return false;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        while let Some(event) = self.event_queue.pop_front() {
            if !callback(&event) {
                return false;
            }
        }
        // Wait for new message
        unsafe {
            let mut msg = MSG::default();
            let ret = GetMessageW(&mut msg, ptr::null_mut(), 0, 0);
            if ret <= 0 {
                return false;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        while let Some(event) = self.event_queue.pop_front() {
            if !callback(&event) {
                return false;
            }
        }
        true
    }
}

// ════════════════════════════════════════════════════════════════════════════
// INativeHandle — 原生窗口句柄
// ════════════════════════════════════════════════════════════════════════════

impl INativeHandle for Win32Platform {
    fn native_window(&self) -> *mut std::ffi::c_void {
        self.hwnd
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Platform — 聚合接口实现（子系统访问器委托给组合实例）
// ════════════════════════════════════════════════════════════════════════════

impl Platform for Win32Platform {
    fn present_pixels(&mut self, pixels: &[u32], width: i32, height: i32) {
        let _ = self.presenter.present(pixels, width, height);
    }
    fn presenter(&mut self) -> &mut dyn IPresenter {
        self.presenter.as_mut()
    }
    fn clipboard(&mut self) -> &mut dyn IClipboard {
        &mut self.clipboard_subsys
    }
    fn cursor(&mut self) -> &mut dyn ICursor {
        &mut self.cursor_subsys
    }
    fn display(&self) -> &dyn IDisplay {
        &self.display_subsys
    }
    fn file_dialog(&mut self) -> &mut dyn IFileDialog {
        &mut self.file_dialog_subsys
    }
    fn keyboard(&self) -> &dyn IKeyboard {
        &self.keyboard_subsys
    }
    fn text_input(&mut self) -> &mut dyn ITextInput {
        &mut self.text_input_subsys
    }
    fn timer(&mut self) -> &mut dyn ITimer {
        &mut self.timer_subsys
    }
    fn notification(&mut self) -> &mut dyn INotification {
        &mut self.notification_subsys
    }
    fn console(&mut self) -> &mut dyn IConsole {
        &mut self.console_subsys
    }
    fn file_system(&self) -> &dyn IFileSystem {
        &self.file_system_subsys
    }
    fn system_info(&self) -> &dyn ISystemInfo {
        &self.system_info_subsys
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Drop — 清理资源
// ════════════════════════════════════════════════════════════════════════════

impl Drop for Win32Platform {
    fn drop(&mut self) {
        // 移除通知图标
        self.notification_subsys.remove_icon();
        // 销毁窗口
        self.destroy_window();
    }
}

// ════════════════════════════════════════════════════════════════════════════
