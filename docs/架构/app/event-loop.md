# event-loop 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 app System 的等待/唤醒、事件路由、活动工作和逐窗帧机会。基础依赖：[platform/windowing](../platform/windowing.md)、[platform/presentation](../platform/presentation.md)和[graphics/renderer](../graphics/renderer.md)的公开契约；与 [window](window.md) 的协作只经 app System 私有队列/调度契约编排。导出：应用运行时内部的主循环与逐窗调度契约。
>
> **当前实现线索**：主循环位于 `src/app/event_loop/`；窗口帧调度位于 `src/app/window/frame_scheduler.rs`；时钟、活动工作、timer、主线程任务与 Agent 命令状态属于 app System 私有边界，位于 `src/app/queues/`。event-loop 消费 window 与 System 私有 queues，不依赖 agent Module。DeepIdle、one-shot 合并、暂停重基和 deadline 都是私有调度细节，不建立项目测试；仅从公开 App 运行契约观察使用方可见结果。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `EventLoop` / `run_widget_loop` | struct/function | 等待平台事件、drain 队列并调度目标窗口 |
| `WindowLoopState` | enum | 表达 DeepIdle、RegisteredActive、Active |
| `ActiveWorkRegistry` | struct | 汇总动画、timer、队列和维护工作的下一 deadline |

## 消费的 app System 私有契约

| 契约 | 所有者 | event-loop 的权限 |
|---|---|---|
| `FrameScheduler` / `FrameOpportunity` | window Module | 只取得带 generation 的逐窗帧机会；不创建或重置调度器 |
| `SurfaceState` / `SurfaceSuspendReason` | window Module | 只读取当前可呈现状态并选择等待策略 |
| `AppTimerQueue` / `TimerHandle` | app System / 目标窗口 | 只在目标窗口轮次按预算 drain 到期任务 |
| `MainThreadQueue` | app System / 目标窗口 | 只在目标窗口轮次按预算 drain 已接纳任务 |

## 组件：EventLoop

一次 Active turn 先非阻塞 drain 平台事件，再执行已到期 timer 和目标窗口队列，随后驱动需要工作的窗口。事件循环只负责公平调度与路由，不实现 reconcile、layout、paint 或 backend 恢复。

`DeepIdle` 表示无事件、队列、due work、reconcile、帧请求或恢复 deadline；`RegisteredActive` 只等待已经登记的 callback/deadline；`Active` 表示存在立即工作。禁止通过固定 16ms tick 维持运行。

## 消费的 window 契约：FrameScheduler

帧调度实现位于 `src/app/window/frame_scheduler.rs`，SMC-06 已将其归入 window Module（旧 event_loop 路径由 compile-fail 契约锁定不得复活）；本模块只消费其逐窗帧机会，不重新定义帧调度。

帧请求是 one-shot、逐窗所有且可合并的，同一窗口同类请求至多一个 outstanding。token 与 surface generation 绑定；resize、suspend、backend fallback 或关闭后到达的旧 callback 必须丢弃。

Hidden、Minimized、ZeroExtent、Occluded 等暂停原因分别登记和解除。恢复首帧以 `dt = 0` 重基；TerminalFailure 不再保留视觉 deadline，也不能用 retained dirty 制造 busy loop。

## 组件：ActiveWorkRegistry

registry 只保存当前 open/due 工作及最近 deadline。动画完成、timer 取消、资源维护完成或 owner 销毁后立即注销。deadline revision 变化时才刷新平台等待条件，避免每轮重新登记。

## 消费的 System 私有队列：MainThreadQueue / AppTimerQueue

队列和 timer 都绑定目标窗口，只能由该窗口工作轮次 drain。取消、drop 或更早 deadline 会 wake 目标窗口重算状态；其他窗口不能看到该队列，也不因一次 wake 被无条件激活。每轮 drain 必须有任务数或时间预算，预算耗尽后保留剩余工作并重新登记 wake，不能以清空单一队列为代价饿死输入、关闭或其他窗口。

## 调度不变量

- 合法 wake 只来自平台事件/生命周期、目标队列、due deadline、帧机会或有界恢复 deadline。
- 输入和关闭事件优先级不能被连续动画、Agent settle 或大量队列工作饿死。
- 不可呈现窗口可以保留 dirty 真相，但不能因此保持 Active。
- closing 后停止接纳新 timer、队列任务和帧请求；已接纳工作按取消契约完成或失败，晚到 wake 不得重新激活已销毁窗口。
- 事件被读取、任务被 drain 或帧机会被消费不等于 UI 已协调或画面已提交；完成事实由 window/renderer 的结果建立。
