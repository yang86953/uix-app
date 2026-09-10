# reactive 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 ui System 的响应式状态、依赖捕获和受控副作用。基础依赖：[core/identity](../core/identity.md)；与[组件运行时框架](widget_runtime.md)的协作由 ui System 通过依赖登记契约编排，二者不直接持有 Module 实例。导出：`State`、`Computed`、`Effect` 与根/节点依赖契约。
>
> **当前实现线索**：相关实现暂位于 `src/ui/reactive/state/mod.rs`、`src/ui/widget_runtime/app_state.rs` 及若干绑定点；重构后应由本模块统一拥有。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `State<T>` | struct | 保存响应式值与 revision，并向已捕获依赖发布变化 |
| `Computed<T>` | struct | 缓存派生值并传播底层依赖 |
| `Effect` | struct | 在目标窗口工作轮次中执行受控副作用 |
| `StateSlotId` | value | 标识状态源并防止旧订阅误命中 |
| `AppState` | internal struct | 连接每窗状态变化、根更新命令和 wake |
| reconcile/paint bind site | internal values | 区分结构依赖与节点视觉依赖 |

## 组件：State

根 View 构建期间读取 `State` 会登记 reconcile 依赖；`scoped` 子树作用域闭包内读取的 `State` 登记为该节点的局部 reconcile 依赖（变化时只重跑该闭包并原位协调子树）；组件绘制绑定只登记所属节点的 Paint 或 Layout 失效。每次成功写入单调推进 revision，并沿已知目标窗口登记可合并失效。状态写入发生在目标窗口 UI 轮次；后台线程必须通过 app 的 `post_to_ui` 投递 owned 结果，不能直接写 State、执行 View/Effect/handler，也不为每个状态源创建 timer、线程或轮询。

## 组件：Computed

`Computed` 在输入未变化时复用缓存，但缓存命中仍把底层依赖转交当前捕获上下文。派生计算必须是可重复的纯计算，不隐式执行 I/O 或修改组件树。

## 组件：Effect

捕获到树中的 `Effect` 后续在所属窗口 UI 工作轮次内运行，并受节点/树生命周期约束；离场中的节点仍存活至真实移除。应用另持有的克隆不由树强制销毁。它不能通过固定 tick 维持窗口活跃，也不能越过事件队列操作其他窗口。

独立 `Effect::new` 的首次计算同步发生；依赖变化后只标记 pending，未交接给树时由调用方持有并按实际工作需要 `tick()`，最后一个克隆释放闭包和订阅。该公开能力不自建窗口、线程或轮询器，不代表任意应用后台任务被框架接管；使用边界见[应用仍须持有的责任](../../使用/能力与边界/不必做.md#应用仍须持有的责任)。

协调进入 fail-stop 后不得再 tick 根或节点 Effect；窗口关闭必须释放其拥有的 Effect 及依赖订阅；外持克隆仍由外部 owner 释放。框架不承诺回滚 Effect 在 panic 前已经完成的外部业务副作用，因此这类逻辑必须自行保持幂等或使用业务层事务。

## 模块不变量

- 状态源不拥有组件或窗口，依赖关系通过带 generation 的目标身份表达。
- 一次工作轮次内相同目标的失效可合并；无观察者、无到期工作时系统可进入 DeepIdle。
- 响应式更新只声明需要完成的工作，实际 reconcile、layout 和 present 由 app 系统调度。
- 订阅、捕获上下文和 Effect 归 owner/tree generation；节点移除、协调失败或窗口关闭时必须解除，晚到 wake 不能恢复旧订阅。
- `State::set` 成功只表示新值与 revision 已发布，不表示依赖方已经运行、业务副作用完成或画面已经呈现。
