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
use crate::native::windowing::shared::nonblocking_read::{
    NonBlockingReadAccumulator, NonBlockingReadStatus,
};
use crate::native::windowing::shared::nonblocking_write::{
    NonBlockingWriteCursor, NonBlockingWriteProgress,
};
use crate::native::{Errc, Error, Result};
use crate::platform::windowing::IClipboard;

// callback disposition 允许 data-source 在 Cancelled 后消费 registry owner。
use super::WaylandBackend;
use super::compat::CallbackDisposition;

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

// Wayland clipboard Module 提供 backend owner-thread 的确定性关闭端口。
impl WaylandBackend {
    // 在任何 clipboard owner 访问前检查 backend 生命周期。
    fn ensure_clipboard_open(&self, operation: &str) -> Result<()> {
        // closed 事实由 WaylandBackend owner-thread 唯一提交。
        if self.closed {
            // 关闭后访问属于稳定生命周期错误，禁止伪造空值或成功。
            return Err(Error::new(
                // 使用 InvalidState 区分生命周期与协议/I/O 失败。
                Errc::InvalidState,
                // 保留具体 clipboard 操作以便 owner-thread 定位调用方。
                format!("Wayland clipboard {operation} requested after backend shutdown"),
            ));
        }
        // 健康 backend 继续进入既有 clipboard 行为。
        Ok(())
    }

    // 在读取系统剪贴板缓存前验证 Wayland selection 协议能力。
    fn ensure_clipboard_data_device(&self, operation: &str) -> Result<()> {
        // 缺少全局 data-device manager 时不能把空缓存解释为系统剪贴板为空。
        if self.data_device_manager.is_none() {
            // 返回稳定 capability absence 供调用方选择降级。
            return Err(Error::new(
                // compositor 未提供协议 global 属于未实现能力。
                Errc::NotImplemented,
                // 保留具体读取操作与缺失协议身份。
                format!("WaylandBackend::{operation}: wl_data_device_manager is unavailable"),
            ));
        }
        // manager 存在但当前 seat 尚未派生 data device 时同样不能读取缓存。
        if self.data_device.is_none() {
            // 返回稳定 session 状态错误。
            return Err(Error::new(
                // 协议存在但当前没有活动 seat 派生对象属于无效操作。
                Errc::InvalidOperation,
                // 保留具体读取操作与 seat/session 上下文。
                format!("WaylandBackend::{operation}: no wl_data_device for active seat"),
            ));
        }
        // 两层协议 owner 均存在后才允许读取最近完成的 selection 缓存。
        Ok(())
    }

