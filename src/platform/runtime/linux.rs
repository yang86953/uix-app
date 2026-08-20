//! Linux 的 Platform 实例生命周期实现。

use crate::core::{Errc, Error, Result};

// Linux 不需要额外持有进程级平台资源。
pub(crate) struct State;

impl State {
    // 创建无资源的 Linux 生命周期状态。
    pub(crate) fn new() -> Result<Self> {
        Ok(Self)
    }
}

// Linux 的线程组 leader TID 与进程 ID 相同。
pub(crate) fn is_main_thread() -> Result<bool> {
    // SAFETY: getpid 与 syscall 不接收指针，也不转移资源所有权。
    let process_id = unsafe { libc::getpid() } as libc::c_long;
    // SAFETY: SYS_gettid 不接收指针，也不转移资源所有权。
    let thread_id = unsafe { libc::syscall(libc::SYS_gettid) };
    if thread_id < 0 {
        return Err(Error::new(
            Errc::PlatformError,
            format!(
                "Platform::new: gettid failed: {}",
                std::io::Error::last_os_error()
            ),
        ));
    }
    Ok(thread_id == process_id)
}
