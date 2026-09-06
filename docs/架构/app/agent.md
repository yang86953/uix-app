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
| `HubRegistration` | internal worker | 将应用实例、直连描述符和存活心跳登记到当前用户 Hub；失败不阻断应用 |
| `AgentWindowRegistration` | RAII handle | 绑定窗口在控制面中的 generation 生命周期 |
| `AgentWorkspace` / `AgentWorkspaceHandle` | public builder / RAII handle | 显式后台根、资源、单进程租约与独立线程生命周期 |
| `OffscreenWindow` | internal adapter | 内存视口、私有剪贴板、真实 CPU 回读，绝不创建 OS 窗口 |

app System 私有边界另外持有 `AgentCommandRequest` / `AgentCommandResponse`、`AgentCommandQueue`、`WindowAgentState`、`AgentCommandExecutor`、`AgentWindowOps` 与 `AgentSemanticsPort`，并从同一命令契约边界公开重导出确认 UI 载荷 `AgentConfirmationRequest`。这些类型不是 agent Module 的私有实现；`event-loop` / `window` 可以依赖该 System 私有契约，但不得引用 `app::agent`。

## 独立后台所有权

`agent_workspace` 是公开配置与线程生命周期组合入口；`session_runtime` 注入 Agent 私有实现；`window/background` 只消费配置和 System 私有端口，复用正式 `WindowSession` / `WindowDriver` / `Renderer::cpu`。后者不能依赖 `agent` 的私有实现，也不能借用前台 `PlatformSystem`。

App 双门禁启用后必须提供 `agent_root`；缺失时在原生窗口创建前诊断失败。前台 runtime 不启用 Agent 目录或 transport，后台持有独立 AppRuntime、WidgetTree、AppState、字体、图像、焦点/指针/草稿与 CPU 像素。Lucide 字体句柄限制到字体服务所属 UI 线程，不能把一个 FontService 的局部索引发布给另一个线程。

本版每进程一份后台工作面，工作线程持有租约；首个离屏帧完成才发布端点。空闲 park、命令 unpark、定时任务按正式 driver deadline 调度。Drop / close 有界等待，panic 栈展开也先关闭 control plane；阻塞用户回调无法安全强杀，超时返回真实失败且实际线程结束前不归还租约。不能由失败转向前台窗口。

`.agent_root` 是显式所有权契约，不是任意 Rust 闭包沙箱。局部 State 必须独立；只显式共享领域服务，业务并发冲突按应用规则处理。AppHandle 只保留到后台 runtime 的**确认结果**转发，不转发导航、输入或窗口命令。确认回调在后台 owner 投递意图，不自动在用户窗口弹模态框或夺焦点。

## 授权模型（三层）

控制面默认不启动；只有构建启用相应 feature 且应用显式开启后，才建立仅本机 IPC。能力按三层授权把关：

1. **连接鉴权**：首请求必须以高熵随机 token 完成 hello；只接受本机同用户连接，发现信息与控制通道分离。
2. **动作策略**（`AgentPolicy`，应用按需收紧，默认不增加额外动作限制）：只读模式拒绝全部写动作；禁止动作类别全局生效；受保护 automation_id 拒绝语义写动作；其余目标可按策略要求确认。检查发生在命令执行器内（UI turn 入口），命中拒绝即 `forbidden`，不进入 UI 语义路径。优先级：只读 > 禁止动作 > 受保护目标 > 需要确认 > 允许；确认不能覆盖任何拒绝规则。
3. **用户确认**（应用注入确认 UI 后启用）：命中 `require_confirm` 的目标，执行器返回 `requires_confirmation`（携带一次性 `confirm_id`），`WindowAgentState` 登记待确认动作及原始 `expected_revision`；AI 随后发 `confirm` 请求，状态机在 UI turn 内调用注入的确认 UI 并挂起响应；应用经 `AppHandle::resolve_agent_confirmation` 交回用户决定（允许 → 重新校验原始修订后执行登记动作；拒绝 → `confirmation_rejected`）。确认期间语义修订变化时以 `stale_revision` 失败；确认有效期 60 秒，过期惰性清理为 `confirmation_not_found`；窗口关闭或不可呈现时未决确认一并失效。

## 组件：AgentProtocolSession

控制面默认不启动；只有构建启用相应 feature 且应用显式开启后，才建立仅本机 IPC。首请求必须以高熵随机 token 完成 hello；同用户本机连接只是传输筛选，不等于业务信任。请求 JSON 正文与响应 JSON Lines 分别有硬上限，响应上限包含截屏 PNG 的 base64 膨胀和信封空间；`hello` 发布两者，旧 `max_message_bytes` 只作为请求上限别名。文本长度、连接数、队列深度和 wait 时间同样有硬上限。

响应信封由 `AgentProtocolSession` 统一生成并回显原始 `request_id`。业务载荷序列化或长度检查失败时，降级错误仍保留该 ID 且记录真实 `internal` 结果，客户端由此可以停止当前调用，而不会因关联信息丢失误重试有副作用动作。

协议提供窗口枚举、语义快照、受控动作（语义动作与窗口动作）、确认流程和等待，不提供 shell、文件系统、网络代理、任意内存访问或 OS 全局输入注入。鉴权失败、超限、未知动作和 stale generation 返回有界错误，不能 panic 或回显 token。

## 多应用 Hub 与动作直连

