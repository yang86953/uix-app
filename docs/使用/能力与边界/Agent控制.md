# Agent 控制（AI 控制）

[← 返回使用索引](../../使用.md)

> **接口**：声明 UIX Agent 控制的公开入口——接入方式、三层授权模型、能力全集（读取 / 语义动作 / 窗口动作）、协议与安全限制。依赖：[快速开始](../入门/快速开始.md)。导出：Agent 架构 → [架构 · agent](../../架构/app/agent.md)。

## 概述

只有同时启用 `agent-control` feature 和 `.enable_agent_control()` 的应用才发布 Agent 端点。已授权的同用户本机客户端可以枚举窗口、读取脱敏语义快照，并执行与用户输入进入同一语义路径的受控动作。启用端点后，默认不增加动作限制；这等价于已鉴权客户端可以执行目标声明支持的全部动作。应用必须在交付前显式判断这一高权限默认值是否可接受，并按风险增加拒绝或确认规则。

- 启用条件是 `agent-control` feature + `.enable_agent_control()`，双门禁缺一不可。
- 只建立本机同用户 IPC，空闲无轮询，敏感值不导出。
- 动作与用户输入走同一条 UI 语义路径，不建立第二条可写管线。

## 使用前安全评估

- “同用户本机”限制连接来源，但不等于调用方天然可信。同一用户会话中的其他进程一旦取得发现信息与 token，就可能以该用户权限操作已放行目标。
- 只在确有自动化、辅助调试或 AI 操作需求的环境启用端点。正式交付前应评估“默认无额外动作限制”是否可接受，并为删除、清空、提交、付款或权限变更等高影响操作设置保护或确认。
- 发现文件中的端点地址和随机 token 属于敏感会话信息，不应复制到日志、错误回包、文档截图或跨用户共享位置。
- 只为需要稳定定位的交互目标设置 `automation_id`，并保证同一可见语义树内唯一。不要把密码、令牌、个人数据或其他秘密编码进标识符、可访问名称和普通 value。
- 用户确认 UI 必须展示足够的目标与动作语义，拒绝、超时、窗口关闭和重复确认都按失败处理；不得在确认失败后自动降级为放行。

## 接入（两步）

**第一步**，Cargo 依赖启用 feature：

```toml
[dependencies]
uix = { path = "../uix-app", features = ["agent-control"] }
```

**第二步**，应用入口显式启用：

```rust uix-compile=agent-enable
use uix::prelude::*;

App::new()
    .enable_agent_control()   // 显式启用本机 uix.agent.v1 端点
    .root(main_view)
    .run();
```

当前仓库主演示 `uix-lang-demo` 已接入相同的显式双门禁，可用以下命令启动 Windows 或
Wayland Linux 真窗验收载体：

```powershell
cargo run --release --manifest-path demo/Cargo.toml --features agent-control --bin uix-lang-demo -- --agent-control
```

只启用 feature 或只传参数都不会发布端点：前者保持普通主演示，后者在创建窗口前以退出码 2 定向失败。

## 授权模型（三层）

Agent 控制的能力按三层授权逐级把关；默认配置下第一层通过后不会再增加动作限制，第二、三层由应用按需收紧。

### 第一层：连接鉴权（默认启用，不可关闭）

- 端点只接受**本机同用户**连接（Unix socket / 命名管道权限 + 用户身份校验）。
- 每会话生成 32 字节随机 token，随发现文件发布；客户端必须用 token 完成 `hello` 握手，失败即断开。
- 发现信息（进程 id、端点地址、token）与控制通道分离。

### 第二层：动作策略（默认无额外限制，应用按需收紧）

应用可声明授权边界；命中策略的动作在进入 UI 语义路径前被拒绝（`forbidden`）：

