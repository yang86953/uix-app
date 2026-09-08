//! 真实终端会话：显式启动 PTY shell、持续双向读写、窗口尺寸同步与资源释放。
//!
//! SMC 职责：本文件是终端 System——唯一拥有子进程、PTY fd 与 VT 屏幕状态的
//! 写入权威；UI 组件（`super::screen_widget`）只是它的只读投影和输入转发。
//! 视图构建与 reconcile 不隐式创建进程：会话只能经 [`TerminalSession::spawn`]
//! 显式创建。读写均不阻塞 UI 线程：读取由专职线程 poll 驱动，字节先入共享
//! 缓冲再由 `pump` 在 UI 线程消费；输入整次接受到有界队列，同一线程按
//! POLLOUT 发送。背压不会丢失已接受输入的后半截，也不阻塞 UI 等待 PTY。
//!
//! 平台边界：PTY 仅在 Linux 实现；其他平台 `spawn` 返回 `NotImplemented`
//! 类型化失败。VT 屏幕解析跨平台可用，但没有会话来源时不会运转。

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use super::vt::{MAX_SCROLLBACK_ROWS, ScrollbackState, TerminalModes, TerminalRow, VtScreen};
use crate::core::{Errc, Error};

// Bound both the primary and lazily allocated alternate screen. The existing
// u16 dimensions alone would permit billions of cells per screen.
const MAX_SCREEN_CELLS: usize = 1_048_576;
const MAX_PENDING_INPUT_BYTES: usize = 1_048_576;

fn validate_screen_size(cols: u16, rows: u16) -> Result<(), Error> {
    if usize::from(cols.max(2)) * usize::from(rows.max(2)) > MAX_SCREEN_CELLS {
        return Err(Error::new(
            Errc::InsufficientResources,
            "terminal screen exceeds the 1048576-cell limit",
        ));
    }
    Ok(())
}

/// 有新输出到达时用于唤醒 UI 线程的回调；在读线程上调用，必须只做投递。
pub type TerminalOutputWaker = Arc<dyn Fn() + Send + Sync>;

/// 真实终端会话的启动配置。
#[derive(Clone)]
pub struct TerminalSessionConfig {
    /// 要执行的命令与参数；首项是可执行文件路径或 `PATH` 中的名字。
    pub command: Vec<String>,
    /// 初始网格列数；最小 2，总网格上限 1,048,576 格。
    pub cols: u16,
    /// 初始网格行数；最小 2。
    pub rows: u16,
    /// 附加环境变量；`TERM` 默认注入 `xterm-256color`，此处可覆盖。
    pub env: Vec<(String, String)>,
    /// 子进程工作目录；`None` 继承当前目录。
    pub working_dir: Option<std::path::PathBuf>,
    /// 输出到达唤醒回调（例如 `AppHandle::post_to_ui` 后驱动 `pump`）。
    pub on_output: Option<TerminalOutputWaker>,
}

impl Default for TerminalSessionConfig {
    fn default() -> Self {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
        Self {
            command: vec![shell],
            cols: 80,
            rows: 24,
            env: Vec::new(),
            working_dir: None,
            on_output: None,
        }
    }
}

/// 会话生命周期状态。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalSessionStatus {
    /// 子进程仍在运行。
    Running,
    /// 子进程已退出；`code` 为 `None` 表示被信号终止。
    Exited { code: Option<i32> },
}

// UI 接受输入的唯一容量与关闭门禁；I/O 线程是唯一发送端。
#[derive(Default)]
struct PendingInput {
    bytes: VecDeque<u8>,
    closed: bool,
}

// I/O 线程与 UI 线程共享的有界通道与生命周期事实。
struct SharedOutput {
    // 读线程 append、UI 线程 drain 的字节缓冲。
    pending: Mutex<Vec<u8>>,
    // 读线程已观察到 EOF/挂断。
    eof: AtomicBool,
    // 读线程收割到的子进程退出信息；Running 时为 None。
    exit_code: Mutex<Option<Option<i32>>>,
    input: Mutex<PendingInput>,
    stop: AtomicBool,
    child_reaped: AtomicBool,
    waker: Option<TerminalOutputWaker>,
}

impl SharedOutput {
    fn append(&self, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        if let Ok(mut pending) = self.pending.lock() {
            // 有界缓冲：异常堆积超过 8 MiB 时丢弃最旧数据，避免无界增长。
            if pending.len() + bytes.len() > 8 * 1024 * 1024 {
                let excess = pending.len() + bytes.len() - 8 * 1024 * 1024;
                pending.drain(0..excess);
            }
            pending.extend_from_slice(bytes);
        }
    }

