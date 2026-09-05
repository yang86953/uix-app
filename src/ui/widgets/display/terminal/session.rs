//! 真实终端会话：显式启动 PTY shell、持续双向读写、窗口尺寸同步与资源释放。
//!
//! SMC 职责：本文件是终端 System——唯一拥有子进程、PTY fd 与 VT 屏幕状态的
//! 写入权威；UI 组件（`super::screen_widget`）只是它的只读投影和输入转发。
//! 视图构建与 reconcile 不隐式创建进程：会话只能经 [`TerminalSession::spawn`]
//! 显式创建。读写均不阻塞 UI 线程：读取由专职线程 poll 驱动，字节先入共享
//! 缓冲再由 `pump` 在 UI 线程消费；写入为非阻塞小流量写，缓冲满返回错误。
//!
//! 平台边界：PTY 仅在 Linux 实现；其他平台 `spawn` 返回 `NotImplemented`
//! 类型化失败。VT 屏幕解析跨平台可用，但没有会话来源时不会运转。

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use super::vt::{TerminalRow, VtScreen};
use crate::core::{Errc, Error};

/// 有新输出到达时用于唤醒 UI 线程的回调；在读线程上调用，必须只做投递。
pub type TerminalOutputWaker = Arc<dyn Fn() + Send + Sync>;

