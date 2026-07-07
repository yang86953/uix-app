# 组件系统

← [Main](../Main.md) · 系统 **#3** · 功能域：`ui`

> 单个 UI 组件的结构、能力与生命周期。

## 索引

| 主题 | 章节 | 决策 |
|------|------|------|
| 设计原则 | [原则](#原则) | #16 #21 |
| Trait 能力 | [能力](#能力) · [ComponentConfigSnapshot](#componentconfigsnapshot) · [SnapshotSource](#snapshotsource) · [snapshot(skip)](#snapshotskip) | #8 #29 #146 #151 #152 |
| WidgetTree | [WidgetTree](#widgettree) | #35 #101 |
| 横切 Manager | [Manager 横切](#manager-横切) | — |
| 动画 | [动画](#动画) | #83 |
| 内置库 | [内置 Widget 目录](#内置-widget-目录) | #58 #80 #99 |
| Authoring | [Authoring](#authoring) | #20 #102 |
| Button v1 | [Button v1](#button-v1) | #23 #30 #77 |
| 标脏 | [标脏规则](#标脏规则) | #9 #105 #107 #122 |
| 模块图 | [ui 域模块图](#ui-域模块图) | — |

**关联**：[view-reactive](view-reactive.md) · [layout](layout.md) · [event](event.md) · [theme-style](theme-style.md) · [rendering](rendering.md) · [demand-driven](demand-driven.md)

---

## 原则

| 规则 | 说明 |
|------|------|
| 改样式 = 改属性 | 无独立 `bg_color` 字段；统一 `style: Style` |
| 颜色来自主题 | `ColorValue::palette` / `neutral`；Custom 仅 debug lint（#3） |
| 回调不进 struct | 业务 handler 在 HandlerTable，不在组件字段（#10） |
| 无 variant enum | 外观差异用 StyleSet 预设（#16） |
| 能力拆分 trait | Layout / Render / Event / Lifecycle 按需 impl |
| 无障碍 | **v2**（[#99](../decisions.md#d99)）；v1 不做 ARIA、屏幕阅读器、键盘导航扩展 |

**struct 放**：配置、交互态（hover/pressed/disabled）、StyleSet。  
**struct 不放**：业务闭包、`on_click` 字段、variant 枚举。

---

## 能力

组件通过 capability trait 组合能力（`src/ui/traits/widget.rs`）：

| Trait | 方法 / 职责 | 决策 |
|-------|-------------|------|
| **WidgetLayout** | `preferred_size`, `flex_grow/shrink`, `layout_children` | #29 |
| **WidgetRender** | `render(frame, ctx, tree)`；可选 `overlay_entry`, `dirty_rect` | |
| **EventHandler** | `on_event`, `semantic_event`, `wants_continuous_pointer_move` | #36 #121 |
| **连续 PointerMove opt-in** | 默认 false；true → hover 框内每 PointerMove dispatch（#121） | #109 |
| **WidgetLifecycle** | mount/unmount/active/inactive；`on_theme_changed` | #8 #9 |
| **PicturePolicyMeta** | authoring 声明默认 `Never`/`Eligible`（#122）；框架 build 时自动推断 |

`WidgetComponent` 是统一上转型入口；`impl_widget_component!` 宏声明能力位（Layout / Render / Event / Lifecycle）。

### Active / Inactive（#8、#19）

**Active** = 有焦点 **或** 与祖先 clip/scroll 视口求交后仍有可见像素。  
Inactive 组件跳过大部分语义派发，Lifecycle 进入 inactive。

### 连续 PointerMove opt-in（#121）

源码落点是 `EventHandler::wants_continuous_pointer_move` hook；默认实现返回 `false`。

| 返回值 | PointerMove 行为 |
|--------|------------------|
| false | [边界感知窄路径](demand-driven.md#pointermove-窄路径)（#109）：同一 hover 框内默认不 dispatch |
| true | 同一 hover 框内仍收到每次 `PointerMove`；适合画布、拖拽预览、SignaturePad |

新内置 widget 默认继承 false；移出窄路径须评审 + 测试证明必要性。

### ComponentHandle（设计 #61、#72、#145）

只读配置句柄；可 `invalidate()`（默认 **窄 Paint**，#119）/ `emit(SemanticEvent)`；**不可**改 style、**不可**读 hover/focus 等运行时态。

框架在 Reconciler **mount** 时向 AppState **自动 register**（#145）；unmount 时 unregister。快照字段见 [ComponentConfigSnapshot](#componentconfigsnapshot)（#146）。App **不手写**注册表。

> **实现注记**：`ComponentHandle` 类型与 `emit` / `invalidate` 已导出；live handle 与 `AppState::get_handle` lookup handle 的 `invalidate()` 均已接窄 Paint。`snapshot()` / `snapshot_fields()` 与首批只读配置 getter（`text` / `placeholder` / `disabled`）已接，当前优先从 live widget 提取静态配置快照，必要时可回退到 `AppState` snapshot registry，并排除交互态。`WidgetTree::set_app_state` 后 mount/unmount 自动 register/unregister snapshot、所属失效队列与当前 dirty rect，`AppState::get_handle` 已可返回 snapshot + invalidate + emit handle；App 默认持有同一 `AppState` 并注入主窗与副窗 `WindowSession`，lookup handle `emit` 经 AppState semantic queue 唤醒并由主/副窗 drain 派发；当前 handler 仍可经 `State<T>` 闭包捕获访问业务数据。

---

## ComponentConfigSnapshot

设计（#146）— mount 时从 widget 实例提取的 **只读配置快照**，供 `ComponentHandle` getter 与 AppState 注册表使用。

```rust
// app/state/component_snapshot.rs（设计）
pub struct ComponentConfigSnapshot {
    pub id: ComponentId,
    pub widget_type: TypeId,
    pub fields: SnapshotFields,  // 框架按 widget 类型提取
}

enum SnapshotFields {
    Label { text: String, /* 静态或可序列化配置 */ },
    Button { label: String, primary: bool },
    // ... 各内置 widget 的 **配置** 子集
    Opaque(Box<dyn SnapshotAccess>),  // 自定义 widget：component! 声明 snapshot trait
}
```

| 纳入快照 | 不纳入 |
|----------|--------|
| authoring / struct **配置**字段（label、placeholder、min/max…） | hover / pressed / focused |
| Style **预设名**或序列化 Style 子集（#72） | 运行时 layout frame |
| 组件声明的只读 props | handler 闭包、State 当前值 |

| 时机 | 行为 |
|------|------|
| **mount** | Reconciler mount → `extract_snapshot(widget)` → `AppState.register(id, snapshot)` |
| **reconcile patch** | 配置字段变 → 更新 snapshot + 窄 paint（#60）；**不**重建 Handle 身份 |
| **unmount** | `AppState.unregister(id)` |
| Handle getter | `handle.label()` 读 snapshot；**不**读 live widget 交互态 |

自定义 widget：见 [SnapshotSource](#snapshotsource)（#151）。

> **实现注记**：`ComponentConfigSnapshot` / `SnapshotFields` / `SnapshotSource` 已在 `ui::component_snapshot` 导出；Button / Label / Input / Container / Grid / Space / Divider / Icon / Typography / Checkbox / Radio / Switch / Slider / Rate / InputNumber / Avatar / Badge / Card / Empty / Image / Tag / Timeline / Calendar / Skeleton / FloatButton 已手写静态配置提取，并排除 hover / pressed / focused / cursor / selection / pending event / layout cache / resource cache 等运行态。`ComponentHandle` 已可读取当前组件快照与首批类型化 getter；`AppState` snapshot registry 已接入 `WidgetTree` mount/unmount，`ViewAdapter::reconcile` patch 后会刷新复用节点 snapshot；`define_widget!` 自定义组件 pub 字段自动提取与 `#[snapshot(skip)]` 排除已接；`component! { name: ..., struct ... }` 与 `component! { struct ... }` 已直接复用该路径。

### SnapshotSource（#151）

自定义 / 内置 widget 向快照系统贡献 **静态配置**；App **不手写**字段列表。

```rust
/// ui/component/snapshot.rs（设计）
pub trait SnapshotSource {
    fn snapshot_fields(&self) -> SnapshotFields;
}

// component! 默认：所有 pub 配置字段自动纳入
component! {
    name: Rating,
    struct Rating {
        pub max_stars: u8,      // → 快照
        pub readonly: bool,     // → 快照
        // hover_stars 等非 pub 或 #[snapshot(skip)] 排除
    }
}

// 显式覆盖（可选）
impl SnapshotSource for Rating {
    fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Rating {
            max_stars: self.max_stars,
            readonly: self.readonly,
        }
    }
}
```

| 规则 | 说明 |
|------|------|
| 默认 | `component!` / `define_widget!` **自动** `impl SnapshotSource`（pub 字段） |
| `#[snapshot(skip)]` | 字段属性；**显式排除**（#152）；见下节 |
| Style | 存 **StyleSet 预设 id** 或序列化子集；非完整 computed Style |
| 动态内容 | `dynamic_label` 文本 **不**进快照（走 State bind） |
| 提取时机 | mount + reconcile patch（#146） |

内置 widget：框架为各类型手写 `SnapshotFields` 变体；与 #146 `enum SnapshotFields` 对齐。

> **实现注记**：内置提取已覆盖 Button、Label、Input、Container、Grid、Space、Divider、Icon、Typography、Checkbox、Radio、Switch、Slider、Rate、InputNumber、Avatar、Badge、Card、Empty、Image、Tag、Timeline、Calendar、Skeleton、FloatButton；`define_widget!` 自定义 widget 已自动生成 `SnapshotSource` 并提取 pub 字段为 `SnapshotFields::Custom`，且支持 `#[snapshot(skip)]` 排除 pub 字段；`component!` 已支持文档示例的 `name: ..., struct ...` 入口和直接 `struct ...` 入口，并直接生成同一提取路径。

### #[snapshot(skip)]（#152）

字段级属性，控制 **#151 自动提取** 的纳入/排除。

```rust
component! {
    name: Rating,
    struct Rating {
        pub max_stars: u8,
        #[snapshot(skip)]
        pub hover_index: Option<u8>,   // pub 但排除：运行时交互态
        cached_layout: f32,            // 非 pub → 默认已排除
    }
}
```

| 规则 | 说明 |
|------|------|
| 默认 | **仅 `pub` 字段**进入自动快照（#151） |
| `#[snapshot(skip)]` | **强制排除**；可对 `pub` 字段使用 |
| 无 `snapshot(include)` | 非 `pub` 字段 **不**默认纳入（避免泄漏内部缓存） |
| 自定义 `impl SnapshotSource` | **完全覆盖**自动推导；属性仅作文档提示 |
| 宏展开 | `component!` 生成 `SnapshotFields` 时跳过标记字段 |

交互态字段（`hovered`、`pressed`、`scroll_offset`…）**必须** `skip` 或保持非 `pub`。

---

## WidgetTree

运行时中心结构（`src/ui/core/widget/`）：

```text
WidgetTree
├── nodes: Vec<Option<BoxedWidget>>     // 扁平存储 + free list
├── root_id, tree_version
├── focused_widget, hovered_widget
├── handler_table: HandlerTable
├── overlay_stack: OverlayStack
├── invalidation: InvalidationQueueHandle
├── drag_gesture: DragGestureState
└── effects: Vec<Effect>
```

| 字段 | 用途 |
|------|------|
| `tree_version` | Reconciler / layout 结构变更计数；LayerTree 同步依据 |
| `cached_traversal` | 布局遍历缓存，version 不匹配时重建 |

**ComponentId**（#35、#101）：`core::ComponentId { slot, generation }` 已作为 generational 稳定 ID 落地；源码中 `WidgetId` / `NodeId` 仍作为同一类型的模块别名使用。

**BoxedWidget** 持有：component、`parent/children` id、frame、visibility、lifecycle 标志、`is_dirty`、opacity、z_index、tab_idx。

---

## Manager 横切

`WidgetManagers`（`src/ui/managers/`）为 per-tree 横切能力容器：

| Manager | 职责 |
|---------|------|
| FocusManager | focused / focusable / tab_index |
| InteractionManager | hovered / pressed；bounds 内 click 检测 |
| StateManager | 字符串键 → `State<T>` 字典 |
| StyleManager | 旧版 preset（**已弃用**，用 `ui::style::Style`） |
| TextManager | text / font / placeholder / alignment |
| DragManager | widget 局部 drag offset |

支持 **per-widget override**（`state_for(id)` / `text_for(id)`）。

> **实现注记**：`WidgetTree` 已持有 per-tree `WidgetManagers`，并通过 `managers()` / `managers_mut()` 暴露树级默认 manager 与 `state_for(id)` / `style_for(id)` / `text_for(id)` per-widget override；节点移除或根替换会清理 stale override，代际不同的旧 ID 不会命中复用 slot 的新节点。`FocusManager` 已记录当前焦点与 Tab 顺序，`collect_focusable` / `focus_next` 由 manager 驱动；`InteractionManager` 已记录 hovered / pressed widget，并作为 PointerMove / Wheel / Timer 目标解析的优先状态源；`DragManager` 已记录拖拽 target / start / last / button / mods / offset，并驱动基础 DragStart / DragMove / DragEnd 热路径。旧 `focused_widget` / `hovered_widget` / `pointer_down_target` / `drag_gesture` 字段保留为事件派发兼容镜像。

---

## 动画

设计（#83、#124）：组件实现 `Animatable`；动画开始/续帧由 **框架自动** register 到内部 ActiveWorkRegistry；`tree.update(dt)` 在 Active/RegisteredActive 推进；结束 **自动** unregister。

| 阶段 | 行为 |
|------|------|
| 开始 | 框架 `register(Animation(id), deadline)` |
| tick | `drain_due` → `tree.update(dt)` → 自动 `dirty_bounds()` 标脏（#126） |
| 结束 | 框架 unregister → Registry 空时可 DeepIdle |

**App 不调用 register**。`Spin`、`ProgressBar` indeterminate、Dropdown fade、Select fade、AutoComplete fade、TreeSelect fade、Cascader fade、ColorPicker fade、Tooltip fade、Popover fade、Popconfirm fade、Modal、Drawer 与 Collapse 已作为内置 `WidgetAnimation` source 托管。

> **实现注记**：`WidgetAnimation` 能力与 `tree.update(dt)` 已接入 event loop；动画 widget 每次 update 后按 `animation_dirty_rect` 窄 Paint 标脏，仍活跃时由 Registry 登记下一帧 deadline。`Spin`、`ProgressBar` indeterminate、Dropdown fade、Select fade、AutoComplete fade、TreeSelect fade、Cascader fade、ColorPicker fade、Tooltip fade、Popover fade、Popconfirm fade、Modal、Drawer 与 Collapse 已作为内置动画源接入。

---

## 内置 Widget 目录

按 Ant Design 分类（#58 Big Bang）；`use uix::prelude::*` 导出完整清单如下。各 Widget 遵循本系统 trait 契约。

**v1 共性**：除 Button 为 reference impl（[#58](../decisions.md#d58)）外，其余均为 Big Bang 落地（#80）；**无障碍 v2** 统一待 [#99](../decisions.md#d99)（v1 不做 ARIA / 屏幕阅读器 / 键盘导航扩展）。

**浮层列**：✓ = 参与 [OverlayStack](overlay.md) 或 `overlay_entry` 调度。

### 通用 general（`widgets/general/`）

| Widget | 浮层/Overlay | v1 备注 |
|--------|:------------:|---------|
| Button | | reference impl（#58） |
| Icon | | Big Bang |
| Typography | | Big Bang |
| Label | | Big Bang |
| Divider | | Big Bang |
| Space | | Big Bang |
| SpaceSize | | 枚举辅助 |

### 布局 containers（`widgets/containers/`）

| Widget | 浮层/Overlay | v1 备注 |
|--------|:------------:|---------|
| Container | | Big Bang |
| Grid | | Big Bang |
| Splitter | | Big Bang |

### 导航 navigation（`widgets/navigation/`）

| Widget | 浮层/Overlay | v1 备注 |
|--------|:------------:|---------|
| Menu | | Big Bang |
| MenuItem | | 项辅助 |
| MenuMode | | 枚举辅助 |
| Tabs | | Big Bang |
| TabPosition | | 枚举辅助 |
| Breadcrumb | | Big Bang |
| BreadcrumbItem | | 项辅助 |
| Pagination | | Big Bang |
| Steps | | Big Bang |
| Step | | 项辅助 |
| StepStatus | | 枚举辅助 |
| Anchor | | Big Bang |
| AnchorItem | | 项辅助 |
| Navigation | | Big Bang |
| Dropdown | ✓ | Big Bang；下拉浮层 |

### 输入 input（`widgets/input/`）

| Widget | 浮层/Overlay | v1 备注 |
|--------|:------------:|---------|
| Input | | Big Bang |
| InputNumber | | Big Bang |
| Select | | Big Bang |
| Checkbox | | Big Bang |
| Radio | | Big Bang |
| Switch | | Big Bang |
| Slider | | Big Bang |
| Rate | | Big Bang |
| Form | | Big Bang |
| DatePicker | | Big Bang |
| DateValue | | 值辅助 |
| TimePicker | | Big Bang |
| TimeValue | | 值辅助 |
| ColorPicker | | Big Bang |
| Cascader | | Big Bang |
| CascaderOption | | 项辅助 |
| AutoComplete | | Big Bang |
| Mentions | | Big Bang |
| Segmented | | Big Bang |

### 数据展示 display（`widgets/display/`）

| Widget | 浮层/Overlay | v1 备注 |
|--------|:------------:|---------|
| Card | | Big Bang |
| List | | Big Bang |
| Tree | | Big Bang |
| TreeNode | | 项辅助 |
| Carousel | | Big Bang |
| Collapse | | Big Bang |
| CollapsePanel | | 项辅助 |
| Descriptions | | Big Bang |
| DescriptionsItem | | 项辅助 |
| Avatar | | Big Bang |
| Badge | | Big Bang |
| Tag | | Big Bang |
| TagColor | | 枚举辅助 |
| Image | | Big Bang |
| Empty | | Big Bang |
| Result | | Big Bang |
| ResultType | | 枚举辅助 |
| Skeleton | | Big Bang |
| SkeletonShape | | 枚举辅助 |
| Timeline | | Big Bang |
| TimelineItem | | 项辅助 |
| Calendar | | Big Bang |

### 反馈 feedback（`widgets/feedback/`）

| Widget | 浮层/Overlay | v1 备注 |
|--------|:------------:|---------|
| Modal | ✓ | Big Bang |
| Drawer | ✓ | Big Bang |
| Tooltip | ✓ | Big Bang（#96） |
| Popover | ✓ | Big Bang |
| Popconfirm | ✓ | Big Bang |
| Alert | | Big Bang；非 OverlayStack |
| ProgressBar | | Big Bang |
| Spin | | Big Bang |

### 其他 other（`widgets/other/`）

| Widget | 浮层/Overlay | v1 备注 |
|--------|:------------:|---------|
| ScrollView | | Big Bang（#73） |
| ScrollDirection | | 枚举辅助 |
| BarChart | | Big Bang |
| BarData | | 数据辅助 |
| LineChart | | Big Bang |
| LineData | | 数据辅助 |
| PieChart | | Big Bang |
| PieData | | 数据辅助 |
| QRCode | | Big Bang |
| Watermark | | Big Bang |
| ThemeToggle | | Big Bang |
| SharedActive | | 共享态辅助 |

> **prelude 导出**：以上已实现组件及其常用辅助类型均应可通过 `use uix::prelude::*` 取得；`src/prelude.rs` 的断言测试负责防止公开入口再次落后于组件目录。
>
> Modal / Tooltip / Drawer / Popover / Popconfirm / Dropdown 同时见 [overlay](overlay.md)。

---

## Authoring

设计决策 [#102](../decisions.md#d102)：设计态 **`component!`**（[#20](../decisions.md#d20)）；当前 `component! { name: ..., struct ... }` 与 `component! { struct ... }` 已作为直接 authoring 入口，`define_widget!` 保留为兼容入口。

### component! / define_widget!（当前实现）

```rust
component! {
    pub struct MyWidget { ... }

    measure => (&self, constraints) -> Size { ... }
    render => (&self, frame, ctx, tree) { ... }
    on_event => (&mut self, event) -> EventResult { ... }
}
```

`component!` 与兼容入口 `define_widget!` 均按已声明的方法自动生成能力、上转型与 trait impl；未覆盖的方法使用 trait 默认实现。旧 `preferred_size` 分支保留为兼容桥，新组件优先写 `measure`。

### prelude（#69）

`use uix::prelude::*` 导出高频符号：App / AppHandle / TimerHandle、View 组合器与 Builder、State、Theme、常用 Widget、布局枚举、语义事件与 HandlerRegistration、ComponentHandle / AppState。

---

## Button v1

| 属性 | 规则 |
|------|------|
| 内容 | 纯文本（#30） |
| 预设 | 默认 `StyleSet::button_default()`（#77） |
| 链式 | `.primary()` / `.ghost()` / `.danger()` |
| 不做 | icon、loading、Ripple（#23） |
| 键盘 | Enter / Space → Semantic Click |

---

## 标脏规则

| 来源 | 失效类型 | present? |
|------|----------|----------|
| 交互态 dispatch（hover/press） | Paint | 是 |
| State 变更 | Paint（可带 rect） | 是 |
| Theme 切换 | Paint，palette-only（#9） | 是 |
| Reconciler 结构/style 变 | Layout + Paint | Layout 否 |
| set_frame 变化 | 旧+新 rect Paint + Layout | 混合 |
| Scroll 内容滚动 | Composite（设计）/ Paint | 是 |

动画 tick → Paint；Layout alone → 不 present（见 [rendering](rendering.md)）。PicturePolicy 由框架自动推断（#122）；**调用方不维护名单**。

---

### PicturePolicy 元数据（#122）

内置 widget 在 authoring 时声明默认策略；运行时 LayerTree **再**按子树信号（handler、State bind、scroll…）推断，二者合并见 **#136**。

### 合并算法（#136）

```text
merge(metadata, runtime) :=
    if metadata == Never OR runtime == Never → Never
    else if metadata == Eligible AND runtime == Eligible → Eligible  // 仍须过 #129 阈值
    else → Never   // 未知/保守默认
```

| metadata | runtime 信号（任一） | effective |
|----------|---------------------|-----------|
| Never | * | **Never** |
| Eligible | 有 handler / State bind / scroll / IME | **Never** |
| Eligible | 纯静态、无交互信号 | **Eligible**（+#129 代价阈值） |

运行时信号由 LayerTree 遍历子树自动收集；**App 不配置**。

```rust
// component! / define_widget! 元数据（设计）
picture_policy: PicturePolicy::Never,  // 默认
// 纯静态只读 widget 可声明 Eligible
```

App / View **不**配置 Picture；调用方零维护。

> **实现注记**：`WidgetComponent::picture_policy()` 与 `define_widget!` 的 `picture_policy =>` 元数据方法已接；LayerTree 会合并 handler、dynamic content、interactive state、continuous pointer、overlay、focusable、clip、scroll 等运行时信号并套用 #129 阈值。当前 Container / Grid 首批声明 `Eligible`；未声明组件默认 `Never`。

---

## ui 域模块图

```text
ui/
├── view/          → view-reactive
├── core/widget/   → component, event
├── event.rs
├── layout/
├── theme/
├── foundation/    → theme-style, event (locale, focus_trap)
├── widgets/
├── managers/      (design)
├── overlay.rs
├── animation/
└── macros.rs
```

`ui` **不**访问 `native::backends`；组件能力经 trait 组合，业务 handler 在 HandlerTable（见 [event](event.md)）。

---

## 源码模块

```text
ui/
├── core/widget/       WidgetTree, BoxedWidget, tree_*
├── traits/widget.rs   WidgetLayout / Render / Event / Lifecycle
├── widgets/           内置库（general / containers / …）
├── managers/          横切 Manager（设计）
├── view/              View DSL + ViewAdapter
├── overlay.rs         OverlayStack
└── animation/         AnimationRegistry（设计）
```

详见 [Main · 源码目录详表](../Main.md#源码目录详表)。