    // 关闭在途 clipboard I/O 与所有过期授权状态。
    pub(crate) fn shutdown_clipboard_io(&mut self) {
        // 共享 owners 清理前先让 active data-source 停止接收 Send/Cancelled。
        if let Some(source) = self.clipboard_source.take() {
            // 只注销兼容 callback，不发送协议 destroy 请求。
            source.clear_callback();
        }
        // 第一把锁沿用 selection Component 的 ownership owner。
        let mut owns_clipboard = self
            // 访问 backend 共享 ownership 标志。
            .owns_clipboard
            // teardown 必须等待并取得唯一可变访问。
            .lock()
            // owner-thread 关闭时恢复中毒 guard 以确定性释放资源。
            .unwrap_or_else(|error| error.into_inner());
        // 第二把锁沿用既有 owns→read 顺序。
        let mut clipboard_read = self
            // 访问当前非阻塞 read FD owner。
            .clipboard_read
            // 保持 ownership guard 期间取得 read guard。
            .lock()
            // teardown 不再读取损坏业务值，只负责 Drop 资源。
            .unwrap_or_else(|error| error.into_inner());
        // 第三把锁沿用既有 read→text 顺序。
        let mut clipboard_text = self
            // 访问已完成 selection 文本缓存。
            .clipboard_text
            // 在 read guard 后取得文本唯一访问。
            .lock()
            // 关闭时必须清除可能保留的敏感剪贴板内容。
            .unwrap_or_else(|error| error.into_inner());
        // 第四把锁取得全部在途 write FD owners。
        let mut clipboard_writes = self
            // 访问 data-source Send callback 的发送队列。
            .clipboard_writes
            // input callbacks 已停止，teardown 可独占整个队列。
            .lock()
            // 中毒队列仍由 owner-thread 负责 Drop 其中的 File owners。
            .unwrap_or_else(|error| error.into_inner());
        // 第五把锁最后取得输入授权 serial owner。
        let mut input_serial = self
            // 访问 pointer/keyboard 最近成功 serial。
            .last_input_serial
            // 沿 clipboard owners 之后的固定关闭顺序取得 guard。
            .lock()
            // 关闭时恢复 guard 只用于清除过期授权身份。
            .unwrap_or_else(|error| error.into_inner());
        // 全部 guards 已取得后失效本地 selection ownership。
        *owns_clipboard = false;
        // Drop 当前 ClipboardRead 及其独占 read FD。
        *clipboard_read = None;
        // 清除 backend 关闭后不应继续暴露的文本缓存。
        clipboard_text.clear();
        // Drop 全部 ClipboardWrite 及其独占 write FDs。
        clipboard_writes.clear();
        // 清除不能跨 backend 生命周期复用的协议 serial。
        *input_serial = Default::default();
        // 所有 guards 随方法返回按逆序释放。
    }
}