    fn take(&self) -> Vec<u8> {
        match self.pending.lock() {
            Ok(mut pending) => std::mem::take(&mut *pending),
            Err(poisoned) => std::mem::take(&mut poisoned.into_inner()),
        }
    }

    fn notify_output(&self) {
        if let Some(waker) = self.waker.as_ref() {
            waker();
        }
    }
}

// UI 线程独占的会话控制状态。
struct ControlState {
    screen: VtScreen,
    // 解析器与会话核心同生命周期；Perform 回调借用屏幕状态。
    parser: vte::Parser,
    status: TerminalSessionStatus,
}

// Linux PTY 资源；Drop 完成唤醒、join、关闭与子进程回收。
#[cfg(target_os = "linux")]
struct PtyResources {
    // UI 线程持有的 master 副本；ioctl 使用，最后关闭触发 SIGHUP。
    master_dup: std::os::fd::RawFd,
    // 非阻塞唤醒管道由资源统一持有，I/O 线程仅借读端直到 join。
    // 保留读端还使线程退出后的唤醒不会产生 SIGPIPE。
    wake_write: std::os::fd::RawFd,
    wake_read: std::os::fd::RawFd,
    pid: libc::pid_t,
    reader: Option<std::thread::JoinHandle<()>>,
    shared: Arc<SharedOutput>,
}

#[cfg(target_os = "linux")]
impl Drop for PtyResources {
    fn drop(&mut self) {
        // 1. 请求读线程退出并等待它关闭自己的 master 副本。
        self.shared.stop.store(true, Ordering::Release);
        let _ = signal_io(self.wake_write);
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        // 2. 关闭全部 master 副本：前台会话收到 SIGHUP。
        unsafe {
            libc::close(self.wake_write);
            libc::close(self.wake_read);
            libc::close(self.master_dup);
        }
        // 3. 兜底回收：忽略 SIGHUP 的子进程补 SIGKILL 后阻塞收割。
        // 读线程已收割的 PID 不再属于会话，绝不能向可能被重用的数字发信号。
        if !self.shared.child_reaped.load(Ordering::Acquire) {
            unsafe {
                libc::kill(self.pid, libc::SIGKILL);
                let mut status: libc::c_int = 0;
                while libc::waitpid(self.pid, &mut status, 0) < 0 && errno() == libc::EINTR {}
            }
        }
    }
}

// 会话核心：Rc 单线程句柄；跨线程部分全部收敛在 SharedOutput。
pub(crate) struct SessionCore {
    shared: Arc<SharedOutput>,
    control: RefCell<ControlState>,
    // 输出代际的公开观察句柄：绘制组件在 render 中 get 以建立精确重绘订阅。
    revision: crate::ui::State<u64>,
    #[cfg(target_os = "linux")]
    pty: RefCell<Option<PtyResources>>,
}

/// 一个真实交互式终端会话句柄；clone 共享同一会话。
///
/// 生命周期：显式 [`spawn`](TerminalSession::spawn) 创建；所有克隆 drop 后
/// 关闭 PTY、回收子进程并 join 读线程。UI 线程拥有；不可跨线程发送。
#[derive(Clone)]
pub struct TerminalSession {
    core: Rc<SessionCore>,
}

impl TerminalSession {
    /// 在新 PTY 上显式启动 `config.command`，返回会话句柄。
    ///
    /// 失败（平台不支持、命令找不到、fork/pty 分配失败）返回类型化错误；
    /// exec 失败经 CLOEXEC 管道同步确认，不会留下半启动会话。
    pub fn spawn(config: &TerminalSessionConfig) -> Result<Self, Error> {
        if config.command.is_empty() {
            return Err(Error::new(
                Errc::InvalidArgument,
                "terminal session command must not be empty",
            ));
        }
        #[cfg(not(target_os = "linux"))]
        return Err(Error::new(
            Errc::NotImplemented,
            "terminal session requires Linux PTY support",
        ));
        validate_screen_size(config.cols, config.rows)?;
        let shared = Arc::new(SharedOutput {
            pending: Mutex::new(Vec::new()),
            eof: AtomicBool::new(false),
            exit_code: Mutex::new(None),
            input: Mutex::new(PendingInput::default()),
            stop: AtomicBool::new(false),
            child_reaped: AtomicBool::new(false),
            waker: config.on_output.clone(),
        });
        let screen = VtScreen::new(config.cols.max(2) as usize, config.rows.max(2) as usize);
        #[cfg(target_os = "linux")]
        let pty = spawn_pty(config, Arc::clone(&shared))?;
        let core = Rc::new(SessionCore {
            shared,
            control: RefCell::new(ControlState {
                screen,
                parser: vte::Parser::new(),
                status: TerminalSessionStatus::Running,
            }),
            revision: crate::ui::State::new(0),
            #[cfg(target_os = "linux")]
            pty: RefCell::new(Some(pty)),
        });
        Ok(Self { core })
    }

