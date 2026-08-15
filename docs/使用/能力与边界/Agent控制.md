# Agent 控制（AI 控制）

[← 返回使用索引](../../使用.md)

> **接口**：声明 UIX Agent 控制的公开入口——接入方式、三层授权模型、能力全集（读取 / 语义动作 / 窗口动作）、协议与安全限制。依赖：[快速开始](../入门/快速开始.md)。导出：Agent 架构 → [架构 · agent](../../架构/app/agent.md)。

## 概述

**任何用 uix-app 框架编写的应用都可以被 AI（Agent）控制**：AI 通过本机端点枚举窗口、读取语义树快照、执行与用户输入等价的受控动作。默认策略下 AI 可以自由控制应用的全部能力——包括所有可交互组件的语义动作（点击、输入、选择、滚动等）与窗口管理动作（缩放、移动、最大化等）。

- 启用条件是 `agent-control` feature + `.enable_agent_control()`，双门禁缺一不可。
- 只建立本机同用户 IPC，空闲无轮询，敏感值不导出。
- 动作与用户输入走同一条 UI 语义路径，不建立第二条可写管线。

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

当前仓库主演示 `uix-lang-demo` 已接入相同的显式双门禁，可用以下命令启动 Windows 真窗验收载体：

```powershell
cargo run --release --manifest-path demo/Cargo.toml --features agent-control --bin uix-lang-demo -- --agent-control
```

只启用 feature 或只传参数都不会发布端点：前者保持普通主演示，后者在创建窗口前以退出码 2 定向失败。

## 授权模型（三层）

Agent 控制的能力按三层授权逐级把关；默认配置下第一层通过后即全放行，第二、三层由应用按需收紧。

### 第一层：连接鉴权（默认启用，不可关闭）

- 端点只接受**本机同用户**连接（Unix socket / 命名管道权限 + 用户身份校验）。
- 每会话生成 32 字节随机 token，随发现文件发布；客户端必须用 token 完成 `hello` 握手，失败即断开。
- 发现信息（进程 id、端点地址、token）与控制通道分离。

### 第二层：动作策略（默认全放行，应用按需收紧）

应用可声明授权边界；命中策略的动作在进入 UI 语义路径前被拒绝（`forbidden`）：

```rust uix-compile=agent-policy
use uix::app::AgentConfirmationRequest;
use uix::ui::SemanticActionKind;
use uix::prelude::*;

App::new()
    .enable_agent_control()
    // 全局只读：AI 只能读快照，不能执行任何动作。
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

策略检查优先级：**只读 > 受保护目标 > 需要确认 > 禁止动作类别**。受保护目标直接拒绝，不进入确认流程。

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
- 未注入确认 UI 时，`confirm` 请求直接失败，不会挂起。
- 用户拒绝时 AI 收到 `confirmation_rejected`。
- 一次确认流程只接受一次 `confirm` 请求（重复请求按失效处理）。

## 能力全集

### 读取

| 请求 | 说明 |
|---|---|
| `list_windows` | 枚举进程内全部窗口（id、代际、标题、可见性、可呈现性、修订号） |
| `snapshot` | 读取目标窗口语义树快照（role / name / state / actions / frame / 选择项，敏感值过滤） |
| `wait` | 等待语义修订前进或已呈现修订达标（上限 30 秒），空闲无轮询 |

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
| `press_key` | `key` / `modifiers` | 按键序列（事件未被消费视为失败） |
| `click_at` / `pointer_move` / `pointer_down` / `pointer_up` | `x` / `y` | 指针输入注入（未命中可交互目标视为失败） |
| `resize_window` | `width` / `height` | 调整窗口客户区尺寸 |
| `move_window` | `x` / `y` | 移动窗口位置 |
| `maximize_window` / `minimize_window` / `restore_window` | — | 窗口状态切换 |

窗口管理动作经平台窗口操作契约执行，失败返回 `window_operation_failed`。

### 错误码

| 错误码 | 含义 |
|---|---|
| `unauthorized` / `unsupported_schema` | 连接鉴权失败 / 协议版本不符 |
| `invalid_request` | 请求格式或字段非法 |
| `window_not_found` / `stale_window` / `stale_revision` | 窗口或代际 / 修订失效 |
| `node_not_found` / `ambiguous_target` | 目标不存在 / automation_id 匹配多个节点 |
| `unsupported_action` / `invalid_value` | 目标不支持该动作 / 值非法 |
| `forbidden` | 命中动作策略（只读 / 受保护 / 禁止类别） |
| `requires_confirmation` | 目标需要用户确认（错误携带 `confirm_id`） |
| `confirmation_rejected` / `confirmation_not_found` | 用户拒绝 / 确认失效或超时 |
| `not_interactable` / `blocked` | 目标不可交互 / 被模态浮层阻挡 |
| `did_not_settle` / `not_presentable` | UI 未在限定轮数内稳定 / 窗口不可呈现 |
| `window_operation_failed` | 窗口管理动作的平台调用失败 |
| `timeout` / `app_closed` / `internal` | 等待超时 / 应用关闭 / 内部错误 |

## 协议

- JSON Lines，本机端点（Unix socket / 命名管道），端点路径含进程 id 与会话 nonce。
- 首请求必须为 `hello`（携带 token）；已认证连接按 `list_windows` → `snapshot` → `perform` / `confirm` → `wait` 循环工作。
- 动作必须命中当前语义快照中的稳定节点（`automation_id` 或 `node_id`）并经过窗口 owner thread。
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

## 安全边界

- 默认关闭：未启用 feature 与应用开关时不发布端点。
- 只接受同用户本机连接；发现信息与控制通道分离。
- 动作必须命中当前语义快照中的稳定节点并经过窗口 owner thread。
- 密码、令牌和标记为敏感的 value 不进入快照、日志或错误回包。
- 组件卸载、窗口关闭或快照代际变化后，旧动作返回可识别失败，不重定向到相似节点。
- 动作策略与确认流程在 UI turn 入口把关，命中策略的动作不进入 UI 语义路径。
- 远程/外部控制不在范围内。

## 理想使用方式

按产品定义（容错可观测、原生优先）：

```rust uix-compile=agent-automation-id
use uix::prelude::*;

// Agent 只读语义树并执行稳定动作，作为自动化测试与 AI 操作通道
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

- Agent 通道用于自动化测试、辅助调试与 AI 操作；业务交互不走 Agent。
- 所有 UI 元素声明 `automation_id` 获得稳定身份，供语义树定位。
- 敏感值（密码、令牌）默认不进快照、日志或错误回包。
- 正式交付的应用应评估动作策略：至少为破坏性操作（删除、清空、提交）声明 `agent_require_confirm` 或 `agent_protect`。

本页只描述 Rust `agent-control` 的本机能力；远程/外部控制不在范围内。
