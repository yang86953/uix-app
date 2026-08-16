// ============================================================================
// platform/linux/wayland/clipboard.rs — IClipboard impl for WaylandBackend
// ============================================================================

use std::fs::File;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use std::sync::Arc;

use wayland_client::protocol::wl_data_source;

use crate::native::windowing::input::IClipboard;
use crate::native::windowing::shared::nonblocking_read::{
    NonBlockingReadAccumulator, NonBlockingReadStatus,
};
use crate::native::windowing::shared::nonblocking_write::{
    NonBlockingWriteCursor, NonBlockingWriteProgress,
};
use crate::native::{Errc, Error, Result};

use super::WaylandBackend;

pub(crate) const CLIPBOARD_READ_BUDGET: usize = 64 * 1024;
pub(crate) const CLIPBOARD_WRITE_BUDGET: usize = 64 * 1024;

fn set_nonblocking(fd: RawFd) -> io::Result<()> {
    // SAFETY: 调用方只传入当前仍打开的文件描述符；F_GETFL 不读取或写入 Rust 内存。
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: fd 在本同步调用期间保持打开，flags 来自同一描述符的 F_GETFL，F_SETFL 不持有 Rust 引用。
    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub(crate) struct ClipboardRead {
    file: File,
    bytes: NonBlockingReadAccumulator,
}

impl ClipboardRead {
    pub(crate) fn create_pipe() -> io::Result<(Self, OwnedFd)> {
        let mut fds = [0_i32; 2];
        // SAFETY: fds 指向两个连续且可写的 i32 槽位，pipe2 只在调用期间写入这两个槽位。
        if unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: pipe2 成功后 fds[0] 是当前函数唯一拥有的读端描述符，此处只转移一次所有权。
        let read_fd = unsafe { OwnedFd::from_raw_fd(fds[0]) };
        // SAFETY: pipe2 成功后 fds[1] 是当前函数唯一拥有的写端描述符，此处只转移一次所有权。
        let write_fd = unsafe { OwnedFd::from_raw_fd(fds[1]) };
        set_nonblocking(read_fd.as_raw_fd())?;

        Ok((
            Self {
                file: File::from(read_fd),
                bytes: NonBlockingReadAccumulator::default(),
            },
            write_fd,
        ))
    }

    pub(crate) fn fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }

    pub(crate) fn read_available(&mut self) -> io::Result<NonBlockingReadStatus> {
        self.bytes
            .read_available(&mut self.file, CLIPBOARD_READ_BUDGET)
    }
}

pub(crate) struct ClipboardWrite {
    file: File,
    bytes: NonBlockingWriteCursor,
}

impl ClipboardWrite {
    pub(crate) fn from_event_fd(fd: RawFd, bytes: Arc<[u8]>) -> io::Result<Self> {
        // SAFETY: Wayland data_source.send 事件把新文件描述符的关闭责任交给接收方，本对象只接管一次。
        let fd = unsafe { OwnedFd::from_raw_fd(fd) };
        set_nonblocking(fd.as_raw_fd())?;
        Ok(Self {
            file: File::from(fd),
            bytes: NonBlockingWriteCursor::new(bytes),
        })
    }

    pub(crate) fn fd(&self) -> RawFd {
        self.file.as_raw_fd()
    }

    pub(crate) fn write_available(
        &mut self,
        budget: usize,
    ) -> io::Result<NonBlockingWriteProgress> {
        self.bytes.write_available(&mut self.file, budget)
    }
}

impl IClipboard for WaylandBackend {
    fn text(&self) -> Result<String> {
        self.clipboard_text.lock().map(|t| t.clone()).map_err(|_| {
            Error::new(
                Errc::InvalidState,
                "WaylandBackend::text: clipboard_text lock poisoned",
            )
        })
    }
    fn set_text(&mut self, text: &str) -> Result<()> {
        {
            let mut t = self.clipboard_text.lock().map_err(|_| {
                Error::new(
                    Errc::InvalidState,
                    "WaylandBackend::set_text: clipboard_text lock poisoned",
                )
            })?;
            *t = text.to_string();
        }
        let Some(ref dm) = self.data_device_manager else {
            return Ok(());
        };
        let Some(ref dd) = self.data_device else {
            return Ok(());
        };
        let serial = self
            .last_input_serial
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .latest();
        let Some(serial) = serial else {
            if let Ok(mut owns) = self.owns_clipboard.lock() {
                *owns = false;
            }
            tracing::warn!("Wayland clipboard selection requires a pointer or keyboard serial",);
            return Err(Error::new(
                Errc::InvalidState,
                "WaylandBackend::set_text: no pointer or keyboard serial for selection",
            ));
        };
        let source = dm.create_data_source();
        source.offer("text/plain;charset=utf-8".to_string());
        let bytes = Arc::<[u8]>::from(text.as_bytes());
        let writes = Arc::clone(&self.clipboard_writes);
        let pending_failures = self.pending_failures.clone();
        source.quick_assign(move |_, event, _| {
            if let wl_data_source::Event::Send { mime_type: _, fd } = event {
                match ClipboardWrite::from_event_fd(fd.into_raw_fd(), Arc::clone(&bytes)) {
                    // 成功构造后，局部 ClipboardWrite 暂时独占协议 FD。
                    Ok(write) => {
                        // 只有健康发送队列才能接管本次 FD owner。
                        let Ok(mut queued) = writes.lock() else {
                            // 队列损坏必须交给 owner-thread failure source。
                            let _ = pending_failures.enqueue(Error::new(
                                // callback 无法提交 cursor 属于稳定 owner 状态错误。
                                Errc::InvalidState,
                                // 保留 data_source Send 的精确失败阶段。
                                "Wayland clipboard Send write queue mutex poisoned",
                            ));
                            // 返回时局部 write Drop，确保协议 FD 不泄漏。
                            return;
                        };
                        // 健康队列正式接管非阻塞 ClipboardWrite owner。
                        queued.push(write);
                    }
                    Err(error) => {
                        let _ = pending_failures.enqueue(Error::new(
                            Errc::IoError,
                            format!("Wayland clipboard send setup failed: {error}"),
                        ));
                    }
                }
            }
        });
        dd.set_selection(Some(&source), serial);
        if let Ok(mut owns) = self.owns_clipboard.lock() {
            *owns = true;
        }
        Ok(())
    }
    fn has_text(&self) -> Result<bool> {
        self.clipboard_text
            .lock()
            .map(|t| !t.is_empty())
            .map_err(|_| {
                Error::new(
                    Errc::InvalidState,
                    "WaylandBackend::has_text: clipboard_text lock poisoned",
                )
            })
    }
}