    /// 把读线程攒下的输出消费进 VT 屏幕状态；返回屏幕是否发生变化。
    ///
    /// 只应在 UI 线程调用（与绘制同线程）。无事件循环的测试可直接轮询本方法。
    pub fn pump(&self) -> bool {
        // 先读取退出事实再 drain：已公布退出时，所有先前输出都已入队。
        // 反过来的顺序会在退出竞争中先发布 Exited、把最后一块留到下次 pump。
        let finished = *self
            .core
            .shared
            .exit_code
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let chunk = self.core.shared.take();
        let mut changed = false;
        let mut control = self.core.control.borrow_mut();
        if !chunk.is_empty() {
            // 字段级拆分借用：屏幕状态与解析器互相独立地可变借用。
            let ControlState { screen, parser, .. } = &mut *control;
            screen.feed(parser, &chunk);
            changed = true;
        }
        if control.status == TerminalSessionStatus::Running {
            if let Some(code) = finished {
                control.status = TerminalSessionStatus::Exited { code };
                changed = true;
            }
        }
        drop(control);
        if changed {
            // 输出代际经 State 广播：render 中订阅的组件被精确重绘。
            self.core.revision.set(self.core.revision.get() + 1);
        }
        changed
    }

    /// 当前会话状态；退出码在读线程收割后经 `pump` 呈现。
    pub fn status(&self) -> TerminalSessionStatus {
        self.core.control.borrow().status
    }

    /// 整次接受原始输入，按序经非阻塞 PTY 发送；Ok 不等于子进程已执行。
    ///
    /// 待发预算为 1 MiB。单次超预算返回 `InsufficientResources`，剩余容量
    /// 不足返回 `WouldBlock`；失败不接受本次任何字节，已接受输入不会因
    /// EAGAIN 丢尾。会话关闭返回 `InvalidOperation`，不持久重试；空输入无操作。
    pub fn write(&self, bytes: &[u8]) -> Result<(), Error> {
        if bytes.is_empty() {
            return Ok(());
        }
        if self.core.control.borrow().status != TerminalSessionStatus::Running {
            return Err(Error::new(
                Errc::InvalidOperation,
                "terminal session is not running",
            ));
        }
        #[cfg(target_os = "linux")]
        {
            let borrow = self.core.pty.borrow();
            let Some(pty) = borrow.as_ref() else {
                return Err(Error::new(
                    Errc::InvalidOperation,
                    "terminal session is not running",
                ));
            };
            if bytes.len() > MAX_PENDING_INPUT_BYTES {
                return Err(Error::new(
                    Errc::InsufficientResources,
                    "terminal input exceeds 1 MiB",
                ));
            }
            let mut input = self
                .core
                .shared
                .input
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            if input.closed {
                return Err(Error::new(
                    Errc::InvalidOperation,
                    "terminal session is closed",
                ));
            }
            if bytes.len() > MAX_PENDING_INPUT_BYTES - input.bytes.len() {
                return Err(Error::warn(
                    Errc::WouldBlock,
                    "terminal pending input buffer is full; input not accepted",
                ));
            }
            let previous_len = input.bytes.len();
            input.bytes.extend(bytes.iter().copied());
            if let Err(error) = signal_io(pty.wake_write) {
                // 锁仍在手，发送线程不可能消费本次输入；回滚后再暴露失败。
                input.bytes.truncate(previous_len);
                return Err(error);
            }
            Ok(())
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = bytes;
            Err(Error::new(
                Errc::NotImplemented,
                "terminal session requires Linux PTY support",
            ))
        }
    }

