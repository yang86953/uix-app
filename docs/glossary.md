# 术语表

← [areas/architecture](areas/architecture.md) · 按需查阅，不必通读

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
| measure_children / layout_children | `WidgetLayout` 子项 preparation / arrange；前者生成 `Vec<LayoutChild>`，后者只消费快照 | [#174](decisions.md#d174) |
| ScrollView | ScrollView（旧文档 ScrollContainer） | [#104](decisions.md#d104) |
| VirtualScroll | `ui::foundation::VirtualScroll`；不经 prelude；Table/Tree/SelectableList/Select/TreeSelect 已接 VirtualListScroll | [layout · VirtualScroll](areas/systems/layout.md#virtual-scroll) |
| AppState + ComponentHandle | `AppState` registry + `ComponentHandle` snapshot/invalidate/emit | [#32](decisions.md#d32), [#101](decisions.md#d101) |
| HandlerTable key ComponentId | `SemanticEvent` / `HandlerTable` 已用 `ComponentId`；`WidgetId` 为树内别名 | [#10](decisions.md#d10), [#101](decisions.md#d101) |

---

## 架构

| 术语 | 含义 |
|------|------|
| 功能域 | 源码维度：`core` / `native` / `draw` / `ui` / `app` / `data` |
| 系统 | 文档维度；与源码目录不一一对应；索引见 [architecture · 系统索引](areas/architecture.md#系统索引) |
| 按需零闲置 | **核心理念 / 最高规则**（#105）；框架 **零维护**（#130）；见 [demand-driven](areas/systems/demand-driven.md) |
| L0 / L1 / L2 | 零闲置三层目标（#105）：零像素 / 零帧循环 / 最小脏区；见 [demand-driven · 三层目标](areas/systems/demand-driven.md#三层目标) |
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
| App | 应用入口：GUI（View 根）或 CLI；见 [application](areas/systems/application.md) |
| AppHandle | 含 `window_id`；`run_after` / `post_to_ui`（#132–#133）已按 session 路由（#141）；session 关闭会清理对应 Timer（#134） |
| AppRuntime | 内部 session 路由（Timer / post_to_ui / update_view / open_window）；见 [application · AppHandle](areas/systems/application.md#apphandle-生命周期) |
| EventLoopWaker | 跨线程唤醒 → [术语对照](#术语对照) · [platform · IEventLoop](areas/systems/platform.md#ieventloop) |
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
| 实现路线图 | #154：P0–P5 分阶段落地；[implementation · 分阶段路线图](areas/implementation.md#分阶段路线图) |
| P0 落地清单 | #157：P0 阶段目标与验收（历史）；详见 [implementation · 已落地阶段](areas/implementation.md#已落地阶段p0p5) |
| State 跨窗标脏 | #150：`State::set` fan-out 至各 `PaintBindSite` |
| SettingsService | 扁平字符串 KV 持久化；见 [data](areas/systems/data.md) |
| FakeTimer | 已有：`ITimer` 平台测试时钟；见 [testing · 测试时钟分层](areas/systems/testing.md#测试时钟分层) |
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
| measure / arrange | 布局两阶段：`measure_children` 按当前父 frame 生成 pass-local `LayoutChild`，`layout_children` 只定子项位置 → [layout · Measure/Arrange](areas/systems/layout.md#measure--arrange-两阶段) · [#174](decisions.md#d174) |
| measure | 唯一测量入口 → [术语对照](#术语对照) · [#103](decisions.md#d103) |
| preferred_size | 已废弃；见 measure |
| ScrollView | 滚动容器 → [术语对照](#术语对照) · [#104](decisions.md#d104) |
| VirtualScroll | 大列表虚拟滚动 helper；不经 prelude；Table/Tree/SelectableList/Select/TreeSelect 已接 VirtualListScroll → [layout · VirtualScroll](areas/systems/layout.md#virtual-scroll) |
| Constraints | `{ min, max, definite }` |
| LayoutChild | 单次父级 arrange 的不可变测量快照；不跨 convergence pass 缓存，`measured_size` 不回退旧 frame |
| intrinsic size | 组件在约束下的自然尺寸；Container 无显式 `width`/`height` 时由 `cached_content_size` 推导 → [layout · Intrinsic](areas/systems/layout.md#intrinsic-尺寸) · [#165](decisions.md#d165) |
| intrinsic_main | Flex 输入：主轴未显式指定时由子项撑开、跳过 shrink → [layout · Intrinsic](areas/systems/layout.md#intrinsic-尺寸) |
| effective_cross | Flex 交叉轴有效尺寸：容器 cross>0 用容器，否则 max(子项 cross) → [layout · Flex](areas/systems/layout.md#flex-布局) |
| cached_content_size | Container 布局后缓存的子 content 尺寸；供 `measure` fallback → [layout · Intrinsic](areas/systems/layout.md#intrinsic-尺寸) |
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
| BackendKind | draw 引擎级后端：`Cpu` / `Gpu` / `Auto` / `Null`；与 `RasterMode` / `PresentMode` / `GraphicsBackend` 正交轴对齐（#169），实际组合受 caps + registry 约束（#172）。见 [可组合渲染轴](areas/systems/graphics-backend-pluggable.md#可组合渲染轴) |
| GpuEngine | GPU 帧调度引擎；委托 `RenderSession` + `GpuBackend` + `IGraphicsContext` |
| SoftwareEngine | CPU 回退引擎；`CpuBackend` + `IPresenter` |

<a id="多图形-api"></a>

## 多图形 API

| 术语 | 含义 |
|------|------|
| GraphicsBackend | #162 / #172 / P6.5：**图形 API 身份**枚举（`Auto` / `OpenGlEs` / `Vulkan` / `D3d11` / `D3d12` / `Metal`）；**非**操作系统；各值为 `IGraphicsContext` 对等实现的标识。供诊断、App builder/env/Settings、native 候选表与 draw adapter registry 使用；普通 engine/pipeline 不按它散落分支。详见 [graphics-backend-pluggable · 图形 API 架构原则](areas/systems/graphics-backend-pluggable.md#图形-api-架构原则) |
| native/graphics | #164：`IGraphicsContext` 对等实现根目录；按 API 分树（`vulkan/`、`opengl/`、`d3d11/`、`metal/` …） |
| RenderPipelineProfile | **已删除**（#169）：旧 bundled 管线枚举；**不是**架构 mental model。分派依据 caps 允许的 `RasterMode` × `PresentMode`，不是完整笛卡尔积（#172）。见 [可组合渲染轴](areas/systems/graphics-backend-pluggable.md#可组合渲染轴) |
| RasterMode | #169：光栅轴 — `Cpu` / `GpuNative`（`native::traits::present`）；映射 `CpuBackend` 或 `RenderBackendRegistry`；与 `PresentMode`、`GraphicsBackend` 独立描述，合法组合受 caps 约束（#172） |
| PresentMode | #169：Present 轴 — `Swapchain` / `PixelUpload` / `CpuPresenter`（`native::traits::present`）；与 `RasterMode`、`GraphicsBackend` 独立描述，合法组合受 caps 约束（#172） |
| RenderBackendRegistry | #163：`draw/backend/registry.rs`；`GraphicsBackend` → GPU `RenderBackend` 构造表；`RasterMode::GpuNative` 时使用。OpenGL ES / D3D11 ✅；随后 Metal / D3D12 |
| GraphicsContextCaps | #163 / #169：native 侧能力快照 — `backend` + `raster` + `present` + `partial_present` + DPR。**不**替代 `GraphicsCapabilities`。见 [核心抽象](areas/systems/graphics-backend-pluggable.md#核心抽象) |
| 可组合组件模型 | #168 / #169 / #172：各层正交能力 + trait/registry 做受 capability 约束的组合；渲染轴独立描述，但只有 caps + registry 声明的稀疏组合合法。见 [decisions · #168](decisions.md#d168) · [#172](decisions.md#d172) |
| GraphicsBackendEntry | #163 / #169：registry 表行；含 `id`、`priority`、`raster`、`present`、`create` 与 `BackendStatus`（`Active` / `Planned` / `Disabled`）；见 [核心抽象](areas/systems/graphics-backend-pluggable.md#核心抽象) |
| bootstrap_graphics_engine | #163 / P6.7：`draw` 域 GPU 初始化**唯一** probe 入口；成功返回 `GpuBootstrap`；失败返回 `ProbeReport` 由 app 建 `SoftwareEngine` |
| GpuBackendKind | 与 `GraphicsBackend` 同义的旧设计名；实现采用 `GraphicsBackend` |
| IGraphicsContext | `native::traits::present`：surface 绑定、`swap_buffers(PresentDamage)`、`present(PresentFrame)`、DPR、`get_proc_address`；**各 GPU API 对上统一契约** |
| IPresenter | CPU 像素 presenter；SoftwareEngine 回退路径 |
| create_gpu_context | `native::factory`：单条目 `try_create_gpu_context`；**无** probe 循环；`Auto` 须 `draw::bootstrap_graphics_engine` |
| 图形 API 回退链 | 初始化时 probe：`draw::bootstrap_graphics_engine` 读 registry `gpu_probe_candidates` Auto 顺序；全失败 → SoftwareEngine；**无**每帧切换 |

详见 [rendering · 多图形 API](areas/systems/rendering.md#多图形-api) · [#162](decisions.md#d162) · [graphics-backend-pluggable · 图形 API 架构原则](areas/systems/graphics-backend-pluggable.md#图形-api-架构原则)。

## 平台

| 术语 | 含义 |
|------|------|
| Platform | OS 能力聚合 trait |
| PlatformId | registry 元数据：标识**当前二进制目标 OS**；用于过滤 `GraphicsBackendEntry` 与 Auto probe 顺序。**非** `GraphicsBackend`（API 身份）；上层不可见 |
| Platform traits | `native::traits` 对外接口集合 |
| FakePlatform | 测试用内存 Platform |
| FakeEventSource | 注入 UiEvent 的 FIFO 队列 |
| EventBus | UiEvent 发布订阅 |
| EventLoopWaker | 跨线程唤醒 blocking `wait_event` / `wait_until`；见 [platform · 事件模型](areas/systems/platform.md#ieventloop) |
| PresentDamage | 物理像素上屏 damage |
| WglContext / EglContext / VulkanContext | OpenGL ES / Vulkan 的 `IGraphicsContext` 实现；位于 `native/graphics/opengl/`（wgl、egl）与 `native/graphics/vulkan/` |
| D3d11Context | Windows D3D11 的 `IGraphicsContext`；caps `GpuNative` × `Swapchain`（soft Canvas2D + RTV；原生几何着色器 backlog）；位于 `native/graphics/d3d11/` |

## 数据

| 术语 | 含义 |
|------|------|
| Settings key | 如 `theme_mode`、`brand_primary`；App 解析为 Theme |

## 测试

详见 [areas/systems/testing.md](areas/systems/testing.md) — 测试策略与术语上下文。

| 术语 | 含义 |
|------|------|
| 语义断言 | 派发事件后断言 SemanticEvent 结果 |
| paint snapshot | 绘制输出快照对比 |
| FakeClock | 已废弃作统称；见 **FakeTimer**（平台）与 **TestClock**（App，#139） |
