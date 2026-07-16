# Agent Bridge (开发预览)

双门禁启用：`agent-control` feature + `.enable_agent_control()`。启用后发布本机端点，供同用户进程通过 JSON Lines 协议枚举窗口、读语义树、执行动作。空闲无轮询，敏感值不导出。

详见[架构](../架构.md#大模型控制协议)；缺口 → [`AGENT-R1`](../进度.md#附能力不计-p6)。
