# event 模块

[← 架构索引](../../架构.md)

> **接口**：声明 ui 系统的输入事件、传播、语义事件与 handler 生命周期。依赖：[component](component.md)、[layout](layout.md)。导出：系统事件与应用语义的统一路由；公开用法见[使用 · 事件](../../使用/事件.md)。
>
> **当前实现线索**：相关实现暂散布于 `src/ui/event.rs`、`system_event_handler.rs`、`semantic_action.rs` 和树事件文件；重构后共同归本模块。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `SystemEvent` / `SystemEventKind` | enum | 键鼠、文本、IME、窗口、计时等统一 UI 事件 |
| `SemanticEvent` / `SemanticKind` / `SemanticPayload` | struct/enum | Click、Change、Submit 等应用语义 |
| `EventResult` | enum | 控制是否继续传播和默认处理 |
| `HandlerTable` / `HandlerRegistration` | struct | 按 `ComponentId` 保存语义 handler |
| `SystemEventHandlerRegistration` | crate 内 struct | 组件 system handler、过滤器和生命周期签名 |
| `SemanticAction` | crate 内 enum | Agent/自动化复用的 focus、invoke、set-value 等动作 |
| `WindowAction` | enum | 由组件请求并交给所属窗口消费的窗口操作 |

## 组件：SystemEvent / SemanticEvent

```text
platform UiEvent
  → app::map_ui_event
  → SystemEvent
  → WidgetTree hit-test / focus target
  → capture → target → bubble
  → 组件默认行为与 State 更新
  → SemanticEvent
  → HandlerTable 中的应用回调
```

窗口 resize、focus、hide/show 等生命周期事件可以不经普通 pointer hit-test，但仍只作用于目标 `WindowSession`。

## 路由规则

- pointer 使用实际 frame、clip、滚动和 visual transform 命中；overlay 先于普通树。
- keyboard/text/IME 路由到当前有效焦点；隐藏、移除或不接收事件的节点不能继续作为目标。
- 捕获使已开始的拖动在 PointerLeave 后仍能收到配对释放；节点销毁前必须取消捕获并结束手势。
- handler 返回结果只控制当前事件传播；不允许直接递归进入 platform event loop。

## 组件：HandlerTable

应用回调按 `ComponentId` 存在 `HandlerTable`，组件只保存稳定 handler signature。reconcile 比较 signature 以复用或替换登记；节点移除、换根或 generation 失效时同步删除，避免闭包进入组件快照或泄漏。

语义事件在目标窗口 UI 线程同步消费；跨线程 `ComponentHandle` 只排队并 wake，`Handled` 表示已进入队列，不表示回调已经执行。

## 默认行为与安全

- Agent 动作和自动化动作进入同一 `SemanticAction`/`SystemEvent` 路径，不直接改 WidgetTree。
- password/sensitive 值不写入语义 payload、日志或 Agent 响应。
- `WindowAction` 只在所属树产生并由本窗 platform window 消费，不进全局广播。
- 无事件时系统不轮询 handler；事件模块本身不登记帧。
