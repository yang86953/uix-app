# 术语表

← [Main](Main.md) · 按需查阅，不必通读

| 分类 | 跳转 |
|------|------|
| 架构 | [功能域与系统](#架构) |
| 应用 | [App 与运行时](#应用) |
| UI | [View 与组件](#ui) |
| 主题样式 | [Theme 与 Style](#主题与样式) |
| 事件 | [事件体系](#事件) |
| 布局 | [布局](#布局) |
| 渲染 | [绘制](#渲染) |
| 图形 API | [多图形 API](#多图形-api) |
| 平台 | [平台](#平台) |
| 数据 | [持久化](#数据) |
| 测试 | [测试](#测试) |
| API 名对照 | [术语对照](#术语对照) |

---

<a id="术语对照"></a>

## 术语对照（#101–#104）

设计文档名 vs 当前源码；重构按「设计」列对齐。详情 → [#101–#104](decisions.md#d101)。

| 设计（文档/重构目标） | 当前实现（源码） | 决策 |
|----------------------|------------------|------|
| ComponentId (Generational) | `core::ComponentId`；WidgetTree 带 tree scope + slot + generation；**公开边界**用 `ComponentId`；树内 `WidgetId`、draw 内 `NodeId` 为同型别名 | [#101](decisions.md#d101) |
| WindowId | `core::WindowId`；`AppHandle` / `prelude` 重导出 | [#116](decisions.md#d116) |
| EventLoopWaker | `native::traits::event::EventLoopWaker`；`IEventLoop::waker()` | [#117](decisions.md#d117) [#133](decisions.md#d133) |
| component! | `component! { name: ..., struct ... }` / `component! { struct ... }` | [#102](decisions.md#d102) |
| measure(constraints) | `WidgetLayout::measure(Constraints)` 唯一入口；旧 `preferred_size` 已移除 | [#103](decisions.md#d103) |
| ScrollView | ScrollView（旧文档 ScrollContainer） | [#104](decisions.md#d104) |
| VirtualScroll | `ui::foundation::VirtualScroll`；不经 prelude；Table/Tree/SelectableList/Select/TreeSelect 已接 VirtualListScroll | [layout · VirtualScroll](systems/layout.md#virtual-scroll) |
| AppState + ComponentHandle | `AppState` registry + `ComponentHandle` snapshot/invalidate/emit | [#32](decisions.md#d32), [#101](decisions.md#d101) |
| HandlerTable key ComponentId | `SemanticEvent` / `HandlerTable` 已用 `ComponentId`；`WidgetId` 为树内别名 | [#10](decisions.md#d10), [#101](decisions.md#d101) |

---

## 架构

| 术语 | 含义 |
|------|------|
| 功能域 | 源码维度：`core` / `native` / `draw` / `ui` / `app` / `data` |
| 系统 | 文档维度；与源码目录不一一对应；索引见 [Main · 系统索引](Main.md#系统索引) |
| 按需零闲置 | **核心理念 / 最高规则**（#105）；框架 **零维护**（#130）；见 [demand-driven](systems/demand-driven.md) |
| L0 / L1 / L2 | 零闲置三层目标（#105）：零像素 / 零帧循环 / 最小脏区；见 [demand-driven · 三层目标](systems/demand-driven.md#三层目标) |
| 开发者零维护 | #130：调用方仅 State/View/opt-in API；Picture/Registry/标脏由框架自动 |
| PicturePolicy | #122：框架按 widget 能力 **自动** Never/Eligible；**非**调用方黑名单 |
| 核心理念 | 同「按需零闲置」— UIX **最重要的一条规则**；统领六域依赖 |
| DeepIdle | 三态之一（#106）：无 pending 且无 RegisteredActive → blocking wait；不 layout/render/**不 tick Effect** |
| RegisteredActive | 三态之一（#111）：ActiveWorkRegistry 中 Animation/Timer/IME；unregister → DeepIdle |
| ActiveWorkRegistry | 框架 **内部** RegisteredActive 注册表（#115、#124）；App 不可访问 |
| follow_system_theme | App builder opt-in（#125）；默认 false；true 时框架自动跟 OS ThemeChanged |
| WindowSession | 单窗运行时包（#116）：tree + 引擎 + 三态 + Registry |
| wait_until | app 层 `wait_timeout(remaining)` 实现（#127）；非固定 interval 探活 |
| Active | 三态之一：UiEvent 或 Invalidation pending → 按需管线 |
| 失效驱动 | Layout / Paint / Composite 须经 `InvalidationQueue` 或 OS 事件触发 |
| prelude | `use uix::prelude::*;` — 应用开发推荐入口 |

## 应用

| 术语 | 含义 |
|------|------|
| App | 应用入口：GUI（View 根）或 CLI；见 [application](systems/application.md) |
| AppHandle | 含 `window_id`；`run_after` / `post_to_ui`（#132–#133）已按 session 路由（#141）；session 关闭会清理对应 Timer（#134） |
| AppRuntime | 内部 session 路由（Timer / post_to_ui / update_view / open_window）；见 [application · AppHandle](systems/application.md#apphandle-生命周期) |
| EventLoopWaker | 跨线程唤醒 → [术语对照](#术语对照) · [platform · IEventLoop](systems/platform.md#ieventloop) |
| WindowId | `core::WindowId`；进程内窗口 ID；`AppHandle` / `prelude` 重导出 |
| on_start | #140：每 WindowSession 首帧前 `Fn(AppHandle)`；运行中 API 的 canonical 注入点 |
| post_to_ui | #133：跨线程 `FnOnce + Send` 投递；路由见 #141 |
| MainThreadQueue | #137：每 WindowSession FIFO；帧内 UiEvent → drain_due → drain_queue |
| TimerHandle | #132：opaque；`cancel` / drop → unregister |
| handler_generation | #135 / #138：View build 自动维护；Reconciler 比较以决定是否重绑 |
| capture 指纹 | #142：`StateSlotId`（#143）+ TypeId + Copy 值 + `WindowId`（窗口作用域 / AppHandle） |
| StateSlotId | #143：`State::new` 单调 id；clone 共享；**≠** `State::generation()` |
| open_window | #144：运行中创建副窗；返回新 `AppHandle` |
| AppState | #145：不替代 `State<T>`；mount 自动 register；跨窗共享；snapshot registry 与 semantic queue 已接 |
| ComponentHandle | #145–#147：只读 snapshot getter + invalidate/`emit` 已接；mount 自动注册 |
| ComponentConfigSnapshot | #146：mount 时静态配置快照；Handle getter 数据源 |
| SnapshotSource | #151：自定义 widget 快照 trait；`component!` 默认自动 |
| Handle emit | #147：`ComponentHandle::emit` → `dispatch_semantic` |
| update_view | #149：`AppHandle::update_view` — 本 session 帧末 reconcile |
| snapshot(skip) | #152：`#[snapshot(skip)]` 字段属性；排除出 ComponentConfigSnapshot |
| reconcile 合并 | #153：`pending_root` 优先；State + update_view 同帧一次 reconcile |
| view_factory | #155：session 创建时固定 `Arc`；State reconcile 长期来源 |
| update_view vs factory | #156：`update_view` 仅 `pending_root`；不替换 factory |
| 实现路线图 | #154：P0–P5 分阶段落地；[roadmap · 分阶段路线图](roadmap.md#分阶段路线图) |
| P0 落地清单 | #157：P0 阶段目标与验收（历史）；详见 [roadmap · 已落地阶段](roadmap.md#已落地阶段p0p5) |
| State 跨窗标脏 | #150：`State::set` fan-out 至各 `PaintBindSite` |
| SettingsService | 扁平字符串 KV 持久化；见 [data](systems/data.md) |
| FakeTimer | 已有：`ITimer` 平台测试时钟；见 [testing · 测试时钟分层](systems/testing.md#测试时钟分层) |
| TestClock | 已接入：App `drain_due` / `wait_until` 测试注入时钟（#139） |

## UI

| 术语 | 含义 |
|------|------|
| View DSL | `column`、`row`、`button`、`dynamic_label` 等声明式 API |
| ViewNode | View 构建产物；经 Reconciler 同步到组件树 |
| Widget | UI 组件实例 |
| ComponentId | Generational 稳定 ID；设计 vs 源码 → [术语对照](#术语对照) · [#101](decisions.md#d101) |
| WidgetId | WidgetTree 内部别名 → [术语对照](#术语对照) |
| NodeId | draw / ScenePaint 内部别名 → [术语对照](#术语对照) |
| component! | 推荐 authoring 宏 → [术语对照](#术语对照) · [#102](decisions.md#d102) |
| WidgetTree | 运行时组件树容器 |
| HandlerTable | 业务回调表 → [术语对照](#术语对照) · [#10](decisions.md#d10) |
| State / Computed / Effect | 响应式原语；Effect 禁止直接改 UI；周期可用 **Timer API**（#132，见 [应用](#应用)） |
| UI 主循环 vs 后台 | #131：禁止裸 Registry；**允许** #132 Timer 与 async→State |
| OverlayStack | Modal / Tooltip / ContextMenu 统一调度 |
| ViewAdapter | ViewNode ↔ WidgetTree build/reconcile 桥接 |
| BoxedWidget | 运行时 widget 节点包装 |
| OverlayEntry | 浮层栈条目：kind、bounds、modal、focus_trap |
| Manager | 注入 WidgetTree 的横切能力（focus、drag、interaction 等） |

## 主题与样式

| 术语 | 含义 |
|------|------|
| Theme | 全局主题快照：`Arc<dyn TokenProvider>` 包装 |
| DesignTokens | Theme 具体 preset；~80 语义 token 字段 + `antd_light/dark` |
| DynTokens | 运行时可切换 Theme；**跟 OS** 须 App opt-in `.follow_system_theme(true)`（#125） |
| TokenProvider | Theme 多态接口；extends ThemeTokens + 默认 Style 工厂 |
| ThemeTokens | draw 层 token getter trait 集合（IColor/ITypography/…） |
| Style / StyleSet | 外观属性；五态 normal / hover / pressed / focused / disabled |
| ColorValue | Palette / Neutral / Custom 颜色引用 |
| NeutralRole | 15 语义中性角色 → 13 阶映射 |
| TypographyToken | 排版令牌；Theme 解析字号 / 行高 / 字体 |

## 事件

| 术语 | 含义 |
|------|------|
| UiEvent | 平台原始事件；app 边界转为 SystemEvent |
| SystemEvent | 指针、键盘、窗口、剪贴板、IME 等 |
| SemanticEvent | Click、Change、FileDrop、ContextMenu 等 |
| CustomEvent | typed payload + TypeId |
| Bubble / Capture | 事件传播方向；Bubble 默认 |

## 布局

| 术语 | 含义 |
|------|------|
| measure | 唯一测量入口 → [术语对照](#术语对照) · [#103](decisions.md#d103) |
| preferred_size | 已废弃；见 measure |
| ScrollView | 滚动容器 → [术语对照](#术语对照) · [#104](decisions.md#d104) |
| VirtualScroll | 大列表虚拟滚动 helper；不经 prelude；Table/Tree/SelectableList/Select/TreeSelect 已接 VirtualListScroll → [layout · VirtualScroll](systems/layout.md#virtual-scroll) |
| Constraints | `{ min, max, definite }` |
| Active / Inactive | **Active**：有焦点 **或** 视口内仍有可见像素（与祖先 clip/scroll 求交）；Inactive 跳过大部分语义派发（#8、#19） |
| BoxModel | margin → border → padding → content 盒模型 |
| FlexLayout / GridLayout | Flex / Grid 布局引擎 |

## 渲染

| 术语 | 含义 |
|------|------|
| ScenePaint | draw↔app 场景只读 trait；WidgetTree 实现 |
| DisplayList | 绘制命令序列 |
| LayerTree | 合成层树：Picture / ClipRect / Direct |
| Picture | PicturePolicy=Eligible 且子树代价达阈值（#122、#129）才离屏缓存；框架自动推断 |
| WantsContinuousPointerMove | opt-in trait（#121）；默认 false |
| RenderObjectTree | DisplayList 缓存树 |
| FrameRenderer | 单帧 render 编排 |
| InvalidationQueue | Layout / Paint / Composite 失效队列 |
| Invalidation | 单条失效记录 |
| DirtyRegion | 逻辑坐标脏矩形集合 |
| DamageRegion | 渲染 damage |
| AnimationRegistry | 动画帧驱动 registry；位于 `draw::pipeline`，按需注册下一帧 deadline |
| FontService / ImageService | 字体与图像资源服务 |
| BackendKind | draw 引擎级后端：`Cpu` / `Gpu` / `Auto` / `Null`；**不**区分 Vulkan/D3D/GL |
| GpuEngine | GPU 帧调度引擎；委托 `RenderSession` + `GpuBackend` + `IGraphicsContext` |
| SoftwareEngine | CPU 回退引擎；`CpuBackend` + `IPresenter` |

<a id="多图形-api"></a>

## 多图形 API

| 术语 | 含义 |
|------|------|
| GraphicsBackend | #162 / P6.1：**具体 GPU API** 枚举（`Auto` / `OpenGlEs` / `Vulkan` / `D3D11` / `D3D12` / `Metal`）；factory 选型结果；供诊断与 native opt-in |
| GpuBackendKind | 与 `GraphicsBackend` 同义的旧设计名；实现采用 `GraphicsBackend` |
| IGraphicsContext | `native::traits::present`：surface 绑定、`swap_buffers(PresentDamage)`、DPR、`get_proc_address`；**各 GPU API 对上统一契约** |
| IPresenter | CPU 像素 presenter；SoftwareEngine 回退路径 |
| create_gpu_context | `native::factory`：按平台与 #162 创建 `Box<dyn IGraphicsContext>` |
| 图形 API 回退链 | 初始化时按平台优先级 probe；全失败 → SoftwareEngine；**无**每帧切换 |

详见 [rendering · 多图形 API](systems/rendering.md#多图形-api) · [#162](decisions.md#d162)。

## 平台

| 术语 | 含义 |
|------|------|
| Platform | OS 能力聚合 trait |
| Platform traits | `native::traits` 对外接口集合 |
| FakePlatform | 测试用内存 Platform |
| FakeEventSource | 注入 UiEvent 的 FIFO 队列 |
| EventBus | UiEvent 发布订阅 |
| EventLoopWaker | 跨线程唤醒 blocking `wait_event` / `wait_until`；见 [platform · 事件模型](systems/platform.md#ieventloop) |
| PresentDamage | 物理像素上屏 damage |
| WglContext / EglContext | Windows WGL / Linux EGL 的 `IGraphicsContext` 实现（**当前** OpenGL ES 路径） |

## 数据

| 术语 | 含义 |
|------|------|
| Settings key | 如 `theme_mode`、`brand_primary`；App 解析为 Theme |

## 测试

详见 [systems/testing.md](systems/testing.md) — 测试策略与术语上下文。

| 术语 | 含义 |
|------|------|
| 语义断言 | 派发事件后断言 SemanticEvent 结果 |
| paint snapshot | 绘制输出快照对比 |
| FakeClock | 已废弃作统称；见 **FakeTimer**（平台）与 **TestClock**（App，#139） |
