# Agent Bridge (开发预览)

[← 返回使用 索引](../使用.md)

> **接口**：声明 UIX Agent 控制的公开入口——agent-control 启用、端点协议、动作与安全限制。依赖：[快速开始](快速开始.md)。导出：Agent 架构 → [架构 · Agent Bridge](../架构/app/agent.md)。

启用条件是 `agent-control` feature + `.enable_agent_control()`。启用后发布本机端点，供同用户进程通过 JSON Lines 协议枚举窗口、读语义树、执行动作。空闲无轮询，敏感值不导出。
