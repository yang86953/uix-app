# 事件系统

← [架构导航](../architecture.md) · 系统 **#5** · 功能域：`ui` · `app`（UiEvent 映射）

> 输入 → 系统 / 语义 / 自定义 三层；业务只绑定后两层。

## 索引

| 主题 | 章节 | 决策 |
|------|------|------|
| 三层模型 | [事件层](#事件层) | #5 #6 #36 |
| 传播与绑定 | [传播 · HandlerTable](#传播--handlertable) · [Handle emit](#handle-emit) | #4 #7 #10 #68 #101 #147 |
| 派发流程 | [派发流程](#派发流程) | #71 #75 #95 #98 |
| 输入专题 | [输入能力](#输入能力) | #71 #75 |
| 测试 | [测试](#测试) → [testing](testing.md#测试策略) | #40 |

**关联**：[platform](platform.md) · [application](application.md) · [component](component.md) · [overlay](overlay.md) · [demand-driven](demand-driven.md)

---

## 事件层

```text
Platform          app 边界           组件 dispatch          业务
UiEvent      →    SystemEvent    →    EventHandler      →    HandlerTable
                                       semantic_event         (Semantic/Custom)
```

| 层 | 类型 | 消费方 | 业务可绑定 |
|----|------|--------|------------|
| 平台 | `UiEvent` | app `map_ui_event` | 否 |
| 系统 | `SystemEvent` | Widget `on_event` | 否 |
| 语义 | `SemanticEvent` | HandlerTable | **是** |
| 自定义 | `SemanticKind::Custom(TypeId)` | HandlerTable | **是** |

### SystemEvent 主要变体

Pointer（Down/Up/Move）、Wheel、Key（Down/Up）、TextInput、IME（Start/Update/End）、Copy/Cut/Paste、Focus（In/Out）、ThemeChanged、Resize、FileDrop、Drag（Start/Move/End，树内合成）。

逻辑像素（#70）；Click payload：`{ button, pos, modifiers }`（#36）。

### SemanticKind（#5、#66）

内置：`Click`、`Change`、`Submit`、`FileDrop`、`ContextMenu`、剪贴板/IME 语义变体。  
扩展：`register_semantic!` 宏注册 → `SemanticKind::Custom(TypeId)`（#6 typed struct payload）。

---

<a id="传播--handlertable"></a>

## 传播 · HandlerTable

### 传播方向（#4、#25）

| 阶段 | 方向 | 默认 |
|------|------|------|
| Capture | 根 → 目标（不含目标） | opt-in |
| Target | 目标 component | — |
| Bubble | 目标 → 根 | **默认** |

`EventResult`：`Handled`（停止）| `NotHandled` | `Bubbled`。

### HandlerTable（#10、#101）

设计与当前源码键均为 **ComponentId**；`WidgetId` 仅为 WidgetTree 内部 `core::ComponentId` 同型别名（#10、#101）。

```rust
HashMap<ComponentId, Vec<HandlerEntry>>
```

| API | 作用 |
|-----|------|
| `register(id, HandlerRegistration)` | 绑定语义 handler |
| `on_click` / `on_change` / … | 类型化 helper |
| `clear_component(id)` | Reconciler **handler 变更**时清空（#123、#135；修订 #62） |
| `dispatch_path(path, event)` | 沿路径顺序调用 |

**HandlerRegistration**：

- `kind: SemanticKind`
- `options: HandlerOptions` — `once`（#90）、`when` 谓词（#91、#120，**O(1) 纯函数**，禁止 I/O/alloc/读 State）
- `handler: FnMut(&mut SemanticEvent)`
- `with_state_capture(&State<T>)`：当前已公开，用于标记 handler 捕获的 State，使 Reconciler 能在指纹不变时复用稳定 generation；裸 `capture_fingerprint` 仍是内部实现细节。
- `with_window_capture(WindowId)`：当前已公开，用于标记窗口作用域 AppHandle 等捕获；`ui` 只记录 `WindowId` 指纹，不依赖 `app`。
- 普通 Rust 闭包若无显式 `handler_generation` / capture fingerprint，按 #160 保守视为 changed 并重绑；不使用 callsite、closure 指针或堆地址伪造稳定身份。

### 多 handler 与 stop（#7、#68）

- 同组件多 handler：**顺序执行**
- `event.stop_propagation()` — 阻后续 handler **与**语义冒泡（#68）
- `event.prevent_default()` — 语义层 `default_prevented`（#92）

Handler 闭包 `'static`；**ComponentHandle**（#32、#145）以窗口作用域 `WindowId` 指纹表达，或通过 `State<T>` 捕获（#26）。`ComponentHandle::emit` 见 [Handle emit](#handle-emit)（#147）。

显式 capture 可组合多个稳定来源（如 `State<T>` + `WindowId`），指纹按条目排序后合并，顺序无关；普通 Rust 闭包不做运行时自动捕获探测。

---

## Handle emit

规格（#147）— 业务经 **ComponentHandle** 注入语义事件，走与 OS 输入 **相同** 的派发路径。

```text
ComponentHandle::emit(event)
  → 使用 ComponentId 同一代际 ID
  → SemanticEvent { target: id, .. event }
  → WidgetTree::dispatch_semantic(event)   // 已有 API
       → semantic_path_to_root(target)
       → HandlerTable::dispatch_path（target-first 向 root）
```

| 规则 | 说明 |
|------|------|
| 线程 | **仅主线程**（#88）；与 Timer / post_to_ui 同约束 |
| 路径 | **同** [语义冒泡](#语义冒泡)；`stop_propagation` / `prevent_default` 有效 |
| target | `event.target` = handle 的 ComponentId；`current_target` 沿 path 更新 |
| payload | 标准 `SemanticEvent` 构造（`change` / `custom` / …）；**非**新并行事件类型 |
| 绘制 | emit **不**默认 layout/render；handler 内 `State::set` / `invalidate` 按需 wake |
| 禁止 | 绕过 HandlerTable 直调 handler 闭包；非主线程 emit |

### 与 OS 事件对比

| 来源 | 入口 | 后续 |
|------|------|------|
| OS / FakePlatform | `dispatch_event(SystemEvent)` → widget `semantic_event` | `dispatch_semantic` |
| **Handle emit** | 直接 `dispatch_semantic(SemanticEvent::…)` | 同上 |

测试：FakePlatform 注入与 `handle.emit` 应对同一 handler 产生 **相同** HandlerTable 副作用（见 [testing · 语义断言](testing.md#语义断言)）。

> **实现注记**：`SemanticEvent` target/current_target、`EventHandler::semantic_event` target 参数、`HandlerTable` 存储与公开 API 已按 `ComponentId` 命名，且覆盖同 slot 不同 generation 不串 handler 的测试。`ComponentHandle::emit` 已导出；live handle 将事件 target 归一到所属组件后直接调用 `WidgetTree::dispatch_semantic`，lookup handle 将事件写入 `AppState` semantic queue 并唤醒 loop，由主窗/副窗 `WindowSession` drain 后派发。`ComponentHandle` 只读 snapshot getter 已接；`AppState` snapshot registry、`get_handle`、lookup handle `invalidate()` 与主窗/副窗 AppState 注入已接。

---

## 派发流程

入口：`WidgetTree::dispatch_event`（`tree_events.rs`）。

### 按事件类型路由

| 事件 | 目标选择 | 阶段 |
|------|----------|------|
| PointerDown | overlay 命中 **或** hit_test | modal 外拦截 → capture → bubble → focus |
| PointerUp | 同上 | capture → bubble → **同目标即发 Click**（不要求 `PointerUp` Handled）；右键 → ContextMenu overlay |
| PointerMove | drag（5px 阈值）/ hover enter-leave | capture → bubble |
| Wheel | overlay **或** hit **或** hover **或** root | capture 优先（ScrollView 消化，#45） |
| KeyDown | Tab → focus_next；否则 focused | capture → bubble → Enter/Space → Click |
| TextInput / IME / Copy/Cut/Paste | focused component | system → optional semantic |
| ThemeChanged | 全树 notify + root invalidate | |
| FileDrop | hit **或** root | system + semantic（#95） |

### 命中测试

- 最深可见节点；子节点按 `z_index` **降序**
- ScrollView 对子坐标补偿 scroll offset
- `hit_test_frame` 可大于 visual frame（扩大点击区）

### 语义冒泡

```text
dispatch_semantic_event:
  path = target → root
  handler_table.dispatch_path(path, event)
```

路径顺序：**target-first 向 root**（与 DOM bubble 一致）。

### 焦点（#18）

单焦点 + `tab_index` 焦点链。`set_focus`：FocusOut 旧 → FocusIn 新 → lifecycle reconcile。

<a id="pointermove-按需零闲置"></a>

### PointerMove（按需零闲置 #109）

[demand-driven · 边界感知窄路径](demand-driven.md#pointermove-窄路径)：

1. `DragManager` active/potential 或 `InteractionManager.pressed_component` → **全 dispatch**
2. `pos` 仍在 `InteractionManager.hovered_component` 扩大 hit 框内 → 仅更新拖拽/hover 状态；**不 hit_test**；默认 **不 dispatch**
3. 否则 `hit_test`；`target ≠ hovered_component` → enter/leave；**仅当组件 `Handled` enter/leave 时窄标脏**，并对新 target dispatch

Widget 可通过 `EventHandler::wants_continuous_pointer_move` opt-in（#121，默认 false）。实现见 [component · 能力](component.md#能力)。

> **实现注记**：`tree_events.rs` 已按 #109 接入：pointer capture/drag 路径全 dispatch；hover hit frame 内跳过 `hit_test` 与默认 dispatch；opt-in widget 保留连续 `PointerMove`。无 hover 视觉态的叶子（Label/Icon 等对 enter/leave 返回 `NotHandled`）不标脏，避免局部清屏挖掉父背景。

---

## 输入能力

| 能力 | 路径 | 决策 |
|------|------|------|
| 剪贴板 | SystemEvent Copy/Cut/Paste → 语义 | #71 |
| 单行 / 多行 / IME | TextInput + Ime* 事件 | #75 |
| 拖放 | FileDrop semantic | #95 |
| 右键菜单 | ContextMenu → OverlayStack | #98 |
| i18n 事件 | LocaleChanged 预留 | #46 不做用户 i18n |

**Locale 与 #46**：裁决 [#46](../../decisions.md#d46) 指 **用户-facing 应用级 i18n 系统**（资源文件、运行时语言切换 API、`LocaleChanged` 业务联动）**不做**；`LocaleChanged` 仅平台事件预留。与之区分：`ui/foundation/locale.rs` 的 **`Locale` struct** 为内置 Widget **默认文案表**（如分页「上一页」、空状态提示），由 `use_locale()` 读取，**不是**应用级 i18n API，也不替代 #46 设计。

`platform.text_input().start/stop()` 仅在 **RegisteredActive**（IME 焦点 Input 会话）或 Active 派发 IME 相关事件时调用；DeepIdle 不调用（#111）。

---

<a id="测试"></a>

## 测试

主入口：[testing · 测试策略](testing.md#测试策略)（#40）。

```text
FakePlatform.inject(UiEvent)
    → map_ui_event → dispatch_event
    → 断言 SemanticEvent / Handler 副作用
    → paint snapshot 对比绘制输出
```

Fake 组件：`native/test_harness/FakePlatform` — 内存实现 + 调用历史记录。详见 [testing · FakePlatform](testing.md#fakeplatform) · [platform · 测试平台](platform.md#测试平台)。

---

## 与 Reconciler 交互

View rebuild 时 **智能重绑** handler（#123、#135，修订 #62）— 仅 handler 变更时 `clear_component`+重注册。判定细则 → [view-reactive · Handler 变更判定](view-reactive.md#handler-变更判定-135)。  
业务 handler 不应把 id 当裸 slot/usize 使用；依赖 **ComponentId** 与 key 匹配复用 widget 实例；`WidgetId` 仅是 WidgetTree 内部别名（[#101](../../decisions.md#d101)）。

---

## 源码模块

```text
ui/event.rs              SystemEvent, SemanticEvent, SemanticKind, EventResult
                         HandlerTable, HandlerRegistration
ui/core/widget/
    tree_events.rs       dispatch_event, hit_test, 语义冒泡
app/shell/application.rs map_ui_event（UiEvent → SystemEvent）
```

详见 [implementation · 源码目录详表](../implementation.md#源码目录详表)。