/// 真实终端会话的启动配置。
#[derive(Clone)]
pub struct TerminalSessionConfig {
    /// 要执行的命令与参数；首项是可执行文件路径或 `PATH` 中的名字。
    pub command: Vec<String>,
    /// 初始网格列数；最小 2。
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

// 读线程与 UI 线程共享的输出通道与生命周期事实。
struct SharedOutput {
    // 读线程 append、UI 线程 drain 的字节缓冲。
    pending: Mutex<Vec<u8>>,
    // 读线程已观察到 EOF/挂断。
    eof: AtomicBool,
    // 读线程收割到的子进程退出信息；Running 时为 None。
    exit_code: Mutex<Option<Option<i32>>>,
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
    // UI 线程持有的 master 副本；写与 ioctl 使用，最后关闭触发 SIGHUP。
    master_dup: std::os::fd::RawFd,
    // 关闭唤醒管道的写端；写一个字节请求读线程退出。
    wake_write: std::os::fd::RawFd,
    pid: libc::pid_t,
    reader: Option<std::thread::JoinHandle<()>>,
}

#[cfg(target_os = "linux")]
impl Drop for PtyResources {
    fn drop(&mut self) {
        // 1. 请求读线程退出并等待它关闭自己的 master 副本。
        unsafe {
            let byte = [1u8];
            let _ = libc::write(self.wake_write, byte.as_ptr().cast(), 1);
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
        // 2. 关闭全部 master 副本：前台会话收到 SIGHUP。
        unsafe {
            libc::close(self.wake_write);
            libc::close(self.master_dup);
        }
        // 3. 兜底回收：忽略 SIGHUP 的子进程补 SIGKILL 后阻塞收割。
        unsafe {
            libc::kill(self.pid, libc::SIGKILL);
            let mut status: libc::c_int = 0;
            let _ = libc::waitpid(self.pid, &mut status, 0);
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
        let shared = Arc::new(SharedOutput {
            pending: Mutex::new(Vec::new()),
            eof: AtomicBool::new(false),
            exit_code: Mutex::new(None),
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
            if let Some(code) = self
                .core
                .shared
                .exit_code
                .lock()
                .ok()
                .and_then(|guard| *guard)
            {
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

    /// 把按键编码或文本字节写入 PTY；非阻塞，缓冲满返回 `WouldBlock`。
    ///
    /// 会话已退出或已关闭时返回 `InvalidOperation`。
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
            write_all_nonblocking(pty.master_dup, bytes)
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

    /// 同步窗口网格尺寸到 PTY（TIOCSWINSZ + SIGWINCH）并调整屏幕状态。
    pub fn resize(&self, cols: u16, rows: u16) -> Result<(), Error> {
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
    if unsafe { libc::pipe2(wake.as_mut_ptr(), libc::O_CLOEXEC) } != 0 {
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

    // UI 线程写端设非阻塞，避免大段粘贴时阻塞 UI。
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

    // 读线程持有独立 fd 副本：它退出时关闭自己的副本，不影响 UI 写端。
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
        .name("uix-terminal-reader".to_string())
        .spawn(move || reader_loop(reader_master, wake[0], pid, reader_shared))
        .map_err(|error| {
            unsafe {
                libc::kill(pid, libc::SIGKILL);
                let mut status: libc::c_int = 0;
                let _ = libc::waitpid(pid, &mut status, 0);
                libc::close(master);
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
        pid,
        reader: Some(reader),
    })
}

// 读线程：poll master 与 wake；字节入共享缓冲并唤醒 UI，EOF 后收割子进程。
#[cfg(target_os = "linux")]
fn reader_loop(
    master: std::os::fd::RawFd,
    wake_read: std::os::fd::RawFd,
    pid: libc::pid_t,
    shared: Arc<SharedOutput>,
) {
    let mut buffer = [0u8; 8192];
    loop {
        let mut polls = [
            libc::pollfd {
                fd: master,
                events: libc::POLLIN,
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
            // UI 线程请求关闭；由 Drop 继续完成资源回收。
            break;
        }
        if polls[0].revents & (libc::POLLIN | libc::POLLHUP) != 0 {
            let read = unsafe { libc::read(master, buffer.as_mut_ptr().cast(), buffer.len()) };
            if read > 0 {
                shared.append(&buffer[..read as usize]);
                shared.notify_output();
                continue;
            }
            if read == 0 {
                shared.eof.store(true, Ordering::Release);
                shared.notify_output();
                break;
            }
            let code = errno();
            if code == libc::EINTR {
                continue;
            }
            if code == libc::EAGAIN {
                // poll 已确认可读，EAGAIN 只可能是偶发竞态。
                continue;
            }
            // EIO/EPERM 等：master 对端已全部关闭，按 EOF 处理。
            shared.eof.store(true, Ordering::Release);
            shared.notify_output();
            break;
        }
    }
    // 读线程关闭自己的 master 副本；UI 副本仍由 Drop 关闭。
    unsafe {
        libc::close(master);
        libc::close(wake_read);
    }
    // EOF 后短轮询收割子进程退出码（父进程在 UI 线程，读线程代收）。
    if shared.eof.load(Ordering::Acquire) {
        for _ in 0..400 {
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
                if let Ok(mut slot) = shared.exit_code.lock() {
                    *slot = Some(code);
                }
                shared.notify_output();
                break;
            }
            if result < 0 {
                // ECHILD 等异常：保留 Running，让 Drop 的兜底 waitpid 处理。
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
}

// 非阻塞写全部字节；EINTR 重试，EAGAIN 暴露为 WouldBlock。
#[cfg(target_os = "linux")]
fn write_all_nonblocking(fd: std::os::fd::RawFd, mut bytes: &[u8]) -> Result<(), Error> {
    while !bytes.is_empty() {
        let written = unsafe { libc::write(fd, bytes.as_ptr().cast(), bytes.len()) };
        if written >= 0 {
            bytes = &bytes[written as usize..];
            continue;
        }
        match errno() {
            libc::EINTR => continue,
            libc::EAGAIN => {
                return Err(Error::warn(
                    Errc::WouldBlock,
                    "terminal session write buffer full; input dropped",
                ));
            }
            libc::EIO | libc::EBADF => {
                return Err(Error::new(
                    Errc::InvalidOperation,
                    "terminal session is closed",
                ));
            }
            code => {
                return Err(Error::new(
                    Errc::WriteFailure,
                    format!("terminal session write failed: errno {code}"),
                ));
            }
        }
    }
    Ok(())
}
