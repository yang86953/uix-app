# agent 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 app System 的可选本机 Agent 控制面、三层授权模型、协议、安全边界和 UI 动作桥。基础依赖：[ui/accessibility](../ui/accessibility.md)、[ui/event](../ui/event.md)和[platform/agent-transport](../platform/agent-transport.md)的公开契约；与 app 兄弟 Module 的协作只经 System 私有命令/语义端口编排。导出：显式启用的 `uix.agent.v1` 控制能力。
>
> **当前实现线索**：Agent Module 位于 `src/app/agent/`（`agent_bridge.rs`、`agent_control.rs`、`agent_policy.rs`、`agent_protocol/`、`agent_transport.rs`），app System 私有命令/状态边界位于 `src/app/queues/agent_command_queue.rs`、`src/app/queues/window_agent_state.rs` 与 `src/app/window_semantics.rs`，OS IPC 传输实现位于 `src/native/agent_transport/`，窗口动作的平台操作适配位于 `src/app/window/window_agent_ops.rs`。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `AgentProcessBridge` | internal struct | 维护进程窗口目录、generation、快照和 wait |
| `AgentProtocolSession` | internal struct | 完成 hello 鉴权、消息解析和串行响应 |
| `AgentCommandExecutorImpl` | internal struct | 实现 System 私有语义/窗口动作执行端口，执行动作策略检查 |
| `AgentPolicy` | internal struct | 动作策略数据：只读 / 受保护目标 / 禁止动作 / 需要确认目标 |
| `AgentTransportHandle` | internal handle | 管理 platform IPC listener 与连接生命周期 |
| `AgentWindowRegistration` | RAII handle | 绑定窗口在控制面中的 generation 生命周期 |

app System 私有边界另外持有 `AgentCommandRequest` / `AgentCommandResponse`、`AgentCommandQueue`、`WindowAgentState`、`AgentCommandExecutor`、`AgentWindowOps` 与 `AgentSemanticsPort`，并从同一命令契约边界公开重导出确认 UI 载荷 `AgentConfirmationRequest`。这些类型不是 agent Module 的私有实现；`event-loop` / `window` 可以依赖该 System 私有契约，但不得引用 `app::agent`。

## 授权模型（三层）

控制面默认不启动；只有构建启用相应 feature 且应用显式开启后，才建立仅本机 IPC。能力按三层授权把关：

1. **连接鉴权**：首请求必须以高熵随机 token 完成 hello；只接受本机同用户连接，发现信息与控制通道分离。
2. **动作策略**（`AgentPolicy`，应用按需收紧，默认不增加额外动作限制）：只读模式拒绝全部写动作；禁止动作类别全局生效；受保护 automation_id 拒绝语义写动作；其余目标可按策略要求确认。检查发生在命令执行器内（UI turn 入口），命中拒绝即 `forbidden`，不进入 UI 语义路径。优先级：只读 > 禁止动作 > 受保护目标 > 需要确认 > 允许；确认不能覆盖任何拒绝规则。
3. **用户确认**（应用注入确认 UI 后启用）：命中 `require_confirm` 的目标，执行器返回 `requires_confirmation`（携带一次性 `confirm_id`），`WindowAgentState` 登记待确认动作及原始 `expected_revision`；AI 随后发 `confirm` 请求，状态机在 UI turn 内调用注入的确认 UI 并挂起响应；应用经 `AppHandle::resolve_agent_confirmation` 交回用户决定（允许 → 重新校验原始修订后执行登记动作；拒绝 → `confirmation_rejected`）。确认期间语义修订变化时以 `stale_revision` 失败；确认有效期 60 秒，过期惰性清理为 `confirmation_not_found`；窗口关闭或不可呈现时未决确认一并失效。

## 组件：AgentProtocolSession

控制面默认不启动；只有构建启用相应 feature 且应用显式开启后，才建立仅本机 IPC。首请求必须以高熵随机 token 完成 hello；同用户本机连接只是传输筛选，不等于业务信任。消息大小、文本长度、连接数、队列深度和 wait 时间均有硬上限。

