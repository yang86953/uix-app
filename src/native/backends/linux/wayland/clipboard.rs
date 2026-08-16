// ============================================================================
// platform/linux/wayland/clipboard.rs — IClipboard impl for WaylandBackend
// ============================================================================

use std::fs::File;
use std::io;
// Selection callback 把 pipe 写端借给 Wayland 协议请求。
use std::os::fd::AsFd;
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use std::sync::Arc;
// Selection Component 对三个共享 owner 使用固定事务锁序。
use std::sync::Mutex;

// data-device Selection 事件由 clipboard Module 处理。
use wayland_client::protocol::wl_data_device;
use wayland_client::protocol::wl_data_source;

// callback typed failure 进入 backend 已有 pending source。
use crate::diagnostics::PendingFailureSource;
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

// 将 Selection owner 状态失败投递到 Wayland backend 的既有 source。
fn enqueue_selection_state_failure(
    // 每个 data-device callback 捕获同一 backend source。
    pending_failures: &PendingFailureSource,
    // owner 名称保持稳定且不携带敏感值。
    owner: &'static str,
    // callback 只入队，不执行 report/recovery/user code。
) {
    // 忽略 source 已关闭结果，保持迟到 callback 的 teardown 语义。
    let _ = pending_failures.enqueue(Error::new(
        // poisoned shared owner 统一分类为 InvalidState。
        Errc::InvalidState,
        // 诊断区分 Selection callback 的具体 owner。
        format!("Wayland clipboard Selection {owner} mutex poisoned"),
    ));
}

// 事务化接管 compositor 提供的新 clipboard offer。
fn apply_selection_offer(
    // 协议 offer 只在全部所需 owner 健康后使用。
    offer: wayland_client::protocol::wl_data_offer::WlDataOffer,
    // 本地 selection ownership 标志 owner。
    owns_clipboard: &Arc<Mutex<bool>>,
    // 当前非阻塞 read FD owner。
    clipboard_read: &Arc<Mutex<Option<ClipboardRead>>>,
    // callback failure 进入同一 backend source。
    pending_failures: &PendingFailureSource,
    // 无 Result 的 callback 用入队表达失败。
) {
    // 先取得 ownership guard，任何 I/O 前验证健康。
    let mut owns = match owns_clipboard.lock() {
        // 健康 guard 暂不修改，等待 read owner。
        Ok(owns) => owns,
        // ownership 状态损坏时立即停止。
        Err(_) => {
            // 入队一次稳定 owner failure。
            enqueue_selection_state_failure(pending_failures, "owns flag");
            // 不创建 pipe、不发送 receive。
            return;
        }
    };
    // 再取得 read guard，保持固定 owns→read 锁序。
    let mut active_read = match clipboard_read.lock() {
        // 两个健康 guards 组成 selection 接管事务。
        Ok(active_read) => active_read,
        // read owner 损坏时保持 ownership 不变。
        Err(_) => {
            // 先释放 ownership guard，避免跨 owner 入队。
            drop(owns);
            // 入队一次稳定 owner failure。
            enqueue_selection_state_failure(pending_failures, "read owner");
            // 不创建 pipe、不发送 receive。
            return;
        }
    };
    // 全部所需 owner 健康后才创建本地非阻塞 pipe。
    match ClipboardRead::create_pipe() {
        // pipe 成功后局部值暂时独占两端 FD。
        Ok((read, write_fd)) => {
            // 使用既有 UTF-8 文本 MIME 请求 compositor 写入。
            offer.receive(
                // 保留既有 MIME 协议值。
                "text/plain;charset=utf-8".to_string(),
                // 只把写端借给本次协议请求。
                write_fd.as_fd(),
            );
            // 新外部 selection 取消本地 ownership。
            *owns = false;
            // 同一事务中用新 read FD 替换旧 owner。
            *active_read = Some(read);
        }
        // pipe setup failure 仍必须提交 selection 已切换事实。
        Err(error) => {
            // 外部 selection 已使本地 ownership 失效。
            *owns = false;
            // 丢弃陈旧 read FD，避免读取上一 selection。
            *active_read = None;
            // 先释放两个 guards，避免跨 owner 入队。
            drop(active_read);
            // 释放 ownership guard 后再转交 I/O failure。
            drop(owns);
            // pipe 创建失败进入同一 pending source。
            let _ = pending_failures.enqueue(Error::new(
                // OS pipe setup 保持 I/O 错误分类。
                Errc::IoError,
                // 保留底层 cause 文本。
                format!("Wayland clipboard pipe creation failed: {error}"),
            ));
        }
    }
}

