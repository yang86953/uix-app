# 按需零闲置

← [Main](../Main.md) · 系统 **#13** · 功能域：跨域

> **UIX 核心理念的操作细则**（[#105](../decisions.md#d105)）。  
> **最高规则**：**用最少资源，做最好效果** — 统领六域依赖与一切子系统；冲突时以本规则为准。  
> 硬约束摘要 → [`AGENTS.md`](../../AGENTS.md)「核心理念」。

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
| 分域要求 | [分域要求](#分域要求) | #105–#158 |
| 豁免 | [豁免机制](#豁免机制) | #113 |
| 审查 | [新 API 审查清单](#新-api-审查清单) | #105 #120 #130 |
| 实现差距 | [实现差距](#实现差距) | — |

**关联**：[application](application.md) · [rendering](rendering.md) · [event](event.md) · [view-reactive](view-reactive.md) · [component](component.md) · [data](data.md) · [theme-style](theme-style.md)

---

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
| DeepIdle | 投递本身 **不** wake；闭包内 `State::set` 才进入 Active |
| 禁止 | 闭包内直接改 WidgetTree / HandlerTable / 裸 invalidate |
| 多窗 | `AppHandle.window_id` → 该 session 队列（#141）；**禁止**跨窗 handle |

与 #132 分工：**Timer** = 框架 register + deadline wake；**post_to_ui** = 外部完成信号 → 主线程一次性闭包。队列调度见 [MainThreadQueue](#mainthreadqueue)（#137）。

> **实现注记**：`App::post_to_ui` 与 `AppHandle::post_to_ui` 已导出；`AppHandle` 经 `AppRuntime` 按 `window_id` 写入目标 `WindowSession` 的 `MainThreadQueue`，session 销毁后丢弃闭包；成功入队后会调用通用 `EventLoopWaker`。副窗 MainThreadQueue、事件路由、运行期 frame drain 与 deadline wait 已接；Windows/fake 后端已提供真实 wake，Linux Wayland 原生 waker 待接。

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

```text
run_active_frame(session):
  1. drain 本窗 UiEvent → dispatch          // 输入优先
  2. active_work.drain_due(now)             // AppTimer / Animation / IME
  3. main_thread_queue.drain()              // post_to_ui 闭包
  4. tick_effects（Active 态）
  5. if reconcile_pending: reconcile 一次
  6. layout → render → present?
  7. 无 pending 且无 register → DeepIdle
```

| 规则 | 说明 |
|------|------|
| FIFO | 同 session 内 `post_to_ui` **先进先出** |
| 优先级 | **UiEvent > drain_due > post_to_ui**；同一步骤内保持 FIFO |
| wake | 入队设 `main_thread_pending`；**不** register ActiveWork（#105） |
| DeepIdle | 队列空且 drain 完毕 → 可回 DeepIdle |
| 跨 session | 各 WindowSession **独立**队列；不跨窗投递（#141） |
| 与 Timer | `AppTimer` 回调走 `drain_due`（步骤 2）；**不**与 post_to_ui 混队 |

`post_to_ui` 闭包内 `State::set` 并入步骤 5 的 reconcile 批次（#118）；禁止在步骤 3 直接改 WidgetTree。

> **实现注记**：`MainThreadQueue` 已落地并在单窗 event loop 中按 UiEvent → due work → post_to_ui 顺序 drain；`AppRuntime` 已按 `window_id` 路由投递并在独立 session 关闭时清理队列；有效 session 成功入队后会唤醒事件循环，关闭后的 late post 不入队也不唤醒。副窗 session bootstrap、MainThreadQueue 消费、事件路由、运行期 frame drain 与 deadline wait 已接；Windows/fake 后端已接真实 wake，Linux Wayland 原生 waker 待接。

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
    where F: FnOnce() + 'static;

    /// 重复：每 interval 主线程执行 f；cancel/drop 停止
    fn run_interval<F>(&self, interval: Duration, f: F) -> TimerHandle
    where F: FnMut() + 'static;
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
| 零闲置 | 无 pending Timer → DeepIdle；`wait_until(min deadline)`（#127） |
| 副作用 | 回调内 `State::set` → 按需 reconcile；**不**默认 layout/render |
| 与内置 UI | 可见周期动画仍 **优先** 内置 widget；Timer API 用于 **业务逻辑**（保存、刷新、倒计时数据） |

> **实现注记**：`App::run_after` / `run_interval` / `TimerHandle` 与 `AppHandle::run_after` / `run_interval` 已导出；`AppHandle` 经 `AppRuntime` 按 `window_id` 路由到所属 AppTimer 队列，独立 session 关闭会批量 cancel。副窗 session bootstrap、运行期 Timer 消费与 deadline wait 已接；外部线程投递 wake 已接入通用 `EventLoopWaker` 与 Windows/fake 后端，Linux Wayland 原生 waker 待接。

---

## 三层目标

| 层级 | 含义 | 验收 |
|------|------|------|
| **L0 零像素** | 无 Paint/Composite 失效 → 不 `render_frame`、不 `present` | `has_render_work()` 门控 |
| **L1 零帧循环** | 无 pending → **DeepIdle**，blocking 等事件 | 无定时 wake 跑 layout/render/tick Effect |
| **L2 最小脏区** | 有变化 → 最小 rect / Composite memmove / Picture 自适应 / PointerMove 窄路径 | 滚动、PicturePolicy、PointerMove |

---

## 唤醒源白名单

**仅以下情况可离开 DeepIdle**（须有明确来源）：

| 唤醒源 | 进入状态 | 允许的工作 |
|--------|----------|------------|
| OS `UiEvent` | Active | dispatch → 按需 layout/render |
| `InvalidationQueue` push | Active | layout / render |
| **RegisteredActive** 注册项到期 | RegisteredActive → 可能 Active | Animation / 内置 UI Timer / **App Timer**（#132）/ IME；窄 tick |
| 窗口 resize / 可见性 | Active | layout + 必要时 full-frame |
| `State` 变更（含 Effect 间接） | Active | 窄 paint 标脏 |

**不在白名单内（禁止作为框架默认行为）**：

- 固定 interval `wait_timeout` 探活并 layout/render
- DeepIdle 下 `tick_effects`
- 框架后台 poll OS 主题（[#125](#d125) 仅 opt-in 时响应 ThemeChanged，不 poll）
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
| **RegisteredActive** | AnimationRegistry / Timer / IME 会话等 **register** | 注册项 tick；仅关联节点 paint | 全树 layout；无关注册 |
| **Active** | UiEvent 或 Invalidation pending | 完整按需管线 | 无条件全树 layout/render |

| 规则 | 决策 |
|------|------|
| unregister 后 | 下一机会 **立即**回 DeepIdle（#111） |
| `window_visible = false` | 该窗不 layout/render |
| 多窗 | **每窗独立状态**；A 窗 Active 不要求 B 窗 wake（#110） |
| 进程级 sleep | 所有窗 DeepIdle 且全局 Registry 空 → blocking `wait_event`（#117） |
| 有 register | app 层 **`wait_until(remaining)`** = 单次 `wait_timeout(remaining)`（#127）；**非**固定 interval 探活 |

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
remaining = min(all next_deadline) - now
if remaining <= 0 → drain_due + 继续帧
else if all DeepIdle → blocking wait_event()
else → wait_timeout(remaining)   // 单次，非固定 100ms 探活
```

不要求 native 新增 API；用现有 `wait_timeout` + 计算 remaining 实现。

每 **WindowSession** 持有一份 Registry（#116）。

> **实现注记**：`ActiveWorkRegistry` 内部类型已落地，并由 `WindowSession` 持有；单窗 event loop 已接 `next_deadline` / `drain_due` 骨架、无 deadline 注册项、到期 `Timer` → `SystemEvent::Timer` 消费、AppTimer 主线程回调执行、Tooltip 内置 timer 托管、WidgetAnimation 下一帧 deadline、`Spin` / `ProgressBar` indeterminate / Modal / Drawer 内置动画源与 IME composition session 托管；其他过渡动画源尚未接入。

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
| A 窗 Active | **不** wake B 窗（#110）；B 可仍 DeepIdle |

> **实现注记**：`run_gui` 当前单窗，已构造 `WindowSession` 并传入 session loop；native 多窗已具备，window_id 路由与多 session 编排待扩展。

### 帧内合并（#118）

Active 帧内 **合并** 多次变更，避免 reconcile / layout / present 抖动：

```text
run_active_frame(session):
  1. drain 本窗 UiEvent → dispatch
  2. active_work.drain_due(now) → 窄 tick / 关联 paint
  3. main_thread_queue.drain() → post_to_ui 闭包（#137）
  4. tick_effects（Active 态）
  5. if reconcile_pending: ViewAdapter::reconcile **一次**
  6. if has_layout(): layout
  7. if has_render_work(): render_frame → present?
  8. coalesce 清空；无 pending 且无 register → DeepIdle
```

| 合并点 | 规则 |
|--------|------|
| State::set | 帧内多次 set → `reconcile_pending`；帧末 **一次** reconcile（#118 #153） |
| update_view | `pending_root` 优先于 `view_factory`（#149 #153） |
| invalidate_paint | 同 node 多 rect → InvalidationQueue merge |
| Effect | 在 reconcile **之前** tick（步骤 4）；Effect → State → 并入步骤 5 |
| reconcile | 合并算法见 [view-reactive · reconcile 合并](view-reactive.md#reconcile-合并)（#153） |

> **实现注记**：单窗主循环已接帧末 reconcile；`update_view` 的 `pending_root` 与响应式 `State` 批次置位会合并到同一次 reconcile。副窗 `MainThreadQueue` / `update_view` root reconcile 消费已接；外部线程投递 wake 已接入通用 `EventLoopWaker` 与 Windows/fake 后端，Linux Wayland 原生 waker 待接。

### 框架托管的周期工作（#111、#124）

**调用方不 register**。下列由框架在组件/IME 生命周期内 **自动** 托管：

| 类型 | 来源 | 框架 unregister 时机 |
|------|------|----------------------|
| Animation | `Animatable` / Spin 等 **内置动画**开始 | 动画结束 / hide / unmount |
| Timer | Tooltip 等 **内置**组件 | hide / unmount / 触发 |
| IME | 焦点 Input + `text_input.start` | 失焦 |

未托管的周期工作 **不得**存在；须内置组件、**#132 Timer API** 或 async→State（#131）。

> **实现注记**：当前已有 RegisteredActive deadline wait 骨架与无 deadline 注册项；单窗 loop 已移除固定 `wait_timeout(100ms)` 探活、写回 `WindowLoopState`，并将 `update` 门控到 Active 帧、将 `tick_effects` 进一步收窄到 Effect pending；Tooltip 内置 timer、AppTimer 注册源、WidgetAnimation 下一帧 deadline、`Spin` / `ProgressBar` indeterminate / Modal / Drawer 内置动画源与 IME composition session 已接，其他过渡动画源待接。

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

### PicturePolicy 自动推断（#122、#129、#136）

**调用方不维护名单**。LayerTree build 时框架对每个子树根 **自动** 判定；**metadata 与 runtime 合并**见 [#136](../decisions.md#d136)（任一为 Never → Never）。

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

内置 widget 在 **authoring 元数据**（`component!` / `define_widget!`）声明默认 PicturePolicy；合并算法详见 [component · PicturePolicy 元数据](component.md#picturepolicy-元数据122)。新 widget **默认 Never**。

### RegisteredActive 关联脏区（#126）

`drain_due` 后框架 **自动** 计算并 push Invalidation，调用方不标脏：

| ActiveWorkKind | 脏区来源 |
|----------------|----------|
| Animation | `Animatable::dirty_bounds()` |
| Timer（Tooltip 等） | 目标 widget `frame` |
| **AppTimer**（#132） | **无**默认 paint；仅回调内 `State::set` / invalidate 触发 |
| ImeSession | 焦点 Input `frame` ∪ caret rect |

---

## PointerMove 窄路径

设计（#109）— 派发见 [event · 派发](event.md#pointermove-按需零闲置)。

**边界感知窄路径**（在 L1 CPU 与 hover/拖拽正确性之间取最优）：

```text
PointerMove 到达
  ├─ drag_gesture.active 或 pointer_down_target 存在？
  │     → 全 dispatch（Scrollbar 拖拽等）
  ├─ pos 仍在 hovered_widget 扩大 hit 框内？
  │     → 仅更新 cursor_pos；不 hit_test；默认不 dispatch
  │       （wants_continuous_pointer_move=true 时仍 dispatch）
  └─ 否则
        → hit_test
        → target ≠ hovered_widget 时 dispatch + 窄标脏
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

| 域 / 系统 | 按需零闲置要求 |
|-----------|----------------|
| **app** | WindowSession + 单 loop；#132 Timer；`post_to_ui`（#133 #141）；`on_start`（#140）；`.follow_system_theme`（#125） |
| **ui / event** | PointerMove 窄路径；`when` O(1)（#120）；handler 智能重绑（#123） |
| **ui / view** | State → 窄 paint；帧末 reconcile（#118）；Effect + #132 Timer |
| **ui / layout** | Scroll → Composite（#107）；框架自动 |
| **ui / component** | PicturePolicy 元数据（#122）；Animatable 自动 register（#124）；内置周期 UI |
| **draw** | Picture 自适应（#129）；Composite memmove |
| **native** | blocking wait；ThemeChanged 作 UiEvent 交付，不 poll |
| **data** | 冷路径（#64） |
| **core** | 诊断不进 UI 热路径（#89） |

---

## 豁免机制

若存在 **无法**通过 register 或事件驱动的定时/轮询需求（如极少数平台 API）：

1. 在 [`decisions.md`](../decisions.md) 追加 **#159+** 公开豁免条目（#158 记录当前无豁免）；
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

无法回答 → 不得合并，或走 [豁免机制](#豁免机制)（#159+）。

---

## 实现差距

| 能力 | 设计 | 当前 | 文档 |
|------|------|------|------|
| 三态主循环 | DeepIdle / RegisteredActive / Active | 单窗 loop 已移除固定 100ms 探活并写回三态；DeepIdle 跳过 update / tick_effects；Active 帧仅在 Effect pending 时 tick；RegisteredActive deadline wait 骨架已接 | [#106](../decisions.md#d106) [#117](../decisions.md#d117) |
| ActiveWorkRegistry | register / next_deadline / drain_due | 内部类型已建并由 WindowSession 持有；event loop 已接 `next_deadline` / `drain_due` 骨架、无 deadline 注册项、到期 `Timer` / `AppTimer` 消费、Tooltip 内置 timer 托管、WidgetAnimation 下一帧 deadline、`Spin` 内置动画源与 IME composition session 托管 | [#115](../decisions.md#d115) |
| 多窗单 loop | WindowSession + window_id 路由 | 单窗 run_gui 已构造 WindowSession 并传入 session loop；副窗创建、独立 `WindowSession` bootstrap、事件按 `window_id` 路由、运行期 frame drain 与 deadline wait 已接；外部线程投递 wake 已接入通用 `EventLoopWaker` 与 Windows/fake 后端，Linux Wayland 原生 waker 待接 | [#116](../decisions.md#d116) |
| 帧内 reconcile 合并 | 帧末一次 reconcile + coalesce | 单窗 `update_view` / `pending_root` / State 批次路径已接入主循环；副窗 MainThreadQueue / root reconcile 消费、运行期 frame drain 与 deadline wait 已接；外部线程投递 wake 已接入通用 `EventLoopWaker` 与 Windows/fake 后端，Linux Wayland 原生 waker 待接 | [#118](../decisions.md#d118) |
| 每窗独立状态 | 每窗独立 DeepIdle/Active | 单窗 WindowSession 与副窗运行期 frame drain 已写回三态；外部线程投递 wake 已接入通用 `EventLoopWaker` 与 Windows/fake 后端，Linux Wayland 原生 waker 待接 | [#110](../decisions.md#d110) |
| Composite scroll | Composite + memmove | Wheel → ScrollView 已接 exposed strip + `scroll_region`；其他滚动来源待接 | [#107](../decisions.md#d107) |
| PicturePolicy 自动推断 | 元数据 + 子树信号 → Never/Eligible | `PicturePolicy` 元数据、运行时信号 Never 合并、`node_count≥8 && est_pixels≥65536` 阈值已接；Container/Grid 首批 Eligible，默认 Never | [#122](../decisions.md#d122) [#129](../decisions.md#d129) |
| Registry 框架托管 | 内置组件/IME 自动 register | Registry 类型已建；Tooltip 内置 timer、单窗 AppTimer、WidgetAnimation 下一帧 deadline、`Spin` / `ProgressBar` indeterminate / Modal / Drawer 内置动画源与 IME composition session 已托管；其他过渡动画源待接 | [#124](../decisions.md#d124) |
| follow_system_theme opt-in | false 默认；true 框架全自动 | App builder + ThemeChanged 事件路径已接；默认 false 忽略 ThemeChanged；无后台 poll | [#125](../decisions.md#d125) |
| App Timer API | run_after / run_interval | `App` / `AppHandle` 的 `run_after` / `run_interval` / `TimerHandle` 已导出；`AppHandle` 已按 `window_id` 路由到所属 AppTimer 队列；副窗 session bootstrap、运行期 Timer 消费与 deadline wait 已接；外部线程投递 wake 已接入通用 `EventLoopWaker` 与 Windows/fake 后端，Linux Wayland 原生 waker 待接 | [#132](../decisions.md#d132) |
| post_to_ui | App / AppHandle 主线程投递 | `App::post_to_ui` + `AppHandle::post_to_ui` + MainThreadQueue 已接；`AppHandle` 已按 `window_id` 路由；副窗 MainThreadQueue、事件路由、运行期 frame drain 与 deadline wait 已接；有效入队会触发 `EventLoopWaker`，Windows/fake 后端已接真实 wake，Linux Wayland 原生 waker 待接 | [#133](../decisions.md#d133) |
| MainThreadQueue | FIFO + 帧内 drain 顺序 | 每 WindowSession 队列已接；单窗 drain 顺序为 UiEvent → due work → post_to_ui；`AppHandle` 多窗队列路由、副窗 bootstrap、MainThreadQueue 消费、运行期 frame drain 与 deadline wait 已接；有效入队会触发 `EventLoopWaker`，Windows/fake 后端已接真实 wake，Linux Wayland 原生 waker 待接 | [#137](../decisions.md#d137) |
| TestClock | App drain_due / wait_until 测试注入 | App 层 `AppClock` / `TestClock` 已接入 AppTimer deadline、RegisteredActive wait_until、drain_due；native `FakeTimer` 仍独立 | [#139](../decisions.md#d139) |
| AppHandle 生命周期 | 窗关闭/run 结束 cancel Timer | `AppRuntime::close_session` 会关闭指定 handle、cancel 该 session AppTimer、清空 MainThreadQueue 并移除待创建副窗请求；主窗 close 退出主循环，副窗真实 close 事件按 `window_id` 关闭对应 session | [#134](../decisions.md#d134) |
| on_start | `.on_start(AppHandle)` 每窗一次 | 单窗 `.on_start(AppHandle)` 与副窗 `.on_window_start(AppHandle)` 已导出，并在对应 WindowSession 创建后、首帧前调用 | [#140](../decisions.md#d140) |
| 多窗 post_to_ui | AppHandle.window_id 路由 | `AppRuntime` 路由表已接；`AppHandle` 投递仅进入自身 `window_id` 的队列，session 销毁后丢弃闭包 | [#141](../decisions.md#d141) |
| open_window | 副窗 API | `WindowConfig` / `AppHandle::open_window` / `.on_window_start` 已导出；可分配新 `window_id`、独立队列/Timer/handle 并暂存副窗创建请求；GUI loop 可 drain 请求并创建 native 窗 | [#144](../decisions.md#d144) |
| open_window 接线 | 副窗独立 build/reconcile | 副窗 native 创建、独立 `WindowSession` bootstrap、MainThreadQueue / `update_view` reconcile 消费、事件按 `window_id` 路由、运行期 frame drain 与 deadline wait 已接；外部线程投递 wake 已接入通用 `EventLoopWaker` 与 Windows/fake 后端，Linux Wayland 原生 waker 待接 | [#148](../decisions.md#d148) |
| AppState register | mount 自动 register Handle | `AppState` snapshot registry 已建；`WidgetTree::set_app_state` 后 mount/unmount 自动 register/unregister snapshot，并记录所属失效队列与当前 dirty rect；App 默认持有同一 `AppState` 并注入主窗与副窗 `WindowSession`；lookup handle 的 live `emit` dispatch 绑定待接 | [#145](../decisions.md#d145) |
| StateSlotId | State::new 单调 id | 已接；clone 共享，`generation()` 不参与身份 | [#143](../decisions.md#d143) |
| Handler 智能重绑 | handler 变才重注册（#135） | 稳定 signature 路径已跳过重绑；带 fingerprint 的 handler 可自动复用/递增 generation；无 generation/fingerprint 的 DSL handler 仍保守重绑 | [#123](../decisions.md#d123) [#135](../decisions.md#d135) |
| handler_generation + 指纹 | build 自动 bump（#142） | 内部 generation 字段、State capture 指纹基础与 fingerprint→generation 解析已接；View build / DSL 自动 capture 收集待接 | [#138](../decisions.md#d138) [#142](../decisions.md#d142) |
| PointerMove 边界窄路径 | 框内不 hit_test | 已接：pointer_down_target/drag 全 dispatch；hover hit frame 内跳过 hit_test 与默认 dispatch；`wants_continuous_pointer_move` opt-in 可连续 dispatch | [#109](../decisions.md#d109) [#121](../decisions.md#d121) |
| Effect DeepIdle 跳过 | 不 tick_effects | 单窗/副窗 loop 已门控到 Active 帧，且仅在 Effect pending 时 tick；动画续帧经 Registry deadline 唤醒；`Spin` / `ProgressBar` indeterminate / Modal / Drawer 内置动画源已接，其他过渡动画源待接 | #105 |
| ComponentConfigSnapshot | mount 提取配置 | 类型与首批内置静态配置提取已接；`ComponentHandle` 直接 snapshot getter 已接；AppState mount/unmount snapshot register 已接；reconcile patch update 已接 | [#146](../decisions.md#d146) |
| Handle emit / invalidate / getter | dispatch_semantic + 窄 Paint + 只读配置 | `ComponentHandle::emit` 已导出并走 `WidgetTree::dispatch_semantic`；live handle 与 `AppState::get_handle` lookup handle 的 `invalidate()` 均已接窄 Paint；`snapshot()` / `text()` / `placeholder()` / `disabled()` 已接；App 默认持有 `AppState`，`AppState::get_handle` 可查 snapshot handle；lookup handle 的 live `emit` dispatch 绑定待接 | [#119](../decisions.md#d119) [#147](../decisions.md#d147) |
| update_view | AppHandle 按 session reconcile | `AppHandle::update_view` / `set_root` 已导出；经 `AppRuntime` 按 `window_id` 写目标 MainThreadQueue 并在帧末 reconcile；副窗 session bootstrap、root reconcile 消费、运行期 frame drain 与 deadline wait 已接；外部线程投递 wake 已接入通用 `EventLoopWaker` 与 Windows/fake 后端，Linux Wayland 原生 waker 待接 | [#149](../decisions.md#d149) |
| State 跨窗标脏 | paint_sites fan-out | `State` / `Computed` 已支持多个 paint site fan-out；`State` reconcile callback 已支持按 site key fan-out 并原地更新重复绑定；副窗 session 路由与 deadline wait 已接；外部线程投递 wake 已接入通用 `EventLoopWaker` 与 Windows/fake 后端，Linux Wayland 原生 waker 待接 | [#150](../decisions.md#d150) |
| SnapshotSource | component! 自动快照 | `SnapshotSource` trait 已导出；Button / Label / Input / Container / Grid 手写实现已接；`define_widget!` 自定义组件 pub 字段自动提取已接；`component! { name: ..., struct ... }` 已复用该路径，完整独立 DSL 待接 | [#151](../decisions.md#d151) |
| snapshot(skip) | 字段属性排除 | `define_widget!` 已解析并消费 `#[snapshot(skip)]`；首批手写内置提取已人工排除运行态字段；`component! { name: ..., struct ... }` 已复用该排除逻辑，完整独立 DSL 待接 | [#152](../decisions.md#d152) |
| reconcile 合并 | pending_root 优先 | 单窗 `update_view` 路径与 State 批次自动置位已接；副窗 MainThreadQueue / root reconcile 消费、运行期 frame drain 与 deadline wait 已接；外部线程投递 wake 已接入通用 `EventLoopWaker` 与 Windows/fake 后端，Linux Wayland 原生 waker 待接 | [#153](../decisions.md#d153) |
| view_factory | session 固定 Arc | 单窗 `App::root(|| ...)` 与副窗 `WindowConfig::new(..., || ...)` 已安装 session factory | [#155](../decisions.md#d155) |
| 豁免台账 | #159+ 条目 + 测试 | 已建立初始台账：#158 明确当前无豁免；新增豁免须从 #159+ 追加并补测试边界 | [#113](../decisions.md#d113) [#158](../decisions.md#d158) |

源码与「设计」列不一致时 **按文档重构**（[`AGENTS.md`](../../AGENTS.md)）。

---

## 源码模块（实现时参考）

跨域；无独立 `src/` 顶层目录。主要落点：

```text
app/window_session.rs            WindowSession 壳、Registry 持有、三态字段（#106 #116）
app/event_loop/event_loop.rs     run_app_loop、三态调度（#106 #117）
app/main_thread_queue.rs        post_to_ui FIFO（#137）
app/test_clock.rs                AppClock / TestClock 注入（#139）
draw/compositor/scene_paint.rs  PicturePolicy 元数据与运行时信号 trait（#122）
draw/compositor/layer_tree.rs   PicturePolicy 子树推断与 #129 阈值
ui/traits/widget.rs             WidgetComponent::picture_policy 默认 Never（#122）
ui/core/active_work.rs          ActiveWorkRegistry 内部（#124）
ui/core/widget/tree_dirty.rs     失效队列
ui/core/widget/tree_events.rs    PointerMove、scroll
draw/pipeline/invalidation.rs    Invalidation
ui/foundation/state.rs          StateSlotId（#143）落地
ui/view/build_context.rs        handler_generation / capture 指纹（#138 #142）
ui/animation/                    AnimationRegistry register
ui/app_state.rs                 AppState snapshot registry（#145，部分落地）
ui/component_snapshot.rs         ComponentConfigSnapshot / SnapshotSource（#146 #151）
```

详见 [Main · 源码目录详表](../Main.md#源码目录详表)。
