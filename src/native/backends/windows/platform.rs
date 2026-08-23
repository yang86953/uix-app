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
use std::sync::{Arc, Mutex, MutexGuard};

use super::bindings::*;
use super::clipboard::WindowsClipboard;
use super::console::WindowsConsole;
use super::consts::*;
use super::cursor::WindowsCursor;
use super::display::WindowsDisplay;
use super::dpi::{PerMonitorV2Scope, dpi_for_system, outer_size_for_logical_client};
use super::ffi::*;
use super::file_dialog::WindowsFileDialog;
use super::filesystem::WindowsFileSystem;
use super::frame_pacer::{SharedWindowsFramePacerState, shared_frame_pacer_state};
use super::gdi_presenter::GdiPresenter;
use super::keyboard::WindowsKeyboard;
use super::notification::WindowsNotification;
use super::system_info::WindowsSystemInfo;
use super::text_input::{WindowsImeState, WindowsTextInput};
use super::timer::WindowsTimer;
use super::util::{to_wide, windows_diag};
use super::window_ops::WindowsWindowOps;
use crate::core::WindowId;
use crate::diagnostics::{PendingFailureQueue, PendingFailureSource};
use crate::platform::presentation::*;
use crate::native::windowing::shared::{OsEventSource, PlatformWindowCore, WindowState};
use crate::platform::display::IDisplay;
use crate::platform::platform::Platform;
use crate::platform::system::console::IConsole;
use crate::platform::system::filesystem::IFileSystem;
use crate::platform::system::info::ISystemInfo;
use crate::platform::system::{IFileDialog, INotification, ITimer};
use crate::platform::windowing::{IClipboard, ICursor, IKeyboard, ITextInput};
use crate::native::{Errc, Error};
use crate::platform::presentation::IPresenter;
use crate::platform::windowing::event::{EventBus, EventLoopWaker, IEventLoop, UiEvent};
use crate::platform::windowing::window::{IWindowManager, PlatformWindow};
// ════════════════════════════════════════════════════════════════════════════
// WindowsPlatform
// ════════════════════════════════════════════════════════════════════════════

pub(crate) struct WindowBinding {
    pub(crate) platform: *mut WindowsPlatform,
    pub(crate) state: Rc<RefCell<WindowState>>,
    pub(crate) ime: RefCell<WindowsImeState>,
    pub(crate) frame_pacer: SharedWindowsFramePacerState,
    // 自定义边框缩放必须留在正常消息泵，避免 Win32 模态循环冻结响应式布局。
    pub(crate) resize_drag: RefCell<Option<super::window_interaction::WindowsResizeDrag>>,
}

// Windows 平台聚合只由 crate 内部工厂创建。
pub(crate) struct WindowsPlatform {
    pub(crate) event_queue: Arc<Mutex<VecDeque<UiEvent>>>,
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
    pending_failures: PendingFailureSource,
    system_dark_mode: bool,
    window_handles: BTreeMap<WindowId, usize>,
    next_window_id: u64,
}

impl Default for WindowsPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowsPlatform {
    // 创建使用独立失败队列的 Windows 平台聚合。
    pub(crate) fn new() -> Self {
        Self::new_with_pending(PendingFailureQueue::new())
    }

    pub(crate) fn new_with_pending(pending_failures: PendingFailureQueue) -> Self {
        let timer_subsys = WindowsTimer::new();
        let single_shot = timer_subsys.non_repeating_set();
        let event_queue = Arc::new(Mutex::new(VecDeque::new()));
        let pending_source = pending_failures.source();
        let display_subsys = WindowsDisplay::new();
        let system_dark_mode = WindowsDisplay::detect_os_theme().unwrap_or(false);
        Self {
            event_queue: Arc::clone(&event_queue),
            hwnd: std::ptr::null_mut(),
            hinstance: std::ptr::null_mut(),
            class_atom: 0,
            single_shot_timers: single_shot,
            next_window_id: 1,
            clipboard_subsys: WindowsClipboard::new(),
            cursor_subsys: WindowsCursor::new(),
            display_subsys,
            file_dialog_subsys: WindowsFileDialog::new(),
            file_system_subsys: WindowsFileSystem::new(),
            keyboard_subsys: WindowsKeyboard::new(),
            text_input_subsys: WindowsTextInput::new(event_queue, pending_source.clone()),
            timer_subsys,
            notification_subsys: WindowsNotification::new(),
            event_bus: EventBus::new(),
            console_subsys: WindowsConsole::new(),
            system_info_subsys: WindowsSystemInfo::new(),
            pending_failures: pending_source,
            system_dark_mode,
            window_handles: BTreeMap::new(),
        }
    }

    pub(crate) fn enqueue_callback_failure(&self, error: Error) {
        let _ = self.pending_failures.enqueue(error);
    }

    pub(crate) fn take_pending_failure(&mut self) -> Option<Error> {
        self.pending_failures.take()
    }

    pub(crate) fn lock_event_queue(&self) -> MutexGuard<'_, VecDeque<UiEvent>> {
        self.event_queue
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    pub(crate) fn route_system_theme_change(&mut self, window_id: WindowId, is_dark: bool) -> bool {
        if self.system_dark_mode == is_dark {
            return false;
        }
        self.system_dark_mode = is_dark;
        self.push_event(window_id, UiEvent::theme_changed(is_dark));
        true
    }

