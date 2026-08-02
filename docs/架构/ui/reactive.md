# reactive 模块

[← 架构索引](../../架构.md)

> **接口**：声明 ui 系统的响应式状态、依赖捕获和受控副作用。依赖：[core/identity](../core/identity.md)、[component](component.md)。导出：`State`、`Computed`、`Effect` 与根/节点依赖契约。
>
> **当前实现线索**：相关实现暂位于 `src/ui/foundation/state.rs`、`src/ui/app_state.rs` 及若干绑定点；重构后应由本模块统一拥有。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `State<T>` | struct | 保存线程安全值并向已捕获依赖发布变化 |
| `Computed<T>` | struct | 缓存派生值并传播底层依赖 |
| `Effect` | struct | 在目标窗口工作轮次中执行受控副作用 |
| `StateSlotId` | value | 标识状态源并防止旧订阅误命中 |
| `AppState` | internal struct | 连接每窗状态变化、根更新命令和 wake |
| reconcile/paint bind site | internal values | 区分结构依赖与节点视觉依赖 |

## 组件：State

根 View 构建期间读取 `State` 会登记 reconcile 依赖；组件绘制绑定只登记所属节点的 Paint 或 Layout 失效。值变化沿已知目标窗口 wake，不为每个状态源创建 timer、线程或轮询。

## 组件：Computed

`Computed` 在输入未变化时复用缓存，但缓存命中仍把底层依赖转交当前捕获上下文。派生计算必须是可重复的纯计算，不隐式执行 I/O 或修改组件树。

## 组件：Effect

`Effect` 在已有目标窗口的 UI 工作轮次内运行，并受取消与生命周期约束。它不能通过固定 tick 维持活跃，也不能越过事件队列直接操作其他窗口。

## 模块不变量

- 状态源不拥有组件或窗口，依赖关系通过带 generation 的目标身份表达。
- 一次工作轮次内相同目标的失效可合并；无观察者、无到期工作时系统可进入 DeepIdle。
- 响应式更新只声明需要完成的工作，实际 reconcile、layout 和 present 由 app 系统调度。
