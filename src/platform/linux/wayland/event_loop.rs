// ============================================================================
// platform/linux/wayland/event_loop.rs — IEventLoop + 事件分发内部方法
// ============================================================================

use std::fs::File;
use std::os::unix::io::FromRawFd;

use libc::{poll, pollfd, POLLIN};

use crate::platform::event::UiEvent;
use crate::platform::IEventLoop;

use super::WaylandBackend;

impl WaylandBackend {
    /// 非阻塞事件分发：刷新待发请求，用 poll() 检查 Wayland socket 是否有数据。
    /// 仅在数据就绪时调用阻塞的 dispatch()，因此从不阻塞等待事件。
    pub(crate) fn try_dispatch(&mut self) -> bool {
        if self.closed {
            return false;
        }
        if let Err(e) = self.display.flush() {
            log::error!("Wayland flush error: {:?}", e);
            self.closed = true;
            return false;
        }
        let fd = self.display.get_connection_fd();
        let mut pfd = pollfd {
            fd,
            events: POLLIN,
            revents: 0,
        };
        // SAFETY: poll() 是标准 libc 调用，pfd 是栈上变量，生命周期有效。
        let ret = unsafe { poll(&mut pfd as *mut pollfd, 1, 0) };
        if ret > 0 && (pfd.revents & POLLIN) != 0 {
            if let Err(e) = self.event_queue.dispatch(&mut (), |_, _, _| {}) {
                log::error!("Wayland dispatch error: {}", e);
                self.closed = true;
                return false;
            }
        }
        self.read_clipboard_pipe();
        true
    }

    /// 读取剪贴板管道中的数据传输结果。
    fn read_clipboard_pipe(&mut self) {
        let fd = {
            let mut f = self.clipboard_read_fd.lock().unwrap_or_else(|e| e.into_inner());
            f.take()
        };
        if let Some(fd) = fd {
            use std::io::Read;
            let mut buf = Vec::new();
            let mut file = unsafe { File::from_raw_fd(fd) };
            if file.read_to_end(&mut buf).is_ok() {
                if let Ok(mut text) = self.clipboard_text.lock() {
                    *text = String::from_utf8_lossy(&buf).to_string();
                }
            }
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// IEventLoop
// ════════════════════════════════════════════════════════════════════════════

impl IEventLoop for WaylandBackend {
    fn poll_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool {
        if !self.try_dispatch() {
            return false;
        }
        let mut q = self.events.lock().unwrap_or_else(|e| e.into_inner());
        while let Some(event) = q.pop_front() {
            if !callback(&event) {
                return false;
            }
        }
        drop(q);
        if self.closed {
            return false;
        }
        true
    }

    fn wait_event(&mut self, callback: &dyn Fn(&UiEvent) -> bool) -> bool {
        if self.closed {
            return false;
        }
        {
            let q = self.events.lock().unwrap_or_else(|e| e.into_inner());
            if !q.is_empty() {
                drop(q);
                return IEventLoop::poll_event(self, callback);
            }
        }
        if let Err(e) = self.event_queue.dispatch(&mut (), |_, _, _| {}) {
            log::error!("Wayland wait_event dispatch error: {}", e);
            self.closed = true;
            return false;
        }
        if let Some(ref s) = self.surface {
            s.commit();
        }
        let _ = self.display.flush();
        IEventLoop::poll_event(self, callback)
    }
}
