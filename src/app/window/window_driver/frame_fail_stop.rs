//! 窗口帧驱动的 fail-stop 调度资源收敛组件。

// 复用父模块拥有的窗口驱动、队列与树类型。
use super::*;

// 将永久失败后的资源释放与正常帧管线分离。
impl WindowDriver {
    // 检查 WidgetTree 是否已在某个可恢复边界后进入永久停止状态。
    pub(super) fn finish_if_tree_fail_stopped(
        &mut self,
        // 借用当前窗口唯一拥有的 WidgetTree 读取 fail-stop 状态。
        tree: &WidgetTree,
        // 接收需要清空的逐窗活动工作注册表。
        active_work: &mut ActiveWorkRegistry,
        // 接收需要取消的应用级定时器队列。
        app_timers: &AppTimerQueue,
        // 接收尚未执行的主线程任务队列。
        main_thread_queue: &MainThreadQueue,
        // 接收需要关闭的自动化命令状态。
        agent_commands: &mut WindowAgentState,
        // 接收尚未进入协调事务的声明根候选。
        pending_root: &mut Option<crate::ui::view::ViewNode>,
        // 接收窗口会话保存的协调请求位。
        reconcile_pending: &mut bool,
        // 接收需要收敛为深度空闲的窗口循环状态。
        loop_state: &mut WindowLoopState,
    ) -> Option<WindowFrameResult> {
        // 正常树继续执行当前帧，保持既有帧管线语义。
        if !tree.is_fail_stopped() {
            // 未进入 fail-stop 时不构造提前返回结果。
            return None;
        }
        // 进入 fail-stop 后由统一的所有者路径释放调度资源。
        Some(self.finish_fail_stopped_frame(
            // 清空当前窗口的活动工作。
            active_work,
            // 取消当前窗口的应用定时器。
            app_timers,
            // 丢弃当前窗口的主线程队列。
            main_thread_queue,
            // 关闭当前窗口的 Agent 状态。
            agent_commands,
            // 释放当前窗口的待协调根。
            pending_root,
            // 清除当前窗口的协调请求位。
            reconcile_pending,
            // 收敛当前窗口循环状态。
            loop_state,
        ))
    }

    // 收敛 fail-stop 窗口的全部调度资源并返回稳定帧结果。
    pub(super) fn finish_fail_stopped_frame(
        &mut self,
        // 接收需要清空的逐窗活动工作注册表。
        active_work: &mut ActiveWorkRegistry,
        // 接收需要取消的应用级定时器队列。
        app_timers: &AppTimerQueue,
        // 接收尚未执行的主线程任务队列。
        main_thread_queue: &MainThreadQueue,
        // 接收需要关闭的自动化命令状态。
        agent_commands: &mut WindowAgentState,
        // 接收尚未进入协调事务的声明根候选。
        pending_root: &mut Option<crate::ui::view::ViewNode>,
        // 接收窗口会话保存的协调请求位。
        reconcile_pending: &mut bool,
        // 接收需要收敛为深度空闲的窗口循环状态。
        loop_state: &mut WindowLoopState,
    ) -> WindowFrameResult {
        // 清空动画、定时器和图形维护等已登记的后续工作。
        active_work.clear();
        // 以下释放序列与 queues::release_window_scheduling_resources（会话关闭 /
        // shutdown 路径）收敛同一组调度队列；此处 Agent 端经 WindowAgentState::close
        // 额外向在途请求回执 AppClosed，并附带帧调度器终态，因此不能直接复用。
        // 取消应用级定时器，避免 teardown 前再次唤醒窗口循环。
        app_timers.cancel_all();
        // 丢弃尚未进入树协调的主线程任务。
        main_thread_queue.clear();
        // 关闭自动化命令端口并向在途请求返回窗口已关闭。
        agent_commands.close();
        // 释放尚未协调的根快照，令其捕获资源按原子边界回滚。
        drop(pending_root.take());
        // 清除协调请求，禁止下一帧重入半提交树。
        *reconcile_pending = false;
        // 丢弃已取出的到期工作 scratch，避免其在后续路径被复用。
        self.due_work_scratch.clear();
        // 终止帧调度器，阻止再申请原生或回退帧。
        self.frame_scheduler.mark_terminal_failure();
        // 将窗口循环置为深度空闲，等待唯一所有者 teardown。
        *loop_state = WindowLoopState::DeepIdle;
        // 故障停止不执行 runtime、回调、协调、布局或渲染。
        WindowFrameResult { did_work: false }
    }
}
