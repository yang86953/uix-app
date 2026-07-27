# agent-transport 模块

[← 架构索引](../../架构.md)

> **接口**：声明 platform 系统的可选 `agent-transport` 模块，权威持有同用户本机 IPC、peer 校验和 discovery 原子发布。依赖：[windowing](windowing.md)、[diagnostics](diagnostics.md)。导出：供 app/agent 使用的字节流与生命周期句柄。

> **当前实现线索**：位于 `src/native/agent_transport/`；受 `agent-control` feature 控制。

## 组件清单

| 组件 | 目标角色 | 职责 |
|---|---|---|
| `AgentListener` | 资源 | Named Pipe 或 UDS listener |
| `AgentStream` | 字节流 | 有界 read/write 与取消 |
| `PeerIdentity` | value | 当前 OS 用户身份 |
| `DiscoveryPublisher` | 资源 | endpoint/token 描述的原子安装与清理 |
| `TransportHandle` | RAII | listener/worker shutdown |

## 组件：PeerIdentity

Windows 拒绝远程 pipe 并限制当前用户；Unix 校验 peer uid，无法可靠取得凭据时 fail closed。

## 模块不变量

不监听 TCP/UDP，不解析 Agent 语义协议，不访问 WidgetTree；端点成功后才发布 discovery，退出时清理。
