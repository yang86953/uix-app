//! `src/native/factory/thread_bound.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl<T: GraphicsContextLifecycle + ?Sized> ThreadBoundGraphicsContext<T>） ——

impl<T: GraphicsContextLifecycle + ?Sized> ThreadBoundGraphicsContext<T> {
    // 为单元测试注入不同 owner thread 身份。
    #[cfg(test)]
    fn with_test_owner(
        // 接收测试仍唯一拥有的类型化 context。
        inner: Box<T>,
        // 接收测试要模拟的 owner thread。
        owner_thread: ThreadId,
    ) -> Self {
        // 先按生产路径构造 wrapper。
        let mut bound = Self::new(inner);
        // 仅在测试构建覆盖线程身份。
        bound.owner_thread = owner_thread;
        // 返回可验证错误线程门禁的 wrapper。
        bound
    }
}