    /// 按当前 2004 模式发送粘贴，与普通键入文本/原始 write 区分。
    ///
    /// 保留 Unicode 与 Tab/CR/LF，去掉其他 C0/C1/DEL 控制字符，防止内容
    /// 提前结束粘贴包围；需要控制字节时使用 write。空内容不产生标记。
    /// 原始文本加包围字节最多 1 MiB；接受与背压语义同 write。
    pub fn paste(&self, text: &str) -> Result<(), Error> {
        if text.is_empty() {
            return Ok(());
        }
        let bracketed = self.modes().bracketed_paste;
        let framing = if bracketed { 12 } else { 0 };
        if text.len() > MAX_PENDING_INPUT_BYTES - framing {
            return Err(Error::new(
                Errc::InsufficientResources,
                "terminal paste exceeds 1 MiB",
            ));
        }
        let mut bytes = Vec::with_capacity(text.len() + framing);
        if bracketed {
            bytes.extend_from_slice(b"\x1b[200~");
        }
        for ch in text
            .chars()
            .filter(|ch| !ch.is_control() || matches!(ch, '\t' | '\r' | '\n'))
        {
            let mut encoded = [0u8; 4];
            bytes.extend_from_slice(ch.encode_utf8(&mut encoded).as_bytes());
        }
        if bytes.len() == framing / 2 {
            return Ok(());
        }
        if bracketed {
            bytes.extend_from_slice(b"\x1b[201~");
        }
        self.write(&bytes)
    }

    /// 当前会话模式快照；由已 pump 的子进程控制序列驱动。
    pub fn modes(&self) -> TerminalModes {
        self.core.control.borrow().screen.modes()
    }

    /// 同步窗口网格尺寸到 PTY（TIOCSWINSZ + SIGWINCH）并调整屏幕状态。
    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), Error> {
        // Reject before ioctl or changing either retained screen.
        validate_screen_size(cols, rows)?;
        #[cfg(target_os = "linux")]
        {
            let borrow = self.core.pty.borrow();
            let Some(pty) = borrow.as_ref() else {
                return Err(Error::new(
                    Errc::InvalidOperation,
                    "terminal session is not running",
                ));
            };
            let winsize = libc::winsize {
                ws_row: rows.max(2),
                ws_col: cols.max(2),
                ws_xpixel: 0,
                ws_ypixel: 0,
            };
            let result = unsafe {
                libc::ioctl(
                    pty.master_dup,
                    libc::TIOCSWINSZ,
                    &winsize as *const libc::winsize,
                )
            };
            if result != 0 {
                return Err(Error::new(
                    Errc::PlatformError,
                    format!("terminal session resize ioctl failed: errno {}", errno()),
                ));
            }
            unsafe {
                libc::kill(pty.pid, libc::SIGWINCH);
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (cols, rows);
            return Err(Error::new(
                Errc::NotImplemented,
                "terminal session requires Linux PTY support",
            ));
        }
        self.core
            .control
            .borrow_mut()
            .screen
            .resize(cols.max(2) as usize, rows.max(2) as usize);
        Ok(())
    }

    /// 整屏文本段快照（行序、列序）。
    pub fn rows(&self) -> Vec<TerminalRow> {
        self.core.control.borrow().screen.snapshot()
    }

    /// 当前保留的主屏滚回行数，交替屏不向这份历史写入。
    pub fn scrollback_len(&self) -> usize {
        self.core.control.borrow().screen.scrollback_state().rows
    }

    /// 历史行数上限，默认 1,000；总格数还受 1,048,576 的独立上限约束。
    pub fn scrollback_limit(&self) -> usize {
        self.core.control.borrow().screen.scrollback_limit()
    }

    /// 设置 0–100,000 行的保留上限；0 禁用并清除现有历史。
    /// 超限拒绝且保留原设置；缩减上限先淘汰最旧行，不改变活动屏。
    pub fn set_scrollback_limit(&self, rows: usize) -> Result<(), Error> {
        if rows > MAX_SCROLLBACK_ROWS {
            return Err(Error::new(
                Errc::InvalidArgument,
                "terminal scrollback limit exceeds 100000 rows",
            ));
        }
        self.core
            .control
            .borrow_mut()
            .screen
            .set_scrollback_limit(rows);
        self.core.revision.set(self.core.revision.get() + 1);
        Ok(())
    }

    /// 从最旧保留行的零基 start 读取最多 count 行；越界返回空或剩余行。
    /// 返回记录时的列宽与颜色，不包含活动网格，不读磁盘或阻塞 PTY。
    pub fn scrollback_rows(&self, start: usize, count: usize) -> Vec<TerminalRow> {
        self.core
            .control
            .borrow()
            .screen
            .scrollback_rows(start, count)
    }

    /// 明确清除主屏滚回内容；活动网格与子进程不变。
    pub fn clear_scrollback(&self) {
        self.core.control.borrow_mut().screen.clear_scrollback();
        self.core.revision.set(self.core.revision.get() + 1);
    }

    pub(crate) fn scrollback_state(&self) -> ScrollbackState {
        self.core.control.borrow().screen.scrollback_state()
    }

    pub(crate) fn viewport_rows(&self, offset: usize) -> Vec<TerminalRow> {
        self.core.control.borrow().screen.viewport_rows(offset)
    }

    /// 光标位置（行、列）。
    pub fn cursor(&self) -> (usize, usize) {
        self.core.control.borrow().screen.cursor()
    }

    /// 当前网格尺寸（列、行）。
    pub fn screen_size(&self) -> (usize, usize) {
        let control = self.core.control.borrow();
        (control.screen.cols(), control.screen.rows())
    }

    /// 输出代际观察句柄：pump 消费到新输出时自增。
    ///
    /// 绘制组件在 render 中读取该句柄即可建立精确重绘订阅；
    /// 应用也可以用它把会话活动接入自己的响应式管道。
    pub fn output_revision(&self) -> crate::ui::State<u64> {
        self.core.revision.clone()
    }

    /// 收到但未实现的控制序列计数（诊断与限制核对用）。
    pub fn ignored_sequences(&self) -> usize {
        self.core.control.borrow().screen.ignored_sequences
    }

    // 两个句柄是否指向同一会话（供组件 reconcile 比较）。
    pub(crate) fn same_session(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.core, &other.core)
    }
}

