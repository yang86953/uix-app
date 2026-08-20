//! UI 线程入口契约：把「在专用大栈 UI 线程上运行」从应用层下沉到平台层。
//!
//! Windows 主线程默认栈较小，深层 ViewNode 树的构建、协调与布局递归需要
//! 有界大栈线程；其余平台沿用当前线程直接执行。本模块对两种策略提供
//! 单一零 cfg 入口，平台差异在本功能域内消化。

// Windows 深层 UI 树使用有界的大栈线程。
#[cfg(windows)]
const UI_THREAD_STACK_BYTES: usize = 8 * 1024 * 1024;

/// 在平台规定的 UI 线程上执行闭包并返回结果。
pub fn run_on_ui_thread<F, R>(thread_name: &str, run: F) -> R
where
    // 统一契约覆盖 Windows 独立线程，闭包与返回值都不得借用调用栈。
    F: FnOnce() -> R + Send + 'static,
    R: Send + 'static,
{
    // Windows 把 UI 工作放入命名的大栈线程。
    #[cfg(windows)]
    {
        // 用目标名与固定大栈创建专用 UI 线程。
        let ui_thread = match std::thread::Builder::new()
            .name(thread_name.to_owned())
            .stack_size(UI_THREAD_STACK_BYTES)
            .spawn(run)
        {
            // 返回已创建的 UI 线程。
            Ok(ui_thread) => ui_thread,
            // 线程创建失败属于不可恢复的平台错误，文案带线程名便于诊断。
            Err(error) => panic!("spawn UI thread {thread_name:?}: {error}"),
        };
        // 等待 UI 线程结束；panic 按原样恢复到调用线程。
        return match ui_thread.join() {
            // 返回闭包结果。
            Ok(result) => result,
            // 把子线程 panic 负载恢复到当前线程继续展开。
            Err(payload) => std::panic::resume_unwind(payload),
        };
    }

    // 其他目标直接在当前线程执行，保持既有线程模型。
    #[cfg(not(windows))]
    {
        // 非 Windows 不使用线程名，但保留统一签名。
        let _ = thread_name;
        run()
    }
}