```rust uix-compile=agent-policy
use uix::ui::SemanticActionKind;
use uix::prelude::*;

App::new()
    .enable_agent_control()
    // 全局只读：AI 只能读快照与截屏，不能执行任何动作。
    .agent_read_only()
    // 保护指定组件：AI 不能对其执行写动作（如"删除"按钮）。
    .agent_protect("delete-button")
    // 全局禁止某类动作（如禁止 AI 修改任何输入值）。
    .agent_deny_action(SemanticActionKind::SetValue)
    // 指定组件需要用户确认（进入第三层确认流程）。
    .agent_require_confirm("danger-button")
    .root(main_view)
    .run();
```

策略检查优先级：**只读 > 禁止动作类别 > 受保护目标 > 需要确认 > 允许**。前三类拒绝规则直接返回 `forbidden`，不进入确认流程；确认只能收紧其余允许动作，不能覆盖拒绝规则。

### 第三层：用户确认（默认关闭，应用注入确认 UI 后启用）

对 `agent_require_confirm` 标记的目标，AI 执行动作时先收到 `requires_confirmation` 错误（携带一次性 `confirm_id`），随后 AI 发起 `confirm` 请求；框架在 UI turn 内调用应用注入的确认 UI，**用户决定后才执行**。

```rust uix-compile=agent-confirm-ui
use uix::app::AgentConfirmationRequest;
use uix::prelude::*;

App::new()
    .enable_agent_control()
    .agent_require_confirm("danger-button")
    // 注入确认 UI：收到请求时展示确认界面（示意；应用可用任意 UI 实现）。
    .agent_confirm_ui(|request: AgentConfirmationRequest| {
        // request: { window_id, confirm_id, target, action }
        // 展示确认界面；用户决定后调用：
        // handle.resolve_agent_confirmation(
        //     request.window_id, request.confirm_id, allow /* true 允许 / false 拒绝 */);
    })
    .root(main_view)
    .run();
```

- 确认请求有效期 60 秒；超时或确认已失效时以 `confirmation_not_found` 失败。
- 原动作携带 `expected_revision` 时，确认后仍针对该修订重新校验；确认期间界面语义变化会以 `stale_revision` 失败，AI 应重新读取快照后再决策。
- 未注入确认 UI 时，`confirm` 请求直接失败，不会挂起。
- 用户拒绝时 AI 收到 `confirmation_rejected`。
- 一次确认流程只接受一次 `confirm` 请求（重复请求按失效处理）。

## 能力全集

### 读取

| 请求 | 说明 |
|---|---|
| `list_windows` | 枚举进程内全部窗口（id、代际、标题、可见性、可呈现性、logical 客户区尺寸、最大化/最小化/全屏状态、焦点 `focused`、修订号） |
| `snapshot` | 读取目标窗口语义树快照（role / name / state / actions / frame / 选择项、节点 `focused` 与 `hovered`，敏感值过滤） |
| `screenshot` | 截取目标窗口下一次真实呈现帧：PNG（RGB8、物理像素、左上原点）以 base64 返回；要求 backend-managed GPU 呈现，载荷上限见 `hello.limits.max_screenshot_bytes` |
| `wait` | 等待语义修订前进、已呈现修订达标或同代际窗口关闭（上限 30 秒），空闲无轮询 |

### 语义动作（作用于语义树节点，须命中快照中的稳定目标）

| 动作 | 载荷 | 适用角色 |
|---|---|---|
| `invoke` | — | 按钮、可点击目标 |
| `focus` | — | 可聚焦目标 |
| `set_value` | `value` | 文本输入 |
| `insert_text` | `text` | 接受文本输入的目标 |
| `select` | `value` | 单选目标（下拉、单选组等） |
| `toggle` | — | 复选框、开关 |
| `increment` / `decrement` | — | 滑块、步进器 |
| `adjust` | `min` / `max` | 连续值目标（能力声明） |
| `scroll` | `delta_x` / `delta_y` | 可滚动视口 |

### 窗口动作（作用于窗口，不携带 target）

