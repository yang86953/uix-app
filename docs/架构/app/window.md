# 托管与多窗

[← 架构索引](../../架构.md)

> **接口**：声明 app 系统中 **window / window_session / window_driver — 窗口管理**模块的内部设计。所属系统：`app`。依赖：[主循环模块](主循环.md)。导出：多窗口用法 → [使用 · 多窗口](../../使用/多窗口.md)。

## 模块定位

窗口管理（`src/app/window/`、`window_session.rs`、`window_driver.rs`）负责每个窗口的独立生命周期、跨线程安全通信和 IME 会话管理。

## 组件清单

| 组件 | 类型 | 职责 |
|------|------|------|
| `WindowSession` | struct | 窗口会话；持有 WidgetTree、渲染目标、事件队列、动画注册 |
| `WindowDriver` | struct | 窗口驱动器；管理 reconcile → layout → paint 流程 |
| `ComponentHandle` | struct | 跨线程安全的组件操作句柄 |
| `FocusHandle` | struct | 跨线程安全的焦点句柄 |
| `WindowConfig` | struct | 窗口配置：标题、尺寸、最小尺寸、标题栏样式 |
| `WindowActions` | struct | 窗口操作：最小化、最大化、关闭、全屏 |
| `WindowSemantics` | struct | 窗口语义：标题、图标、无障碍信息 |

## 组件：WindowSession

**接口**：每个窗口的独立会话。

| 持有资源 | 说明 |
|----------|------|
| WidgetTree | 该窗口的组件树 |
| RenderTarget | 渲染目标表面 |
| Event Queue | 本窗的事件队列 |
| Animation Registry | 本窗的活跃动画注册 |
| Timer Registry | 本窗的定时器注册 |

**隔离约束**：任一窗口 wake 不得把其他窗口拉成 Active。`TerminalFailure` 窗口不因普通生命周期或 surface 信号重开视觉帧。

## 组件：WindowDriver

**接口**：管理帧内流程。

```
消费截至该点的最后一次 root
  → reconcile
  → 按需 layout
  → paint
  → 至多一次 present
```

- 成功才消费 dirty
- 提交后仍有动画或 dirty 才申请下一次帧
- 开放动画快照复用每窗口峰值容量 scratch
- AppTimer deadline 快照只在 revision 变化时刷新

## 组件：ComponentHandle / FocusHandle

**接口**：跨线程安全句柄。

| 句柄 | 用途 | 约束 |
|------|------|------|
| `ComponentHandle` | 读取组件状态、触发语义动作 | 操作在目标窗 UI 线程排队执行 |
| `FocusHandle` | focus() / blur() | 同 handle 不允许多节点/多窗口绑定 |

- `focus()` / `blur()` 可从后台线程登记，`Ok(())` 表示命令已进入目标队列
- 目标隐藏或不接收事件时，UI 轮次消费命令但不强制聚焦
- `blur()` 只清除该 handle 持有的逻辑焦点，不影响同窗其他节点

## 组件：WindowConfig

**接口**：窗口创建配置。

| 字段 | 说明 |
|------|------|
| `title` | 窗口标题 |
| `width` / `height` | 初始尺寸 |
| `min_width` / `min_height` | 最小尺寸 |
| `title_bar_style` | 系统标题栏 / 自定义标题栏 |
| `resizable` | 是否可调整大小 |
| `maximized` | 是否最大化启动 |
