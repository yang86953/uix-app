# agent-transport 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 platform System 的可选 `agent-transport` Module，权威持有同用户本机 IPC、peer 校验和 discovery 原子发布。基础依赖：独立 [diagnostics System](diagnostics.md) 的公开契约；[windowing](windowing.md)事件由 platform System 编排，Module 间不直接持有实例。导出：供 app/agent 使用的字节流与生命周期句柄。

> **当前实现线索**：位于 `src/platform/adapters/transport/`；受 `agent-control` feature 控制。

## 组件清单

| 组件 | 目标角色 | 职责 |
|---|---|---|
| `AgentListener` | 资源 | Named Pipe 或 UDS listener |
| `AgentStream` | 字节流 | 有界 read/write、主动连接与取消 |
| `PeerIdentity` | value | 当前 OS 用户身份 |
| `DiscoveryPublisher` | 资源 | endpoint/token 描述的原子安装与清理 |
| `HubEndpointName` | value | 当前用户唯一的固定 Hub UDS / Named Pipe 名称 |
| `TransportHandle` | RAII | listener/worker shutdown |

## 组件：PeerIdentity

Windows 拒绝远程 pipe 并限制当前用户；Unix 校验 peer uid，无法可靠取得凭据时 fail closed。

## 所有权与生命周期

- platform System 创建并拥有 listener、discovery publisher 和 worker；app/agent 只通过 platform 的公开传输契约取得有界字节流与可关闭句柄，不取得 platform 私有 Module 实例。
- endpoint 成功监听且权限设置完成后才能原子发布 discovery；关闭时先停止接收新连接、取消或排空在途 I/O，再删除 discovery 并释放 listener。
- Hub 入口由外部同用户进程监听；platform 只计算平台正确的固定名称并建立主动字节流，不持有 Hub 注册表。应用断开时字节流关闭即撤销租约。
- discovery 中的端点与会话 token 属于敏感凭据。文件权限、原子替换、会话轮换和退出清理必须 fail closed；不得写入普通日志或跨用户位置。
- 同用户本机校验限制连接范围，但不构成业务信任、沙箱或远程身份体系。动作授权仍由 app/agent System 在语义边界执行。

## 模块不变量

不监听 TCP/UDP，不解析 Agent 或 Hub 语义协议，不访问 WidgetTree；字节流有界并支持取消。传输读取按 app/agent 发布的请求帧上限拒绝超长 JSON Lines，响应上限和信封关联仍由协议组件持有。传输接收或写入成功不等于业务请求已授权、执行或完成。
