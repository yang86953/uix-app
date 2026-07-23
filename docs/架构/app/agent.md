# Agent Bridge

[← 架构索引](../../架构.md)

> **接口**：声明 app 系统中 **agent — Agent 系统**模块的内部设计。所属系统：`app`。依赖：[窗口管理模块](window.md)、[无障碍模块](../ui/accessibility.md)（语义树）。导出：Agent 控制用法 → [使用 · Agent控制](../../使用/Agent控制.md)。

## 模块定位

Agent 系统（`src/app/agent_*.rs`）实现 `uix.agent.v1` 协议，允许大模型通过本机进程间协议枚举窗口、读取语义树和执行语义动作。Optionally gated by `agent-control` feature。
