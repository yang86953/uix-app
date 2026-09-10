# identity 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 core 基础契约中的目标 `identity` 边界及其组件契约。core 的 System 定级仍由[系统列表](../系统列表.md#core基础契约system-定级待定)持有。依赖：无。导出：跨 ui、graphics、platform、app 传递的稳定身份值。

> **当前实现线索**：`src/core/identity/widget_id.rs`、`src/core/identity/window_id.rs`；路径可重构，identity 边界保持稳定。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `WidgetId` | struct | 以 `tree_scope + slot + generation` 标识组件节点 |
| `WindowId` | struct | 以进程内 `u64` 标识应用窗口 |

## 组件：WidgetId

`slot` 支持槽位复用，`generation` 使旧句柄在槽位复用后失效，`tree_scope` 隔离不同 WidgetTree。它只表达身份，不持有节点或延长生命周期。

## 组件：WindowId

`WindowId` 是窗口路由键；窗口 session generation 另行校验迟到回调。`ROOT`/`root()` 表示根窗口约定，不等同于空身份。

## 模块不变量

- 身份值可复制、排序和哈希；解析身份必须由拥有存储的上层完成。
- 后台句柄可保存身份，但不得据此直接访问 WidgetTree 或原生窗口。
- 身份不延长目标生命周期，也不证明当前调用方仍获授权；每次使用都需由 owner 校验 generation、作用域和当前状态。