// ── Linux PTY 实现细节 ──

#[cfg(target_os = "linux")]
fn errno() -> i32 {
    std::io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

// 在 PATH 中解析可执行文件完整路径；含路径分隔符时原样返回。
#[cfg(target_os = "linux")]
fn resolve_executable(command: &str) -> Option<std::ffi::CString> {
    use std::os::unix::ffi::OsStrExt;
    let Ok(text) = std::ffi::CString::new(command) else {
        return None;
    };
    if command.contains('/') {
        return Some(text);
    }
    let paths = std::env::var_os("PATH").unwrap_or_default();
    for dir in std::env::split_paths(&paths) {
        let candidate = dir.join(command);
        let Ok(path) = std::ffi::CString::new(candidate.as_os_str().as_bytes()) else {
            continue;
        };
        if candidate.is_file() && unsafe { libc::access(path.as_ptr(), libc::X_OK) } == 0 {
            return Some(path);
        }
    }
    None
}

// 组装 execve 环境：当前环境保证 TERM 非空后再应用显式附加项。
#[cfg(target_os = "linux")]
fn build_env(config: &TerminalSessionConfig) -> Vec<std::ffi::CString> {
    let mut pairs: Vec<(String, String)> = std::env::vars().collect();
    if let Some(term) = pairs.iter_mut().find(|(key, _)| key == "TERM") {
        if term.1.is_empty() {
            term.1 = "xterm-256color".to_string();
        }
    } else {
        pairs.push(("TERM".to_string(), "xterm-256color".to_string()));
    }
    for (key, value) in &config.env {
        match pairs.iter_mut().find(|(existing, _)| existing == key) {
            Some(slot) => slot.1.clone_from(value),
            None => pairs.push((key.clone(), value.clone())),
        }
    }
    pairs
        .into_iter()
        .filter_map(|(key, value)| std::ffi::CString::new(format!("{key}={value}")).ok())
        .collect()
}

// 启动 PTY、fork/exec 子进程并接管读线程。
#[cfg(target_os = "linux")]
fn spawn_pty(
    config: &TerminalSessionConfig,
    shared: Arc<SharedOutput>,
) -> Result<PtyResources, Error> {
    let Some(program) = resolve_executable(&config.command[0]) else {
        return Err(Error::new(
            Errc::FileNotFound,
            format!("terminal session command not found: {}", config.command[0]),
        ));
    };
    let mut argv: Vec<std::ffi::CString> = Vec::with_capacity(config.command.len());
    for argument in &config.command {
        let Some(text) = std::ffi::CString::new(argument.as_str()).ok() else {
            return Err(Error::new(
                Errc::InvalidArgument,
                "terminal session command contains NUL byte",
            ));
        };
        argv.push(text);
    }
    let envp = build_env(&config);

    // CLOEXEC readiness 管道：exec 成功即 EOF，失败回传 errno 字节。
    let mut readiness = [0 as libc::c_int; 2];
    if unsafe { libc::pipe2(readiness.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
        return Err(Error::new(
            Errc::InsufficientResources,
            format!("terminal session readiness pipe failed: errno {}", errno()),
        ));
    }
    // 关闭唤醒管道：读线程 poll master 与 wake 读端。
    let mut wake = [0 as libc::c_int; 2];
    if unsafe { libc::pipe2(wake.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK) } != 0 {
        unsafe {
            libc::close(readiness[0]);
            libc::close(readiness[1]);
        }
        return Err(Error::new(
            Errc::InsufficientResources,
            format!("terminal session wake pipe failed: errno {}", errno()),
        ));
    }

    let master = unsafe { libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY) };
    if master < 0 {
        unsafe {
            libc::close(readiness[0]);
            libc::close(readiness[1]);
            libc::close(wake[0]);
            libc::close(wake[1]);
        }
        return Err(Error::new(
            Errc::InsufficientResources,
            format!("terminal session posix_openpt failed: errno {}", errno()),
        ));
    }
    let fail_closed = |master: libc::c_int| unsafe {
        libc::close(master);
        libc::close(readiness[0]);
        libc::close(readiness[1]);
        libc::close(wake[0]);
        libc::close(wake[1]);
    };
    if unsafe { libc::grantpt(master) } != 0 || unsafe { libc::unlockpt(master) } != 0 {
        fail_closed(master);
        return Err(Error::new(
            Errc::PlatformError,
            format!("terminal session pty setup failed: errno {}", errno()),
        ));
    }
    let mut pts_name = [0u8; 64];
    if unsafe { libc::ptsname_r(master, pts_name.as_mut_ptr().cast(), pts_name.len()) } != 0 {
        fail_closed(master);
        return Err(Error::new(
            Errc::PlatformError,
            format!("terminal session ptsname failed: errno {}", errno()),
        ));
    }
    let pts_path_len = pts_name.iter().position(|byte| *byte == 0).unwrap_or(0);
    let pts_name = std::ffi::CString::new(pts_name[..pts_path_len].to_vec()).unwrap_or_default();

    // 初始窗口尺寸在 fork 前生效，子进程首屏 stty 即正确。
    let winsize = libc::winsize {
        ws_row: config.rows.max(2),
        ws_col: config.cols.max(2),
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    unsafe {
        libc::ioctl(master, libc::TIOCSWINSZ, &winsize as *const libc::winsize);
    }

    let child_working_dir = config
        .working_dir
        .as_deref()
        .and_then(|dir| dir.to_str())
        .and_then(|dir| std::ffi::CString::new(dir).ok());

    let argv_ptr: Vec<*const libc::c_char> = argv
        .iter()
        .map(|text| text.as_ptr().cast())
        .chain(std::iter::once(std::ptr::null()))
        .collect();
    let envp_ptr: Vec<*const libc::c_char> = envp
        .iter()
        .map(|text| text.as_ptr().cast())
        .chain(std::iter::once(std::ptr::null()))
        .collect();

    let pid = unsafe { libc::fork() };
    if pid < 0 {
        fail_closed(master);
        return Err(Error::new(
            Errc::InsufficientResources,
            format!("terminal session fork failed: errno {}", errno()),
        ));
    }
    if pid == 0 {
        // 子进程：新会话、slave 作为控制终端、dup2 到 0/1/2 后 execve。
        unsafe {
            libc::setsid();
            let slave = libc::open(pts_name.as_ptr(), libc::O_RDWR);
            if slave < 0 {
                libc::_exit(127);
            }
            libc::ioctl(slave, libc::TIOCSCTTY, 0 as libc::c_ulong);
            libc::dup2(slave, 0);
            libc::dup2(slave, 1);
            libc::dup2(slave, 2);
            if slave > 2 {
                libc::close(slave);
            }
            libc::close(master);
            libc::close(readiness[0]);
            libc::close(wake[0]);
            libc::close(wake[1]);
            // Rust 运行时忽略 SIGPIPE，shell 期望默认行为。
            libc::signal(libc::SIGPIPE, libc::SIG_DFL);
            if let Some(dir) = child_working_dir.as_ref() {
                if libc::chdir(dir.as_ptr()) != 0 {
                    // 工作目录失败按启动失败处理，回传退出码 126。
                    let code = 126u8;
                    libc::write(readiness[1], (&code as *const u8).cast(), 1);
                    libc::_exit(126);
                }
            }
            libc::execve(
                program.as_ptr(),
                argv_ptr.as_ptr().cast_mut(),
                envp_ptr.as_ptr().cast_mut(),
            );
            // exec 失败：回传errno 后以 127 退出，父进程同步转成类型化错误。
            let code = errno().clamp(1, 254) as u8;
            libc::write(readiness[1], (&code as *const u8).cast(), 1);
            libc::_exit(127);
        }
    }

    // 父进程：先确认 exec 成败，再接管资源。
    unsafe {
        libc::close(readiness[1]);
    }
    let mut readiness_byte = [0u8; 1];
    let seen = unsafe { libc::read(readiness[0], readiness_byte.as_mut_ptr().cast(), 1) };
    unsafe {
        libc::close(readiness[0]);
    }
    if seen != 0 {
        // exec/chdir 失败：子进程已退出，回收后返回类型化错误。
        let mut status: libc::c_int = 0;
        unsafe {
            let _ = libc::waitpid(pid, &mut status, 0);
            libc::close(master);
            libc::close(wake[0]);
            libc::close(wake[1]);
        }
        return Err(Error::new(
            Errc::FileNotFound,
            format!(
                "terminal session child failed to start: reported code {}",
                readiness_byte[0]
            ),
        ));
    }

    // I/O 线程读写设非阻塞，poll 决定何时可以继续传输。
    if unsafe {
        let flags = libc::fcntl(master, libc::F_GETFL);
        libc::fcntl(master, libc::F_SETFL, flags | libc::O_NONBLOCK)
    } != 0
    {
        unsafe {
            libc::close(master);
            libc::close(wake[0]);
            libc::close(wake[1]);
        }
        return Err(Error::new(
            Errc::PlatformError,
            format!("terminal session fcntl failed: errno {}", errno()),
        ));
    }

    // I/O 线程持有独立 fd 副本，UI 保留 ioctl 与最终关闭的所有权。
    let reader_master = unsafe { libc::dup(master) };
    if reader_master < 0 {
        unsafe {
            libc::close(master);
            libc::close(wake[0]);
            libc::close(wake[1]);
        }
        return Err(Error::new(
            Errc::InsufficientResources,
            format!("terminal session dup failed: errno {}", errno()),
        ));
    }

    let reader_shared = Arc::clone(&shared);
    let reader = std::thread::Builder::new()
        .name("uix-terminal-io".to_string())
        .spawn(move || reader_loop(reader_master, wake[0], pid, reader_shared))
        .map_err(|error| {
            unsafe {
                libc::kill(pid, libc::SIGKILL);
                let mut status: libc::c_int = 0;
                let _ = libc::waitpid(pid, &mut status, 0);
                libc::close(master);
                libc::close(reader_master);
                libc::close(wake[0]);
                libc::close(wake[1]);
            }
            Error::new(
                Errc::InsufficientResources,
                format!("terminal session reader thread failed: {error}"),
            )
        })?;

    Ok(PtyResources {
        master_dup: master,
        wake_write: wake[1],
        wake_read: wake[0],
        pid,
        reader: Some(reader),
        shared,
    })
}

// 同一 I/O 线程公平处理读/写与关闭；无输入时不订阅 POLLOUT，避免空转。
#[cfg(target_os = "linux")]
fn reader_loop(
    master: std::os::fd::RawFd,
    wake_read: std::os::fd::RawFd,
    pid: libc::pid_t,
    shared: Arc<SharedOutput>,
) {
    let mut buffer = [0u8; 8192];
    loop {
        if shared.stop.load(Ordering::Acquire) {
            break;
        }
        let writable = !shared
            .input
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .bytes
            .is_empty();
        let mut polls = [
            libc::pollfd {
                fd: master,
                events: libc::POLLIN | if writable { libc::POLLOUT } else { 0 },
                revents: 0,
            },
            libc::pollfd {
                fd: wake_read,
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        let result = unsafe { libc::poll(polls.as_mut_ptr(), 2, -1) };
        if result < 0 {
            let code = errno();
            if code == libc::EINTR {
                continue;
            }
            crate::diagnostics::observe_boundary_error(
                "terminal::session-reader",
                &Error::new(
                    Errc::IoError,
                    format!("terminal session poll failed: errno {code}"),
                ),
            );
            break;
        }
        if polls[1].revents & (libc::POLLIN | libc::POLLHUP) != 0 {
            // 唤醒可以是新输入或关闭；关闭事实独立存储，不依赖管道剩余容量。
            loop {
                let read =
                    unsafe { libc::read(wake_read, buffer.as_mut_ptr().cast(), buffer.len()) };
                if read > 0 || (read < 0 && errno() == libc::EINTR) {
                    continue;
                }
                break;
            }
            if shared.stop.load(Ordering::Acquire) {
                break;
            }
        }
        if polls[0].revents & (libc::POLLIN | libc::POLLHUP | libc::POLLERR) != 0 {
            let read = unsafe { libc::read(master, buffer.as_mut_ptr().cast(), buffer.len()) };
            if read > 0 {
                shared.append(&buffer[..read as usize]);
                shared.notify_output();
            } else if read == 0 {
                shared.eof.store(true, Ordering::Release);
                shared.notify_output();
                break;
            } else if !matches!(errno(), libc::EINTR | libc::EAGAIN) {
                // EIO 等：master 对端已全部关闭，按 EOF 处理。
                shared.eof.store(true, Ordering::Release);
                shared.notify_output();
                break;
            }
        }
        if polls[0].revents & libc::POLLOUT != 0 {
            if let Err(error) = send_pending_input(master, &shared) {
                crate::diagnostics::observe_boundary_error("terminal::session-writer", &error);
                shared.eof.store(true, Ordering::Release);
                shared.notify_output();
                break;
            }
        }
        if polls[0].revents & libc::POLLNVAL != 0 {
            break;
        }
    }
    {
        let mut input = shared.input.lock().unwrap_or_else(|e| e.into_inner());
        input.closed = true;
        input.bytes = VecDeque::new();
    }
    // 读线程关闭自己的 master 副本；UI 副本仍由 Drop 关闭。
    unsafe {
        libc::close(master);
    }
    // EOF 后短轮询收割子进程退出码（父进程在 UI 线程，读线程代收）。
    if shared.eof.load(Ordering::Acquire) {
        for _ in 0..400 {
            if shared.stop.load(Ordering::Acquire) {
                break;
            }
            let mut status: libc::c_int = 0;
            let result = unsafe { libc::waitpid(pid, &mut status, libc::WNOHANG) };
            if result == pid {
                let code = if libc::WIFEXITED(status) {
                    Some(libc::WEXITSTATUS(status))
                } else if libc::WIFSIGNALED(status) {
                    None
                } else {
                    // 停止/继续不构成退出事实，继续轮询。
                    continue;
                };
                shared.child_reaped.store(true, Ordering::Release);
                if let Ok(mut slot) = shared.exit_code.lock() {
                    *slot = Some(code);
                }
                shared.notify_output();
                break;
            }
            if result < 0 {
                if errno() == libc::ECHILD {
                    // 即使由宿主其他收割者消费，PID 也不再归本会话所有。
                    shared.child_reaped.store(true, Ordering::Release);
                }
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}

// 非阻塞唤醒。管道满意味着已有唤醒可读，不要求为每条输入保留一个通知。
#[cfg(target_os = "linux")]
fn signal_io(fd: std::os::fd::RawFd) -> Result<(), Error> {
    loop {
        let written = unsafe { libc::write(fd, [1u8].as_ptr().cast(), 1) };
        if written == 1 {
            return Ok(());
        }
        if written == 0 {
            return Err(Error::new(
                Errc::WriteFailure,
                "terminal I/O wake made no progress",
            ));
        }
        match errno() {
            libc::EINTR => continue,
            libc::EAGAIN if written < 0 => return Ok(()),
            code => {
                return Err(Error::new(
                    Errc::WriteFailure,
                    format!("terminal I/O wake failed: errno {code}"),
                ));
            }
        }
    }
}

// 每轮最多写 8 KiB，让持续输入与输出都获得处理机会；EAGAIN 保留全部余量。
#[cfg(target_os = "linux")]
fn send_pending_input(fd: std::os::fd::RawFd, shared: &SharedOutput) -> Result<(), Error> {
    let mut input = shared.input.lock().unwrap_or_else(|e| e.into_inner());
    let (first, second) = input.bytes.as_slices();
    let bytes = if first.is_empty() { second } else { first };
    if bytes.is_empty() {
        return Ok(());
    }
    let written = unsafe { libc::write(fd, bytes.as_ptr().cast(), bytes.len().min(8192)) };
    if written > 0 {
        input.bytes.drain(..written as usize);
        return Ok(());
    }
    if written < 0 && matches!(errno(), libc::EINTR | libc::EAGAIN) {
        return Ok(());
    }
    Err(Error::new(
        Errc::WriteFailure,
        format!(
            "terminal input delivery stopped: bytes written {written}, errno {}",
            errno()
        ),
    ))
}
