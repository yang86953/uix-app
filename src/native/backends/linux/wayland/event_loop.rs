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

use std::os::fd::{AsFd, AsRawFd, RawFd};
use std::time::{Duration, Instant};

use libc::{poll, pollfd, POLLERR, POLLHUP, POLLIN, POLLNVAL, POLLOUT};
use wayland_client::backend::WaylandError;

use crate::core::{Errc, Error};
use crate::native::windowing::event::{EventLoopWaker, UiEvent};
use crate::native::windowing::input::KeyMod;

use super::keycode::keycode_to_char;
use super::WaylandBackend;
use crate::native::windowing::shared::nonblocking_read::NonBlockingReadStatus;
use crate::native::windowing::shared::nonblocking_write::NonBlockingWriteStatus;

impl WaylandBackend {
    /// 非阻塞事件分发。
    /// 先 dispatch 已在内部队列中的事件，再用 poll() 检查 socket。
    pub(crate) fn try_dispatch(&mut self) -> bool {
        if self.closed {
            return false;
        }
        if !self.dispatch_pending_checked("dispatch_pending") {
            return false;
        }
        self.dispatch_polled(0, "dispatch")
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
        self.dispatch_polled(-1, "dispatch_blocking")
    }

    /// 带超时的 Wayland 事件分发。
    pub(crate) fn dispatch_timeout(&mut self, timeout: Duration) -> bool {
        if self.closed {
            return false;
        }
        let timeout_ms = timeout.as_millis().min(i32::MAX as u128) as i32;
        self.dispatch_polled(timeout_ms, "dispatch_timeout")
    }

    pub(crate) fn waker(&self) -> EventLoopWaker {
        let fd = self.wake_write_fd;
        let pending_failures = self.pending_failures.clone();
        EventLoopWaker::new(move || {
            let byte = [1_u8];
            let result = unsafe { libc::write(fd, byte.as_ptr().cast(), byte.len()) };
            if result < 0 {
                let error = std::io::Error::last_os_error();
                if !matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted
                ) {
                    let _ = pending_failures.enqueue(Error::new(
                        Errc::IoError,
                        format!("Wayland wake pipe write failed: {error}"),
                    ));
                }
            }
        })
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

    fn dispatch_polled(&mut self, timeout_ms: i32, context: &str) -> bool {
        if !self.flush_checked(context) {
            return false;
        }
        let wayland_fd = self.display.as_fd().as_raw_fd();
        let clipboard_fd = self
            .clipboard_read
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
            .map(super::clipboard::ClipboardRead::fd);
        self.poll_fds.clear();
        self.poll_fds.push(pollfd {
            fd: wayland_fd,
            events: POLLIN,
            revents: 0,
        });
        self.poll_fds.push(pollfd {
            fd: self.wake_read_fd,
            events: POLLIN,
            revents: 0,
        });
        let clipboard_read_index = clipboard_fd.map(|fd| {
            let index = self.poll_fds.len();
            self.poll_fds.push(pollfd {
                fd,
                events: POLLIN,
                revents: 0,
            });
            index
        });
        let clipboard_write_start = self.poll_fds.len();
        {
            let writes = self
                .clipboard_writes
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            self.poll_fds.extend(writes.iter().map(|write| pollfd {
                fd: write.fd(),
                events: POLLOUT,
                revents: 0,
            }));
        }
        let poll_len = self.poll_fds.len();
        let ret = unsafe {
            poll(
                self.poll_fds.as_mut_ptr(),
                poll_len as libc::nfds_t,
                timeout_ms,
            )
        };
        if ret < 0 {
            let error = std::io::Error::last_os_error();
            if error.kind() == std::io::ErrorKind::Interrupted {
                return true;
            }
            return self.close_after_failure(
                Errc::IoError,
                format!("Wayland {context} poll error: {error}"),
            );
        }

        let wayland_revents = self.poll_fds[0].revents;
        let wake_revents = self.poll_fds[1].revents;
        if (wake_revents & POLLIN) != 0 {
            self.drain_wake_pipe();
        }
        if (wayland_revents & (POLLERR | POLLHUP | POLLNVAL)) != 0 {
            return self.close_after_failure(
                Errc::IoError,
                format!("Wayland {context} display fd reported an error"),
            );
        }
        if (wayland_revents & POLLIN) != 0 {
            if let Some(read_guard) = self.display.prepare_read() {
                if let Err(e) = read_guard.read() {
                    return self.close_after_failure(
                        Errc::IoError,
                        format!("Wayland {context} read error: {e}"),
                    );
                }
            }
            if !self.dispatch_pending_checked(context) {
                return false;
            }
        }
        if let Some(polled_fd) = clipboard_fd {
            let clipboard_revents = clipboard_read_index
                .map(|index| self.poll_fds[index].revents)
                .unwrap_or(0);
            if (clipboard_revents & (POLLIN | POLLHUP)) != 0 {
                self.read_clipboard_pipe(polled_fd);
            } else if (clipboard_revents & (POLLERR | POLLNVAL)) != 0 {
                let mut active = self
                    .clipboard_read
                    .lock()
                    .unwrap_or_else(|error| error.into_inner());
                if active.as_ref().is_some_and(|read| read.fd() == polled_fd) {
                    *active = None;
                    self.enqueue_failure(Error::new(
                        Errc::IoError,
                        "Wayland clipboard read fd reported an error",
                    ));
                }
            }
        }
        let mut write_budget = super::clipboard::CLIPBOARD_WRITE_BUDGET;
        for index in clipboard_write_start..poll_len {
            if write_budget == 0 {
                break;
            }
            let polled_fd = self.poll_fds[index].fd;
            let revents = self.poll_fds[index].revents;
            let slots_left = poll_len - index;
            let budget = (write_budget / slots_left).max(1);
            let written = self.write_clipboard_pipe(polled_fd, revents, budget);
            write_budget = write_budget.saturating_sub(written);
        }
        self.generate_key_repeats();
        true
    }

