# agent 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 app 系统的可选本机 Agent 控制面、协议、安全边界和 UI 动作桥。依赖：app System 私有的命令/语义端口、[ui/accessibility](../ui/accessibility.md)、[ui/event](../ui/event.md)、[platform/agent-transport](../platform/agent-transport.md)。导出：显式启用的 `uix.agent.v1` 控制能力。
>
> **当前实现线索**：Agent Module 位于 `src/app/agent/`，app System 私有命令/状态边界位于 `src/app/queues/agent_command_queue.rs`、`src/app/queues/window_agent_state.rs` 与 `src/app/window_semantics.rs`，OS IPC 传输实现位于 `src/native/agent_transport/`。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `AgentProcessBridge` | internal struct | 维护进程窗口目录、generation、快照和 wait |
| `AgentProtocolSession` | internal struct | 完成 hello 鉴权、消息解析和串行响应 |
| `AgentCommandExecutorImpl` | internal struct | 实现 System 私有语义/窗口动作执行端口 |
| `AgentTransportHandle` | internal handle | 管理 platform IPC listener 与连接生命周期 |
| `AgentWindowRegistration` | RAII handle | 绑定窗口在控制面中的 generation 生命周期 |

app System 私有边界另外持有 `AgentCommandRequest` / `AgentCommandResponse`、`AgentCommandQueue`、`WindowAgentState`、`AgentCommandExecutor` 与 `AgentSemanticsPort`。这些类型不是 agent Module 的私有实现；`event-loop` / `window` 可以依赖该 System 私有契约，但不得引用 `app::agent`。

## 组件：AgentProtocolSession

控制面默认不启动；只有构建启用相应 feature 且应用显式开启后，才建立仅本机 IPC。首请求必须以高熵随机 token 完成 hello；消息大小、文本长度、连接数、队列深度和 wait 时间均有硬上限。

协议只提供窗口枚举、语义快照、受控动作和等待，不提供 shell、文件系统、网络代理、任意内存访问或 OS 全局输入注入。鉴权失败、超限、未知动作和 stale generation 返回有界错误，不能 panic 或回显 token。

## 组件：AgentProcessBridge / System 私有命令边界

IPC worker 只解析、鉴权和排队：

```text
platform IPC
  → AgentProtocolSession
  → AgentProcessBridge 定位 WindowId + generation
  → app System 私有 AgentCommandQueue 有界 FIFO
  → wake 目标 WindowSession
  → WindowAgentState 在 UI turn 调用已注入的 AgentCommandExecutor
  → UI 线程转为 SemanticAction / SystemEvent
  → 正常 reconcile / layout / paint / present
```

`session_runtime` 是唯一组合根：它创建命令队列，把 `agent` 提供的 `AgentCommandExecutorImpl` 和 `AgentWindowRegistration` 分别注入 System 私有状态与语义端口。后台线程不得直接修改 State、WidgetTree、焦点或 platform window。窗口关闭先使 registration stale 并失败待处理命令，再销毁组件树。

## System 私有契约：AgentCommandRequest / AgentCommandResponse

快照复用 accessibility 模块派生的语义树，携带窗口 generation、`revision` 与 `presented_revision`。节点 ID 只在当前 generation 内有效；跨重建稳定定位使用应用声明的 automation ID。

`wait` 可以等待语义 revision 前进或已呈现 revision 达标。窗口隐藏、最小化、遮挡或终止失败时，要求已呈现状态的请求返回 not-presentable 语义，不能通过 busy loop 强制呈现。

## 安全不变量

- password 或 sensitive value 不进入快照、响应、discovery 或日志。
- Agent 动作进入与用户输入相同的目标窗口事件/语义路径，不建立第二条可写 UI 管线。
- listener、discovery、worker、窗口 registration 和 in-flight 请求都与应用关闭联动并有界释放。
