// ============================================================================
// native/backends/windows/timer.rs — Windows timer (ITimer)
// ============================================================================

#![cfg(windows)]

use super::ffi::{KillTimer, SetTimer};
use crate::native::{Errc, Error, Result};
use crate::platform::system::ITimer;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

// Windows 定时器后端只在 crate 内部平台注册表中构造。
pub(crate) struct WindowsTimer {
    hwnd: *mut std::ffi::c_void,
    next_id: u32,
    non_repeating: Arc<Mutex<HashSet<u32>>>,
}

impl WindowsTimer {
    // 创建未绑定窗口的定时器后端。
    pub(crate) fn new() -> Self {
        Self {
            hwnd: std::ptr::null_mut(),
            next_id: 1,
            non_repeating: Arc::new(Mutex::new(HashSet::new())),
        }
    }
    // 绑定当前平台窗口句柄。
    pub(crate) fn set_hwnd(&mut self, hwnd: *mut std::ffi::c_void) {
        self.hwnd = hwnd;
    }
    // 共享一次性定时器集合给窗口过程完成清理。
    pub(crate) fn non_repeating_set(&self) -> Arc<Mutex<HashSet<u32>>> {
        self.non_repeating.clone()
    }
}

impl Default for WindowsTimer {
    fn default() -> Self {
        Self::new()
    }
}

impl ITimer for WindowsTimer {
    fn set(&mut self, interval_ms: u32, repeating: bool) -> Result<u32> {
        if self.hwnd.is_null() {
            return Err(Error::new(
                Errc::InvalidState,
                "WindowsTimer::set: no window handle bound",
            ));
        }
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        if !repeating {
            if let Ok(mut set) = self.non_repeating.lock() {
                set.insert(id);
            }
        }
        // SetTimer 的可选函数指针必须保持为空：WM_TIMER 由已保护的
        // wnd_proc 统一消费，不能让 Rust callback 直接跨 Win32 ABI。
        // SAFETY: hwnd 来自存活平台窗口，SetTimer 返回新定时器 ID 或空指针。
        let timer = unsafe { SetTimer(self.hwnd, id, interval_ms, None) };
        if timer == 0 {
            Err(Error::new(
                Errc::PlatformError,
                "WindowsTimer::set: SetTimer failed",
            ))
        } else {
            Ok(id)
        }
    }
    fn clear(&mut self, id: u32) -> Result<()> {
        if self.hwnd.is_null() {
            return Ok(());
        }
        if let Ok(mut set) = self.non_repeating.lock() {
            set.remove(&id);
        }
        // SAFETY: hwnd 来自存活平台窗口；KillTimer 返回 0 表示失败。
        if unsafe { KillTimer(self.hwnd, id) } != 0 {
            Ok(())
        } else {
            Err(Error::new(
                Errc::PlatformError,
                "WindowsTimer::clear: KillTimer failed",
            ))
        }
    }
}
