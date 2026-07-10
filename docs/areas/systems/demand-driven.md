# 按需零闲置

← [架构导航](../architecture.md) · 系统 **#13** · 功能域：跨域

> **UIX 核心理念的操作细则**（[#105](../../decisions.md#d105)）。
> **最高规则**：**用最少资源，做最好效果** — 统领六域依赖与一切子系统；冲突时以本规则为准。  
> 硬约束摘要 → [`AGENTS.md`](../../../AGENTS.md)「核心理念」。

## 索引

| 主题 | 章节 | 决策 |
|------|------|------|
| 北极星 | [设计美学](#设计美学最高规则-105) | #105 |
| 开发者契约 | [开发者契约（零维护）](#开发者契约零维护) | #130 #131 #132 |
| 异步边界 | [UI 主循环 vs 后台](#ui-主循环-vs-后台) · [post_to_ui](#post_to_ui) · [MainThreadQueue](#mainthreadqueue) | #131 #132 #133 #137 #88 |
| App 定时 | [App Timer API](#app-timer-api) | #132 |
| 三层目标 | [三层目标](#三层目标) | #105 |
| 唤醒 | [唤醒源白名单](#唤醒源白名单) | #105 #111 |
| 主循环 | [主循环状态机](#主循环状态机) | #106 #110 #111 #116 #117 |
| 注册表 | [ActiveWorkRegistry](#activeworkregistry) | #115 |
| 多窗编排 | [多窗单 loop](#多窗单-loop) | #116 |
| 帧内合并 | [帧内合并](#帧内合并) | #118 |
| 失效 | [失效与窄标脏](#失效与窄标脏) | #107 #122 #129 |
| 输入 | [PointerMove 窄路径](#pointermove-窄路径) | #109 #121 |
| Effect / Theme | [Effect 与 Theme](#effect-与-theme) | #79 #125 #128 |
| 分域要求 | [分域要求](#分域要求) | #105–#159 |
| 豁免 | [豁免机制](#豁免机制) | #113 |
| 审查 | [新 API 审查清单](#新-api-审查清单) | #105 #120 #130 |
| 已实现 | [implementation · 实现进度总览](../implementation.md#实现进度总览) | — |
| 剩余差距 | [剩余差距](#剩余差距) | — |

**关联**：[application](application.md) · [rendering](rendering.md) · [event](event.md) · [view-reactive](view-reactive.md) · [component](component.md) · [data](data.md) · [theme-style](theme-style.md)

---

<a id="设计美学最高规则-105"></a>

## 设计美学（最高规则 #105）

> **用最少资源，做最好效果。**

| 信条 | 含义 |
|------|------|
| **有触发才工作** | 无 OS 事件、无失效、无 register → DeepIdle |
| **无 pending 零开销** | 不无条件 layout / render / present / tick Effect / 定时 wake |
| **变化尽量窄** | L0 + L1 + L2 同时成立 |
| **效果不妥协** | active 期须完整正确（拖拽、hover、IME、动画） |
| **全框架统一** | 非渲染特例；`app` / `ui` / `draw` / `native` / `data` 均须遵守 |

与六域依赖、API 设计冲突时 **以本节为准**；不得以兼容层或「每帧保底」规避。

---

<a id="开发者契约零维护"></a>

## 开发者契约（零维护）

**#130**：在满足 #105 的前提下，调用方 **不必** 维护 Picture 名单、ActiveWorkRegistry、标脏范围或 Theme 广播。框架在内部自动推断与托管。

### 调用方只需

| 做什么 | 说明 |
|--------|------|
| **View + State** | 声明 UI；`State::set` 驱动更新 |
| **Effect** | 业务副作用；**禁止**轮询式 Effect（#131） |
| **Timer API** | `run_after` / `run_interval`（#132）；框架内 register，主线程回调 |
| **opt-in API** | `.follow_system_theme(true)`（#125）；`.theme(...)`；`EventHandler::wants_continuous_pointer_move`（#121） |

### 调用方禁止

| 禁止 | 替代 |
|------|------|
| 直接访问 ActiveWorkRegistry | 用 **#132 Timer API** 或内置组件 |
| 固定 interval 主循环 `wait_timeout` 探活 | 框架 `wait_until(remaining)`（#127） |
| 轮询式 Effect | Timer API、async→State，或内置周期 UI |
| 维护 Picture 名单 / 手动推断 | 框架 **PicturePolicy 自动推断**（#122） |
| 手动全树 invalidate | `State` 绑定或 `ComponentHandle::invalidate`（窄 Paint，#119） |
| 后台 poll OS 主题 | opt-in `.follow_system_theme(true)` 或显式 `.theme()`（#125） |

### 框架自动负责

PicturePolicy · ActiveWorkRegistry · 窄标脏 · Composite scroll · PointerMove 窄路径 · 帧内 reconcile · 多窗 Theme 广播 · Handler 智能重绑（#123）

---

<a id="ui-主循环-vs-后台"></a>

## UI 主循环 vs 后台

**#131 修订**：禁止 **裸 Registry** 与 **轮询 Effect**；**允许** 框架提供的 **App Timer API**（#132）。见 [App Timer API](#app-timer-api)。

```text
┌─────────────────────────────────────────────────────────────┐
│  UI 主循环（须 #105 零闲置）                                  │
│  DeepIdle ← UiEvent / State / RegisteredActive（含 #132）    │
│  ✓ App::run_after / run_interval（框架内 register）           │
│  ✗ 裸 ActiveWorkRegistry · 固定 interval 主循环探活          │
│  ✗ 轮询式 Effect                                              │
└─────────────────────────────────────────────────────────────┘
         ▲
         │ 主线程：Timer 回调 · State::set
         │
┌────────┴────────────────────────────────────────────────────┐
│  后台 / 异步（允许）                                          │
│  std::thread · async · 平台 API                               │
│  ✓ 耗时 IO → 完成 post 主线程 State::set                      │
│  ✗ 直接改 WidgetTree / invalidate                             │
└─────────────────────────────────────────────────────────────┘
```

### 允许（App 定时 #132）

| API | 行为 | 零闲置 |
|-----|------|--------|
| `App::run_after(d, f)` | 一次；到期 **主线程** 调 `f`；自动 unregister | 无 Timer 时 DeepIdle |
| `App::run_interval(d, f) -> TimerHandle` | 重复；每次 `f` 后 register 下一 deadline | `cancel()` / drop → unregister |
| `TimerHandle::cancel()` | 立即停止；unregister | — |

规则：

- 回调 **仅主线程**（#88）；可直接 `State::set`，**不必**再 spawn。
- 框架内部 `ActiveWorkKind::AppTimer(id)` register（#124）；**App 不碰 Registry**。
- 回调 **默认不** layout/render；仅 `State::set` / 窄 invalidate 时按需 wake。
- 到期走 `drain_due` → 执行 `f`；**不**无条件全树 layout。

### 允许（后台）

| 场景 | 做法 | 回到 UI |
|------|------|---------|
| 定时自动保存 | 后台线程 / async sleep | 完成 → **主线程** `State::set` |
| 轮询 API / 任务状态 | async / 线程 + channel | 结果到达 → `State::set` |
| 文件监视、网络事件 | OS / 第三方库回调 | post 到主线程 → `State::set` |

规则：

- 后台 **不得** 直接操作 WidgetTree、HandlerTable 或 `invalidate`（#79、#88）。
- 跨线程更新 UI **必须** 经 **`post_to_ui`**（#133）或等价主线程队列，再 `State::set`（#88）；`State` 变更会唤醒 Active 并走帧内 reconcile（#118）。
- 后台运行 **不** 保证唤醒主循环；**无 `State` 变更 → 主循环可仍 DeepIdle**（符合 #105）。

### 禁止

| 禁止 | 原因 | 替代 |
|------|------|------|
| 直接 `ActiveWorkRegistry::register` | 绕过托管 | #132 Timer API |
| 固定 interval 主循环 wake | 破坏 DeepIdle | `wait_until(remaining)` |
| 轮询式 Effect | 变相每帧 wake | Timer API 或 async→State |
| PointerMove 热路径频繁 `State::set` | 比定时器更糟 | 窄路径 |
| `std::thread::sleep` 后直改 UI | 非主线程 | `post_to_ui` → State |

---

## post_to_ui

设计（#133）— 规格见 [application · post_to_ui](application.md#post_to_ui)。

async / 后台线程完成工作后，**唯一**合法 UI 更新路径：投递闭包到主线程队列，在闭包内 `State::set` / 窄 invalidate。

```rust
impl App {
    /// 将 `f` 排入主线程队列；下一 Active 帧或 drain 时执行（与 Timer 回调同线程）
    fn post_to_ui<F>(&self, f: F)
    where F: FnOnce() + Send + 'static;
}

impl AppHandle {
    fn post_to_ui<F>(&self, f: F)
    where F: FnOnce() + Send + 'static;
}
```

| 规则 | 说明 |
|------|------|
| 线程 | `f` **仅主线程**执行（#88）；`Send` 约束闭包可跨线程 move |
| 与 Timer | 与 #132 回调同队列语义；**不** register ActiveWork |
| DeepIdle | 有效入队会 wake 事件循环以便 drain；**不** register ActiveWork，闭包无 `State::set` / invalidate 时 drain 后可回 DeepIdle |
| 禁止 | 闭包内直接改 WidgetTree / HandlerTable / 裸 invalidate |
| 多窗 | `AppHandle.window_id` → 该 session 队列（#141）；**禁止**跨窗 handle |

与 #132 分工：**Timer** = 框架 register + deadline wake；**post_to_ui** = 外部完成信号 → 主线程一次性闭包。队列调度见 [MainThreadQueue](#mainthreadqueue)（#137）。

> **实现注记**：`App::post_to_ui` / `AppHandle::post_to_ui` 已导出，按 `window_id` 路由到目标 session 的 `MainThreadQueue`；session 销毁后丢弃闭包。详见 [application · post_to_ui](application.md#post_to_ui)。

---

## MainThreadQueue

设计（#137）— `post_to_ui` / `AppTimer` 回调与 UiEvent 在同一 **WindowSession** 主线程帧内调度。

```rust
// app/main_thread_queue.rs（设计）
struct MainThreadQueue {
    pending: VecDeque<Box<dyn FnOnce() + Send>>,
}

impl MainThreadQueue {
    fn enqueue<F: FnOnce() + Send + 'static>(&mut self, f: F);
    fn drain(&mut self);  // 依次执行并清空
    fn is_empty(&self) -> bool;
}
```

### Active 帧顺序（修订 #118）

Active 帧 **`run_active_frame` 完整顺序与合并规则** → [帧内合并](#帧内合并)（#118）。

| 规则 | 说明 |
|------|------|
| FIFO | 同 session 内 `post_to_ui` **先进先出** |
| 优先级 | **UiEvent > drain_due > post_to_ui**；同一步骤内保持 FIFO |
| wake | 入队设 `main_thread_pending`；**不** register ActiveWork（#105） |
| DeepIdle | 队列空且 drain 完毕 → 可回 DeepIdle |
| 跨 session | 各 WindowSession **独立**队列；不跨窗投递（#141） |
| 与 Timer | `AppTimer` 回调走 `drain_due`（步骤 2）；**不**与 post_to_ui 混队 |

`post_to_ui` 闭包内 `State::set` 并入步骤 6 的 reconcile 批次（#118）；禁止在步骤 3 直接改 WidgetTree。

> **实现注记**：`MainThreadQueue` 已落地，单窗 event loop 按 UiEvent → due work → post_to_ui 顺序 drain。详见 [application · MainThreadQueue](application.md#mainthreadqueue)。

### 周期可见 UI

Spinner、Tooltip 延迟、光标闪烁等 → **优先内置 widget**（框架 register）；也可用 #132 驱动 **业务逻辑**（如定时刷新数据 → `State::set`）。

### 示例

```rust
// ✓ 框架 Timer API（主线程回调）
let handle = app.run_interval(Duration::from_secs(30), move || {
    save_draft();           // 或 counter.set(counter.get() + 1)
});

// ✓ 一次性
app.run_after(Duration::from_millis(500), || show_toast.set(true));

// ✓ 后台 + State（async 完成后 post 到主线程再 set）
Effect::new(move || {
    spawn_fetch(move |data| {
        app.post_to_ui(move || state.set(data));
    });
});

// ✗ 禁止：裸 Registry、轮询 Effect
```

---

## App Timer API

设计（#132）— 规格见 [application · App 定时 API](application.md#app-定时-api)。

App **可用** 高层定时 API；框架在内部 register，**不**暴露 `ActiveWorkRegistry`。

```rust
impl App {
    /// 一次：到期主线程执行 f，随后 unregister
    fn run_after<F>(&self, delay: Duration, f: F) -> TimerHandle
    where F: FnOnce() + Send + 'static;

    /// 重复：每 interval 主线程执行 f；cancel/drop 停止
    fn run_interval<F>(&self, interval: Duration, f: F) -> TimerHandle
    where F: FnMut() + Send + 'static;
}

struct TimerHandle { /* opaque */ }
impl TimerHandle {
    fn cancel(self);  // 立即 unregister
}
// drop 等价 cancel
```

| 规则 | 说明 |
|------|------|
| 线程 | 回调 **仅主线程**（#88） |
| register | `ActiveWorkKind::AppTimer(id)` 内部写入 Registry（#124） |
| 零闲置 | 无 pending Timer → DeepIdle；成功注册会 wake event loop 并进入 `wait_until(min deadline)`（#127） |
| 副作用 | 回调内 `State::set` → 按需 reconcile；**不**默认 layout/render |
| 与内置 UI | 可见周期动画仍 **优先** 内置 widget；Timer API 用于 **业务逻辑**（保存、刷新、倒计时数据） |

> **实现注记**：`App::run_after` / `run_interval` / `TimerHandle` 已导出，按 `window_id` 路由到所属 AppTimer 队列。详见 [application · App 定时 API](application.md#app-定时-api)。

---

## 三层目标

| 层级 | 含义 | 验收 |
|------|------|------|
| **L0 零像素** | 无 Paint/Composite 失效 → 不 `render_frame`、不 `present` | `has_render_work()` 门控 |
| **L1 零帧循环** | 无 pending → **DeepIdle**，blocking 等事件 | 无定时 wake 跑 layout/render/tick Effect |
| **L2 最小脏区** | 有变化 → 最小 rect / Composite memmove / Picture 自适应 / PointerMove 窄路径 | 滚动、PicturePolicy、PointerMove |

---

## 唤醒源白名单

[#173](../../decisions.md#d173) 将“唤醒”拆成两层：

1. **进程 loop wake**：让唯一 event loop 从 blocking wait 返回，只表示“重新检查工作”。
2. **窗口 session 激活**：只有命中 `window_id` / `ComponentId` tree scope 或确有 pending 的 session 才能离开 DeepIdle。

`EventLoopWaker::wake()` **不等于**将所有窗口置为 Active。仅以下来源可唤醒进程 loop：

| 唤醒源 | session 范围 | session 状态 | 允许的工作 |
|--------|--------------|--------------|------------|
| OS `UiEvent` | 事件 `window_id` 目标窗 | Active | dispatch → 按需 layout/render |
| `InvalidationQueue` push / `State::set` | 仅有订阅或失效的 session | Active | reconcile / layout / render 均须再受 pending 门控 |
| `MainThreadQueue` 入队（`post_to_ui` / `update_view`） | `AppHandle.window_id` 目标窗 | Active | 按 FIFO drain；闭包无副作用时不得制造 frame work |
| AppState semantic queue 入队 | `ComponentId` 所属 tree scope | Active | 仅目标树 dispatch semantic event |
| **RegisteredActive** 注册 / deadline 到期 | 拥有该 Registry 的 session | 注册后 RegisteredActive；到期后可能 Active | Animation / 内置 UI Timer / **App Timer**（#132）/ IME；窄 tick |
| 窗口创建、关闭、resize / 可见性请求 | 目标窗；创建请求由进程 loop 处理 | 按请求决定 | 生命周期编排；必要时 layout + full-frame |

进程 loop 被唤醒后，必须先检查每个 session 的定向队列、失效、semantic target 和 Registry；无命中的 session 保持 DeepIdle，不执行 `tick_effects` / reconcile / layout / render / present。

**不在白名单内（禁止作为框架默认行为）**：

- 固定 interval `wait_timeout` 探活并 layout/render
- DeepIdle 下 `tick_effects`
- 框架后台 poll OS 主题（[#125](../../decisions.md#d125) 仅 opt-in 时响应 ThemeChanged，不 poll）
- 无 register 的未托管周期工作 — 须 **内置组件**、**#132 Timer API** 或 async→State（#131）

---

## 主循环状态机

设计（#106、#111）— 编排见 [application · 主循环](application.md#主循环)。**无 LightIdle**；周期性工作一律走 RegisteredActive。

```text
                    ┌─────────────────────────────────────┐
                    │            DeepIdle                  │
                    │  blocking wait_event（无 timeout）   │
                    │  不 tick Effect / layout / render    │
                    └──────────────┬──────────────────────┘
           UiEvent / Invalidation │        register (Timer/Animation/IME…)
                                   ▼                        ▼
                    ┌──────────────────────┐   ┌──────────────────────┐
                    │       Active          │   │  RegisteredActive     │
                    │  dispatch·layout·     │   │  窄 tick + 关联 paint │
                    │  render 按需          │   │  unregister → DeepIdle│
                    └──────────┬───────────┘   └──────────┬───────────┘
                               │                            │
                               └──────── 队列空且无人注册 ────┘
                                              → DeepIdle
```

| 状态 | 进入 | 允许 | 禁止 |
|------|------|------|------|
| **DeepIdle** | 无 Invalidation；RegisteredActive 为空 | blocking `wait_event`；更新 OS 级 cursor（O(1)） | `tick_effects`、layout、render、定时 wake |
| **RegisteredActive** | Registry 非空；注册项可有 deadline，也可是 IME 这类无 deadline 会话 | 到期项窄 tick；仅关联节点 paint；无 deadline 时 blocking `wait_event` | 全树 layout；无关注册；固定间隔探活 |
| **Active** | UiEvent 或 Invalidation pending | 完整按需管线 | 无条件全树 layout/render |

| 规则 | 决策 |
|------|------|
| unregister 后 | 下一机会 **立即**回 DeepIdle（#111） |
| `window_visible = false` | 该窗不 layout/render |
| 多窗 | **每窗独立状态**；A 窗的工作可唤醒进程 loop，但不得将 B 窗置 Active，也不执行 B 的 layout/render/present（#110 #173） |
| 进程级 sleep | 所有窗 DeepIdle 且全局 Registry 空 → blocking `wait_event`（#117） |
| 有未到期 deadline | app 层 **`wait_until(deadline)`** = 单次 `wait_timeout(deadline - now)`（#127）；**非**固定 interval 探活 |
| 有 register 但无 deadline | 状态仍为 RegisteredActive，但等待使用 blocking `wait_event`；不伪造 timeout |

<a id="activeworkregistry"></a>

### ActiveWorkRegistry（#115、#124）

RegisteredActive 的**框架内部注册表** — **App 不可访问**。Animation / Tooltip Timer / IME 等由组件生命周期与 dispatch 层 **自动** register/unregister；调用方零维护。

```rust
// 框架内部（app / ui）；不暴露给 App
enum ActiveWorkKind {
    Animation(NodeId),
    Timer(TimerId),       // 内置 UI（Tooltip 等）
    AppTimer(TimerId),    // App::run_after / run_interval（#132）
    ImeSession(NodeId),
}

struct ActiveWorkRegistry { /* min-heap by next_deadline */ }

impl ActiveWorkRegistry {
    fn register(&mut self, kind: ActiveWorkKind, next_deadline: Instant);  // 框架内部
    fn unregister(&mut self, kind: ActiveWorkKind);
    fn next_deadline(&self) -> Option<Instant>;
    fn drain_due(&mut self, now: Instant) -> Vec<ActiveWorkKind>;
}
```

| 自动 register 来源 | 触发 | unregister |
|-------------------|------|------------|
| `Animatable` 动画开始/续帧 | 组件 / Registry tick | 动画结束 |
| Tooltip 等内置周期 UI | 组件 mount / show | hide / unmount |
| **App Timer**（#132） | `run_after` / `run_interval` | 触发一次 / `cancel` / drop |
| IME 焦点 Input | `text_input.start` | 失焦 / `stop` |

| 规则 | 说明 |
|------|------|
| drain_due | 到期 → 执行回调或窄 tick + 关联 Invalidation（#126） |
| 续期 | interval Timer 在 tick 内 **自动** register 下一 deadline |
| App | **仅**通过 #132 API；**禁止**直接 `register` |

### wait_until（#117、#127）

```text
deadline = min(all sessions' next_deadline)
if deadline <= now → drain_due(所属 session) + 继续帧
else if deadline exists → wait_timeout(deadline - now)  // 单次，非固定探活
else → wait_event()                                  // 可为 DeepIdle，也可为无 deadline RegisteredActive
```

不要求 native 新增 API；`wait_until` 是 app 层语义，用现有 `wait_timeout` + 计算 remaining 实现。`RegisteredActive` 描述“Registry 非空”，不代表一定使用 timeout。

每 **WindowSession** 持有一份 Registry（#116）。

> **实现注记**：`ActiveWorkRegistry` 内部类型已落地，并由 `WindowSession` 持有；单窗/副窗 event loop 已接 `next_deadline` / `drain_due`、无 deadline 注册项、到期 `Timer` 定点派发到 widget scoped timer route、AppTimer 主线程回调执行、Tooltip 内置 timer 零维护托管、WidgetAnimation 以 `Animation(id)` 登记下一帧 deadline；内置 Animation 源见 [component · 动画](component.md#动画)；IME composition session 已托管。

<a id="多窗单-loop"></a>

### 多窗单 loop（#116）

**一个**进程级 `run_app_loop`；每窗独立 **WindowSession**，不共享 WidgetTree / 引擎 / 三态：

```rust
struct WindowSession {
    window_id: WindowId,
    tree: WidgetTree,
    engine: Box<dyn GraphicsEngine>,
    loop_state: WindowLoopState,   // DeepIdle | RegisteredActive | Active
    active_work: ActiveWorkRegistry,
    window_visible: bool,
}
```

```text
run_app_loop(sessions):
  loop {
    if all sessions DeepIdle && all registries empty:
        blocking wait_event()           // #117
    else:
        deadline = min(all session.next_deadline())
        wait_until(deadline) or poll/drain events

    for each session with pending work OR matching window_id event:
        run_active_frame(session)       // 见「帧内合并」
  }
```

| 规则 | 说明 |
|------|------|
| UiEvent 路由 | 平台事件带 **window_id** → 仅目标 session 进入 Active |
| 共享资源 | AppState + Theme **全局一份**（#93–#94） |
| 独立 present | 各 session 独立 `FrameRenderer` + presenter |
| A 窗 Active | 可唤醒共享的进程 loop；**不**将 B 窗置 Active，B 保持 DeepIdle 且不跑 frame work（#110 #173） |

> **实现注记**：`run_gui` 已构造主窗 `WindowSession` 并在同一 loop 内编排副窗 `WindowSession`；native 多窗、window_id 路由、队列/Timer 与运行期 frame drain 已接。

<a id="帧内合并"></a>

### 帧内合并（#118）

Active 帧内 **合并** 多次变更，避免 reconcile / layout / present 抖动：

```text
run_active_frame(session):
  1. drain 本窗 UiEvent → dispatch
  2. active_work.drain_due(now) → 窄 tick / 关联 paint
  3. main_thread_queue.drain() → post_to_ui 闭包（#137）
  4. drain AppState semantic queue
  5. update due animation + tick_effects（仅 Effect pending）
  6. if reconcile_pending: ViewAdapter::reconcile **一次**
  7. if has_layout(): layout
  8. if has_render_work(): render_frame → present?
  9. coalesce 清空；无 pending 且无 register → DeepIdle
```

| 合并点 | 规则 |
|--------|------|
| State::set | 帧内多次 set → `reconcile_pending`；帧末 **一次** reconcile（#118 #153） |
| update_view | `pending_root` 优先于 `view_factory`（#149 #153） |
| invalidate_paint | 同 node 多 rect → InvalidationQueue merge |
| Effect | 在 reconcile **之前** tick（步骤 5）；Effect → State → 并入步骤 6 reconcile 批次 |
| reconcile | 合并算法见 [view-reactive · reconcile 合并](view-reactive.md#reconcile-合并)（#153） |

> **实现注记**：单窗主循环已接帧末 reconcile；`pending_root` 与 State 批次合并为同一次 reconcile。详见 [application · update_view](application.md#update_view)。

### 框架托管的周期工作（#111、#124）

**调用方不 register**。下列由框架在组件/IME 生命周期内 **自动** 托管：

| 类型 | 来源 | 框架 unregister 时机 |
|------|------|----------------------|
| Animation | `Animatable` / Spin 等 **内置动画**开始 | 动画结束 / hide / unmount |
| Timer | Tooltip 等 **内置**组件 | hide / unmount / 触发 |
| IME | 焦点 Input + `text_input.start` | 失焦 |

未托管的周期工作 **不得**存在；须内置组件、**#132 Timer API** 或 async→State（#131）。

> **实现注记**：RegisteredActive deadline wait 与无 deadline 注册项已接；单窗 loop 已移除固定 `wait_timeout(100ms)` 探活、写回 `WindowLoopState`，`update` 门控到 Active 帧、`tick_effects` 收窄到 Effect pending；隐藏窗口不 layout/render，pending dirty 保留到恢复可见后消费。内置 Animation / Timer 源见 [component · 动画](component.md#动画)；IME composition session 已接。

---

## 失效与窄标脏

统一入口：`InvalidationQueue`（[rendering · 管线](rendering.md#管线与失效)）。

| 场景 | 要求 | 决策 |
|------|------|------|
| State 变更 | 仅绑定 widget 的 `Paint { rect }` | #24 |
| hover 变化 | 旧 + 新 widget | #109 |
| Theme | palette-only paint | #9 |
| **Scroll** | `Composite` + memmove；exposed strip 补绘 | #107 |
| **Picture** | **PicturePolicy 自动推断** + 自适应代价阈值（#122、#129） | #122 |
| Reconcile | 增量 diff | #31 #49 |

<a id="picturepolicy-自动推断122129"></a>

### PicturePolicy 自动推断（#122、#129、#136）

**调用方不维护名单**。LayerTree build 时框架对每个子树根 **自动** 判定；**metadata 与 runtime 合并**见 [#136](../../decisions.md#d136)（任一为 Never → Never）。

```rust
enum PicturePolicy {
    Never,      // 默认；交互/滚动/State/浮层等
    Eligible,   // 静态只读深子树；或 widget 显式 opt-in
}
```

**自动 Never**（任一成立）：

| 信号 | 来源 |
|------|------|
| 交互五态 / hover·pressed | StyleSet 非 static-only |
| 事件 handler 绑定 | HandlerTable 有语义 handler |
| State / dynamic 内容 | `dynamic_label`、State bind 标记 |
| 滚动视口 | `children_clip` / ScrollView |
| 浮层 | `overlay_entry` / OverlayKind |
| 连续 pointer | `EventHandler::wants_continuous_pointer_move` |
| 显式覆盖 | widget metadata `PicturePolicy::Never` |

**Eligible + 自适应阈值**（#129，框架常量，App 不可配）：

```text
PicturePolicy == Eligible
AND subtree_node_count >= 8
AND subtree_estimated_pixels >= 65536
→ LayerTree 使用 Picture；否则 Direct
```

内置 widget 在 **authoring 元数据**（`component!`）声明默认 PicturePolicy；合并算法详见 [component · PicturePolicy 元数据](component.md#picturepolicy-元数据122)。新 widget **默认 Never**。

### RegisteredActive 关联脏区（#126）

`drain_due` 后框架 **自动** 计算并 push Invalidation，调用方不标脏：

| ActiveWorkKind | 脏区来源 |
|----------------|----------|
| Animation | `Animatable::dirty_bounds()` |
| Timer（Tooltip 等） | 目标 component `frame` |
| **AppTimer**（#132） | **无**默认 paint；仅回调内 `State::set` / invalidate 触发 |
| ImeSession | 焦点 Input `frame` ∪ caret rect |

---

## PointerMove 窄路径

设计（#109）— 派发见 [event · 派发](event.md#pointermove-按需零闲置)。

**边界感知窄路径**（在 L1 CPU 与 hover/拖拽正确性之间取最优）：

```text
PointerMove 到达
  ├─ DragManager active/potential 或 InteractionManager.pressed_component 存在？
  │     → 全 dispatch（Scrollbar 拖拽等）
  ├─ pos 仍在 InteractionManager.hovered_component 扩大 hit 框内？
  │     → 仅更新拖拽/hover 状态；不 hit_test；默认不 dispatch
  │       （wants_continuous_pointer_move=true 时仍 dispatch）
  └─ 否则
        → hit_test
        → target ≠ hovered_component 时 dispatch + 窄标脏
```

| 扩展 | 规则 |
|------|------|
| `EventHandler::wants_continuous_pointer_move`（#121） | opt-in hook；默认 **false**；为 true 时 hover 框内每 move 仍 dispatch（SignaturePad、画布涂鸦等） |
| debug_mode hover 链 | 可走独立路径；不强制每 move 全树 dispatch |

---

## Effect 与 Theme

### Effect（#79 + #105）

| 状态 | Effect 行为 |
|------|-------------|
| **DeepIdle** | **不** `tick_effects` |
| **Active**（UiEvent 或 State 唤醒后） | 检查依赖；变化则执行闭包 |
| 闭包内改 State | 窄 paint 标脏；禁止直接改 WidgetTree |

Effect **不得**作为 DeepIdle 的定时 wake 源。**禁止**轮询式 Effect（#131）；周期逻辑可用 **#132 Timer API** 或 async→State。

### Theme 运行中切换（#125、#128，修订 #112）

| 模式 | 行为 |
|------|------|
| **默认** | `.follow_system_theme(false)`（默认）；运行中 **不**跟 OS；App 调用 `.theme(...)` 切换 |
| **opt-in 跟 OS** | `.follow_system_theme(true)` → 框架监听 `ThemeChanged` → 读 `IDisplay::is_dark_mode()` → 更新 DynTokens → **遍历所有 WindowSession** palette-only invalidate（#128） |
| **启动** | 可读 OS 初始明暗（#74）；Settings `theme_mode` 优先 |
| **禁止** | 框架后台 poll（无 UiEvent 不 wake）；`follow_system_theme` 仅在 opt-in 时响应 ThemeChanged |

App **无需**手写 ThemeChanged handler（opt-in 时）；**无需**手动逐窗 invalidate。

---

## 分域要求

> **可组合组件**（[#168](../../decisions.md#d168)）：各域按正交能力划分、trait/registry 组装；bundled 分派仅作便利入口。渲染轴 → [graphics-backend-pluggable · 可组合渲染轴](graphics-backend-pluggable.md#可组合渲染轴)。

| 域 / 系统 | 按需零闲置要求 | 可组合要点（#168） |
|-----------|----------------|-------------------|
| **app** | WindowSession + 单 loop；#132 Timer；`post_to_ui`（#133 #141）；`on_start`（#140）；`.follow_system_theme`（#125） | 编排 probe + engine，**不**硬编码 API 分支 |
| **ui / event** | PointerMove 窄路径；`when` O(1)（#120）；handler 智能重绑（#123） | 平台·系统·语义三层；HandlerTable 按组件绑定 |
| **ui / view** | State → 窄 paint；帧末 reconcile（#118）；Effect + #132 Timer | View 节点 = 可替换子树 |
| **ui / layout** | Scroll → Composite（#107）；框架自动 | measure / arrange 分离；Flex/Grid 纯函数引擎 |
| **ui / component** | PicturePolicy 元数据（#122）；Animatable 自动 register（#124）；内置周期 UI | capability trait 自由组合 |
| **draw** | Picture 自适应（#129）；Composite memmove | 光栅 / present / API **正交**（`RasterMode` × `PresentMode` × `GraphicsBackend`） |
| **native** | blocking wait；ThemeChanged 作 UiEvent 交付，不 poll | `Platform` 聚合子 trait；graphics API peer |
| **data** | 冷路径（#64） | 不参与 UI 热路径 |
| **core** | 诊断不进 UI 热路径（#89） | 共享几何 / damage 类型 |

---

## 豁免机制

若存在 **无法**通过 register 或事件驱动的定时/轮询需求（如极少数平台 API）：

1. 在 [`decisions.md`](../../decisions.md) 追加公开豁免条目（#158 记录当前无豁免；当前新豁免从 **#175+** 起）；
2. 说明触发源、wake 频率、允许的工作范围、为何无法 register；
3. [testing · 零闲置验收](testing.md#测试策略) 须覆盖：无事件时 assert **不** present/layout（或豁免边界）；
4. 默认 **不豁免**；从严审查。

---

## 新 API 审查清单

新增或修改公开 API / 主循环路径 / 每帧回调时，须能回答：

1. **何时工作？** 触发源是什么（事件 / 失效 / register）？
2. **何时休眠？** pending 清空且 unregister 后是否回 DeepIdle？
3. **标脏范围？** 能否窄到 rect / Composite？
4. **能否合并？** 同帧多次变更是否 batch？
5. **测试如何证明零闲置？** 无事件时无 present / layout（或 #113 豁免范围）？
6. **多窗？** 是否仅影响本窗状态（#110）？
7. **调用方能否零维护？** 是否须 App register/名单/手动标脏？（#130 应答「否」）

无法回答 → 不得合并，或走 [豁免机制](#豁免机制)（当前新豁免从 #175+ 起）。

---

<a id="剩余差距"></a>

## 剩余差距

P0–P5 与按需零闲置主体机制 **已落地**；按域能力清单见 [implementation · 实现进度总览](../implementation.md#实现进度总览)。

**未实现 backlog（权威清单）** → [implementation · 后续工作](../implementation.md#后续工作)。各条的设计细节与域内边界见 implementation 表格「文档」列链至的系统章节（如 [view-reactive · 热更新](view-reactive.md#热更新设计)、[component · PicturePolicy](component.md#picturepolicy-元数据122)）。

源码与「设计」列不一致时 **按文档重构**（[`AGENTS.md`](../../../AGENTS.md)）。

与本系统直接相关的关键文件 → [implementation · 源码目录详表](../implementation.md#源码目录详表)（`app/window_session.rs`、`app/active_work_registry.rs`、`app/main_thread_queue.rs`、`app/event_loop/event_loop.rs` 等）。
