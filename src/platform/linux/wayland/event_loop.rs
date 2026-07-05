// ============================================================================
// platform/linux/wayland/event_loop.rs — Wayland 事件分发
//
// WaylandBackend 的事件分发方法：
//   try_dispatch()      — 非阻塞分发
//   dispatch_blocking() — 阻塞等待分发
//   dispatch_timeout()  — 带超时分发
//
// 注意：不再直接实现 IEventLoop，由 LinuxPlatform 通过 OsEventSource 获得。
// ============================================================================

use std::fs::File;
use std::os::unix::io::FromRawFd;
use std::time::{Duration, Instant};

use libc::{poll, pollfd, POLLIN};

use crate::platform::api::event::UiEvent;
use crate::platform::KeyMod;

use super::keycode::keycode_to_char;
use super::WaylandBackend;

impl WaylandBackend {
    /// 非阻塞事件分发。
    /// 先 dispatch 已在内部队列中的事件，再用 poll() 检查 socket。
    pub(crate) fn try_dispatch(&mut self) -> bool {
        if self.closed {
            return false;
        }
        if let Err(e) = self.event_queue.dispatch_pending(&mut (), |_, _, _| {}) {
            crate::platform::log::error_fn(format!("Wayland dispatch_pending error: {}", e));
            self.closed = true;
            return false;
        }
        let fd = self.display.get_connection_fd();
        let mut pfd = pollfd {
            fd,
            events: POLLIN,
            revents: 0,
        };
        let ret = unsafe { poll(&mut pfd as *mut pollfd, 1, 0) };
        if ret > 0 && (pfd.revents & POLLIN) != 0 {
            let _ = self.display.flush();
            if let Err(e) = self.event_queue.dispatch(&mut (), |_, _, _| {}) {
                crate::platform::log::error_fn(format!("Wayland dispatch error: {}", e));
                self.closed = true;
                return false;
            }
        }
        self.read_clipboard_pipe();
        self.generate_key_repeats();
        true
    }

    /// 阻塞等待 Wayland 事件。
    ///
    /// 当有按键按住且客户端侧按键重复已启用时，使用 poll 超时循环
    /// 确保 generate_key_repeats() 被周期性调用。否则使用传统阻塞 dispatch。
    pub(crate) fn dispatch_blocking(&mut self) -> bool {
        if self.closed {
            return false;
        }

        // 检查是否需要客户端侧按键重复
        let (has_held, repeat_rate) = {
            let hki = self.held_key_info.lock().unwrap_or_else(|e| e.into_inner());
            let rate = *self.repeat_rate.lock().unwrap_or_else(|e| e.into_inner());
            (hki.is_some() && rate > 0, rate)
        };

        if has_held {
            // 按键按住 + 客户端重复启用：使用 poll 循环，
            // 超时时间基于重复速率，确保 generate_key_repeats() 按时触发
            let interval_ms = (1000 / repeat_rate).max(10).min(100) as i32;
            return self.dispatch_timeout(Duration::from_millis(interval_ms as u64));
        }

        // 无按键按住：传统阻塞 dispatch
        let _ = self.display.flush();
        if let Err(e) = self.event_queue.dispatch(&mut (), |_, _, _| {}) {
            crate::platform::log::error_fn(format!("Wayland dispatch_blocking error: {}", e));
            self.closed = true;
            return false;
        }
        self.read_clipboard_pipe();
        self.generate_key_repeats();
        true
    }

    /// 带超时的 Wayland 事件分发。
    pub(crate) fn dispatch_timeout(&mut self, timeout: Duration) -> bool {
        if self.closed {
            return false;
        }
        let _ = self.display.flush();
        let fd = self.display.get_connection_fd();
        let mut pfd = pollfd {
            fd,
            events: POLLIN,
            revents: 0,
        };
        let timeout_ms = timeout.as_millis().min(u32::MAX as u128) as i32;
        let ret = unsafe { poll(&mut pfd, 1, timeout_ms) };
        if ret > 0 && (pfd.revents & POLLIN) != 0 {
            if let Err(e) = self.event_queue.dispatch(&mut (), |_, _, _| {}) {
                crate::platform::log::error_fn(format!("Wayland dispatch_timeout dispatch error: {}", e));
                self.closed = true;
                return false;
            }
        }
        self.read_clipboard_pipe();
        self.generate_key_repeats();
        true
    }

    /// 从事件队列弹出下一个事件。
    pub(crate) fn next_event(&self) -> Option<UiEvent> {
        self.events
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pop_front()
    }

    /// 检查是否已关闭。
    pub(crate) fn is_closed(&self) -> bool {
        self.closed
    }

    /// 检查事件队列是否为空。
    pub(crate) fn has_pending_events(&self) -> bool {
        self.events.lock().map(|q| !q.is_empty()).unwrap_or(false)
    }

    // ── 内部辅助 ──────────────────────────────────────────

    fn generate_key_repeats(&mut self) {
        let (_linux_key, code, mods, first_press) = match self
            .held_key_info
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .as_ref()
        {
            Some(info) => (info.0, info.1, info.2, info.3),
            None => return,
        };

        let rate = *self.repeat_rate.lock().unwrap_or_else(|e| e.into_inner());
        if rate <= 0 {
            return;
        }
        let delay_ms = *self.repeat_delay.lock().unwrap_or_else(|e| e.into_inner());

        let now = Instant::now();
        let elapsed = now.duration_since(first_press);
        let delay = Duration::from_millis(delay_ms.max(1) as u64);
        if elapsed < delay {
            return;
        }

        let interval = Duration::from_secs_f32(1.0 / rate as f32);
        let min_interval = Duration::from_millis(10);
        let interval = interval.max(min_interval);

        let should_fire = match *self
            .last_repeat_time
            .lock()
            .unwrap_or_else(|e| e.into_inner())
        {
            Some(last) => now.duration_since(last) >= interval,
            None => now >= first_press + delay,
        };
        if !should_fire {
            return;
        }

        *self
            .last_repeat_time
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(now);

        let shift_down = mods.intersects(KeyMod::SHIFT);
        let mut q = self.events.lock().unwrap_or_else(|e| e.into_inner());
        q.push_back(UiEvent::key_down(code, mods));
        if let Some(text) = keycode_to_char(code, shift_down) {
            q.push_back(UiEvent::key_press(text));
        }
    }

    fn read_clipboard_pipe(&mut self) {
        let fd = {
            let mut f = self
                .clipboard_read_fd
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            f.take()
        };
        if let Some(fd) = fd {
            let mut pfd = pollfd {
                fd,
                events: POLLIN,
                revents: 0,
            };
            let ret = unsafe { poll(&mut pfd as *mut pollfd, 1, 0) };
            if ret > 0 && (pfd.revents & POLLIN) != 0 {
                use std::io::Read;
                let mut buf = Vec::new();
                let mut file = unsafe { File::from_raw_fd(fd) };
                if file.read_to_end(&mut buf).is_ok() {
                    if let Ok(mut text) = self.clipboard_text.lock() {
                        *text = String::from_utf8_lossy(&buf).to_string();
                    }
                }
            } else {
                let _ = unsafe { libc::close(fd) };
            }
        }
    }
}
