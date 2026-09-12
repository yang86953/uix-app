//! 协作式取消原语：纯 `std`，不依赖引擎或上层模块。
//!
//! 宿主持有令柄并可在任意线程请求取消；引擎在求值检查点观察到取消后
//! 以 `SchemeError::Cancelled` 终止，不可被脚本捕获。

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// 共享取消信号；克隆共享同一状态。
#[derive(Clone, Default)]
pub struct CancelToken {
    flag: Arc<AtomicBool>,
}

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    /// 请求取消；对已取消状态幂等。
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }
}

impl fmt::Debug for CancelToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CancelToken")
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}