impl IClipboard for WaylandBackend {
    fn text(&self) -> Result<String> {
        // 生命周期 gate 必须先于文本 owner lock。
        self.ensure_clipboard_open("text")?;
        // 协议 capability gate 必须先于缓存读取。
        self.ensure_clipboard_data_device("text")?;
        self.clipboard_text.lock().map(|t| t.clone()).map_err(|_| {
            Error::new(
                Errc::InvalidState,
                "WaylandBackend::text: clipboard_text lock poisoned",
            )
        })
    }
    fn set_text(&mut self, text: &str) -> Result<()> {
        // 生命周期 gate 必须先于缓存写入与任何协议 owner 访问。
        self.ensure_clipboard_open("set_text")?;
        // 缺少 selection global 时不得把本地缓存伪装成系统剪贴板。
        let dm = self.data_device_manager.clone().ok_or_else(|| {
            // 构造稳定的 capability absence。
            Error::new(
                // compositor 未提供协议 global 属于未实现能力。
                Errc::NotImplemented,
                // 保留 set_text 与缺失协议身份。
                "WaylandBackend::set_text: wl_data_device_manager is unavailable",
            )
        })?;
        // 当前 seat 尚未建立 data device 时不得提交局部状态。
        let dd = self.data_device.clone().ok_or_else(|| {
            // 构造稳定的 session 状态错误。
            Error::new(
                // global 存在但当前没有 seat 派生对象属于无效操作。
                Errc::InvalidOperation,
                // 保留 set_text、data device 与 seat 上下文。
                "WaylandBackend::set_text: no wl_data_device for active seat",
            )
        })?;
        // callback 需要独立 Arc，不借用同步事务 guard。
        let callback_owns_clipboard = Arc::clone(&self.owns_clipboard);
        // ownership owner 必须在任何协议动作与旧 source 替换前健康。
        let mut owns_clipboard = self.owns_clipboard.lock().map_err(|_| {
            // 构造稳定的 selection ownership 状态错误。
            Error::new(
                // 同步共享 owner 损坏统一分类为 InvalidState。
                Errc::InvalidState,
                // 保留 set_text 与 ownership owner 阶段。
                "WaylandBackend::set_text: owns_clipboard lock poisoned",
            )
        })?;
        // 文本缓存 owner 同样先验证，失败不得失效旧 selection。
        let mut clipboard_text = self.clipboard_text.lock().map_err(|_| {
            // 构造稳定的缓存 owner 状态错误。
            Error::new(
                // 同步共享 owner 损坏统一分类为 InvalidState。
                Errc::InvalidState,
                // 保留 set_text 与文本缓存阶段。
                "WaylandBackend::set_text: clipboard_text lock poisoned",
            )
        })?;
        // 输入 serial owner 损坏时不得继续创建或提交 selection source。
        let serial_owner = self.last_input_serial.lock().map_err(|_| {
            // 构造稳定的剪贴板 serial 状态错误。
            Error::new(
                // 共享输入状态已无法安全读取。
                Errc::InvalidState,
                // 保留 Wayland set_text 与输入 serial 阶段。
                "Wayland clipboard input serial mutex poisoned during set_text",
            )
        })?;
        // 健康 guard 只读取最近一次 pointer/keyboard serial。
        let serial = serial_owner.latest().ok_or_else(|| {
            // 缺少一次性协议授权时返回错误并保持旧 selection 全部状态。
            Error::new(
                // 当前调用缺少必要输入身份，属于无效状态。
                Errc::InvalidState,
                // 保留 set_text 与 selection serial 上下文。
                "WaylandBackend::set_text: no pointer or keyboard serial for selection",
            )
        })?;
        // 新 selection 建立前先注销并释放旧 active source callback owner。
        if let Some(previous) = self.clipboard_source.take() {
            // 迟到旧 Send/Cancelled 不得再修改当前 clipboard owners。
            previous.clear_callback();
        }
        // 旧 owner 已失效后创建新 data-source 协议对象。
        let source = dm.create_data_source();
        source.offer("text/plain;charset=utf-8".to_string());
        let bytes = Arc::<[u8]>::from(text.as_bytes());
        let writes = Arc::clone(&self.clipboard_writes);
        // callback failure 继续复用 backend 唯一 pending source。
        let pending_failures = self.pending_failures.clone();
        // data-source 需要按具体事件决定 callback owner 是否继续登记。
        source.quick_assign_with_lifecycle(move |_, event, _| {
            // Send 可重复而 Cancelled 是当前 source 的生命周期终点。
            match event {
                // 每个 Send 事件建立独立非阻塞 write owner。
                wl_data_source::Event::Send { mime_type: _, fd } => {
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
                                return CallbackDisposition::Keep;
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
                    // Send 完成后继续服务当前 selection 的后续读取请求。
                    CallbackDisposition::Keep
                }
                // Cancelled 表示 compositor 不再使用当前 data-source。
                wl_data_source::Event::Cancelled => {
                    // ownership owner 健康时提交本地 selection 失效事实。
                    if let Ok(mut owns) = callback_owns_clipboard.lock() {
                        // Cancelled 后不得继续声称拥有系统剪贴板。
                        *owns = false;
                    } else {
                        // callback 无返回通道，poisoned owner 进入同一 pending source。
                        enqueue_selection_state_failure(
                            &pending_failures,
                            "owns flag on Cancelled",
                        );
                    }
                    // lifecycle adapter 消费 callback owner，禁止 dispatch 回插。
                    CallbackDisposition::Remove
                }
                // Target/Action 等非终点事件保持 callback owner。
                _ => CallbackDisposition::Keep,
            }
        });
        dd.set_selection(Some(&source), serial);
        // selection 请求成功排队后才发布调用方可读取的系统剪贴板文本事实。
        *clipboard_text = text.to_string();
        // 同一事务随后发布本进程当前拥有 selection。
        *owns_clipboard = true;
        // selection 请求提交后才发布新的 active source owner。
        self.clipboard_source = Some(source);
        Ok(())
    }
    fn has_text(&self) -> Result<bool> {
        // 生命周期 gate 必须先于文本 owner lock。
        self.ensure_clipboard_open("has_text")?;
        // 协议 capability gate 必须先于缓存读取。
        self.ensure_clipboard_data_device("has_text")?;
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