| 动作 | 载荷 | 说明 |
|---|---|---|
| `press_key` | `key` / `modifiers` | 应用内按键序列（事件未被消费视为失败） |
| `click_at` / `pointer_move` / `pointer_down` / `pointer_up` | `x` / `y` | 应用内指针事件（未命中可交互目标视为失败） |
| `resize_window` | `width` / `height` | 调整 logical 客户区尺寸；仍受窗口最小/最大约束 |
| `move_window` | `x` / `y` | 移动平台窗口位置；Wayland 等禁止任意定位的平台会返回 `window_operation_failed` |
| `maximize_window` / `minimize_window` / `restore_window` | — | 窗口状态切换 |
| `activate_window` | — | 请求合成器激活并聚焦窗口（Wayland 复用 xdg-activation）；成功只表示请求已提交，不保证已获焦点，自动化用 `list_windows` 的 `focused` 复核 |
| `close_window` | — | 提交平台关闭请求；成功不声明窗口已关闭，调用方必须另行等待关闭事实 |

窗口管理动作会由 `hello.capabilities.window_actions` 正式发布，并经平台窗口操作契约执行；平台不支持、
窗口约束拒绝或原生调用失败均返回 `window_operation_failed`，不得伪造成功或隐式降级。

指针动作（`click_at` / `pointer_move` / `pointer_down` / `pointer_up`）不要求窗口焦点；`press_key` 与文本类动作要求窗口已聚焦且事件被组件消费，未消费按失败处理。自动化驱动键盘前先执行 `activate_window`；焦点与悬停用 `focused` / `hovered` 事实断言，不依赖截图猜测界面状态。

`screenshot` 属于读取类请求：只读策略下与快照一样可用，且不携带 `target`。请求会使空闲窗口强制出帧并等待下一次真实 present 完成，返回 `window_id`、`width`、`height`、`format` 与 base64 PNG 载荷。像素为物理分辨率（含设备缩放），载荷上限 32 MiB（`hello.limits.max_screenshot_bytes`，超限返回 `payload_too_large`）；仅 backend-managed GPU 呈现支持截屏，软件回退路径以 `unsupported_action` 明确失败；同一窗口上一个截屏未完成前，新请求按 `window_operation_failed` 拒绝。

### 错误码

| 错误码 | 含义 |
|---|---|
| `unauthorized` / `unsupported_schema` | 连接鉴权失败 / 协议版本不符 |
| `invalid_request` | 请求格式或字段非法 |
| `window_not_found` / `stale_window` / `stale_revision` | 窗口或代际 / 修订失效 |
| `outcome_unknown` | 命令已经开始但等待响应超时；动作可能已生效，必须先读取当前状态，禁止盲目重试 |
| `node_not_found` / `ambiguous_target` | 目标不存在 / automation_id 匹配多个节点 |
| `unsupported_action` / `invalid_value` | 目标不支持该动作 / 值非法 |
| `forbidden` | 命中动作策略（只读 / 受保护 / 禁止类别） |
| `requires_confirmation` | 目标需要用户确认（错误携带 `confirm_id`） |
| `confirmation_rejected` / `confirmation_not_found` | 用户拒绝 / 确认失效或超时 |
| `not_interactable` / `blocked` | 目标不可交互 / 被模态浮层阻挡 |
| `did_not_settle` / `not_presentable` | UI 未在限定轮数内稳定 / 窗口不可呈现 |
| `window_operation_failed` | 窗口管理动作的平台调用失败 |
| `payload_too_large` | 截屏 PNG 编码结果超出协议载荷上限 |
| `timeout` / `app_closed` / `internal` | 等待超时 / 应用关闭 / 内部错误 |

协议等待超时时会原子取消尚未进入 UI turn 的命令，此时返回 `timeout`；若动作已经开始，则返回 `outcome_unknown`，明确表示结果未知。两者的重试语义不同。

## 协议