// 事务化清除 compositor 报告为空的 clipboard selection。
fn clear_selection(
    // 本地 selection ownership 标志 owner。
    owns_clipboard: &Arc<Mutex<bool>>,
    // 当前非阻塞 read FD owner。
    clipboard_read: &Arc<Mutex<Option<ClipboardRead>>>,
    // 已完成 clipboard 文本缓存 owner。
    clipboard_text: &Arc<Mutex<String>>,
    // callback failure 进入同一 backend source。
    pending_failures: &PendingFailureSource,
    // 无 Result 的 callback 用入队表达失败。
) {
    // 先取得 ownership guard，尚不修改。
    let mut owns = match owns_clipboard.lock() {
        // 健康 guard 暂不提交清理。
        Ok(owns) => owns,
        // ownership 状态损坏时立即停止。
        Err(_) => {
            // 入队一次稳定 owner failure。
            enqueue_selection_state_failure(pending_failures, "owns flag");
            // 不产生任何半清理。
            return;
        }
    };
    // 再取得 read guard，保持固定 owns→read 锁序。
    let mut active_read = match clipboard_read.lock() {
        // 两个健康 guards 继续等待 text owner。
        Ok(active_read) => active_read,
        // read owner 损坏时保持 ownership 不变。
        Err(_) => {
            // 先释放 ownership guard，避免跨 owner 入队。
            drop(owns);
            // 入队一次稳定 owner failure。
            enqueue_selection_state_failure(pending_failures, "read owner");
            // 不产生任何半清理。
            return;
        }
    };
    // 最后取得 text guard，保持固定 read→text 锁序。
    let mut text = match clipboard_text.lock() {
        // 三个健康 guards 组成清空事务。
        Ok(text) => text,
        // text owner 损坏时保持前两份状态不变。
        Err(_) => {
            // 先释放 read guard，避免跨 owner 入队。
            drop(active_read);
            // 再释放 ownership guard。
            drop(owns);
            // 入队一次稳定 owner failure。
            enqueue_selection_state_failure(pending_failures, "text owner");
            // 不产生任何半清理。
            return;
        }
    };
    // 全部 owners 健康后提交 ownership 失效。
    *owns = false;
    // 在同一事务中释放 read FD owner。
    *active_read = None;
    // 最后清除上一 selection 的文本缓存。
    text.clear();
}

// 处理 wl_data_device 的 selection 状态变更 callback。
pub(crate) fn handle_selection_event(
    // seat callback 转交完整 data-device 事件。
    event: wl_data_device::Event,
    // 本地 selection ownership 标志 owner。
    owns_clipboard: &Arc<Mutex<bool>>,
    // 当前非阻塞 read FD owner。
    clipboard_read: &Arc<Mutex<Option<ClipboardRead>>>,
    // 已完成 clipboard 文本缓存 owner。
    clipboard_text: &Arc<Mutex<String>>,
    // callback failure 进入同一 backend source。
    pending_failures: &PendingFailureSource,
    // Component 忽略非 Selection data-device 事件。
) {
    // 只处理 compositor selection 变更。
    let wl_data_device::Event::Selection { id } = event else {
        // 其他 data-device 事件保持既有忽略语义。
        return;
    };
    // Some 与 None 使用各自最小 owner 事务。
    match id {
        // 新 offer 只需 ownership 与 read owners。
        Some(offer) => apply_selection_offer(
            // 转交协议 offer owner。
            offer,
            // 转交 ownership 状态 owner。
            owns_clipboard,
            // 转交 read FD owner。
            clipboard_read,
            // 转交同一 failure source。
            pending_failures,
        ),
        // 空 selection 需要同步清除三份状态。
        None => clear_selection(
            // 转交 ownership 状态 owner。
            owns_clipboard,
            // 转交 read FD owner。
            clipboard_read,
            // 转交文本缓存 owner。
            clipboard_text,
            // 转交同一 failure source。
            pending_failures,
        ),
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
        // 输入 serial owner 损坏时不得继续创建或提交 selection source。
        let serial = self.last_input_serial.lock().map_err(|_| {
            // 构造稳定的剪贴板 serial 状态错误。
            Error::new(
                // 共享输入状态已无法安全读取。
                Errc::InvalidState,
                // 保留 Wayland set_text 与输入 serial 阶段。
                "Wayland clipboard input serial mutex poisoned during set_text",
            )
        })?;
        // 健康 guard 只读取最近一次 pointer/keyboard serial。
        let serial = serial.latest();
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
