// ============================================================================
// uix-platform/src/windows/timer.rs — Windows timer (ITimer)
// ============================================================================

#![cfg(windows)]

use crate::platform::ITimer;

pub struct WindowsTimer {
    hwnd: *mut std::ffi::c_void,
    next_id: u32,
}

impl WindowsTimer {
    pub fn new() -> Self {
        Self {
            hwnd: std::ptr::null_mut(),
            next_id: 1,
        }
    }
    pub fn set_hwnd(&mut self, hwnd: *mut std::ffi::c_void) {
        self.hwnd = hwnd;
    }
}

impl Default for WindowsTimer {
    fn default() -> Self {
        Self::new()
    }
}

impl ITimer for WindowsTimer {
    fn set(&mut self, interval_ms: u32, _repeating: bool) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        unsafe {
            SetTimer(self.hwnd, id, interval_ms, None);
        }
        id
    }
    fn clear(&mut self, id: u32) {
        unsafe {
            KillTimer(self.hwnd, id);
        }
    }
}

#[link(name = "user32")]
extern "system" {
    fn SetTimer(
        hwnd: *mut std::ffi::c_void,
        nIDEvent: u32,
        uElapse: u32,
        lpTimerFunc: Option<unsafe extern "system" fn()>,
    ) -> usize;
    fn KillTimer(hwnd: *mut std::ffi::c_void, uIDEvent: u32) -> i32;
}