- JSON Lines，本机端点（Unix socket / 命名管道），端点路径含进程 id 与会话 nonce。
- 首请求必须为 `hello`（携带 token）；已认证连接按 `list_windows` → `snapshot` → `perform` / `confirm` → `wait` 循环工作。
- 请求帧与响应帧分别有界：`hello.limits.max_request_bytes` 是请求 JSON 正文上限，`max_response_bytes` 是含结尾换行的单条响应上限；旧字段 `max_message_bytes` 保留为请求上限别名。响应上限已计入 32 MiB 截屏 PNG 的 base64 膨胀和 JSON 信封，不会把合法截屏误报为 `internal`。
- 每个响应必须原样回显请求的 `request_id`；客户端应同时校验 `schema`、`request_id` 与协商后的响应长度，关联不一致时停止该连接，不能把回包归给其他动作。
- `hello.capabilities.window_state_fields` 发布 `list_windows` 可读取的窗口状态字段；客户端必须先协商再消费。字段来自 UIX 跨平台窗口属性，是框架当前观测，不承诺窗口管理器或 compositor 已确认动作终态。
- 动作必须命中当前语义快照中的稳定节点（`automation_id` 或 `node_id`）并经过窗口 owner thread。
- `wait` 返回同 generation 的 `closed` 后，该连接已到达终态并由服务端关闭；应用 teardown 会先给
  在途终态回复保留有界写回窗口，再强制回收其他连接。
- 确认流程时序：

```txt
AI                        UIX 应用                      用户
 │  perform(danger-button) │                            │
 │ ──────────────────────> │ requires_confirmation      │
 │ <────────────────────── │  + confirm_id              │
 │  confirm{confirm_id}    │                            │
 │ ──────────────────────> │ ── 确认 UI（应用注入）───> │
 │                         │ <── 允许 / 拒绝 ─────────── │
 │ <── performed / rejected │                            │
```

- 空闲无轮询；敏感值不导出（password 与标记敏感的值不进快照、日志或错误回包）。

## 本机自动化操作回路（仓库内置客户端）

