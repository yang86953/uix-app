# 应用系统

← [Main](../Main.md) · 系统 **#1** · 功能域：`app`

> 启动、主循环、桥接平台 / View / 渲染。编排者，不实现组件或绘制算法。

## 索引

| 主题 | 章节 | 决策 |
|------|------|------|
| GUI / CLI | [运行模式](#运行模式) | — |
| 启动流程 | [启动](#启动) | #59 |
| 帧循环 | [主循环](#主循环) | #59 #83 #105–#157 |
| App 定时 | [App 定时 API](#app-定时-api) | #132 #134 |
| 主线程投递 | [post_to_ui](#post_to_ui) · [MainThreadQueue](#mainthreadqueue) | #133 #137 |
| AppHandle | [AppHandle 生命周期](#apphandle-生命周期) · [on_start](#on_start) · [update_view](#update_view) | #132 #134 #140 #141 #144 #149 |
| 桥接契约 | [桥接](#桥接) | #70 |
| 全局状态 | [AppState · 多窗 · Settings](#appstate--多窗--settings) | #32 #55 #64 #74 #88 #93 #101 #145 |
| 副窗 | [open_window](#open_window) | #144 #148 |
| 依赖注入 | [CLI 与 DI](#cli-与-di) | — |

**关联**：[platform](platform.md) · [view-reactive](view-reactive.md) · [rendering](rendering.md) · [data](data.md) · [demand-driven](demand-driven.md)

> **按需零闲置**（#105–#157）：框架 **零维护**（#130）；App 可用 **Timer API**（#132）与 **post_to_ui**（#133）。详见 [demand-driven · 开发者契约](demand-driven.md#开发者契约零维护)。

---

## 运行模式

| 模式 | 入口 | 行为 |
|------|------|------|
| **GUI**（默认） | `App::new().root(...).run()` | 创建窗口、引擎、WidgetTree、进入 `run_widget_loop` |
| **CLI** | `App::mode(AppMode::CLI).cli(...).run()` | 无窗口；解析 `argv`，分派已注册命令 |

GUI 必须调用 `.root(|| view)`；CLI 须 `.cli(Cli)` 注册 handler。

---

## 启动

`App::run_gui()` 顺序：

```text
1. create_platform()
2. window_manager().create_window() → center / show / raise
3. create_preferred_engine()
      GPU: create_gpu_context → GpuEngine::new
      失败 → shutdown GPU → SoftwareEngine 回退          (#59)
4. FontService::load_default_system_font
   ImageService::new
5. WindowSession::from_root_factory(root)
      → WidgetTree::build → layout → mark_full_frame_dirty
      → 同时将 `.root` 闭包存入 `WindowSession.view_factory`（#155）
6. run_widget_loop(platform, window, engine, tree, ...)
```

**App builder** 常用链接：

| 方法 | 作用 |
|------|------|
| `.title()` / `.size()` | 窗口标题与初始尺寸 |
| `.theme(Theme)` | 全局 Theme；框架 **自动** 全窗 palette-only invalidate（#128） |
| `.follow_system_theme(bool)` | 默认 **false**；**true** 时框架监听 ThemeChanged 自动跟 OS（#125） |
| `.root(|| view)` | 捕获为 session **`view_factory`**（#155）；冷启动 `build` + 热路径 reconcile 共用 |
| `.on_exit(Fn(&UiEvent) -> bool)` | 返回 `true` 退出主循环 |
| `.singleton<T>(instance)` | 注册 DI 单例 |

---

## 主循环

入口：`run_widget_loop`（`src/app/event_loop/event_loop.rs`）。

### 状态机（设计 #106、#110、#111）

| 状态 | 行为 |
|------|------|
| **DeepIdle** | blocking `wait_event`；不 layout/render/**不 tick Effect** |
| **RegisteredActive** | Animation / Timer / IME **register** 中；窄 tick + 关联 paint |
| **Active** | UiEvent 或 Invalidation → 按需 dispatch / layout / render |

多窗（#110、#116）：**每窗独立** `WindowSession`（树 + 引擎 + 三态 + Registry）；**单** `run_app_loop`；UiEvent 按 **window_id** 路由。

> **实现注记**：当前单窗已接 `WindowSession`、`ActiveWorkRegistry`、AppTimer、MainThreadQueue、root factory、`pending_root` / State 批次 reconcile 与三态写回；DeepIdle 不再固定 100ms 探活且不跑 `tick_effects`。多窗编排仍待接。

### 单帧顺序（Active 态，设计 #106、#137）

设计态 `run_active_frame` 顺序（修订 #118）：

```text
1. drain UiEvent → dispatch
2. drain_due(now) — AppTimer / Animation / IME
3. main_thread_queue.drain() — post_to_ui
4. tick_effects
5. reconcile（至多一次）
6. layout → render → present?
```

当前单窗实现顺序：

```mermaid
flowchart TD
  A[wait_event / wait_timeout(deadline)] --> B[drain UiEvent]
  B --> C[drain_due AppTimer / Widget Timer]
  C --> D[main_thread_queue.drain]
  D --> E{Active frame?}
  E -->|是| F[tree.update + tick_effects]
  E -->|否| G[跳过 update/effects]
  F --> H{Layout 脏?}
  G --> H
  H -->|是| I[tree.layout + on_frame]
  H -->|否| J{Paint/Composite 脏?}
  I --> J
  J -->|是| K[FrameRenderer.render_frame]
  J -->|否| L[跳过 present]
  K --> M{outcome}
  M -->|Present| N[presenter.present damage]
  M -->|Idle| L
  N --> O[tree.reset_dirty]
  L --> O
```

| 阶段 | 触发条件 | present? |
|------|----------|----------|
| Layout | `invalidation.has_layout()` 或 resize 等布局事件 | **否** |
| Paint/Composite | `tree.has_render_work()` | **是**（非 Idle） |
| Idle | 无绘制工作 | **否** |

**关键规则**（#59、rendering 系统）：

- Layout 失效 alone **不**触发 present。
- 首帧强制 full-frame dirty + layout。
- `reset_dirty()` 在帧末清空 InvalidationQueue。

### 事件轮询策略

设计（#106、#117）：**DeepIdle** 下 blocking `wait_event`（无 timeout）；**RegisteredActive** 由 `ActiveWorkRegistry::next_deadline` → `wait_until` 唤醒；**Active** 在事件 drain 后若无 pending 则回 DeepIdle。详见 [demand-driven · 唤醒源白名单](demand-driven.md#唤醒源白名单) · [ActiveWorkRegistry](demand-driven.md#activeworkregistry)。

> **实现注记**：单窗 loop 已用 Registry deadline 决定 `wait_event` / `wait_timeout(remaining)`；无 deadline 时 DeepIdle blocking；IME composition session 作为无 deadline 注册项保持 RegisteredActive 但不制造定时探活。AppTimer、内置 Timer、WidgetAnimation 下一帧 deadline 与 `Spin` 内置动画源已接入；其他内置组件动画源与多窗调度仍待接。

| 状态 | 设计 | 当前实现 |
|------|------|----------|
| DeepIdle | blocking `wait_event`；不 layout/render/tick Effect | 无 Registry deadline 时 blocking `wait_event` |
| RegisteredActive | `wait_until(next_deadline)` 窄 tick | `ActiveWorkRegistry::next_deadline` → `wait_timeout(remaining)` |
| Active / 动画中 | `tree.update` 返回 true → Registry 登记下一帧 deadline | Active 帧运行 `update` / `tick_effects`；`Spin` 已作为内置 Animation 源接入，其他动画源待接 |
| 首帧 | 单次 `poll_event` | 同左 |

### 窗口生命周期事件

Resize → `engine.resize` + layout invalidation。Maximize/Restore/Minimize → 更新可见性标志；Minimize 跳过后续 layout/render。

---

## 桥接

应用层负责把 `ui` 与 `draw` / `native` 接在一起，自身不含绘制算法。

### 事件映射

```text
Platform UiEvent
    → map_ui_event (application.rs)
    → SystemEvent
    → WidgetTree::dispatch_event
```

逻辑像素坐标（#70）；HiDPI scale 在 engine resize 时处理。

### ScenePaint 桥接

`WidgetTree` 实现 `ScenePaint`（`src/app/bridge/scene_paint.rs`），供 `FrameRenderer` 读取树结构与发起绘制：

| ScenePaint 方法 | WidgetTree 行为 |
|-----------------|-----------------|
| `node_frame` / `node_children` / `node_z_index` | 布局结果 |
| `node_dirty` | InvalidationQueue 查询 |
| `children_clip` / `scroll_offset` | Scroll 容器裁剪与偏移 |
| `hit_test` / `focused_node` | 事件与焦点环 |
| `paint(id, frame, ctx)` | 委托 `WidgetRender::render` |

`NodeId` ≡ `WidgetId`（`usize`）。

### 主题注入

全局 `Theme` 在 `PaintContext` 中作为 `&dyn TokenProvider` 传入各 Widget `render`。Theme 切换走 `SystemEvent::ThemeChanged` → palette-only invalidate（#9）。

### 辅助桥接

`TextRenderService` / `DebugRenderService`（`bridge/bridges.rs`）把 draw 层 FontService 适配为 ui trait，供需要文本测量的组件使用。

---

## AppState · 多窗 · Settings

### AppState（设计 #32、#55、#88、#101）

| 属性 | 规则 |
|------|------|
| 所有权 | App 单例持有 |
| 线程 | 仅主线程 |
| 多窗 | 共享同一 AppState（#93） |
| 访问 | 业务通过 `State<T>` + **ComponentHandle**（设计，#61、#72） |

**ComponentHandle**（设计）：只读配置字段；可 `invalidate` / `emit`；不可改 style 或读 hover/pressed 等交互态。

> **实现注记**（#101）：`ComponentHandle` 类型与 `emit` / `invalidate` 已导出，`invalidate()` 走窄 Paint；`ComponentConfigSnapshot` 类型与首批内置组件静态配置提取已接；`ComponentHandle::snapshot()` / `snapshot_fields()` 与首批只读配置 getter 已接。`AppState` 类型已导出，`WidgetTree::set_app_state` 后 mount/unmount 会自动注册/注销 snapshot，`AppState::get_handle` 可返回只读 snapshot handle。App 默认持有 `AppState`，`AppHandle::app_state()` 可访问同一 registry，单窗 `WindowSession` 已注入；跨窗共享与 lookup handle 的 live tree 绑定尚未接。当前业务数据仍可直接经 `State<T>` 闭包捕获，树节点键为 **WidgetId**（当前实现）而非设计态 **ComponentId**（#35、#101）。

### AppState · ComponentHandle 设计规格（设计 #32、#61、#72、#101、#145）

**#145**：AppState **不替代** `State<T>`；二者并存。`State<T>` 仍为响应式主路径；AppState 提供 **跨窗共享业务 struct** + **ComponentId → ComponentHandle** 查找。

**AppState**

| 项 | 规格 |
|----|------|
| 所有权 | 由 `App` 持有；`Arc<AppState>` 或 `Rc<RefCell<AppState>>` 模式 |
| 查找 | `get_handle(id: ComponentId) -> Option<ComponentHandle>` |
| 线程 | 仅主线程访问（#88） |
| 注册 | Reconciler **mount** 时框架自动 `register(id, config_snapshot)`；**App 不手写** |
| 卸载 | unmount 时自动 `unregister(id)` |
| 与 State | 业务可同时用 `State<T>` clone；AppState 不持有每个 State 实例 |

**ComponentHandle**

| 能力 | 规格 |
|------|------|
| 只读配置 | 组件声明的配置字段经类型化 getter 暴露（#72） |
| 失效 | `invalidate()` → 标记对应组件 **窄 Paint**（#119）；Layout 仅结构/约束变更 |
| 事件 | `emit(SemanticEvent)` → [event · Handle emit](event.md#handle-emit)（#147） |
| 禁止 | 不可改 style；不可读 hover / pressed 等交互态 |

```rust
// 伪代码 — 设计态 API，尚未完全落地
struct App { state: Arc<AppState>, /* ... */ }

impl AppState {
    fn get_handle(&self, id: ComponentId) -> Option<ComponentHandle> { /* ... */ }
    // 框架内部；App 不直接调用
    fn register(&mut self, id: ComponentId, snapshot: ComponentConfigSnapshot);
    fn unregister(&mut self, id: ComponentId);
}

struct ComponentHandle { id: ComponentId, /* weak ref to tree / registry */ }

impl ComponentHandle {
    fn label(&self) -> &str { /* 只读配置 getter */ }
    fn invalidate(&self) { /* mark dirty */ }
    fn emit(&self, event: SemanticEvent) { /* dispatch semantic */ }
}

// handler 闭包内 — 设计目标
button().on_click(|handle| {
    let n = handle.counter(); // 只读
    handle.emit(SemanticEvent::Custom(/* ... */));
    handle.invalidate();
});

// 当前过渡期 — 仍有效
button().on_click(move || count.set(count.get() + 1));
```

| 迁移 | 说明 |
|------|------|
| v1 现在 | `State<T>` 闭包捕获；`AppState` snapshot registry 与单窗 App 默认注入已接，跨窗共享待接 |
| 落地后 | 新组件优先 `ComponentHandle`；旧代码无需改即可运行 |
| 零维护 | register/unregister 由框架 mount 路径自动完成（#145） |

### 多窗（设计 #93–#94、#116）

| 项 | 规则 |
|----|------|
| 共享 | AppState + Theme 全局一份 |
| 独立 | 每窗 **WindowSession**：WidgetTree + 引擎 + 三态 + **ActiveWorkRegistry** |
| 编排 | 单 `run_app_loop`；UiEvent 带 **window_id** 路由至目标 session |
| Present | 各窗独立 presenter |

```rust
struct WindowSession {
    window_id: WindowId,
    tree: WidgetTree,
    engine: Box<dyn GraphicsEngine>,
    loop_state: WindowLoopState,
    active_work: ActiveWorkRegistry,
}
```

详见 [demand-driven · 多窗单 loop](demand-driven.md#多窗单-loop)。副窗创建见 [open_window](#open_window)（#144）。

> **实现注记**：`App::run_gui` 当前仅创建单窗；native 层 `IWindowManager` 已支持多窗，应用编排待扩展。

---

## open_window

设计（#144）— 运行中创建 **副窗** 并取得该窗 **AppHandle**。

```rust
pub struct WindowConfig {
    pub title: String,
    pub width: i32,
    pub height: i32,
    pub root: Box<dyn Fn() -> ViewNode + Send>,  // 或 View 类型
}

impl AppHandle {
    /// 主线程调用；创建 WindowSession + native 窗；返回 **新窗** handle
    pub fn open_window(&self, config: WindowConfig) -> Result<AppHandle, AppError>;
}

impl App {
    /// 可选：副窗创建后回调（等同 on_start，仅新 session）
    pub fn on_window_start<F>(mut self, f: F) -> Self
    where
        F: Fn(AppHandle) + Send + Sync + 'static;
}
```

| 规则 | 说明 |
|------|------|
| 线程 | **主线程**；与 `post_to_ui` / Timer 同约束（#88） |
| 返回值 | 新 `AppHandle { window_id: 新 id }`；**主窗 handle 不变** |
| 生命周期 | 副窗 close → cancel 该 session Timer + unregister（#134） |
| on_start | 首窗仍走 `.on_start`（#140）；副窗走 `on_window_start` 或 `open_window` 后立即使用返回值 |
| 共享 | AppState + Theme + 全局 `State<T>` **跨窗共享**（#93、#145） |
| DeepIdle | 创建本身 register 仅在新 session 首帧；不全局 wake 所有窗 |
| 错误 | native `create_window` 失败 → `Err`；不泄漏半成品 session |

### 示例

```rust
App::new()
    .on_start(|primary| {
        primary.open_window(WindowConfig {
            title: "Inspector".into(),
            width: 320,
            height: 600,
            root: Box::new(|| inspector_view()),
        }).map(|inspector| {
            // inspector.window_id != primary.window_id
            inspector.run_after(Duration::from_secs(1), || { /* ... */ });
        });
    })
    .on_window_start(|handle| {
        // 每个 open_window 成功后也会调用（若 builder 设置了）
    })
    .root(|| main_view())
    .run();
```

> **实现注记**：`WindowConfig`、`AppHandle::open_window` 与 `.on_window_start` 已导出；`open_window` 当前会分配新 `window_id`、独立 AppTimer / MainThreadQueue / AppHandle，并把副窗创建请求暂存到 `AppRuntime`。GUI loop 会在首窗 `.on_start` 后与活动轮次中 drain 请求，创建 native 窗、校验 native `window_id`、构造独立 `WindowSession` 并调用 `.on_window_start`；副窗 MainThreadQueue / `update_view` reconcile 已消费，副窗事件已按 `window_id` 路由，副窗渲染帧循环与完整多 session loop drain 仍待接入。

### 副窗 bootstrap（#148）

`open_window` 内部顺序 — **不**复用主窗 `WidgetTree` / Reconciler 状态：

```text
open_window(config) [主线程]
  1. platform.window_manager().create_window(title, w, h)
  2. create_preferred_engine() → 绑定新窗
  3. new WindowSession {
       view_factory: Arc::new(config.root),  // #155：副窗独立 factory
       ...
     }
  4. ViewAdapter::build_nodes((view_factory)())  → 仅写入 **本 session.tree**
  5. tree.layout() + mark_full_frame_dirty（首帧）
  6. sessions.insert(window_id, session)
  7. on_window_start?(new AppHandle { window_id })
  8. 新 session 进入 Active（首帧 present）；其他 session 不受影响（#110）
```

| 规则 | 说明 |
|------|------|
| 独立 tree | 每窗 **独立** WidgetTree + HandlerTable + InvalidationQueue |
| 独立 reconcile | `ViewAdapter::reconcile(session.tree, view)` **按 session**；无跨窗 diff |
| 共享 | Theme、AppState、全局 `State<T>`、FontService（#93–#94） |
| View 更新 | 副窗 root 变更 → 仅 reconcile **该 session**；主窗 tree 不变 |
| Effect | 各 session `tree.effects` **独立**；同名 State 依赖可跨窗共享 |
| 关闭 | 移除 session + destroy native 窗 + cancel AppTimer（#134） |

```rust
// WindowConfig.root — 与 App::root 同型
pub root: impl Fn() -> ViewNode + Send + 'static;
// 或 Box<dyn View>；expand 后走同一 ViewAdapter 路径
```

详见 [view-reactive · 多窗 Reconcile](#多窗-reconcile)（#148）。

---

## Settings（#64）

`SettingsService` 由 App **可选**注入；**默认不**自动 load/save。`App::settings(path)` 会在 `run()` 进入 GUI/CLI 模式前加载一次并注册到 App DI；持久化 key 如 `theme_mode`、`brand_primary` 可由业务解析为 `Theme`；缺文件用 DefaultTheme（#48）。

Light/Dark 默认跟 OS（#74）于**启动**（可读 Settings / OS）。

**运行中**（#125）：

| `.follow_system_theme` | 行为 |
|------------------------|------|
| **false**（默认） | 不跟 OS；仅 `.theme(...)` 切换；ThemeChanged **忽略** |
| **true** | 框架收 ThemeChanged → 更新 DynTokens → **全 WindowSession** palette invalidate |

App **无需**手写 ThemeChanged handler（opt-in 时）。

**异步边界**（#131、#132、#133）：禁止裸 Registry 与轮询 Effect；允许 **`run_after` / `run_interval`**、**`post_to_ui`** 与 async→State。详见 [demand-driven · UI 主循环 vs 后台](demand-driven.md#ui-主循环-vs-后台) · [post_to_ui](#post_to_ui) · [App Timer API](#app-定时-api)。

---

## App 定时 API

设计（#132）— App 层 **公开** 定时能力；框架内部 register，满足 #105 零闲置。

```rust
impl App {
    /// 延迟一次：到期在 **主线程** 执行 `f`，随后自动 unregister
    pub fn run_after<F>(&self, delay: Duration, f: F) -> TimerHandle
    where
        F: FnOnce() + 'static;

    /// 固定间隔重复：每次在 **主线程** 执行 `f`；`cancel` 或 drop 停止
    pub fn run_interval<F>(&self, interval: Duration, mut f: F) -> TimerHandle
    where
        F: FnMut() + 'static;
}

pub struct TimerHandle { /* opaque; 不暴露 Registry */ }

impl TimerHandle {
    /// 立即停止并 unregister
    pub fn cancel(self);
}
// Drop 等价于 cancel
```

| 规则 | 说明 |
|------|------|
| 线程 | 回调 **仅主线程**（#88）；可直接 `State::set` |
| 内部 | `ActiveWorkKind::AppTimer` → ActiveWorkRegistry（#124）；App **不**直接 register |
| DeepIdle | 无 pending Timer 且无其他 register → blocking wait |
| 唤醒 | `wait_until(min(next_deadline))`（#127）；**非**固定 100ms 探活 |
| 绘制 | 回调 **不**默认 layout/render；`State::set` 后按需 reconcile |
| 多窗 | 绑定 `AppHandle.window_id` 对应 **WindowSession**（#141）；不跨窗 wake |

### 用法示例

```rust
// 每 30 秒自动保存
let _autosave = app.run_interval(Duration::from_secs(30), || {
    save_draft();
});

// 500ms 后显示提示
app.run_after(Duration::from_millis(500), || {
    toast_visible.set(true);
});

// 停止
autosave.cancel();
```

### 与内置组件的关系

| 场景 | 推荐 |
|------|------|
| 可见周期 UI（Spin、Tooltip 延迟、光标） | **内置 widget**（组件内 register） |
| 业务逻辑（保存、轮询刷新、倒计时数据） | **#132 Timer API** 或 async→State |
| 耗时 IO | async / 线程 → 主线程 `State::set` |

> **实现注记**：`App::run_after` / `run_interval` / `TimerHandle` 与 `AppHandle::run_after` / `run_interval` 已导出；`AppHandle` 经 `AppRuntime` 按 `window_id` 路由到所属 session 的 AppTimer 队列，并可在 session 关闭时批量 cancel。副窗 session bootstrap 已接，副窗 Timer 帧循环消费仍待接。

### 调用入口

| 时机 | API |
|------|-----|
| **Builder** | `App::new().run_after(...)` — 随 `run()` 进入主循环后生效 |
| **运行中** | `AppHandle::run_after` / `run_interval`（设计）— `run()` 后由 builder 注入或通过 `State`/Effect 捕获的 `AppHandle` clone |

`AppHandle` 为 `App` 的轻量 cloneable 句柄（设计）；**不**暴露 Registry 或主循环内部状态。多窗时 Timer 绑定 **创建时所在 WindowSession**（#116）。生命周期见 [AppHandle 生命周期](#apphandle-生命周期)。

---

## post_to_ui

设计（#133）— async / 后台线程完成 IO 后，将 UI 更新投递到主线程。

```rust
impl App {
    /// 排入主线程队列；与 Timer 回调同线程约束（#88）
    pub fn post_to_ui<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static;
}

impl AppHandle {
    pub fn post_to_ui<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static;
}
```

| 规则 | 说明 |
|------|------|
| 用途 | 后台 fetch / 文件 IO / channel 收到结果 → 主线程 `State::set` |
| wake | 投递 **不** register；闭包内 `State::set` 才进入 Active |
| 禁止 | 闭包内直接改 WidgetTree、HandlerTable 或裸 invalidate |
| 多窗 | 见 [多窗 post_to_ui](#多窗-post_to_ui)（#141） |

### 示例

```rust
std::thread::spawn({
    let handle = app_handle.clone();
    let state = counter.clone();
    move || {
        let n = expensive_fetch();
        handle.post_to_ui(move || state.set(n));
    }
});
```

### 多窗 post_to_ui（#141）

`AppHandle` 携带不可变 **`window_id`**；`post_to_ui` **仅**写入该窗 `MainThreadQueue`。

```rust
pub struct AppHandle {
    window_id: WindowId,  // opaque；创建时绑定
    // ...
}
```

| 规则 | 说明 |
|------|------|
| 路由 | `handle.post_to_ui(f)` → `sessions[handle.window_id].main_thread_queue` |
| 跨窗 | **禁止**用 A 窗 handle 更新 B 窗 UI；须持有 **B 的 AppHandle** clone |
| 共享 State | 全局 `State<T>` 可跨 session `set`；标脏 **fan-out** 至各 bind site（#150） |
| session 已关闭 | 闭包 **丢弃**（debug log）；不 panic、不跨窗 fallback |
| 后台线程 | 捕获 **目标窗** handle；多窗时各窗独立 clone |

```rust
// ✓ 各窗独立 handle
let handle_a = handles.get(WindowId(0)).clone();
let handle_b = handles.get(WindowId(1)).clone();
handle_a.post_to_ui(move || state_for_a.set(v));

// ✗ 禁止：用 A handle 期望更新 B 的树
```

> **实现注记**：`App::post_to_ui`、`AppHandle::post_to_ui` 与 `WindowSession.main_thread_queue` 已落地；`AppHandle` 经 `AppRuntime` 按 `window_id` 仅写入目标 session 队列，并在 session 销毁后丢弃闭包。单窗 loop 已在 UiEvent / due work 后、`tick_effects` 前 drain；副窗 MainThreadQueue 已在运行时任务 hook 中消费，副窗事件已按 `window_id` 路由；阻塞等待中的真实 wake 与副窗渲染帧循环尚未接。

---

## MainThreadQueue

设计（#137）— 与 [demand-driven · MainThreadQueue](demand-driven.md#mainthreadqueue) 一致。

每 `WindowSession` 持有一个 `MainThreadQueue`；`post_to_ui` 入队，`run_active_frame` 步骤 3 `drain`。

| 阶段 | 内容 |
|------|------|
| 1 | UiEvent dispatch |
| 2 | `drain_due` — AppTimer / Animation |
| 3 | `main_thread_queue.drain` — post_to_ui |
| 4+ | tick_effects → reconcile → layout → render |

入队 **不** register ActiveWork；队列空且其余 pending 清空后可回 DeepIdle。

> **实现注记**：`MainThreadQueue` 已实现 FIFO drain，并由 `WindowSession` 持有；`AppRuntime` 已按 `window_id` 路由投递，独立 session 关闭会清空队列。副窗 session bootstrap、MainThreadQueue 消费与事件路由已接，副窗完整 loop 消费尚未接。

---

## AppHandle 生命周期

设计（#134）— `AppHandle` / `TimerHandle` 与 `WindowSession` 的归属与销毁。

```text
App::run()
  ├─ 创建 WindowSession(s)
  ├─ 注入 AppHandle（cloneable；共享 event loop 弱引用）
  └─ 主循环直至全部窗关闭或 quit

WindowSession 销毁（窗 close）
  └─ cancel 该 session 下 **全部** AppTimer（#132）；unregister Registry

App::run() 返回
  └─ cancel **剩余** 全局 AppTimer；drain 主线程队列一次（可选）

AppHandle drop
  └─ **不** cancel 已注册 Timer（句柄仅为投递入口）

TimerHandle drop / cancel
  └─ 立即 unregister；无其他 Timer → 该 session 可回 DeepIdle
```

| 对象 | drop / 销毁行为 |
|------|----------------|
| `TimerHandle` | cancel + unregister（#132） |
| `AppHandle` | 无 side effect；Timer 仍存活直至 cancel 或 session 结束 |
| `WindowSession` | cancel 本 session 全部 AppTimer |
| `App::run()` 结束 | cancel 所有未结束 AppTimer |

多窗：A 窗关闭 **不** cancel B 窗 Timer；各 session Registry 独立（#116）。

### 获取方式

`App::run()` 阻塞主线程；运行中 Timer / `post_to_ui` 须通过 **`on_start`**（#140）或等价注入取得 `AppHandle`。

---

## on_start

设计（#140）— 运行中 API 的 **canonical** 注入点。

```rust
impl App {
    /// 每个 WindowSession 创建后、进入主循环首帧前调用一次
    pub fn on_start<F>(mut self, f: F) -> Self
    where
        F: Fn(AppHandle) + Send + 'static;
}
```

| 规则 | 说明 |
|------|------|
| 调用时机 | `WindowSession` + `AppHandle` 就绪后；**首帧 / blocking wait 前** |
| 多窗 | **每窗各调一次** `f(handle)`；handle 的 `window_id` 不同（#141） |
| 线程 | **主线程**同步调用；`f` 内可立即 `run_after` / 写 `State` |
| 与 builder Timer | builder 阶段 `run_after` 排队；**on_start 后**再 register |
| 替代 | 根 View 工厂接收 `AppHandle` 仍可用；**推荐** `on_start` + `State` 槽 |

### 示例

```rust
let app_handle = State::<Option<AppHandle>>::new(None);

App::new()
    .on_start({
        let slot = app_handle.clone();
        move |handle| slot.set(Some(handle))
    })
    .root(move || {
        let h = app_handle.get().unwrap().clone();
        column([ /* 闭包内 h.run_after(...) */ ])
    })
    .run();
```

**禁止**在 `run()` 返回后再调用 Timer / `post_to_ui`（主循环已结束）。

> **实现注记**：单窗 `.on_start(AppHandle)` 与 `AppHandle` / `WindowId` 已导出；`AppHandle::resolve<T: Clone>()` 可读取运行期 DI 单例 clone；多窗每窗注入待接。

> **实现注记**（#134）：`TimerHandle` 已落地并支持 `cancel` / drop unregister；`AppRuntime::close_session` 会关闭指定 handle、cancel 该 session AppTimer、清空 MainThreadQueue 并移除待创建副窗请求。真实窗口 close → session 销毁路由仍待接。

---

## update_view

设计（#149）— 运行中替换 **本 session** 根 View 并 reconcile。

```rust
impl AppHandle {
    /// 替换本窗 root View；帧末 reconcile 一次（#118）
    pub fn update_view<F>(&self, build_root: F)
    where
        F: FnOnce() -> ViewNode + Send + 'static;

    ///  sugar：传入 `impl View`
    pub fn set_root<V: View + 'static>(&self, view: V) {
        self.update_view(move || view.build());
    }
}
```

| 规则 | 说明 |
|------|------|
| 作用域 | **仅** `self.window_id` 对应 `WindowSession.tree`（#148） |
| 线程 | **主线程**（#88） |
| reconcile | 写入 `session.pending_root` → 本帧 Active **末** `ViewAdapter::reconcile` **一次**（#118） |
| 与 factory | **不**替换 `view_factory`（#156）；消费 `pending_root` 后 State 批次仍走 builder `.root` 闭包 |
| 与 State | `State::set` 触发的 reconcile **同 session 合并**；无 `pending_root` 时调用 `view_factory`（#155） |
| 与 open_window | 副窗创建后可用 **该窗** handle `update_view`；主窗 handle **不能**改副窗树 |
| DeepIdle | 仅替换 pending root **不** wake；reconcile 后若有 paint/layout 才 Active |

```rust
inspector_handle.update_view(|| inspector_panel_v2(data.get()));
```

详见 [view-reactive · reconcile 合并](view-reactive.md#reconcile-合并)（#153）与 [view_factory 生命周期](view-reactive.md#view_factory-生命周期)（#155–#156）。

> **实现注记**：`AppHandle::update_view` / `set_root` 已导出，并经 `AppRuntime` 按 `window_id` 投递到目标 `MainThreadQueue` 写入 `pending_root`；响应式 `State` 批次会自动置位；单窗主循环会在 `tick_effects` 后、layout/render 前至多 reconcile 一次。副窗 session bootstrap 与 root reconcile 消费已接，副窗渲染帧循环待接。

---

## CLI 与 DI

### CLI

`Cli` 解析 `argv`：

- 首个非选项参数 = 子命令名
- `--key=val` / `-k val` → options map
- `help` / `-h` → 打印已注册命令

`CommandHandler = fn(&CliArgs) -> i32`；未匹配命令走 default handler 或报错。

### DI Container

```rust
Container { singletons: HashMap<TypeId, Box<dyn Any + Send>> }
```

API：`singleton<T>()`、`resolve<T>()`、`resolve_mut<T>()`、`has<T>()`、`remove<T>()`。

用途：App 级服务注册（Settings、自定义 repo 等）。**不参与 UI 热路径**；启动前可由业务在 builder 阶段自行 `resolve`，运行中可通过注入的 `AppHandle` resolve clone。

> **实现注记**：`Container` 已随 `AppHandle` 注入运行期；`AppHandle::resolve<T: Clone>()` 可读取 builder `.singleton()` 与 `.settings()` 注册的单例 clone。组件 layout/render/event 热路径仍不主动 resolve；副窗 bootstrap 已复用同一 container 注入新窗 handle。

---

## Window 类型

`Window`（`src/app/window/window.rs`）是对 `PlatformWindow` 的轻量包装：create / show / hide / close / run。不参与 layout/render；Present 由主循环在 `FrameRenderer` 成功后触发。

---

## 源码模块

```text
app/
├── shell/
│   ├── application.rs   App builder, run_gui, map_ui_event
│   ├── cli.rs           CLI 解析与命令分派
│   └── di.rs            Container 单例 DI
├── event_loop/
│   └── event_loop.rs    run_widget_loop, 帧调度
├── bridge/
│   ├── scene_paint.rs   WidgetTree → ScenePaint
│   └── bridges.rs       TextRenderService, DebugRenderService
└── window/
    └── window.rs        Window 轻量包装
```

详见 [Main · 源码目录详表](../Main.md#源码目录详表)。