    pub(crate) fn select_window(&mut self, hwnd: *mut std::ffi::c_void) {
        if hwnd.is_null() {
            return;
        }
        self.hwnd = hwnd;
        self.display_subsys.set_hwnd(hwnd);
        self.clipboard_subsys.set_hwnd(hwnd);
        self.cursor_subsys.set_hwnd(hwnd);
        self.file_dialog_subsys.set_hwnd(hwnd);
        self.timer_subsys.set_hwnd(hwnd);
        self.notification_subsys.set_hwnd(hwnd);
    }

    pub(crate) fn forget_window(&mut self, window_id: WindowId) {
        self.text_input_subsys.clear_target_window(window_id);
        let removed = self.window_handles.remove(&window_id);
        if removed == Some(self.hwnd as usize) {
            if let Some(hwnd) = self.window_handles.values().next().copied() {
                self.select_window(hwnd as *mut std::ffi::c_void);
            } else {
                self.hwnd = std::ptr::null_mut();
                self.display_subsys.set_hwnd(std::ptr::null_mut());
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
            // SAFETY: 保存的整数只还原为不透明 HWND；窗口已销毁时 PostMessageW 会安全失败且不会解引用该值。
            unsafe {
                PostMessageW(hwnd as *mut std::ffi::c_void, WM_NULL, 0, 0);
            }
        })
    }

    fn dispatch_pending(&mut self) -> bool {
        // SAFETY: MSG 是已初始化的可写结构，取得的消息只交给同线程的标准 Win32 分派函数。
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
            // SAFETY: MSG 在每轮均初始化，PeekMessageW/TranslateMessage/DispatchMessageW 按同一线程消息队列契约配套使用。
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
            // SAFETY: 等待句柄数为零，因此空句柄数组合法；调用仅等待当前线程的消息队列或超时。
            unsafe { MsgWaitForMultipleObjects(0, std::ptr::null(), 0, timeout_ms, QS_ALLINPUT) };
        if result != WAIT_TIMEOUT && !self.dispatch_pending() {
            return false;
        }
        true
    }

    fn next_event(&mut self) -> Option<UiEvent> {
        let event = self.lock_event_queue().pop_front()?;
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

        // SAFETY: UTF-16 参数在同步创建期间有效；WindowBinding 的裸指针在失败路径回收，成功路径重新封装并与窗口同寿命。
        unsafe {
            let dpi_scope = PerMonitorV2Scope::enter()?;
            let frame_pacer = shared_frame_pacer_state();
            let binding = Box::new(WindowBinding {
                platform: self as *mut WindowsPlatform,
                state: Rc::clone(&state),
                ime: RefCell::new(WindowsImeState::default()),
                frame_pacer: Arc::clone(&frame_pacer),
                resize_drag: RefCell::new(None),
            });
            let binding_ptr = Box::into_raw(binding);

            let creation_result = (|| {
                let dpi = dpi_for_system();
                let (win_w, win_h) =
                    outer_size_for_logical_client(width, height, style, WS_EX_APPWINDOW, dpi)?;
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
                    Err(windows_diag(
                        Errc::WindowCreationFailed,
                        "CreateWindowExW returned null",
                    ))
                } else {
                    Ok(hwnd)
                }
            })();
            let restore_result = dpi_scope.finish();
            let hwnd = match (creation_result, restore_result) {
                (Ok(hwnd), Ok(())) => hwnd,
                (Ok(hwnd), Err(error)) => {
                    let _ = DestroyWindow(hwnd);
                    drop(Box::from_raw(binding_ptr));
                    return Err(error);
                }
                (Err(error), Ok(())) => {
                    drop(Box::from_raw(binding_ptr));
                    return Err(error);
                }
                (Err(error), Err(restore)) => {
                    let code = error.code();
                    let message = format!(
                        "{}; DPI context restore also failed: {}",
                        error.message(),
                        restore.message()
                    );
                    drop(Box::from_raw(binding_ptr));
                    return Err(Error::new(code, message).with_source(error));
                }
            };
            let binding = Box::from_raw(binding_ptr);
            self.window_handles.insert(window_id, hwnd as usize);
            self.select_window(hwnd);
            let presenter: Box<dyn IPresenter> = match GdiPresenter::new(hwnd, width, height) {
                Ok(p) => Box::new(p),
                Err(e) => {
                    tracing::warn!("GdiPresenter failed ({}), using null", e.short_what());
                    Box::new(crate::native::presentation::presenter::NullPresenter::new())
                }
            };

            let ops =
                WindowsWindowOps::new(hwnd, binding, frame_pacer, self.pending_failures.clone());
            let core = PlatformWindowCore::new(state, ops, presenter);
            Ok(Box::new(core))
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Platform 访问器
// ════════════════════════════════════════════════════════════════════════════

impl Platform for WindowsPlatform {
    fn take_pending_failure(&mut self) -> Option<Error> {
        Self::take_pending_failure(self)
    }

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
        self.pending_failures.close();
        self.notification_subsys.remove_icon();
    }
}

#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../tests/unit/native/backends/windows/platform__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
