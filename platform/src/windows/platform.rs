// ============================================================================
// platform/windows/platform.rs — WindowsPlatform 核心
//
// 结构体定义、OsEventSource、IWindowManager、Platform 访问器。
// 窗口过程 → wnd_proc.rs  ·  辅助方法 → helpers.rs
// ============================================================================

#![cfg(windows)]
#![allow(non_snake_case)]

use std::cell::RefCell;
use std::collections::{HashSet, VecDeque};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use crate::event::UiEvent;
use crate::shared::{OsEventSource, PlatformWindowCore, WindowState};
use crate::*;
use crate::{Errc, Error};

use super::bindings::*;
use super::clipboard::WindowsClipboard;
use super::console::WindowsConsole;
use super::consts::*;
use super::cursor::WindowsCursor;
use super::display::WindowsDisplay;
use super::ffi::*;
use super::file_dialog::WindowsFileDialog;
use super::filesystem::WindowsFileSystem;
use super::gpu::GdiPresenter;
use super::keyboard::WindowsKeyboard;
use super::notification::WindowsNotification;
use super::system_info::WindowsSystemInfo;
use super::text_input::WindowsTextInput;
use super::timer::WindowsTimer;
use super::util::to_wide;
use super::window_ops::WindowsWindowOps;

// ════════════════════════════════════════════════════════════════════════════
// WindowsPlatform
// ════════════════════════════════════════════════════════════════════════════

pub struct WindowsPlatform {
    pub(crate) window: Rc<RefCell<WindowState>>,
    pub(crate) event_queue: VecDeque<UiEvent>,
    pub(crate) event_bus: EventBus,
    pub(crate) hwnd: *mut std::ffi::c_void,
    pub(crate) hinstance: *mut std::ffi::c_void,
    pub(crate) class_atom: u16,
    pub(crate) clipboard_subsys: WindowsClipboard,
    pub(crate) cursor_subsys: WindowsCursor,
    pub(crate) display_subsys: WindowsDisplay,
    pub(crate) file_dialog_subsys: WindowsFileDialog,
    pub(crate) file_system_subsys: WindowsFileSystem,
    pub(crate) keyboard_subsys: WindowsKeyboard,
    pub(crate) text_input_subsys: WindowsTextInput,
    pub(crate) timer_subsys: WindowsTimer,
    pub(crate) notification_subsys: WindowsNotification,
    pub(crate) console_subsys: WindowsConsole,
    pub(crate) system_info_subsys: WindowsSystemInfo,
    pub(crate) single_shot_timers: Arc<Mutex<HashSet<u32>>>,
}

impl Default for WindowsPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowsPlatform {
    pub fn new() -> Self {
        let timer_subsys = WindowsTimer::new();
        let single_shot = timer_subsys.non_repeating_set();
        Self {
            window: Rc::new(RefCell::new(WindowState::default())),
            event_queue: VecDeque::new(),
            hwnd: std::ptr::null_mut(),
            hinstance: std::ptr::null_mut(),
            class_atom: 0,
            single_shot_timers: single_shot,
            clipboard_subsys: WindowsClipboard::new(),
            cursor_subsys: WindowsCursor::new(),
            display_subsys: WindowsDisplay::new(),
            file_dialog_subsys: WindowsFileDialog::new(),
            file_system_subsys: WindowsFileSystem::new(),
            keyboard_subsys: WindowsKeyboard::new(),
            text_input_subsys: WindowsTextInput::new(),
            timer_subsys,
            notification_subsys: WindowsNotification::new(),
            event_bus: EventBus::new(),
            console_subsys: WindowsConsole::new(),
            system_info_subsys: WindowsSystemInfo::new(),
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// OsEventSource
// ════════════════════════════════════════════════════════════════════════════

impl OsEventSource for WindowsPlatform {
    fn dispatch_pending(&mut self) -> bool {
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_QUIT {
                    return false;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        true
    }

    /// 阻塞等待 OS 事件，但最多等待约 16ms（约 60fps）。
    /// 即使无事件也周期性返回，保证动画帧节奏。
    fn dispatch_blocking(&mut self) -> bool {
        loop {
            unsafe {
                // 先非阻塞检查是否有待处理消息
                let mut msg = MSG::default();
                if PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                    if msg.message == WM_QUIT {
                        return false;
                    }
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                    return true;
                }
                // 无消息——等待消息或超时（16ms 帧间隔）
                let result = MsgWaitForMultipleObjects(
                    0, // 不等待任何内核对象
                    std::ptr::null(),
                    0,           // fWaitAll = FALSE
                    16,          // 16ms 超时 ≈ 60fps
                    QS_ALLINPUT, // 任何输入消息都能唤醒
                );
                if result == WAIT_TIMEOUT {
                    // 超时：无事件，但返回以继续帧循环
                    return true;
                }
                // 否则有消息到达，循环回去 PeekMessage 处理
            }
        }
    }

    fn dispatch_timeout(&mut self, timeout: std::time::Duration) -> bool {
        if !self.dispatch_pending() {
            return false;
        }
        unsafe {
            let mut msg = MSG::default();
            if PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_QUIT {
                    return false;
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            } else {
                std::thread::sleep(timeout);
            }
        }
        true
    }

    fn next_event(&mut self) -> Option<UiEvent> {
        self.event_queue.pop_front()
    }
}

// ════════════════════════════════════════════════════════════════════════════
// IWindowManager
// ════════════════════════════════════════════════════════════════════════════

impl IWindowManager for WindowsPlatform {
    fn create_window(
        &mut self,
        title: &str,
        width: i32,
        height: i32,
    ) -> Result<Box<dyn PlatformWindow>, Error> {
        if self.class_atom == 0 {
            self.register_class()?;
        }
        {
            let mut state = self.window.borrow_mut();
            state.width = width;
            state.height = height;
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
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                self.hinstance,
                self as *mut WindowsPlatform as *mut std::ffi::c_void,
            );
            if hwnd.is_null() {
                return Err(Error::new(
                    Errc::WindowCreationFailed,
                    "CreateWindowExW returned null",
                ));
            }
            self.hwnd = hwnd;

            self.clipboard_subsys.set_hwnd(hwnd);
            self.cursor_subsys.set_hwnd(hwnd);
            self.file_dialog_subsys.set_hwnd(hwnd);
            self.text_input_subsys.set_hwnd(hwnd);
            self.timer_subsys.set_hwnd(hwnd);
            self.notification_subsys.set_hwnd(hwnd);

            let presenter: Box<dyn IPresenter> = match GdiPresenter::new(hwnd, width, height) {
                Ok(p) => Box::new(p),
                Err(e) => {
                    crate::log::warn_fn(format!(
                        "GdiPresenter failed ({}), using null",
                        e.short_what()
                    ));
                    Box::new(crate::presenter::NullPresenter::new())
                }
            };

            let ops = WindowsWindowOps::new(hwnd);
            let core = PlatformWindowCore::new(Rc::clone(&self.window), ops, presenter);
            Ok(Box::new(core))
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Platform 访问器
// ════════════════════════════════════════════════════════════════════════════

impl Platform for WindowsPlatform {
    fn window_manager(&mut self) -> &mut dyn IWindowManager {
        self
    }
    fn event_loop(&mut self) -> &mut dyn IEventLoop {
        self
    }
    fn event_bus(&mut self) -> &mut EventBus {
        &mut self.event_bus
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

impl Drop for WindowsPlatform {
    fn drop(&mut self) {
        self.notification_subsys.remove_icon();
    }
}
