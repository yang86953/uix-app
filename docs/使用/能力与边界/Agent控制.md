# Agent Bridge (开发预览)

[← 返回使用索引](../../使用.md)

> **接口**：声明 UIX Agent 控制的公开入口——agent-control 启用、端点协议、动作与安全限制。依赖：[快速开始](../入门/快速开始.md)。导出：Agent 架构 → [架构 · Agent Bridge](../../架构/app/agent.md)。

启用条件是 `agent-control` feature + `.enable_agent_control()`。启用后发布本机端点，供同用户进程通过 JSON Lines 协议枚举窗口、读语义树、执行动作。空闲无轮询，敏感值不导出。

## 启用

```toml
[dependencies]
uix = { path = "../uix-app", features = ["agent-control"] }
```

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

## 图形恢复验收载体

主演示另有独立的 `test-harness` 图形恢复页面。需要通过 Agent 自动执行设备丢失、表面
丢失与恢复后交互时，feature 与运行时参数必须分别同时启用：

```powershell
cargo run --release --manifest-path demo/Cargo.toml --features "agent-control,test-harness" --bin uix-lang-demo -- --agent-control --test-graphics-recovery
```

页面保留 `runtime-graphics-recovery-status`、`runtime-inject-device-lost`、
`runtime-inject-surface-lost` 与 `runtime-assert-recovered-interaction` 稳定标识。声明页只
展示状态和接收事件；Rust 组合模块只持有 `on_start` 交付的逐窗 `AppHandle`，并调用公开
test-harness 注入入口。图形会话、恢复状态机和最终呈现仍由 Application System 与 RHI
唯一拥有。

DeviceLost、SurfaceLost、每轮 `presented_revision` 与恢复后交互的真窗注入验收由
`uix-lang-demo` 的 `--test-graphics-recovery` 页面承担，运行方式见
[`uix-lang-demo/README.md`](../../../demo/uix-lang-demo/README.md)；仓库测试已精简为
公开 API 契约测试（`tests/*_public_api.rs`）。

## 端点协议

- JSON Lines 协议，同用户本机端点。
- 枚举窗口 → 读语义树（role/name/value/state/actions）→ 执行动作。
- 动作必须命中当前语义快照中的稳定节点并经过窗口 owner thread。
- 空闲无轮询；敏感值不导出。

## 安全边界

- 默认关闭，未启用 feature 与应用开关时不发布端点。
- 只接受同用户本机连接；发现信息与控制通道分离。
- 动作必须命中当前语义快照中的稳定节点并经过窗口 owner thread。
- 密码、令牌和标记为敏感的 value 不进入快照、日志或错误回包。
- 组件卸载、窗口关闭或快照代际变化后，旧动作返回可识别失败，不重定向到相似节点。

## 理想使用方式

按产品定义（容错可观测、原生优先）：

```rust uix-compile=agent-automation-id
use uix::prelude::*;

// Agent 只读语义树并执行稳定动作，作为自动化测试通道
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

- Agent 通道用于自动化测试与辅助调试；业务交互不走 Agent。
- 所有 UI 元素声明 `automation_id` 获得稳定身份，供语义树定位。
- 敏感值（密码、令牌）默认不进快照、日志或错误回包。

本页只描述 Rust `agent-control` 的本机开发预览能力；远程/外部控制不在范围内。
