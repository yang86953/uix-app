// ============================================================================
// platform/windows/platform.rs — WindowsPlatform 核心
//
// 结构体定义、OsEventSource、IWindowManager、Platform 访问器。
// 窗口过程 → wnd_proc.rs  ·  辅助方法 → helpers.rs
// ============================================================================

#![cfg(windows)]
#![allow(non_snake_case)]

use std::cell::RefCell;
use std::collections::{BTreeMap, HashSet, VecDeque};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use super::bindings::*;
use super::clipboard::WindowsClipboard;
use super::console::WindowsConsole;
use super::consts::*;
use super::cursor::WindowsCursor;
use super::display::WindowsDisplay;
use super::ffi::*;
use super::file_dialog::WindowsFileDialog;
use super::filesystem::WindowsFileSystem;
use super::gdi_presenter::GdiPresenter;
use super::keyboard::WindowsKeyboard;
use super::notification::WindowsNotification;
use super::system_info::WindowsSystemInfo;
use super::text_input::{WindowsImeState, WindowsTextInput};
use super::timer::WindowsTimer;
use super::util::{to_wide, windows_diag};
use super::window_ops::WindowsWindowOps;
use crate::core::WindowId;
use crate::native::shared::{OsEventSource, PlatformWindowCore, WindowState};
use crate::native::traits::event::{EventLoopWaker, UiEvent};
use crate::native::traits::*;
use crate::native::{Errc, Error};
// ════════════════════════════════════════════════════════════════════════════
// WindowsPlatform
// ════════════════════════════════════════════════════════════════════════════

pub(crate) struct WindowBinding {
    pub(crate) platform: *mut WindowsPlatform,
    pub(crate) state: Rc<RefCell<WindowState>>,
    pub(crate) ime: RefCell<WindowsImeState>,
}

