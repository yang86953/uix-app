// ============================================================================
// uix-platform/src/windows/timer.rs — Windows timer (ITimer)
// ============================================================================

#![cfg(windows)]

use super::ffi::{KillTimer, SetTimer};
use crate::ITimer;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

pub struct WindowsTimer {
    hwnd: *mut std::ffi::c_void,
    next_id: u32,
    non_repeating: Arc<Mutex<HashSet<u32>>>,
}

impl WindowsTimer {
    pub fn new() -> Self {
        Self {
            hwnd: std::ptr::null_mut(),
            next_id: 1,
            non_repeating: Arc::new(Mutex::new(HashSet::new())),
        }
    }
    pub fn set_hwnd(&mut self, hwnd: *mut std::ffi::c_void) {
        self.hwnd = hwnd;
    }
    pub fn non_repeating_set(&self) -> Arc<Mutex<HashSet<u32>>> {
        self.non_repeating.clone()
    }
}

impl Default for WindowsTimer {
    fn default() -> Self {
        Self::new()
    }
}

impl ITimer for WindowsTimer {
    fn set(&mut self, interval_ms: u32, repeating: bool) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        if self.hwnd.is_null() {
            return id;
        }
        if !repeating {
            if let Ok(mut set) = self.non_repeating.lock() {
                set.insert(id);
            }
        }
        unsafe {
            SetTimer(self.hwnd, id, interval_ms, None);
        }
        id
    }
    fn clear(&mut self, id: u32) {
        if self.hwnd.is_null() {
            return;
        }
        if let Ok(mut set) = self.non_repeating.lock() {
            set.remove(&id);
        }
        unsafe {
            KillTimer(self.hwnd, id);
        }
    }
}