每个启用控制面的 UIX 进程仍权威持有自己的 `AgentProcessBridge`、协议会话和随机 token；新增的
`uix.agent.hub.v1` 是外部控制面的注册表，不是状态代理。应用 transport 启动后向当前用户固定 Hub
入口登记 `app_id`、随机 `instance_id`、进程 id、显示名和直连描述符，并以心跳维持租约。Hub
不可用时登记线程有界退避重连，应用的窗口、呈现和原直连端点不依赖 Hub。

```text
UIX App A ── register(instance A, endpoint A) ─┐
UIX App B ── register(instance B, endpoint B) ─┼─> per-user Agent Hub
                                               │       ↑ list / attach
AI connector ──────────────────────────────────┘       │
AI connector ══ control / wait / media ═════════════> selected UIX App
```

Hub 的 `list_apps` 只返回非敏感实例目录；`attach(instance_id)` 才在同用户控制通道交付目标描述符。
连接器取得描述符后直接对该应用完成 `uix.agent.v1` hello，并将自身 `session_id` 固定绑定到该
`instance_id`。应用断线或重启使绑定终止，连接器不得按 `app_id`、标题、pid 或文件时间自动选择
替代实例。多条动作连接只消除连接级队头阻塞，不构成事务；状态事实仍以每次 UIX 响应为准。

Agent 输入只访问后台树，不借用 OS 焦点、指针或剪贴板。`visible=false` 不意味着暂停离屏驱动：`PlatformWindow::is_offscreen` 区分内存视口和隐藏原生窗口，CPU presenter 完成真实绘制与回读，不需要 native surface。截图请求排入 UI turn 后才武装回读，早到的其他帧不能消费其票据；`presented_revision` 对此目录表示离屏完成。桌面前台隐藏、遮挡或缩放变化不影响后台。

`hello` 必须宣告 `background_control.isolated_workspace=true`、私有剪贴板和无桌面访问。官方客户端缺少该能力即失败，不从旧应用共享窗口自动迁移，不改绑实例，也不回退为全局输入或桌面截图。

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

`session_runtime` 负责创建、注入、登记和停止命令队列、策略、执行器、确认与 registration；`agent_workspace` 负责独立 owner 线程生命周期。具体策略仍归 `AgentPolicy` / 确认状态机。IPC worker 不直接改 UI；后台 UI owner 可以像前台 owner 一样操作**自己的**树，绝不修改用户树。关闭先使后台 registration stale 并失败待处理命令，再销毁该树。

命令票据与 UI 队列共享单向生命周期门。协议等待超时时，仍处于待执行阶段的命令原子取消，UI turn 不得再执行；已经开始的命令不能伪装成取消成功，协议返回 `outcome_unknown`，调用方必须读取当前语义状态后再决定是否重试。

`WindowAgentState` 可以在一次 UI turn 中连续完成不产生在途状态的读取、拒绝或确认登记；任一语义动作、窗口动作、截屏或确认放行动作成功进入 `in_flight` 后立即停止本轮队列消费。后续请求保持 FIFO，直到前一动作完成 reconcile、layout、语义刷新与 settle，下一轮再对新快照校验。这样大子树切换后的紧随动作不会命中旧节点；显式携带旧 `expected_revision` 时返回 `stale_revision`，而不是把旧树的 `NotHandled` 暴露成 `internal`。

## 窗口动作

窗口动作中的按键 / 指针经后台 UI 事件路径派发，未消费如实失败。`resize_window` 只改变有界内存视口，`close_window` 只关闭后台操作面；激活、移动、最大化、最小化、还原、原生标题栏 Invoke、系统拖拽均拒绝。执行器不持有平台窗口引用，经 `AgentWindowOps` 借用离屏适配；回调排入原生动作时也须显式失败，不把“扔掉队列”当作成功。

## System 私有契约：AgentCommandRequest / AgentCommandResponse

快照复用 accessibility 模块派生的语义树，携带窗口 generation、`revision` 与 `presented_revision`。节点 ID 只在当前 generation 内有效；跨重建稳定定位使用应用声明的 automation ID。

命令请求含 `Snapshot`、`Perform`、`PerformWindow`、`Screenshot`、`Confirm` 与宿主 `ResolveConfirmation`。普通票据丢弃取消未开始命令；宿主确认属于单向投递，显式 detach 票据只丢响应，不取消决定。批准通过单次 `perform_confirmed` 入口重新验证代际/修订/拒绝策略，只跳过这次已满足的确认要求，不改变全局策略。`wait` 继续等待修订、离屏呈现或关闭；渲染错误不伪装成成功或靠 busy loop 重试。

## 安全不变量

- password 或 sensitive value 不进入快照、响应、discovery 或日志。
- discovery 路径、端点信息和 hello token 都按敏感凭据处理；只授予当前用户最小文件权限，日志和协议错误不得回显 token。
- Hub 枚举不得包含端点或 token；Hub attach、应用登记连接和 connector 内存中的描述符按会话凭据处理。Hub 不得代理、缓存或合成 WidgetTree 状态。
- `automation_id` 是窗口 generation 内外的定位标识，不是授权凭据；重复 ID 必须被应用消歧或拒绝，不能据此绕过策略检查。
- Agent 与用户输入复用 UI 语义实现，但所有可写界面状态属于不同树；仅显式共享业务服务，不发布可见窗口。
- 动作策略与确认流程在 UI turn 入口把关；命中策略的动作不进入 UI 语义路径。
- 确认请求绑定窗口 generation、原始 `expected_revision`、目标、动作和一次性 `confirm_id`；超时、重复提交、窗口关闭、语义修订变化或策略变更后只能失败，不能退回默认允许。
- listener、discovery、worker、窗口 registration、确认挂起和 in-flight 请求都与应用关闭联动并有界释放。
