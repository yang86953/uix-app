//! 未适配目标的 Platform 实例生命周期实现。

use crate::core::{Errc, Error, Result};

// 未适配目标不允许创建 Platform 实例。
pub(crate) struct State;

impl State {
    // 返回稳定的未实现错误，不伪造有效生命周期。
    pub(crate) fn new() -> Result<Self> {
        Err(unsupported())
    }
}

// 未适配目标无法验证进程主线程。
pub(crate) fn is_main_thread() -> Result<bool> {
    Err(unsupported())
}

// 构造 Platform 创建阶段的统一错误。
fn unsupported() -> Error {
    Error::new(
        Errc::NotImplemented,
        "Platform::new: this target has no UIX platform runtime",
    )
}
