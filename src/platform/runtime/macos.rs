//! macOS 的 Platform 实例生命周期实现。

use crate::core::Result;

// macOS 不需要额外持有进程级平台资源。
pub(crate) struct State;

impl State {
    // 创建无资源的 macOS 生命周期状态。
    pub(crate) fn new() -> Result<Self> {
        Ok(Self)
    }
}

// 使用系统谓词判断当前线程是否为进程主线程。
pub(crate) fn is_main_thread() -> Result<bool> {
    // SAFETY: pthread_main_np 不接收参数，也不产生资源所有权。
    Ok(unsafe { pthread_main_np() } != 0)
}

#[link(name = "System")]
// SAFETY: 声明与 Darwin pthread ABI 一致。
unsafe extern "C" {
    fn pthread_main_np() -> i32;
}