    fn drain_wake_pipe(&self) {
        let mut buf = [0_u8; 64];
        loop {
            let ret = unsafe { libc::read(self.wake_read_fd, buf.as_mut_ptr().cast(), buf.len()) };
            if ret > 0 {
                continue;
            }
            if ret < 0 {
                let error = std::io::Error::last_os_error();
                if error.kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                if error.kind() != std::io::ErrorKind::WouldBlock {
                    self.enqueue_failure(Error::new(
                        Errc::IoError,
                        format!("Wayland wake pipe read failed: {error}"),
                    ));
                }
            }
            break;
        }
    }

    fn generate_key_repeats(&mut self) {
        let Some(held) = *self.held_key_info.lock().unwrap_or_else(|e| e.into_inner()) else {
            return;
        };
        let code = held.code;
        let mods = held.mods;
        let first_press = held.first_press;
        let window_id = held.window_id;
        let current_target = self
            .surface_windows
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .keyboard_target();
        if current_target != Some(window_id) {
            *self
                .held_key_info
                .lock()
                .unwrap_or_else(|error| error.into_inner()) = None;
            *self
                .last_repeat_time
                .lock()
                .unwrap_or_else(|error| error.into_inner()) = None;
            return;
        }

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
        q.push_back(UiEvent::key_down(code, mods).for_window(window_id));
        if let Some(text) = keycode_to_char(code, shift_down) {
            q.push_back(UiEvent::text_input(text).for_window(window_id));
        }
    }

    fn read_clipboard_pipe(&mut self, polled_fd: RawFd) {
        let outcome = {
            let mut active = self
                .clipboard_read
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            let Some(read) = active.as_mut() else {
                return;
            };
            if read.fd() != polled_fd {
                return;
            }
            match read.read_available() {
                Ok(NonBlockingReadStatus::Pending) => None,
                Ok(NonBlockingReadStatus::Complete(bytes)) => {
                    *active = None;
                    Some(Ok(bytes))
                }
                Err(error) => {
                    *active = None;
                    Some(Err(error))
                }
            }
        };

        match outcome {
            Some(Ok(bytes)) => {
                *self
                    .clipboard_text
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) =
                    String::from_utf8_lossy(&bytes).into_owned();
            }
            Some(Err(error)) => {
                self.enqueue_failure(Error::new(
                    Errc::IoError,
                    format!("Wayland clipboard read failed: {error}"),
                ));
            }
            None => {}
        }
    }

    fn write_clipboard_pipe(&mut self, polled_fd: RawFd, revents: i16, budget: usize) -> usize {
        let mut writes = self
            .clipboard_writes
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let Some(index) = writes.iter().position(|write| write.fd() == polled_fd) else {
            return 0;
        };

        if (revents & (POLLERR | POLLHUP | POLLNVAL)) != 0 {
            writes.swap_remove(index);
            self.enqueue_failure(Error::new(
                Errc::IoError,
                "Wayland clipboard receiver closed before send completed",
            ));
            return 0;
        }
        if (revents & POLLOUT) == 0 {
            return 0;
        }

        match writes[index].write_available(budget) {
            Ok(progress) => {
                if progress.status == NonBlockingWriteStatus::Complete {
                    writes.swap_remove(index);
                }
                progress.written
            }
            Err(error) => {
                writes.swap_remove(index);
                self.enqueue_failure(Error::new(
                    Errc::IoError,
                    format!("Wayland clipboard send failed: {error}"),
                ));
                0
            }
        }
    }

    fn close_after_failure(&mut self, code: Errc, message: String) -> bool {
        self.enqueue_failure(Error::new(code, message));
        self.closed = true;
        false
    }

    pub(crate) fn dispatch_pending_checked(&mut self, context: &str) -> bool {
        match self.event_queue.dispatch_pending(&mut self.dispatch_state) {
            Ok(_) => true,
            Err(error) => self.close_after_failure(
                Errc::PlatformError,
                format!("Wayland {context} dispatch error: {error}"),
            ),
        }
    }

    pub(crate) fn flush_checked(&self, context: &str) -> bool {
        match self.display.flush() {
            Ok(()) => true,
            Err(WaylandError::Io(error)) if error.kind() == std::io::ErrorKind::WouldBlock => true,
            Err(WaylandError::Io(error)) => {
                self.enqueue_failure(Error::new(
                    Errc::IoError,
                    format!("Wayland {context} flush error: {error}"),
                ));
                false
            }
            Err(WaylandError::Protocol(error)) => {
                self.enqueue_failure(Error::new(
                    Errc::PlatformError,
                    format!("Wayland {context} protocol error: {error}"),
                ));
                false
            }
        }
    }
}
