//! UI 线程入口契约：把「在专用大栈 UI 线程上运行」从应用层下沉到平台层。
//!
//! Windows 主线程默认栈较小，深层 ViewNode 树的构建、协调与布局递归需要
//! 有界大栈线程；其余平台沿用当前线程直接执行。本模块对两种策略提供
//! 单一零 cfg 入口，由 `imp` 按目标平台分别实现。

// 把闭包移动到平台线程执行并返回其结果。
pub fn run_on_ui_thread<F, R>(thread_name: &str, run: F) -> R
where
    // 闭包只运行一次、可跨线程移动，且与返回值都可以跨线程发送。
    F: FnOnce() -> R + Send + 'static,
    R: Send,
{
    // 平台差异（大栈线程 vs 当前线程直接执行）全部藏在 imp 内。
    super::imp::run_on_ui_thread(thread_name, run)
}
