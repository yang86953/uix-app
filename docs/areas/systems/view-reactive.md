# View 与响应式系统

← [Main](../architecture.md) · 系统 **#2** · 功能域：`ui`

> 声明式 View 描述 UI；State 驱动重建与 diff。业务**仅**通过 View DSL 创建 UI（#21）。

## 索引

| 主题 | 章节 | 决策 |
|------|------|------|
| 组合器 API | [View DSL](#view-dsl) | #21 #67 #73 |
| 树同步 | [Reconciler](#reconciler) · [Handler 变更判定](#handler-变更判定-135) | #49 #60 #62 #101 #123 #135 #138 #142 #159 #160 #161 |
| 响应式原语 | [响应式](#响应式) · [StateSlotId](#stateslotid) | #24 #78 #79 #143 |
| 样式链 | [StyleExt](#styleext) | #21 |
| 热更新 | [热更新](#热更新设计) · [reconcile 合并](#reconcile-合并) · [view_factory 生命周期](#view_factory-生命周期) · [多窗 Reconcile](#多窗-reconcile) · [State 跨窗标脏](#state-跨窗标脏) | #49 #60 #118 #148 #149 #150 #153 #155 #156 |
| 未实现或后续 | [未实现或后续](#未实现或后续) | #159 #160 |

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
| `space(height)` | Space | 固定高度间距 |

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

<a id="reconciler"></a>

## Reconciler

> **术语（[#101](../../decisions.md#d101)）**
>
> - **ComponentId**（#35、#101）：Generational ID；Reconciler diff 复用依据。
> - **WidgetId**（内部源码名，#101）：`core::ComponentId` 的 WidgetTree 内部别名。
> - **HandlerTable** 键：`ComponentId`（`WidgetId` 仅为 WidgetTree 内部同型别名，#10、#101）。
> - **Handler 重绑**（#123、#135，修订 #62）：reconcile 时 **仅 handler 变更** 才 `clear_component`+重注册；未变则保留。
> - **AppState** / **ComponentHandle**（#32、#145）：snapshot registry、lookup handle、`invalidate` / `emit` 已接；业务数据主路径仍可经 `State<T>` 闭包捕获。

实现：`ViewAdapter`（`src/ui/view/adapter.rs`）。策略：声明式重建 + diff，复用稳定 ID（#31、#49、#60）。

### 入口

| API | 行为 |
|-----|------|
| `build(view)` | 捕获 → 新建 WidgetTree → expand → build |
| `reconcile(tree, view)` | 增量更新或整树重建 |

> **实现注记**：`reconcile` 已实现；单窗 `App::root(|| ...)` 会安装 session factory，`AppHandle::update_view` / `set_root` 与响应式 `State` 批次已接入帧末一次 reconcile。多窗支持见 [application · 主循环](application.md#主循环)。

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
| 类型不同 | 卸载旧 subtree，分配新 ComponentId（#35、#101） |

---

## Handler 变更判定（#135）

#123 智能重绑的 **可实现判定规则**（修订 #60、#62）。

View build 只为具备稳定身份的 handler 维护 **`handler_generation: u32`**：显式 `handler_generation`、显式 capture 指纹，或未来宏 / DSL 在语法层可见捕获集并生成的 `capture_fingerprint`。普通 Rust 闭包若没有 generation / fingerprint，按 #159 **保守视为变更**，避免误保留已变化 capture。Reconciler 在 `reconcile_existing` 步骤 3 比较：

```text
handlers_changed(old, new) :=
    old.semantic_kinds() != new.semantic_kinds()   // 集合相等（顺序无关）
    OR ∃ kind ∈ intersection:
         old.generation(kind) != new.generation(kind)
         OR old.options(kind) != new.options(kind)
```

| 比较项 | 规则 |
|--------|------|
| **SemanticKind 集合** | `Click`、`Changed`、`Submit`… 增删 → **变更** |
| **handler_generation** | 同 kind 代际不同 → **变更**；任一侧缺 generation → **变更** |
| **HandlerOptions** | `once` / `when` 语义不同 → **变更** |
| **闭包指针 / TypeId** | **不**比较；避免误杀稳定 handler |
| **静态 props / Style** | 走 #60 paint/layout 路径；**不**触发 clear_component |

代际 bump 时机（View build）：

| 事件 | generation |
|------|------------|
| 稳定 capture 指纹变化 | +1 |
| 普通闭包无 generation / fingerprint | 保守变更并重绑 |
| 仅 Style / 文本变 | 不变 |
| kind 移除后同 slot 新 kind | 新 kind 从 0 起 |

`handlers_changed == false` → **跳过** `clear_component` + 重注册；`true` → 清空该 ComponentId 全部 handler 再注册新表。

测试：勿断言闭包指针跨 rebuild 稳定；可断言 **同 generation + 同 kind 集 + 同 HandlerOptions + 同 capture 指纹集合** 时 HandlerTable 条目保留（见 [testing · 语义断言](testing.md#语义断言)）。

### handler_generation 作者化（#138）

**调用方不可配**；由 View DSL / `ViewAdapter` build 阶段基于稳定 signature / capture 指纹维护。无显式 capture 的普通闭包不做运行时捕获探测，按 #159 保守重绑。

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
| `.on_click(f)` 等普通闭包 | 无显式 capture 时不生成稳定 generation；保守重绑 |
| 显式 capture API / `semantic_handler!` | build 时写入 `HandlerSlot { kind, generation }` |
| capture 指纹 | 由显式 capture API，或 `semantic_handler!` 在语法层显式 capture list 生成 **稳定 hash**（#138、#159） |
| 指纹不变 | **复用**上代 `generation`（同 reconcile 周期内闭包重建但 capture 相同） |
| 指纹变化 | 该 kind `generation += 1` |
| kind 新增/移除 | 更新 SemanticKind 集合（#135） |
| App API | **无** `set_handler_generation`；零维护 |

实现落点：当前显式 capture 路径在 `src/ui/event.rs`、`src/ui/view/mod.rs` 与各 DSL builder；`semantic_handler!` 宏在语法层显式列出 `state` / `computed` / `window` capture，并复用同一 `capture_fingerprint` 管线。任意 Rust 闭包不做运行时自动探测（#159）。

### capture 指纹字段（#142）

`capture_fingerprint = hash(fields)` → `u64`；**仅**下列字段参与；顺序固定：

| 捕获类型 | 参与 hash 的字段 | 不参与 |
|----------|------------------|--------|
| `State<T>` | `TypeId::of::<T>()` + **`StateSlotId`**（#143） | 当前值、`generation()`、指针地址 |
| `WindowId`（代表窗口作用域 / AppHandle） | `WindowId` 值 | AppHandle 内部指针；`ui` 不依赖 `app` |
| `Copy` 标量 / 枚举 | 未来语法层 capture API 生成后参与 | 当前普通 Rust 闭包不自动探测 |
| `&'static T` | 未来语法层 capture API 生成后参与 | 当前普通 Rust 闭包不自动探测 |
| 闭包本体 | — | **fn 指针、vtable、堆地址** |
| 未捕获 | — | — |

```text
fingerprint(kind, captures) :=
    hash((
        kind as u16,
        sorted capture entries by TypeId tag and payload,
        per-entry payload per table above,
    )) → u64
```

| 规则 | 说明 |
|------|------|
| 指纹相等 | 复用上代 `generation`（#138） |
| 指纹不等 | `generation += 1` |
| 稳定性 | 同源码 rebuild、同 capture 集 → **同指纹**（测试可断言） |
| 算法 | 框架内部固定（如 `FxHasher` → `u64`）；App **不可配** |

> **实现注记**：内部 `HandlerSignature` / `handler_generation` 存储与 reconcile 比较已接；稳定 generation 且 options 未变的 handler 可跳过重注册。State / Computed / WindowId capture 指纹管线已接（同 fingerprint 复用 generation，变化时 bump，多个 capture 合并为顺序无关指纹）。公开 API：`HandlerRegistration::with_state_capture` / `with_computed_capture` / `with_window_capture`；Button/Input DSL 显式 capture 入口；`semantic_handler!` 宏语法层 capture。任意 Rust 闭包不做运行时自动收集（#159）；无 fingerprint 的 handler 保守全清重绑。

---

## StateSlotId

设计（#143）— `State<T>` 的 **稳定身份**，供 capture 指纹（#142）、测试与调试；**不同于** `StateInner::generation`（值变更计数，供 Computed/Effect）。

源码：`src/ui/foundation/state.rs`（经 `ui::state` / `prelude` 重导出）。

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
| Computed | `Computed::new` 分配独立 slot id；clone 共享同一逻辑派生状态 |

### 实现状态

**已实现**（#143）：`StateSlotId` 字段；`State::new` / `Computed::new` 分配单调 slot，`State` / `Computed` clone 共享；capture 指纹 `TypeId + slot_id`；`HandlerRegistration::with_state_capture` / `with_computed_capture` / Button·Input DSL 等显式 capture 已接。

> **实现注记**：`StateSlotId` 已落地；`State::generation()` 保持值变更计数语义，不参与 State capture 指纹。

---

<a id="响应式"></a>

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
    → invalidate_paint_handle(component_id, rect)
    → （设计 #150）register PaintBindSite { window_id, component_id, queue, rect }
```

跨窗 fan-out 见 [State 跨窗标脏](#state-跨窗标脏)（#150）。

### Effect 约束

Effect 闭包 `'static`；禁止在 Effect 内直接操作 WidgetTree 或改 layout。**禁止**轮询式 Effect（#131）。

周期逻辑：**App Timer API**（#132）或 async→主线程 `State::set`。见 [application · App 定时 API](application.md#app-定时-api) · [demand-driven · UI 主循环 vs 后台](demand-driven.md#ui-主循环-vs-后台)。

DeepIdle **不** `tick_effects`（[#105](../../decisions.md#d105)）。State 变更会先标记依赖它的 Effect pending；UiEvent 或 State 变更唤醒进入 Active 后，仅 pending Effect 会执行。详见 [demand-driven · Effect 与 Theme](demand-driven.md#effect-与-theme)。

### 业务数据

Handler 须 `'static`（#26）；业务数据经 **AppState** + **ComponentHandle**（#32、#145）或闭包捕获 `State` clone。组件 ID 与 HandlerTable 键术语见 [Reconciler · 术语](#reconciler)。

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

<a id="热更新设计"></a>

## 热更新

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

<a id="reconcile-合并"></a>

### reconcile 合并（#153）

每 `WindowSession` 在帧末 **至多一次** reconcile；合并 `State::set` 批次与 `update_view` 的 `pending_root`。

<a id="view_factory-生命周期"></a>

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

// 帧内合并步骤 6 — reconcile（见 demand-driven · 帧内合并 #118）
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

> **实现注记**：单窗 `WindowSession` 已持有 `view_factory` / `pending_root` / `reconcile_pending`；`AppHandle::update_view` 经 MainThreadQueue 写入一次性 `pending_root`，State 批次在无 `pending_root` 时调用 factory。

### 与 build 的关系

| API | 用途 |
|-----|------|
| `ViewAdapter::build(view)` | 冷启动：新建 WidgetTree + expand + layout |
| `ViewAdapter::reconcile(tree, view)` | 热路径：复用稳定 ComponentId，patch / keyed children diff |

Reconcile 完成后仍走既有 layout → overlay rebuild → paint invalidation 管线（见 [layout · 布局管线](layout.md#布局管线)）。

> **实现注记**：`reconcile` 已实现；单窗主循环已在 `tick_effects` 后、layout/render 前消费 `pending_root` 或 State 批次置位，并至多 reconcile 一次；State 批次无 `pending_root` 时会调用 session factory。

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

> **实现注记**：单窗 `update_view` 已接入主循环帧末 reconcile。多窗路由见 [application · update_view](application.md#update_view)。

---

## State 跨窗标脏

设计（#150）— 共享 `State<T>` 在 **多窗** 绑定时，一次 `set` **窄标脏** 所有绑定 widget，且 **仅 wake 有关 session**。

```rust
// 规格 — State 内部（修订单 slot paint_binding）
struct PaintBindSite {
    window_id: WindowId,
    component_id: ComponentId,
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
| wake | State 变更只标记含 bind 的 queue / reconcile requester；后台来源须经 `post_to_ui` / Timer 等入口唤醒 event loop；无 bind 的 session **不**产生 pending |
| reconcile | 每 session **独立** reconcile；A 的 bind **不**触发 B 的 diff |
| 无 bind | 仅 `dirty_fn`（View 级）→ wake **创建 bind 的 session** |
| ComponentHandle | `invalidate()` → 单组件单 session（#119） |

### 实现状态

| 设计（#150） | 当前 `state.rs` |
|--------------|-----------------|
| `paint_sites: Vec<_>` fan-out | `State` / `Computed` 已支持多 paint site fan-out |
| 按 `window_id` wake session | 副窗 session 路由已接；State 绑定通过每窗 `InvalidationQueueHandle` 与 reconcile requester fan-out，`State` 本体不保存 `window_id` |
| `State::set` 多 queue / reconcile | 多个已绑定 queue 均收到 Paint；多个 reconcile callback 按 site key fan-out |

实现须 **保留** 窄 rect 标脏；多 site fan-out 已接，**禁止**回退为全树 invalidate。

> **实现注记**：`State` / `Computed` 已将单个 paint binding 扩展为多个 paint site，`set` / `update` / recompute 时 fan-out 到所有已绑定 queue；同一 `(component_id, queue)` 重复绑定原地更新 rect。`State` reconcile callback 也按 site key fan-out。

---

## 未实现或后续

本域已接显式 capture API 与 `semantic_handler!` 宏层 fingerprint；普通闭包仍按 #159/#160 保守重绑。跨域剩余项 → [implementation · 后续工作](../implementation.md#后续工作)。

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
