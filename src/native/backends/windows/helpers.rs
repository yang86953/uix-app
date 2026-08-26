// ============================================================================
// platform/windows/helpers.rs — WindowsPlatform 内部辅助方法
//
// 窗口类注册、虚拟键码映射、鼠标/文件拖放处理、修饰键状态。
// ============================================================================

#![cfg(windows)]
#![allow(non_snake_case)]

use super::bindings::*;
use super::consts::*;
use super::ffi::*;
use super::platform::WindowsPlatform;
use super::util::{to_utf8, to_wide, windows_diag};
use super::wnd_proc::wnd_proc;
use crate::core::Point;
use crate::native::{Errc, Error};
use crate::platform::windowing::event::{UiEvent, UiEventPayload};
use crate::platform::windowing::{KeyCode, KeyMod, MouseButton};

const ERROR_CLASS_ALREADY_EXISTS: u32 = 1410;
// ════════════════════════════════════════════════════════════════════════════
// 窗口类注册
// ════════════════════════════════════════════════════════════════════════════

impl WindowsPlatform {
    pub(crate) fn class_name(&self) -> Vec<u16> {
        to_wide("UIX_WindowClass")
    }

    pub(crate) fn register_class(&mut self) -> Result<(), Error> {
        let class_name = self.class_name();
        // SAFETY: GetModuleHandleW(null) 返回当前进程模块句柄且无指针输入；wc 为完整初始化的类描述，class_name 为存活 NUL 结尾 UTF-16；RegisterClassExW 同步注册。
        unsafe {
            let hinstance = GetModuleHandleW(std::ptr::null_mut());
            if hinstance.is_null() {
                return Err(windows_diag(
                    Errc::PlatformError,
                    "register_class: GetModuleHandleW failed",
                ));
            }
            self.hinstance = hinstance;

            let wc = WNDCLASSEXW {
                cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
                style: DEFAULT_CLASS_STYLE,
                lpfnWndProc: Some(wnd_proc),
                cbClsExtra: 0,
                cbWndExtra: std::mem::size_of::<*mut std::ffi::c_void>() as i32,
                hInstance: hinstance,
                hIcon: LoadIconW(std::ptr::null_mut(), IDI_APPLICATION),
                hCursor: std::ptr::null_mut(),
                hbrBackground: (COLOR_APPWORKSPACE + 1) as *mut std::ffi::c_void,
                lpszMenuName: std::ptr::null(),
                lpszClassName: class_name.as_ptr(),
                hIconSm: std::ptr::null_mut(),
            };

            let atom = RegisterClassExW(&wc);
            if atom == 0 {
                if GetLastError() == ERROR_CLASS_ALREADY_EXISTS {
                    self.class_atom = 1;
                    return Ok(());
                }
                return Err(windows_diag(
                    Errc::ClassRegistrationFailed,
                    "register_class: RegisterClassExW failed",
                ));
            }
            self.class_atom = atom;
            Ok(())
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 键码映射 & 修饰键
// ════════════════════════════════════════════════════════════════════════════

impl WindowsPlatform {
    pub(crate) fn get_modifier_state() -> KeyMod {
        let mut mods = KeyMod::NONE;
        // SAFETY: GetAsyncKeyState 只接收虚拟键码常量，不依赖窗口句柄，无指针参数。
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

    pub(crate) fn vk_to_keycode(vk: u32) -> KeyCode {
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

    pub(crate) fn hiword(value: isize) -> u16 {
        ((value >> 16) & 0xFFFF) as u16
    }
    pub(crate) fn loword(value: isize) -> u16 {
        (value & 0xFFFF) as u16
    }
    pub(crate) fn loword_signed(value: isize) -> i16 {
        (value & 0xFFFF) as i16
    }
    pub(crate) fn hiword_signed(value: isize) -> i16 {
        ((value >> 16) & 0xFFFF) as i16
    }
    pub(crate) fn hiword_usize(value: usize) -> u16 {
        ((value >> 16) & 0xFFFF) as u16
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 鼠标 & 文件拖放
// ════════════════════════════════════════════════════════════════════════════

impl WindowsPlatform {
    pub(crate) fn mouse_pos_from_lparam(
        &self,
        hwnd: *mut std::ffi::c_void,
        lparam: isize,
    ) -> Point {
        let dpi = super::dpi::dpi_for_window(hwnd);
        let x = super::dpi::physical_point_to_logical(Self::loword_signed(lparam) as i32, dpi);
        let y = super::dpi::physical_point_to_logical(Self::hiword_signed(lparam) as i32, dpi);
        Point::new(x, y)
    }

    pub(crate) fn handle_mouse_down(
        &mut self,
        hwnd: *mut std::ffi::c_void,
        window_id: crate::core::WindowId,
        lparam: isize,
        btn: MouseButton,
    ) {
        self.handle_mouse_press(hwnd, window_id, lparam, btn, UiEvent::pointer_down);
    }

    pub(crate) fn handle_mouse_double_click(
        &mut self,
        hwnd: *mut std::ffi::c_void,
        window_id: crate::core::WindowId,
        lparam: isize,
        btn: MouseButton,
    ) {
        self.handle_mouse_press(hwnd, window_id, lparam, btn, UiEvent::pointer_double_click);
    }

    fn handle_mouse_press(
        &mut self,
        hwnd: *mut std::ffi::c_void,
        window_id: crate::core::WindowId,
        lparam: isize,
        btn: MouseButton,
        create_event: fn(Point, MouseButton) -> UiEvent,
    ) {
        let pos = self.mouse_pos_from_lparam(hwnd, lparam);
        let mods = Self::get_modifier_state();
        let mut ev = create_event(pos, btn);
        if let UiEventPayload::PointerButton(ref mut data) = ev.payload {
            data.mods = mods;
        }
        self.push_event(window_id, ev);
        // SAFETY: hwnd 为当前同步消息的窗口句柄且存活；SetCapture 同步设置捕获。
        unsafe {
            SetCapture(hwnd);
        }
    }

    pub(crate) fn handle_mouse_up(
        &mut self,
        hwnd: *mut std::ffi::c_void,
        window_id: crate::core::WindowId,
        lparam: isize,
        btn: MouseButton,
    ) {
        let pos = self.mouse_pos_from_lparam(hwnd, lparam);
        let mods = Self::get_modifier_state();
        let mut ev = UiEvent::pointer_up(pos, btn);
        if let UiEventPayload::PointerButton(ref mut data) = ev.payload {
            data.mods = mods;
        }
        self.push_event(window_id, ev);
        // SAFETY: ReleaseCapture 无句柄参数，释放当前线程的鼠标捕获。
        unsafe {
            ReleaseCapture();
        }
    }

    pub(crate) fn handle_file_drop(
        &mut self,
        hwnd: *mut std::ffi::c_void,
        window_id: crate::core::WindowId,
        hdrop: *mut std::ffi::c_void,
    ) {
        // SAFETY: hdrop 为 WM_DROPFILES 提供的有效 HDROP 句柄，在本次消息处理期间存活；缓冲与输出指针均为有效栈上存储；DragFinish 在最后释放该句柄一次。
        unsafe {
            let count = DragQueryFileW(hdrop, 0xFFFFFFFF, std::ptr::null_mut(), 0);
            let mut files = Vec::with_capacity(count as usize);
            let mut pt = POINT { x: 0, y: 0 };
            DragQueryPoint(hdrop, &mut pt);
            for i in 0..count {
                let len = DragQueryFileW(hdrop, i, std::ptr::null_mut(), 0);
                if len > 0 {
                    let mut buf = vec![0u16; (len + 1) as usize];
                    DragQueryFileW(hdrop, i, buf.as_mut_ptr(), len + 1);
                    files.push(to_utf8(&buf));
                }
            }
            DragFinish(hdrop);
            let dpi = super::dpi::dpi_for_window(hwnd);
            let position = Point::new(
                super::dpi::physical_point_to_logical(pt.x, dpi),
                super::dpi::physical_point_to_logical(pt.y, dpi),
            );
            self.push_event(window_id, UiEvent::file_drop(files, position));
        }
    }

    pub(crate) fn def_window_proc(
        &self,
        hwnd: *mut std::ffi::c_void,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize {
        // SAFETY: hwnd/msg/wparam/lparam 为窗口过程转发的当前消息参数，DefWindowProcW 同步处理。
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    }
}
