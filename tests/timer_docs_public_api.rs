// 声明本文件只编译调度使用文档，不创建 AppHandle、线程或计时器。
#![allow(dead_code)]

// 隔离 app-timers 围栏中的计时器句柄生命周期。
mod app_timers {
    // 引入标准时长类型。
    use std::time::Duration;
    // 引入文档承诺的公开 AppHandle prelude。
    use uix::prelude::*;

    // 编译一次性与周期计时器的显式所有权策略。
    fn schedule_work(handle: &AppHandle) {
        // 安排两秒后在目标窗口会话执行的一次性任务。
        let detached = handle.run_after(Duration::from_secs(2), || {
            // 文档示例不执行额外副作用。
        });
        // 让计时器存活到执行完成或窗口会话拆除。
        detached.detach();

        // 安排每一百毫秒执行的周期任务。
        let cancelled = handle.run_interval(Duration::from_millis(100), || {
            // 文档示例不执行额外副作用。
        });
        // 通过句柄显式取消周期任务。
        cancelled.cancel();
    }
}

// 隔离 background-post-to-ui 围栏中的跨线程回 UI 路由。
mod background_post_to_ui {
    // 引入文档承诺的公开 AppHandle 与 State prelude。
    use uix::prelude::*;

    // 编译后台线程只经目标窗口队列更新业务状态的模式。
    fn load_in_background(handle: &AppHandle, data: &State<u32>) {
        // 克隆可发送的目标窗口应用句柄。
        let handle_clone = handle.clone();
        // 克隆由业务作用域拥有的共享状态句柄。
        let data = data.clone();
        // 把后台计算与 UI 状态写入分隔到两个执行上下文。
        std::thread::spawn(move || {
            // 在后台线程建立加载结果。
            let result = 42;
            // 把唯一 State 写入投递回目标窗口 owner thread。
            handle_clone.post_to_ui(move || {
                // 在 UI 线程发布新业务状态。
                data.set(result);
            });
        });
    }
}
