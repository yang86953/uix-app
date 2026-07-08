# 公开 API 参考

← [Main](../Main.md) · 跨域 · 功能域：`prelude`

> **应用作者单页索引**：`use uix::prelude::*;` 的完整导出清单与分组说明。行为语义见各系统文档；设计名 vs 源码名 → [glossary · 术语对照](../glossary.md#术语对照)。

## 索引

| 分组 | 章节 |
|------|------|
| 入口与宏 | [入口](#入口) |
| core | [core](#core) |
| native | [native](#native) |
| draw | [draw](#draw) |
| 响应式与样式 | [State · Style · Theme](#state--style--theme) |
| 布局 | [布局](#布局) |
| 事件与 Handle | [事件 · AppState · Handle](#事件--appstate--handle) |
| 管理器与浮层 | [管理器 · Overlay](#管理器--overlay) |
| View DSL | [View DSL](#view-dsl) |
| 内置组件 | [内置 Widget](#内置-widget) |
| App | [App 与运行时](#app-与运行时) |
| 域外符号 | [未经 prelude 导出](#未经-prelude-导出) |

**关联**：[application](application.md) · [view-reactive](view-reactive.md) · [component](component.md) · [glossary · prelude](../glossary.md#架构)

---

<a id="入口"></a>

## 入口

| 符号 | 说明 |
|------|------|
| `use uix::prelude::*;` | 推荐应用入口（[#69](../decisions.md#d69)） |
| `component!` | 声明自定义 Widget（[#102](../decisions.md#d102)） |
| `impl_widget_component!` | 为已有 struct 补 Widget trait 桥接 |
| `tree` | WidgetTree 构建辅助模块 |

源码：`src/prelude.rs` · 导出完整性由 `prelude` 内断言测试守护。

---

<a id="core"></a>

## core

| 类型 | 用途 |
|------|------|
| `ComponentId` | 组件 generational ID（[#101](../decisions.md#d101)） |
| `WindowId` | 进程内窗口 ID |
| `Point` · `Rect` · `Size` | 几何（逻辑像素） |
| `EdgeInsets` | 四边 inset |
| `Constraints` | measure 约束（[#103](../decisions.md#d103)） |
| `Error` · `Errc` | 统一错误 |

---

<a id="native"></a>

## native

prelude **仅**导出平台工厂与跨层输入枚举；上层 **禁止** `use native::backends::*`。

| 符号 | 说明 |
|------|------|
| `create_platform()` | 当前 OS 的 `Platform` 实例 |
| `ControlSize` · `CursorType` · `KeyCode` · `KeyMod` · `MouseButton` · `ScrollDirection` | 输入枚举 |
| `StatusLevel` | 系统通知级别 |

窗口 / 事件 / 呈现 trait → [platform](platform.md#traits-清单)。

---

<a id="draw"></a>

## draw

| 符号 | 说明 |
|------|------|
| `Color` · `colors` | 颜色常量与类型 |
| `GraphicsEngine` | 引擎 trait |
| `SoftwareEngine` · `NullEngine` | CPU / 空引擎 |
| `FontService` · `ImageService` | 字体与图片服务 |

绘制管线 → [rendering](rendering.md)。

---

<a id="state--style--theme"></a>

## State · Style · Theme

| 符号 | 说明 |
|------|------|
| `State<T>` · `Computed<T>` · `Effect` · `StateSlotId` | 响应式（[#21](../decisions.md#d21)） |
| `Style` · `StyleSet` · `StyleState` | 组件样式 |
| `ColorValue` · `DisplayMode` · `BoxShadowDef` · `PaletteColor` · `TypographyToken` | 样式字段类型 |
| `Theme` · `DesignTokens` · `DynTokens` · `ThemePrimitives` · `NeutralRole` · `ShadowToken` | 主题（[#1–#3](../decisions.md#d1)） |
| `Animation<T>` · `Easing` | 动画 |

---

<a id="布局"></a>

## 布局

| 符号 | 说明 |
|------|------|
| `FlexLayout` · `GridLayout` · `LayoutEngine` · `LayoutOutput` | 布局引擎 |
| `LayoutChild` | 布局输入单元 |
| `FlexDirection` · `JustifyContent` · `AlignItems` · `GridTrack` | Flex / Grid 配置 |
| `BoxModel` | margin / padding / content rect |

Scroll 组件与虚拟列表 → [layout · Scroll](layout.md#scroll) · [VirtualScroll](layout.md#virtual-scroll)。

---

<a id="事件--appstate--handle"></a>

## 事件 · AppState · Handle

| 符号 | 说明 |
|------|------|
| `SystemEvent` · `SystemEventKind` | 框架内统一事件 |
| `SemanticEvent` · `SemanticKind` · `SemanticPayload` | 语义事件 |
| `ClickEvent` | 点击载荷 |
| `EventResult` | Handler 返回值 |
| `HandlerId` · `HandlerOptions` · `HandlerRegistration` | Handler 注册 |
| `AppState` · `ComponentHandle` | 跨窗共享 registry + lookup handle（[#145](../decisions.md#d145)） |
| `ComponentConfigSnapshot` · `SnapshotSource` · `SnapshotFields` · `SnapshotValue` 等 | 配置快照提取 |
| `PaintContext` | 组件 render 上下文 |
| `IntoWidgetNode` · `WidgetChildren` | 树节点转换 |

事件传播 → [event · 传播](event.md#传播--handlertable) · Handle emit → [event · Handle emit](event.md#handle-emit)。

---

<a id="管理器--overlay"></a>

## 管理器 · Overlay

| 符号 | 说明 |
|------|------|
| `WidgetManagers` | 每树管理器容器 |
| `FocusManager` · `InteractionManager` · `DragManager` · `StateManager` · `TextManager` | 焦点 / 交互 / 拖拽 / 状态 / 文本 |
| `OverlayStack` · `OverlayEntry` · `OverlayId` · `OverlayKind` | 浮层栈 |

---

<a id="view-dsl"></a>

## View DSL

| 符号 | 说明 |
|------|------|
| `View` · `ViewNode` · `ViewAdapter` · `Ui` | View trait 与节点 |
| `column` · `row` · `grid` · `scroll` · `label` · `dynamic_label` · `button` · `input` · `space` · `embed` | 组合器 |
| `ButtonBuilder` · `GridBuilder` · `InputBuilder` · `ScrollBuilder` | Builder |
| `StyleExt` | View 链式样式 |

入门 → [view-reactive](view-reactive.md) · 示例 → [demo/README](../../demo/README.md)。

---

<a id="内置-widget"></a>

## 内置 Widget

以下均可通过 `prelude` 直接使用；trait 契约见 [component · 能力](component.md#能力)。

| 分类 | 符号 |
|------|------|
| 通用 | `Button` · `Label` · `Icon` · `Typography` · `Divider` · `Space` · `Container` · `Grid` · `FloatButton` · `FloatButtonBackTop` · `Affix` · `BackTop` |
| 布局 | `Layout` · `Header` · `Sider` · `Content` · `Footer` · `Splitter` · `ScrollView` |
| 导航 | `Menu` · `MenuItem` · `MenuMode` · `Breadcrumb` · `BreadcrumbItem` · `Pagination` · `Anchor` · `AnchorItem` · `Tabs` · `Tab` · `TabPosition` · `Steps` · `Step` · `StepStatus` · `Navigation` · `NavGroup` · `NavItem` · `Dropdown` |
| 输入 | `Input` · `InputNumber` · `Checkbox` · `Radio` · `RadioDirection` · `Switch` · `Slider` · `Rate` · `Select` · `OptGroup` · `AutoComplete` · `TreeSelect` · `Cascader` · `CascaderOption` · `CascaderValue` · `ColorPicker` · `DatePicker` · `DateValue` · `TimePicker` · `TimeValue` · `Mentions` · `Segmented` · `Form` · `FormItem` · `FormLayout` · `FieldDef` · `ValidationRule` · `ValidationResult` · `ValidateStatus` |
| 数据展示 | `Table` · `TableColumn` · `TableRow` · `TableChange` · `SortDirection` · `List` · `Tree` · `TreeNode` · `Descriptions` · `DescriptionsItem` · `Calendar` · `Timeline` · `TimelineItem` · `Tag` · `TagColor` · `Badge` · `BadgeStatus` · `Avatar` · `Card` · `Empty` · `Image` · `Skeleton` · `SkeletonShape` · `QRCode` · `RichText` · `RichTextSegment` · `RichTextStyle` · `SelectableList` · `SelectableItem` · `SharedActive` |
| 反馈 | `Alert` · `Message` · `MessageItem` · `MessagePlacement` · `Notification` · `NotificationItem` · `NotifPlacement` · `Modal` · `Drawer` · `DrawerPlacement` · `Tooltip` · `TooltipPlacement` · `Popover` · `PopoverPlacement` · `PopoverTrigger` · `Popconfirm` · `PopconfirmPlacement` · `ProgressBar` · `ProgressMode` · `ProgressType` · `Spin` · `SpinSize` · `Result` · `ResultType` |
| 图表 | `BarChart` · `BarData` · `LineChart` · `LineData` · `PieChart` · `PieData` |
| 其他 | `Collapse` · `CollapsePanel` · `Carousel` · `Transfer` · `TransferItem` · `Upload` · `UploadFile` · `UploadStatus` · `Watermark` · `ThemeToggle` · `TriggerMode` |

> `src/prelude.rs` 断言测试与组件目录同步；新增 Widget 须同时更新 prelude 与本表。

---

<a id="app-与运行时"></a>

## App 与运行时

| 符号 | 说明 |
|------|------|
| `App` · `AppHandle` · `AppMode` · `WindowConfig` · `TimerHandle` | 应用入口与运行中句柄（[#132–#144](../decisions.md#d132)） |
| `DiContainer` | DI 容器（`app::Container`） |
| `map_ui_event` | `UiEvent` → `SystemEvent` 桥接 |

生命周期、Timer、`post_to_ui`、多窗 → [application](application.md)。

---

<a id="未经-prelude-导出"></a>

## 未经 prelude 导出

应用作者按需显式 `use`；**不**经 `prelude` 的常见符号：

| 路径 | 符号 | 用途 |
|------|------|------|
| `uix::data` | `SettingsService` | 配置持久化 → [data](data.md) |
| `uix::ui::foundation` | `VirtualScroll` | 虚拟列表 helper（viewport/layout 已接）→ [layout · VirtualScroll](layout.md#virtual-scroll) |
| `uix::ui::core::widget` | `WidgetTree` · `WidgetNode` | 高级 / demo GUI 演示路径 |
| `uix::app::event_loop` | `run_widget_loop` | 绕过 `App` 的主循环入口 |
| `uix::native::traits::*` | Platform traits | 平台集成测试；生产 UI **禁止** backends |

功能域全貌 → [Main · 功能域](Main.md#功能域--系统) · 迁移锚点 → [CHANGELOG](../../CHANGELOG.md)。
