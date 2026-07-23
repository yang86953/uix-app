# 主循环

[← 架构索引](../../架构.md)

> **接口**：声明 app 系统中 **event_loop / frame_scheduler — 事件循环与帧调度**模块的内部设计。所属系统：`app`。依赖：[系统列表](../系统列表.md)、`platform` 系统（平台事件源）、`ui` 系统（组件树）。导出：无。（应用层最终消费者）

## 模块定位

event_loop 模块（`src/app/event_loop/`）和 frame_scheduler（`src/app/frame_scheduler.rs`）协作管理应用的心跳：有事处理（事件、动画、帧），无事休眠（DeepIdle）。每个窗口独立调度，互不干扰。
