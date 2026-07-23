# Agent Bridge

[← 返回架构 索引](../架构.md)

> 分类：Agent 控制  
> 权威范围：本机 Agent 协议、安全、语义动作和资源边界

## 附：大模型控制协议

<a id="大模型控制协议"></a>

产品附能力（开发预览），不改变主路径应用写法。协议名 `uix.agent.v1`。用法 → [`使用 · Agent Bridge`](../使用/Agent控制.md#agent-bridge-开发预览)；缺口 → [`平台与硬件`](../进度/平台与硬件.md#agent-附能力)。

### 边界与组成

- 显式启用（`agent-control` feature + `.enable_agent_control()`）、仅限本机同一 OS 用户的进程级端点。不是 TCP、跨机器远控、UIA/读屏桥；不提供 shell、文件或网络访问。
- 三层：语义树、动作执行器、传输。`TestApp` 与真窗 Bridge 共用前两层；传输差异不得进入组件。
- 内置组件自动提供语义与动作；自定义复用 `AccessibilityRole`/`AccessibilityState`。v1 不负责截图或 GPU readback。

### 发现与安全

- Discovery：Windows `%LOCALAPPDATA%\uix-agent`；Linux/macOS `$XDG_RUNTIME_DIR/uix-agent-<uid>`。描述文件含 pid、endpoint、随机 token；端点成功后原子公布，退出删除。
- 传输：Windows Named Pipe（受保护 DACL 只授权当前进程 `TokenUser` SID）；Linux/macOS UDS（目录 `0700`，accept 后核 peer effective uid），其他 Unix 无可靠 peer credential API 时 fail-closed。不监听 TCP/UDP。Wire：UTF-8 JSON Lines，单条 ≤ 4 MiB。
- 首条必须带 token 的 `hello`；连接内串行，并发用多连接（≤ 8）。同窗动作容量 64 FIFO。

### 语义与动作

- 快照含 `window_id`、generation、`revision`、`presented_revision`、节点树。节点含 opaque `node_id`、可选 `automation_id`、role、state、bounds、actions。
- `node_id` 仅同 generation 有效；跨启动定位用 `automation_id`。password/sensitive value 永不出现在快照、响应或日志。
- 动作经目标 `WindowSession` 正常 semantic/`SystemEvent` 路径；禁 IPC 线程直接改 Widget/State/绘制树。窗口动作 `press_key`、`click_at`、`pointer_move`、`pointer_down`、`pointer_up` 只合成既有 `SystemEvent`，不调用 OS 输入注入；拆分的指针按下/释放只表达左键，客户端负责成对结束手势。
- 动作后 State → Effect → reconcile → 最多 32 settle 轮次。视觉证据用 `wait` 等到 `presented_revision >= revision`；不可呈现 → typed `not_presentable`。
- Demo 隔离库存的逐组件视觉数据覆盖公开导出和组件声明；记录包含非零语义目标、浅色桌面、深色紧凑及适用状态。单体 focus 状态来自语义 `focus`；公开构建器以其可见子树内真实可聚焦节点为目标。open/scroll/change/focus-trap 等交互数据包含动作、settle、真实 present 与独立截图，并由 CSV 和联系表汇总。

### 错误与资源

- typed error 覆盖 unauthorized/window_not_found/stale_window/node_not_found/unsupported_action/timeout 等。
- 上限：消息 4 MiB、文本 64 KiB、连接 8、每窗队列 64、settle 32、wait 30 s。
- 除 transport worker 外无轮询线程；无客户端时主循环 DeepIdle。审计只记结果码与耗时；token/输入文本/password 不记录。