协议提供窗口枚举、语义快照、受控动作（语义动作与窗口动作）、确认流程和等待，不提供 shell、文件系统、网络代理、任意内存访问或 OS 全局输入注入。鉴权失败、超限、未知动作和 stale generation 返回有界错误，不能 panic 或回显 token。

## 组件：AgentProcessBridge / System 私有命令边界

IPC worker 只解析、鉴权和排队：

```text
platform IPC
  → AgentProtocolSession
  → AgentProcessBridge 定位 WindowId + generation
  → app System 私有 AgentCommandQueue 有界 FIFO
  → wake 目标 WindowSession
  → WindowAgentState 在 UI turn 调用已注入的 AgentCommandExecutor
  → 策略检查 / 确认流程 → UI 线程转为 SemanticAction / SystemEvent / 窗口操作
  → 正常 reconcile / layout / paint / present
```

`session_runtime` 是这条 app System 流程的唯一组合根：它只负责创建、注入、登记、启动和停止命令队列、动作策略、执行器、确认 UI 回调与窗口 registration；具体允许/拒绝决定仍由 `AgentPolicy` 和确认状态机持有，组合根不得复制业务判断。后台线程不得直接修改 State、WidgetTree、焦点或 platform window。窗口关闭先使 registration stale 并失败待处理命令，再销毁组件树。

命令票据与 UI 队列共享单向生命周期门。协议等待超时时，仍处于待执行阶段的命令原子取消，UI turn 不得再执行；已经开始的命令不能伪装成取消成功，协议返回 `outcome_unknown`，调用方必须读取当前语义状态后再决定是否重试。

## 窗口动作

窗口级动作分两类：输入注入（按键 / 点击 / 指针）经正常 UI 事件路径派发，未消费视为失败；窗口管理动作（resize / move / maximize / minimize / restore）经 `AgentWindowOps` 契约调用平台窗口——契约由 window 系统实现（`PlatformWindowAgentOps` 适配 `PlatformWindow::properties_mut`），由窗口驱动层在每帧 UI turn 内以借用传入，执行器不持有平台窗口引用。窗口操作失败映射为 `window_operation_failed`。

## System 私有契约：AgentCommandRequest / AgentCommandResponse

快照复用 accessibility 模块派生的语义树，携带窗口 generation、`revision` 与 `presented_revision`。节点 ID 只在当前 generation 内有效；跨重建稳定定位使用应用声明的 automation ID。

命令请求包含 `Snapshot`、`Perform`（语义动作）、`PerformWindow`（窗口动作）、`Confirm`（确认流程）与 `ResolveConfirmation`（应用交回用户决定）。`wait` 可以等待语义 revision 前进或已呈现 revision 达标。窗口隐藏、最小化、遮挡或终止失败时，要求已呈现状态的请求返回 not-presentable 语义，不能通过 busy loop 强制呈现。

## 安全不变量

- password 或 sensitive value 不进入快照、响应、discovery 或日志。
- discovery 路径、端点信息和 hello token 都按敏感凭据处理；只授予当前用户最小文件权限，日志和协议错误不得回显 token。
- `automation_id` 是窗口 generation 内外的定位标识，不是授权凭据；重复 ID 必须被应用消歧或拒绝，不能据此绕过策略检查。
- Agent 动作进入与用户输入相同的目标窗口事件/语义路径，不建立第二条可写 UI 管线。
- 动作策略与确认流程在 UI turn 入口把关；命中策略的动作不进入 UI 语义路径。
- 确认请求绑定窗口 generation、原始 `expected_revision`、目标、动作和一次性 `confirm_id`；超时、重复提交、窗口关闭、语义修订变化或策略变更后只能失败，不能退回默认允许。
- listener、discovery、worker、窗口 registration、确认挂起和 in-flight 请求都与应用关闭联动并有界释放。
