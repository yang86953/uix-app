# View 与响应式系统

← [Main](../Main.md) · 系统 **#2** · 功能域：`ui`

> 声明式 View 描述 UI；State 驱动重建与 diff。业务**仅**通过 View DSL 创建 UI（#21）。

## 索引

| 主题 | 章节 | 决策 |
|------|------|------|
| 组合器 API | [View DSL](#view-dsl) | #21 #67 #73 |
| 树同步 | [Reconciler](#reconciler) · [Handler 变更判定](#handler-变更判定-135) | #49 #60 #62 #123 #135 #138 #142 #101 |
| 响应式原语 | [响应式](#响应式) · [StateSlotId](#stateslotid) | #24 #78 #79 #143 |
| 样式链 | [StyleExt](#styleext) | #21 |
| 热更新 | [热更新（设计）](#热更新设计) · [reconcile 合并](#reconcile-合并) · [view_factory 生命周期](#view_factory-生命周期) · [多窗 Reconcile](#多窗-reconcile) · [State 跨窗标脏](#state-跨窗标脏) | #49 #60 #118 #148 #149 #150 #153 #155 #156 |

**关联**：[component](component.md) · [event](event.md) · [theme-style](theme-style.md) · [application](application.md) · [demand-driven](demand-driven.md)

---

## View DSL

用户层 API 位于 `src/ui/view/`。原则：**不暴露** WidgetTree / BoxedWidget / WidgetNode 内部概念（#21）。

### 核心类型

| 类型 | 职责 |
|------|------|
| `View` | `'static` trait；`build() -> ViewNode` |
| `ViewNode` | 中间树：widget + children + style + z_index + key + handlers |
| `ViewAdapter` | ViewNode → WidgetTree 桥接与 Reconciler |

### 组合器

| 组合器 | 产出 | 备注 |
|--------|------|------|
| `column([...])` / `row([...])` | Container | flex 方向由 Style 控制 |
| `label("...")` / `dynamic_label(closure)` | Label | 后者绑定 State |
| `button("...").primary().on_click(...)` | Button | StyleSet 预设链式 |
| `input("...").on_change(...)` | Input | |
| `grid([...]).columns([...])` | Grid | #67 |
| `scroll([...]).vertical()` | ScrollView | #73 |
| `space()` | Space | 占位 |

Builder 类型（`ButtonBuilder` 等）实现 `View`，支持链式修饰。

### ViewNode 构建

```rust
ViewNode::leaf(widget)
ViewNode::new(widget, children)
    .key("stable-id")
    .on_semantic(SemanticKind::Click, handler)
```

`IntoWidgetNode` 实现调用 `ViewAdapter::expand` 递归展开子树。

### DynamicLabel 与 State 捕获

构建阶段：

1. `ViewAdapter::begin_state_capture()`
2. `view.build()` — DynamicLabel 内 `State::get()` 注册 pending bind
3. `ViewAdapter::end_state_capture()`

布局后 `bind_reactive_widget_states` 把 DynamicLabel 的 State 绑定到 paint invalidation。

---

## Reconciler

> **术语（[#101](../decisions.md#d101)）**
>
> - **ComponentId**（设计，#35、#101）：Generational ID；Reconciler diff 复用依据。
> - **WidgetId**（当前实现，#101）：`usize` + free list；源码与 WidgetTree 使用此名。
> - **HandlerTable** 键：设计 **ComponentId** / 当前 **WidgetId**（#10、#101）。
> - **Handler 重绑**（#123、#135，修订 #62）：reconcile 时 **仅 handler 变更** 才 `clear_component`+重注册；未变则保留。
> - **AppState** / **ComponentHandle**（设计，#32）：当前业务数据经 `State<T>` 闭包捕获。

实现：`ViewAdapter`（`src/ui/view/adapter.rs`）。策略：声明式重建 + diff，复用稳定 ID（#31、#49、#60）。

### 入口

| API | 行为 |
|-----|------|
| `build(view)` | 捕获 → 新建 WidgetTree → expand → build |
| `reconcile(tree, view)` | 增量更新或整树重建 |

> **实现注记**：`reconcile` 已实现，App 启动仍用 `build_nodes`；热更新路径待接入主循环。

### 根节点决策

```text
if root 存在 AND can_reuse(旧根, 新根):
    reconcile_existing(根)
else:
    tree.build(expand(新根))    // 整树重建
bind_orphan_pending_states()
bind_pending_effects()
```

`can_reuse`：旧节点与新 ViewNode 的 widget **TypeId 相同**。

### reconcile_existing（同类型就地更新）

1. `apply_style` + `patch_widget`（类型感知同步）
2. 更新 `key`、`z_index`
3. **handler 智能重绑**（#123）— 仅当 #60 判定 handler 变更时 `clear_component` + 重注册；未变则 **跳过**
4. `invalidate_paint(id)` + layout invalidation + 向上传播
5. `reconcile_children`

### patch_widget 策略

| 类型 | 策略 |
|------|------|
| Container | 就地复制 style |
| Label / Button / Input | `sync_from` 就地同步字段 |
| Grid | `replace_component` |
| 类型不匹配 | `replace_component` |

Style / 文本 / 静态 props 比较（#60）：Style 变 → paint invalidate；handler **智能重绑**（#123，修订 #62）。

### reconcile_children（keyed diff）

1. 旧子节点按 `key` 建索引
2. 遍历新子节点（保序）：
   - **有 key**：按 key 匹配；匹配且 `can_reuse` → reconcile；否则 remove + 新建
   - **无 key**：同层索引匹配（仅未 key 的旧节点）
3. 移除未匹配旧子节点
4. 子序变化 → 更新 parent children + `tree_version += 1`

### key 规则（#49）

| 情况 | 匹配方式 |
|------|----------|
| 双方有 key | 字符串相等 |
| 无 key | 同层索引 |
| 类型不同 | 卸载旧 subtree，新建 设计态 ComponentId（#35、#101） |

---

## Handler 变更判定（#135）

#123 智能重绑的 **可实现判定规则**（修订 #60、#62）。

View **每次 build** 为带 handler 的 ViewNode 分配递增 **`handler_generation: u32`**（同 reconcile 周期内稳定）。Reconciler 在 `reconcile_existing` 步骤 3 比较：

```text
handlers_changed(old, new) :=
    old.semantic_kinds() != new.semantic_kinds()   // 集合相等（顺序无关）
    OR ∃ kind ∈ intersection:
         old.generation(kind) != new.generation(kind)
```

| 比较项 | 规则 |
|--------|------|
| **SemanticKind 集合** | `Click`、`Changed`、`Submit`… 增删 → **变更** |
| **handler_generation** | 同 kind 代际不同 → **变更**（含闭包 capture 变化导致 rebuild） |
| **闭包指针 / TypeId** | **不**比较；避免误杀稳定 handler |
| **静态 props / Style** | 走 #60 paint/layout 路径；**不**触发 clear_component |

代际 bump 时机（View build）：

| 事件 | generation |
|------|------------|
| 同 kind 闭包重新创建（新 capture） | +1 |
| 仅 Style / 文本变 | 不变 |
| kind 移除后同 slot 新 kind | 新 kind 从 0 起 |

`handlers_changed == false` → **跳过** `clear_component` + 重注册；`true` → 清空该 ComponentId/WidgetId 全部 handler 再注册新表。

测试：勿断言闭包指针跨 rebuild 稳定；可断言 **同 generation + 同 kind 集** 时 HandlerTable 条目保留（见 [testing · 语义断言](testing.md#语义断言)）。

### handler_generation 作者化（#138）

**调用方不可配**；由 View DSL / `ViewAdapter` build 阶段 **自动**维护。

```rust
// ViewNode 内部（设计）
struct HandlerSlot {
    kind: SemanticKind,
    generation: u32,
    // 闭包本体不暴露给 Reconciler 比较
}

struct BuildContext {
    next_generation: HashMap<(NodePath, SemanticKind), u32>,
    capture_fingerprint: HashMap<(NodePath, SemanticKind), u64>,
}
```

| 规则 | 说明 |
|------|------|
| `.on_click(f)` 等宏 | build 时写入 `HandlerSlot { kind, generation }` |
| capture 指纹 | 对闭包捕获的 `State` 句柄 / 静态 env 做 **稳定 hash**（#138） |
| 指纹不变 | **复用**上代 `generation`（同 reconcile 周期内闭包重建但 capture 相同） |
| 指纹变化 | 该 kind `generation += 1` |
| kind 新增/移除 | 更新 SemanticKind 集合（#135） |
| App API | **无** `set_handler_generation`；零维护 |

实现落点：`src/ui/view/build_context.rs`（设计）；`button().on_click(...)` 等链式 API 在 `expand` 时更新 slot。

### capture 指纹字段（#142）

`capture_fingerprint = hash(fields)` → `u64`；**仅**下列字段参与；顺序固定：

| 捕获类型 | 参与 hash 的字段 | 不参与 |
|----------|------------------|--------|
| `State<T>` | `TypeId::of::<T>()` + **`StateSlotId`**（#143） | 当前值、`generation()`、指针地址 |
| `AppHandle` | `window_id` | 句柄内部指针 |
| `Copy` 标量 / 枚举 | `discriminant` + `bits()` / 各字段 | — |
| `&'static T` | `TypeId::of::<T>()` | 运行时地址 |
| 闭包本体 | — | **fn 指针、vtable、堆地址** |
| 未捕获 | — | — |

```text
fingerprint(kind, captures) :=
    hash((
        kind as u16,
        sorted capture entries by TypeId tag,
        per-entry payload per table above,
    )) → u64
```

| 规则 | 说明 |
|------|------|
| 指纹相等 | 复用上代 `generation`（#138） |
| 指纹不等 | `generation += 1` |
| 稳定性 | 同源码 rebuild、同 capture 集 → **同指纹**（测试可断言） |
| 算法 | 框架内部固定（如 `FxHasher` → `u64`）；App **不可配** |

> **实现注记**：`handler_generation` 字段与指纹逻辑尚未落地；当前 reconcile 仍全清 handler。`StateSlotId`（#143）尚未分配；落地前指纹实现不得误用 `generation()`。

---

## StateSlotId

设计（#143）— `State<T>` 的 **稳定身份**，供 capture 指纹（#142）、测试与调试；**不同于** `StateInner::generation`（值变更计数，供 Computed/Effect）。

```rust
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct StateSlotId(u64);

struct StateInner<T> {
    slot_id: StateSlotId,   // State::new 时分配，clone 共享
    value: T,
    generation: u64,        // set/update 递增；**不**参与 capture 指纹
    watchers: Vec<...>,
}

static NEXT_STATE_SLOT: AtomicU64 = AtomicU64::new(1);
```

| 规则 | 说明 |
|------|------|
| 分配 | 每个 `State::new` → `slot_id = NEXT_STATE_SLOT.fetch_add(1)` |
| clone | `State::clone` **共享**同一 `slot_id`（同一逻辑状态） |
| 指纹 | #142 使用 `TypeId::of::<T>() + slot_id.0` |
| 不用 generation | `generation()` 随 `set` 变化；用于 Computed/Effect **依赖追踪**，非身份 |
| Computed | 设计态可分配独立 slot id；v1 指纹以捕获的 `State` slot 为准 |

### 与当前实现差距

| 设计（#143） | 当前 `state.rs` |
|--------------|-----------------|
| `StateSlotId` 字段 | 无；`Arc` 共享 inner |
| 指纹用 slot id | 尚未实现 capture 指纹 |
| `generation()` | 已有；用于 Computed/Effect |

落地 #143 时 **保留** 现有 `generation()` 语义；仅 **新增** `slot_id` 字段与 accessor。

> **实现注记**：`StateSlotId` 未落地；`State::generation()` 已存在但含义不同。

---

## 响应式

| 原语 | 行为 | 决策 |
|------|------|------|
| **State\<T\>** | 变更 → 绑定 widget paint invalidate | #24 |
| **Computed** | 依赖追踪，读时重算 | #78 |
| **Effect** | **Active** 态且依赖变化时执行；**DeepIdle 不 tick**（#105）；须经 State 间接改 UI | #79 |

### State 绑定路径

```text
View build 阶段 State::get()
    → pending_state_binds
    → bind_orphan_pending_states (根) / bind_reactive_widget_states (布局后)
    → invalidate_paint_handle(widget_id, rect)
    → （设计 #150）register PaintBindSite { window_id, widget_id, queue, rect }
```

跨窗 fan-out 见 [State 跨窗标脏](#state-跨窗标脏)（#150）。

### Effect 约束

Effect 闭包 `'static`；禁止在 Effect 内直接操作 WidgetTree 或改 layout。**禁止**轮询式 Effect（#131）。

周期逻辑：**App Timer API**（#132）或 async→主线程 `State::set`。见 [application · App 定时 API](application.md#app-定时-api) · [demand-driven · UI 主循环 vs 后台](demand-driven.md#ui-主循环-vs-后台)。

DeepIdle **不** `tick_effects`（[#105](decisions.md#d105)）。UiEvent 或 State 变更唤醒进入 Active 后，再检查依赖并执行 Effect。详见 [demand-driven · Effect 与 Theme](demand-driven.md#effect-与-theme)。

### 业务数据

Handler 须 `'static`（#26）；业务数据经 **AppState** + **ComponentHandle**（设计，#32）或闭包捕获 `State` clone。组件 ID 与 HandlerTable 键术语见 [Reconciler · 术语](#reconciler)。

---

## StyleExt

任何 `Into<ViewNode>` 的类型自动获得链式样式方法（`view/mod.rs`）：

```rust
column([...])
    .gap(12.0)
    .padding(16.0)
    .background(ColorValue::neutral(NeutralRole::BgContainer))
```

覆盖 Style 字段：margin/padding、flex/grid、border、opacity、color、font_size 等。颜色应走 `ColorValue`（见 [theme-style](theme-style.md)）。

---

## 热更新（设计）

State / View 变更后，应用层应调用 **`ViewAdapter::reconcile(tree, view)`** 做增量 diff，而非每次 `build` 整树重建。

### 触发点

| 时机 | 行为 |
|------|------|
| **State invalidate 批次结束** | Active 帧内多次 `State::set` **合并** → 帧末 **一次** reconcile（#118） |
| **显式 App API** | `AppHandle::update_view`（#149）或 builder root 替换 |

### 帧内合并（#118）

与 [application · 主循环](application.md#主循环) · [demand-driven · 帧内合并](demand-driven.md#帧内合并) 对齐：

```text
dispatch / drain_due / tick_effects
    → reconcile（至多一次）
    → layout → render
```

禁止在 handler 或 Effect 内嵌套触发 reconcile；同帧多次 State 变更 coalesce 为单次 diff。合并算法见 [reconcile 合并](#reconcile-合并)（#153）。

### reconcile 合并（#153）

每 `WindowSession` 在帧末 **至多一次** reconcile；合并 `State::set` 批次与 `update_view` 的 `pending_root`。

### view_factory 生命周期（#155–#156）

`view_factory` 是 session 级 **长期**根 View 来源；`pending_root` 是 **单次**覆盖。

```rust
struct WindowSession {
    tree: WidgetTree,
    view_factory: Arc<dyn Fn() -> ViewNode + Send + Sync>,  // #155：创建时固定
    pending_root: Option<ViewNode>,   // #156：update_view 写入；优先、一次性
    reconcile_pending: bool,
}
```

| 时机 | 行为 |
|------|------|
| **主窗** `App::run_gui` | 将 `.root(f)` 捕获为 `Arc::new(f)` 存入 session（#155） |
| **副窗** `open_window` | `WindowConfig.root` **独立** factory；与主窗 **不共享** Arc（#148 #155） |
| **会话内** | factory **不可变**；无 API 替换 factory |
| **State 批次** | `reconcile_pending` 且 `pending_root` 为空 → `(view_factory)()` |
| **update_view** | `pending_root = Some(f())`；**不**改 factory（#156） |
| **帧末 reconcile** | `pending_root.take().unwrap_or_else(|| (view_factory)())` |
| **消费后** | 下一帧 State reconcile 回到 factory；`update_view` 须再次调用才再覆盖 |

```text
// 帧内（handler / Effect / update_view）
State::set        → session.reconcile_pending = true
update_view(f)    → session.pending_root = Some(f())
                  → session.reconcile_pending = true

// 步骤 5 — run_active_frame
if session.reconcile_pending {
    let view = session.pending_root.take()
        .unwrap_or_else(|| (session.view_factory)());
    ViewAdapter::reconcile(&mut session.tree, view);
    session.reconcile_pending = false;
}
```

| 规则 | 说明 |
|------|------|
| 优先级 | **`pending_root` > `view_factory`**；同帧多次 `update_view` → **最后一次** root |
| factory 不变 | `update_view` **永不**替换 `view_factory`；长期 State 驱动仍依赖 builder `.root` 闭包 |
| State 批次 | 多次 `set` 仅置位一次；无 `pending_root` 时用 **同一** factory 重建 ViewNode |
| 与 Effect | Effect tick **后** reconcile；Effect 内 `set` 并入本帧 |
| 禁止 | reconcile 内再次 `set` 触发 **嵌套** reconcile（同帧 coalesce 已合并） |
| 多窗 | 各 session **独立** factory / `pending_root` / `reconcile_pending`（#148） |
| layout | reconcile 后若结构变 → layout 标脏；仅 paint 变 → 窄 paint |

> **实现注记**：`WindowSession` / `view_factory` / `pending_root` 未接入；`reconcile` 函数已实现。

### 与 build 的关系

| API | 用途 |
|-----|------|
| `ViewAdapter::build(view)` | 冷启动：新建 WidgetTree + expand + layout |
| `ViewAdapter::reconcile(tree, view)` | 热路径：复用稳定 WidgetId，patch / keyed children diff |

Reconcile 完成后仍走既有 layout → overlay rebuild → paint invalidation 管线（见 [layout · 布局管线](layout.md#布局管线)）。

> **实现注记**：`reconcile` 已实现；`App::run_gui` 启动仍仅调用 `build_nodes`，主循环尚未在 State 批次末或 App API 接入 reconcile。热更新路径待接线。

---

## 多窗 Reconcile

设计（#148）— 每 **WindowSession** 持有 **独立** `WidgetTree`；Reconciler **不跨窗**。

```text
WindowSession A                    WindowSession B
├── tree_A                         ├── tree_B
├── ViewAdapter::reconcile(tree_A) ├── ViewAdapter::reconcile(tree_B)
└── 独立 HandlerTable / effects    └── 独立 HandlerTable / effects
```

| 规则 | 说明 |
|------|------|
| build | `open_window` → `ViewAdapter::build` **仅**写入新 session.tree（#148） |
| reconcile | 帧末合并 **按 session**；A 窗 State 变更 **不** reconcile B 树 |
| 共享 View 工厂 | 同一 `Fn() -> ViewNode` 可被多窗调用；各 session 各自 expand + diff |
| State | 全局 `State<T>` clone 跨窗共享；标脏 fan-out 见 [State 跨窗标脏](#state-跨窗标脏)（#150） |
| AppState | `get_handle(id)` 全局；`invalidate` 路由至 id 所在 session 的树 |
| update_view | 仅本 session；`AppHandle::update_view`（#149） |

副窗 root 热更新：

```rust
inspector_handle.update_view(|| inspector_view_v2());  // 仅 reconcile session B（#149）
```

> **实现注记**：多窗 reconcile / `update_view` 未接线；当前仅单窗 `build_nodes`。

---

## State 跨窗标脏

设计（#150）— 共享 `State<T>` 在 **多窗** 绑定时，一次 `set` **窄标脏** 所有绑定 widget，且 **仅 wake 有关 session**。

```rust
// 设计态 — State 内部（修订单 slot paint_binding）
struct PaintBindSite {
    window_id: WindowId,
    widget_id: WidgetId,  // 当前实现 usize
    queue: InvalidationQueueHandle,
    rect: Option<Rect>,
}

struct StateInner<T> {
    slot_id: StateSlotId,
    value: T,
    generation: u64,
    paint_sites: Vec<PaintBindSite>,  // 多 bind；fan-out
}
```

```text
State::set(value)
  for site in paint_sites:
      push Invalidation::Paint { rect } → site.queue
      sessions[site.window_id].mark_active()   // 仅有关窗 wake（#110）
  → 各 session 帧末 reconcile **独立**（#148）
```

| 规则 | 说明 |
|------|------|
| bind 时机 | layout 后 `bind_reactive_widget_states` / DynamicLabel 探测（现有路径） |
| 多 bind | 同 State 在 A、B 两窗各绑一次 → `paint_sites.len() == 2` |
| wake | **仅**含 bind 的 session 进入 Active；无 bind 的 session **不** wake |
| reconcile | 每 session **独立** reconcile；A 的 bind **不**触发 B 的 diff |
| 无 bind | 仅 `dirty_fn`（View 级）→ wake **创建 bind 的 session** |
| ComponentHandle | `invalidate()` → 单组件单 session（#119） |

### 与当前实现差距

| 设计（#150） | 当前 `state.rs` |
|--------------|-----------------|
| `paint_sites: Vec<_>` fan-out | 单个 `paint_binding: Option<...>` |
| 按 `window_id` wake session | 无多窗 session 模型 |
| `State::set` 多 queue | 至多一个 queue 收到 Paint |

落地 #150 时 **保留** 窄 rect 标脏；扩展为多 site，**禁止**全树 invalidate 替代。

> **实现注记**：fan-out 与 session wake 未实现；单 bind slot 与单窗主循环。

---

## 与下游消费

Reconciler 产出 WidgetTree 供：

| 消费者 | 用途 |
|--------|------|
| LayoutEngine | measure + 排布 |
| dispatch_event | 命中 / 焦点 / 语义 |
| HandlerTable | 业务回调 |
| ScenePaint | 绘制遍历 |
| OverlayStack | 浮层 rebuild |
