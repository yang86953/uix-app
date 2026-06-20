// ============================================================================
// uix-platform/src/windows/platform.rs — Windows Platform trait implementation
//
// 职责范围：
//   仅保留 struct 定义、构造方法。所有方法实现已拆分到同目录子模块：
//     consts.rs            — 平台常量
//     helpers.rs           — 内部辅助方法（register_class, vk_to_keycode 等）
//     wnd_proc.rs          — 窗口过程回调 & 消息分发
//     iwindow_manager.rs   — IWindowManager 实现
//     iwindow_properties.rs — IWindowProperties 实现
//     ievent_loop.rs       — IEventLoop 实现
//     platform_impl.rs     — INativeHandle / Platform / Drop 实现
// ============================================================================
//
// # Safety
//
// 窗口过程 `wnd_proc` 通过 `GWLP_USERDATA` 存储的 `*mut WindowsPlatform` 指针
// 重建 `&mut WindowsPlatform` 引用。安全前提：
//   1. WindowsPlatform 必须在创建窗口的同一线程上存活
//   2. `WM_NCCREATE` 中必须写入有效指针
//   3. 窗口过程运行时不能有其他 `&mut` 引用共存
//   4. WindowsPlatform 的 Drop 必须在窗口销毁后执行
//
// ============================================================================

use crate::platform::event::UiEvent;
use crate::platform::presenter::NullPresenter;
use crate::platform::windows::clipboard::WindowsClipboard;
use crate::platform::windows::console::WindowsConsole;
use crate::platform::windows::cursor::WindowsCursor;
use crate::platform::windows::display::WindowsDisplay;
use crate::platform::windows::file_dialog::WindowsFileDialog;
use crate::platform::windows::filesystem::WindowsFileSystem;
use crate::platform::windows::keyboard::WindowsKeyboard;
use crate::platform::windows::notification::WindowsNotification;
use crate::platform::windows::system_info::WindowsSystemInfo;
use crate::platform::windows::text_input::WindowsTextInput;
use crate::platform::windows::timer::WindowsTimer;

use crate::platform::*;

use std::collections::VecDeque;
use std::ptr;

// ════════════════════════════════════════════════════════════════════════════
// WindowsPlatform — 主平台结构体
// ════════════════════════════════════════════════════════════════════════════

pub struct WindowsPlatform {
    // ── 窗口句柄与状态 ──────────────────────────────────────────────
    pub(crate) hwnd: *mut std::ffi::c_void,
    pub(crate) hinstance: *mut std::ffi::c_void,
    pub(crate) class_atom: u16,

    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) pos_x: i32,
    pub(crate) pos_y: i32,

    pub(crate) min_w: i32,
    pub(crate) min_h: i32,
    pub(crate) max_w: i32,
    pub(crate) max_h: i32,

    pub(crate) visible: bool,
    pub(crate) resizable: bool,
    pub(crate) borderless: bool,
    pub(crate) fullscreen: bool,
    pub(crate) always_on_top: bool,
    pub(crate) minimized: bool,
    pub(crate) maximized: bool,
    pub(crate) opacity: f32,
    pub(crate) file_drop_enabled: bool,
    pub(crate) text_input_active: bool,

    // ── 事件队列（窗口过程写入，事件循环读取）───────────────────
    pub(crate) event_queue: VecDeque<UiEvent>,

    // ── 单次定时器追踪（来自 timer_subsys，WM_TIMER 触发后自动清理）─
    pub(crate) single_shot_timers: Arc<std::sync::Mutex<std::collections::HashSet<u32>>>,

    // ── 像素呈现器（默认 NullPresenter，窗口创建后替换为 GdiPresenter）─
    pub(crate) presenter: Box<dyn IPresenter>,

    // ── 组合子系统（独立 struct，通过 accessor 暴露）─────────────
    pub(crate) clipboard_subsys: WindowsClipboard,
    pub(crate) cursor_subsys: WindowsCursor,
    pub(crate) display_subsys: WindowsDisplay,
    pub(crate) file_dialog_subsys: WindowsFileDialog,
    pub(crate) keyboard_subsys: WindowsKeyboard,
    pub(crate) text_input_subsys: WindowsTextInput,
    pub(crate) timer_subsys: WindowsTimer,
    pub(crate) notification_subsys: WindowsNotification,
    pub(crate) console_subsys: WindowsConsole,
    pub(crate) file_system_subsys: WindowsFileSystem,
    pub(crate) system_info_subsys: WindowsSystemInfo,
}

// ════════════════════════════════════════════════════════════════════════════
// 构造
// ════════════════════════════════════════════════════════════════════════════

impl WindowsPlatform {
    pub fn new() -> Self {
        let timer_subsys = WindowsTimer::new();
        let single_shot = timer_subsys.non_repeating_set();
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
            single_shot_timers: single_shot,
            presenter: Box::new(NullPresenter::new()),
            clipboard_subsys: WindowsClipboard::new(),
            cursor_subsys: WindowsCursor::new(),
            display_subsys: WindowsDisplay::new(),
            file_dialog_subsys: WindowsFileDialog::new(),
            keyboard_subsys: WindowsKeyboard::new(),
            text_input_subsys: WindowsTextInput::new(),
            timer_subsys,
            notification_subsys: WindowsNotification::new(),
            console_subsys: WindowsConsole::new(),
            file_system_subsys: WindowsFileSystem::new(),
            system_info_subsys: WindowsSystemInfo::new(),
        }
    }
}

impl Default for WindowsPlatform {
    fn default() -> Self {
        Self::new()
    }
}
