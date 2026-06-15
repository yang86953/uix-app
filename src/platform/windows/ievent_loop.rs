// ============================================================================
// uix-platform/src/windows/ievent_loop.rs — IEventLoop 实现
// ============================================================================

use super::bindings::MSG;
use super::consts::*;
use super::ffi::*;
use super::platform::WindowsPlatform;
use crate::platform::event::*;
use crate::platform::*;

use std::ptr;

impl IEventLoop for WindowsPlatform {
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