**前置条件**：目标应用已按[接入](#接入两步)双门禁启用端点，且客户端与应用属同一操作系统用户。本仓库主演示用以下命令启动：

```bash
cargo run --release --manifest-path demo/Cargo.toml --features agent-control --bin uix-lang-demo -- --agent-control
```

**执行步骤**：仓库提供 `scripts/agent_client.py`（Python 3；Windows 命名管道依赖 pywin32）。脚本从发现目录读取最新 `uix-*.json`（Linux 为 `$XDG_RUNTIME_DIR/uix-agent-$UID/`），每条命令独立完成 `hello` 握手后执行：

```bash
python3 scripts/agent_client.py hello           # 握手并发布能力目录
python3 scripts/agent_client.py list_windows    # window_id、generation、focused 等
python3 scripts/agent_client.py snapshot        # 默认第一个窗口的语义树
python3 scripts/agent_client.py screenshot out.png   # 像素级截屏（默认第一个窗口）
python3 scripts/agent_client.py click 120 40    # 应用内 logical 客户区坐标
```

AI 连续操作时不要为每一步重启客户端；使用 `session` 保持同一条已认证连接。启动后 stdout 首行是一次 `hello` 回包，随后 stdin 每输入一行请求 JSON，stdout 就输出一行对应响应 JSON；请求缺省的 `schema` 与 `request_id` 仍由客户端补齐：

```bash
python3 scripts/agent_client.py session
{"type":"list_windows"}
{"type":"snapshot","window_id":1}
```

单次命令仍会在内部完成握手，但只输出目标命令的有效回包，不再重复打印能力目录。持久模式避免每步重新启动 Python、发现端点、建连和握手，也减少 AI 反复读取相同能力清单的输出开销。

语义动作与窗口动作写入 JSON 后用 `perform` 提交；语义动作的 `target` 在请求级（`automation_id` 或 `node_id` 二选一），窗口动作禁止携带 `target`：

```txt
{"type": "perform", "window_id": 1, "generation": 0,
 "target": {"automation_id": "save-button"}, "action": {"kind": "invoke"}}
```

**预期结果**：单次模式每条命令只输出目标命令的一行 JSON 响应；持久模式额外在首行输出一次 `hello`，此后每个请求对应一行响应。内置客户端会按 `hello` 协商请求/响应/截屏上限，并校验回包的 schema 与 `request_id`。`perform` 成功表示动作已进入 UI 语义路径执行。标准回路是 `list_windows` → `snapshot` → `perform` / `confirm` → `wait`，等待界面稳定后再读下一次快照断言。

**可见失败**：未找到 discovery 文件说明端点未启用（应用未按双门禁启动）；`unauthorized` / `unsupported_schema` 表示 token 或协议版本不符；动作失败语义见[错误码](#错误码)。确认与等待等多次往返优先使用 `session`，业务错误仍作为正常协议回包输出，不会自动重试动作。

**适用限制**：只支持本机同用户；多个应用实例并存时脚本以最新启动的 discovery 为准，操作指定实例需自行选择对应发现文件；截屏仅 backend-managed GPU 呈现可用。`session` 是前台 JSON Lines 进程，stdin 关闭、输入非法 JSON、传输/关联/schema 校验失败或应用退出时结束；需要继续操作时重新建立会话，不自动重连或重放动作。

## 安全边界

- 默认关闭：未启用 feature 与应用开关时不发布端点。
- 只接受同用户本机连接；发现信息与控制通道分离。
- 动作必须命中当前语义快照中的稳定节点并经过窗口 owner thread。
- 密码、令牌和标记为敏感的 value 不进入快照、日志或错误回包。
- 截屏按呈现帧原样交付像素：屏幕上可见的一切内容（包括明文展示的敏感值）都会进入图片，掩码显示的字段只以掩码形态出现。应用应在敏感界面考虑遮挡或限制，AI 操作方不得把截屏当作读取敏感值的手段。
- 组件卸载、窗口关闭或快照代际变化后，旧动作返回可识别失败，不重定向到相似节点。
- 动作策略与确认流程在 UI turn 入口把关，命中策略的动作不进入 UI 语义路径。
- 远程/外部控制不在范围内。
- 同用户本机连接仍是高权限控制面；应用必须把发现文件与 token 视为会话凭据，并审查“默认无额外动作限制”是否适合交付环境。
- Agent 控制不提供沙箱、跨用户授权、远程身份体系或业务级审计存储；应用需要这些能力时必须在产品边界外另行设计，不能扩展本机端点冒充远程控制。

## 推荐用法

按产品定义（容错可观测、原生优先）：

```rust uix-compile=agent-automation-id
use uix::prelude::*;

// Agent 只读语义树并执行稳定动作，作为应用自动化与 AI 操作通道
let app = App::new()
    .enable_agent_control()
    .root(|| column((
        button("保存").automation_id("save-button"),
        label("状态").automation_id("status-label"),
    )));

// 同用户 agent 客户端：
//   list windows → read snapshot → invoke "save-button"
//   组件卸载/窗口关闭后，旧动作返回可识别失败
```

- Agent 通道用于使用方应用自动化、辅助调试与 AI 操作；它不扩大 UIX 项目的公开 API 测试范围，业务交互不走 Agent。
- 只为需要被自动化稳定定位的目标声明唯一 `automation_id`；普通展示节点不必全部设置。
- 敏感值（密码、令牌）默认不进快照、日志或错误回包。
- 正式交付的应用应评估动作策略：至少为破坏性操作（删除、清空、提交）声明 `agent_require_confirm` 或 `agent_protect`。

本页只描述 Rust `agent-control` 的本机能力；远程/外部控制不在范围内。
