# window 模块

[← 返回架构索引](../../架构.md)

> **接口**：声明 app System 的窗口创建、逐窗会话、单帧驱动、文本输入和关闭协议。基础依赖：[ui/widget_runtime](../ui/widget_runtime.md)、[ui/view](../ui/view.md)、[platform/windowing](../platform/windowing.md)、[platform/presentation](../platform/presentation.md)和[graphics/renderer](../graphics/renderer.md)的公开契约；app 兄弟 Module 只经 System 私有队列/语义/调度契约协作。导出：`Window`、`WindowConfig`、逐窗 drive/result 契约和内部会话。
>
> **当前实现线索**：相关实现暂分布于 `src/app/window/`、`window_session.rs`、`window_driver.rs`、`text_input.rs`、`bridge/` 等位置；重构后统一服从本模块的逐窗所有权。

## 组件清单

| 组件 | 类型 | 职责 |
|---|---|---|
| `AppHandle` / `WindowConfig` | public structs | 描述逐窗存活快照、owner-thread 激活请求、根 View 工厂、标题和初始几何 |
| `WindowSession` | internal struct | 独占组件树、渲染目标、队列、调度状态、IME 和语义状态 |
| `SessionRuntime` | internal struct | 管理窗口身份预留、创建、查找和销毁 |
| `WindowDriver` | internal struct | 按确定顺序推进单个窗口的一轮 UI/图形管线 |
| `FrameScheduler` | internal struct | 合并逐窗 one-shot 帧请求并管理帧机会（`src/app/window/frame_scheduler.rs`，SMC-06 归本模块） |
| `WindowTextInputState` | internal struct | 协调窗口焦点、目标节点与 composition |
| `WindowAction` | enum | 表达拖动、最小化、最大化、关闭等一次性请求 |
| scene paint bridge | internal adapter | 把 UI 绘制遍历适配为 graphics scene 输入 |

## 组件：Window / WindowConfig

配置在创建原生资源前完成验证。未支持的标题栏、透明度或输入能力返回 typed error，不能静默退回不同语义。公开 `Window` 只暴露稳定窗口操作，不泄漏 platform backend、surface 或可变组件树。

`AppHandle::is_open` 只提供该次读取时的会话存活快照，不建立跨线程租约；
`AppHandle::request_activate` 经逐窗 main-thread queue 投递到 owner thread，接受成功不承诺窗口管理器
最终授予焦点。关闭竞态返回 `InvalidState`，不得把请求重定向到标题相同或后来创建的窗口。

## 组件：WindowSession

每个窗口独占 `WidgetTree`、RenderTarget、timer、动画、frame request、surface 状态、IME、System 私有 `WindowAgentState` 和语义 revision。一窗关闭、暂停或失败不销毁、唤醒或污染其他窗口资源。

创建时先预留带 generation 的 `WindowId`，再依次建立 platform window、graphics target 和根组件树；任一步失败按逆序回收。迟到 callback 或旧 handle 只能被识别为 stale 并丢弃。

## 组件：WindowDriver

```text
due work / queue / Agent / Effect
  → consume latest root input
  → reconcile（至多一次）
  → 按需 layout
  → paint / scene
  → present（至多一次）
  → 汇总下一 WindowLoopState
```

只有成功提交后才能消费对应 present dirty；提交失败保留真实 damage 并交由 graphics/app 的 typed 恢复协议分类。event-loop 不复制这条逐窗 pipeline。

若 UI 协调已在发布段发生 panic，`WindowDriver` 把该树视为终止实例：停止消费 timer、Agent、Effect、动画和待协调根，取消帧机会并进入 DeepIdle，不再调用 runtime task、layout、scene paint 或 present。逻辑 panic 仍按原 payload 展开；平台 ABI 的 panic 防越界只负责保护调用约定，不得把同一半提交窗口恢复为可运行状态。

原生最大化与还原分别形成 `WindowMaximize` / `WindowRestore` 状态事实，并由同一次平台生命周期同时提供当前 logical 客户区的 `WindowResize`。只有 `WindowResize` 拥有 `RenderTarget`、platform presenter、surface generation 与根 frame 的几何事务；状态事实只恢复或保持调度并请求完整重绘，不得从显示器边界或初始窗口配置合成尺寸。该约束使 Windows 的“状态后 resize”和 Wayland 的“resize 后状态”得到相同结果，也避免重复 surface 重建。

## 组件：WindowTextInputState

同一共享原生输入能力在任一时刻只有一个有效 owner，由 WindowId、native view identity 和 generation 共同约束。焦点切换、节点隐藏/移除和窗口关闭都先停止旧 IME 会话，再让目标身份失效；未 commit composition 不写入受控状态。

## 关闭不变量

- 关闭顺序是停止新工作与 Agent 命令 → 结束 pointer/focus/IME → 清空窗口队列与 side table → shutdown graphics → 销毁原生窗口。
- fail-stop 树关闭时不再执行不可信的半初始化组件生命周期；所有框架持有资源仍须幂等释放，随后只能创建新的 `WidgetTree` owner。
- `WindowAction` 只由所属树产生并由本窗消费，不进入全局广播。
- app 中的 scene bridge 只转换调用形状，不拥有第二份组件树、场景或 Renderer。
