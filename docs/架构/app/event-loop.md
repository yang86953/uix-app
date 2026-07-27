# event-loop 模块

[← 架构索引](../../架构.md)

> **接口**：声明 app 系统的等待/唤醒、事件路由、活动工作和逐窗帧机会。依赖：[platform/windowing](../platform/windowing.md)、[platform/presentation](../platform/presentation.md)、[window](window.md)、[graphics/renderer](../graphics/renderer.md)。导出：应用运行时内部的主循环与逐窗调度契约。
>
> **当前实现线索**：相关实现暂分布于 `src/app/event_loop/`、`frame_scheduler.rs`、`active_work_registry.rs`、`app_timer.rs` 和 `main_thread_queue.rs`；重构后共同归本模块。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `EventLoop` / `run_widget_loop` | struct/function | 等待平台事件、drain 队列并调度目标窗口 |
| `WindowLoopState` | enum | 表达 DeepIdle、RegisteredActive、Active |
| `FrameScheduler` | struct | 合并逐窗 one-shot 帧请求并管理帧机会 |
| `FrameOpportunity` | value | 携带 generation、单调帧时间和目标提交时间 |
| `SurfaceState` / `SurfaceSuspendReason` | enums | 表达可呈现、暂停、恢复和终止失败 |
| `ActiveWorkRegistry` | struct | 汇总动画、timer、队列和维护工作的下一 deadline |
| `AppTimerQueue` / `TimerHandle` | structs | 管理应用一次性/周期 timer 与取消 |
| `MainThreadQueue` | struct | 把跨线程闭包投递到指定窗口 UI 轮次 |

## 组件：EventLoop

一次 Active turn 先非阻塞 drain 平台事件，再执行已到期 timer 和目标窗口队列，随后驱动需要工作的窗口。事件循环只负责公平调度与路由，不实现 reconcile、layout、paint 或 backend 恢复。

`DeepIdle` 表示无事件、队列、due work、reconcile、帧请求或恢复 deadline；`RegisteredActive` 只等待已经登记的 callback/deadline；`Active` 表示存在立即工作。禁止通过固定 16ms tick 维持运行。

## 组件：FrameScheduler

帧请求是 one-shot、逐窗所有且可合并的，同一窗口同类请求至多一个 outstanding。token 与 surface generation 绑定；resize、suspend、backend fallback 或关闭后到达的旧 callback 必须丢弃。

Hidden、Minimized、ZeroExtent、Occluded 等暂停原因分别登记和解除。恢复首帧以 `dt = 0` 重基；TerminalFailure 不再保留视觉 deadline，也不能用 retained dirty 制造 busy loop。

## 组件：ActiveWorkRegistry

registry 只保存当前 open/due 工作及最近 deadline。动画完成、timer 取消、资源维护完成或 owner 销毁后立即注销。deadline revision 变化时才刷新平台等待条件，避免每轮重新登记。

## 组件：MainThreadQueue / AppTimerQueue

队列和 timer 都绑定目标窗口，只能由该窗口工作轮次 drain。取消、drop 或更早 deadline 会 wake 目标窗口重算状态；其他窗口不能看到该队列，也不因一次 wake 被无条件激活。

## 调度不变量

- 合法 wake 只来自平台事件/生命周期、目标队列、due deadline、帧机会或有界恢复 deadline。
- 输入和关闭事件优先级不能被连续动画、Agent settle 或大量队列工作饿死。
- 不可呈现窗口可以保留 dirty 真相，但不能因此保持 Active。