pub struct WindowsPlatform {
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
    window_handles: BTreeMap<WindowId, usize>,
    next_window_id: u64,
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
            event_queue: VecDeque::new(),
            hwnd: std::ptr::null_mut(),
            hinstance: std::ptr::null_mut(),
            class_atom: 0,
            single_shot_timers: single_shot,
            next_window_id: 1,
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
            window_handles: BTreeMap::new(),
        }
    }

    pub(crate) fn select_window(&mut self, hwnd: *mut std::ffi::c_void) {
        if hwnd.is_null() {
            return;
        }
        self.hwnd = hwnd;
        self.clipboard_subsys.set_hwnd(hwnd);
        self.cursor_subsys.set_hwnd(hwnd);
        self.file_dialog_subsys.set_hwnd(hwnd);
        self.text_input_subsys.set_hwnd(hwnd);
        self.timer_subsys.set_hwnd(hwnd);
        self.notification_subsys.set_hwnd(hwnd);
    }

    pub(crate) fn forget_window(&mut self, window_id: WindowId) {
        let removed = self.window_handles.remove(&window_id);
        if removed == Some(self.hwnd as usize) {
            if let Some(hwnd) = self.window_handles.values().next().copied() {
                self.select_window(hwnd as *mut std::ffi::c_void);
            } else {
                self.hwnd = std::ptr::null_mut();
            }
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// OsEventSource
// ════════════════════════════════════════════════════════════════════════════

impl OsEventSource for WindowsPlatform {
    fn waker(&self) -> EventLoopWaker {
        let hwnd = self.hwnd as usize;
        EventLoopWaker::new(move || {
            if hwnd == 0 {
                return;
            }
            unsafe {
                PostMessageW(hwnd as *mut std::ffi::c_void, WM_NULL, 0, 0);
            }
        })
    }

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
                MsgWaitForMultipleObjects(0, std::ptr::null(), 0, INFINITE, QS_ALLINPUT);
                // 否则有消息到达，循环回去 PeekMessage 处理
            }
        }
    }

    fn dispatch_timeout(&mut self, timeout: std::time::Duration) -> bool {
        if !self.dispatch_pending() {
            return false;
        }
        let timeout_ms = timeout.as_millis().min(u32::MAX as u128) as u32;
        let result =
            unsafe { MsgWaitForMultipleObjects(0, std::ptr::null(), 0, timeout_ms, QS_ALLINPUT) };
        if result != WAIT_TIMEOUT && !self.dispatch_pending() {
            return false;
        }
        true
    }

    fn next_event(&mut self) -> Option<UiEvent> {
        let event = self.event_queue.pop_front()?;
        if let Some(window_id) = event.window_id {
            if let Some(hwnd) = self.window_handles.get(&window_id).copied() {
                self.select_window(hwnd as *mut std::ffi::c_void);
            }
        }
        Some(event)
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
        let window_id = WindowId::new(self.next_window_id);
        self.next_window_id += 1;
        let state = Rc::new(RefCell::new(WindowState {
            window_id,
            width,
            height,
            ..WindowState::default()
        }));
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

            let binding = Box::new(WindowBinding {
                platform: self as *mut WindowsPlatform,
                state: Rc::clone(&state),
                ime: RefCell::new(WindowsImeState::default()),
            });
            let binding_ptr = Box::into_raw(binding);

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
                binding_ptr as *mut std::ffi::c_void,
            );
            if hwnd.is_null() {
                drop(Box::from_raw(binding_ptr));
                return Err(windows_diag(
                    Errc::WindowCreationFailed,
                    "CreateWindowExW returned null",
                ));
            }
            let binding = Box::from_raw(binding_ptr);
            self.window_handles.insert(window_id, hwnd as usize);
            self.select_window(hwnd);

            let presenter: Box<dyn IPresenter> = match GdiPresenter::new(hwnd, width, height) {
                Ok(p) => Box::new(p),
                Err(e) => {
                    crate::core::log::warn_fn(format!(
                        "GdiPresenter failed ({}), using null",
                        e.short_what()
                    ));
                    Box::new(crate::native::presenter::NullPresenter::new())
                }
            };

            let ops = WindowsWindowOps::new(hwnd, binding);
            let core = PlatformWindowCore::new(state, ops, presenter);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;
    use crate::native::traits::event::{UiEventPayload, UiEventType};

    fn size_lparam(width: u16, height: u16) -> isize {
        (u32::from(width) | (u32::from(height) << 16)) as isize
    }

    #[test]
    fn native_windows_keep_independent_state_and_route_resize_events() {
        let mut platform = WindowsPlatform::new();
        let mut first = platform
            .create_window("UIX multi-window route A", 320, 200)
            .expect("first native window");
        let mut second = platform
            .create_window("UIX multi-window route B", 480, 300)
            .expect("second native window");

        let first_id = first.window_id();
        let second_id = second.window_id();
        assert_ne!(first_id, second_id);
        assert_eq!(
            (first.properties().width(), first.properties().height()),
            (320, 200)
        );
        assert_eq!(
            (second.properties().width(), second.properties().height()),
            (480, 300)
        );

        let first_hwnd = first.native_handle().native_window();
        let second_hwnd = second.native_handle().native_window();
        assert!(!first_hwnd.is_null());
        assert!(!second_hwnd.is_null());

        platform.dispatch_pending();
        while platform.next_event().is_some() {}

        unsafe {
            assert_ne!(
                PostMessageW(first_hwnd, WM_SIZE, SIZE_RESTORED, size_lparam(321, 222)),
                0
            );
            assert_ne!(
                PostMessageW(second_hwnd, WM_SIZE, SIZE_RESTORED, size_lparam(654, 333),),
                0
            );
        }
        assert!(platform.dispatch_pending());

        let mut resize_events = BTreeMap::new();
        while let Some(event) = platform.next_event() {
            if event.type_ == UiEventType::WindowResize {
                let UiEventPayload::Resize(data) = event.payload else {
                    panic!("WindowResize must carry ResizeData");
                };
                resize_events.insert(event.window_id.expect("routed window id"), data);
            }
        }

        let first_resize = resize_events.get(&first_id).expect("first resize event");
        let second_resize = resize_events.get(&second_id).expect("second resize event");
        assert_eq!((first_resize.width, first_resize.height), (321, 222));
        assert_eq!((second_resize.width, second_resize.height), (654, 333));
        assert_eq!(
            (first.properties().width(), first.properties().height()),
            (321, 222)
        );
        assert_eq!(
            (second.properties().width(), second.properties().height()),
            (654, 333)
        );

        second.close().expect("close secondary window");
        assert!(platform.dispatch_pending());
        first.close().expect("close primary window");
        assert!(platform.dispatch_pending());
    }

    #[test]
    fn native_text_input_routes_ime_lifecycle_and_surrogate_pair() {
        let mut platform = WindowsPlatform::new();
        let mut window = platform
            .create_window("UIX text input route", 320, 200)
            .expect("native window");
        let window_id = window.window_id();
        let hwnd = window.native_handle().native_window();
        assert!(!hwnd.is_null());

        assert!(platform.dispatch_pending());
        while platform.next_event().is_some() {}

        platform.text_input().start().expect("start text input");
        platform
            .text_input()
            .set_cursor_rect(Rect::new(12.0, 16.0, 2.0, 18.0))
            .expect("position IME candidate window");

        unsafe {
            assert_ne!(PostMessageW(hwnd, WM_IME_STARTCOMPOSITION, 0, 0), 0);
            assert_ne!(PostMessageW(hwnd, WM_CHAR, 0xD83D, 0), 0);
            assert_ne!(PostMessageW(hwnd, WM_CHAR, 0xDE00, 0), 0);
            assert_ne!(PostMessageW(hwnd, WM_IME_ENDCOMPOSITION, 0, 0), 0);
        }
        assert!(platform.dispatch_pending());

        let events: Vec<_> = std::iter::from_fn(|| platform.next_event()).collect();
        assert_eq!(
            events
                .iter()
                .map(|event| event.type_)
                .collect::<Vec<_>>(),
            vec![
                UiEventType::ImeCompositionStart,
                UiEventType::TextInput,
                UiEventType::ImeCompositionEnd,
            ]
        );
        assert!(events.iter().all(|event| event.window_id == Some(window_id)));
        let UiEventPayload::TextInput(text) = &events[1].payload else {
            panic!("expected text payload");
        };
        assert_eq!(text.text, "😀");

        platform.text_input().stop().expect("stop text input");
        window.close().expect("close native window");
        assert!(platform.dispatch_pending());
    }
}
