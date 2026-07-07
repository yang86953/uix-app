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
| 平台 | [平台](#平台) |
| 数据 | [持久化](#数据) |
| 测试 | [测试](#测试) |

---

## 架构

| 术语 | 含义 |
|------|------|
| 功能域 | 源码维度：`core` / `native` / `draw` / `ui` / `app` / `data` |
| 系统 | 文档维度；与源码目录不一一对应；索引见 [Main · 系统索引](Main.md#系统索引) |
| 按需零闲置 | **核心理念 / 最高规则**（#105）；框架 **零维护**（#130）；见 [demand-driven](systems/demand-driven.md) |
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
| AppHandle | 设计：含 `window_id`；`run_after` / `post_to_ui`（#132–#133）；仅路由至本 session（#141）；drop 不 cancel Timer（#134） |
| on_start | #140：每 WindowSession 首帧前 `Fn(AppHandle)`；运行中 API 的 canonical 注入点 |
| post_to_ui | #133：跨线程 `FnOnce + Send` 投递；路由见 #141 |
| MainThreadQueue | #137：每 WindowSession FIFO；帧内 UiEvent → drain_due → drain_queue |
| TimerHandle | #132：opaque；`cancel` / drop → unregister |
| handler_generation | #135 / #138：View build 自动维护；Reconciler 比较以决定是否重绑 |
| capture 指纹 | #142：`StateSlotId`（#143）+ TypeId + Copy 值 + AppHandle.window_id |
| StateSlotId | #143：`State::new` 单调 id；clone 共享；**≠** `State::generation()` |
| open_window | #144：运行中创建副窗；返回新 `AppHandle` |
| AppState | **设计**（#145）：不替代 `State<T>`；mount 自动 register；跨窗共享 |
| ComponentHandle | **设计**（#145–#147）：只读 snapshot getter + invalidate/`emit`；mount 自动注册 |
| ComponentConfigSnapshot | #146：mount 时静态配置快照；Handle getter 数据源 |
| SnapshotSource | #151：自定义 widget 快照 trait；`component!` 默认自动 |
| Handle emit | #147：`ComponentHandle::emit` → `dispatch_semantic` |
| update_view | #149：`AppHandle::update_view` — 本 session 帧末 reconcile |
| snapshot(skip) | #152：`#[snapshot(skip)]` 字段属性；排除出 ComponentConfigSnapshot |
| reconcile 合并 | #153：`pending_root` 优先；State + update_view 同帧一次 reconcile |
| view_factory | #155：session 创建时固定 `Arc`；State reconcile 长期来源 |
| update_view vs factory | #156：`update_view` 仅 `pending_root`；不替换 factory |
| 实现路线图 | #154：P0–P5 分阶段落地；[roadmap · 分阶段路线图](roadmap.md#分阶段路线图) |
| P0 落地清单 | #157：按 `src/app/*` 路径的 P0 接线表；[roadmap](roadmap.md#p0-落地清单) |
| State 跨窗标脏 | #150：`State::set` fan-out 至各 `PaintBindSite` |
| SettingsService | 扁平字符串 KV 持久化；见 [data](systems/data.md) |
| FakeTimer | 已有：`ITimer` 平台测试时钟；见 [testing · 测试时钟分层](systems/testing.md#测试时钟分层) |
| TestClock | 设计：App `drain_due` 注入时钟（#139） |

## UI

| 术语 | 含义 |
|------|------|
| View DSL | `column`、`row`、`button`、`dynamic_label` 等声明式 API |
| ViewNode | View 构建产物；经 Reconciler 同步到组件树 |
| Widget | UI 组件实例 |
| ComponentId | Generational 稳定 ID（[#35](decisions.md#d35)、[#101](decisions.md#d101)）；布局 / 事件 / HandlerTable 语义键 |
| WidgetId | WidgetTree 内部源码别名：`core::ComponentId`（[#101](decisions.md#d101)） |
| component! | authoring 宏（[#20](decisions.md#d20)、[#102](decisions.md#d102)）；当前直接支持 `name + struct` 与 `struct` 入口 |
| component! | authoring 宏；支持 `name + struct` 与直接 `struct` 入口 |
| WidgetTree | 运行时组件树容器 |
| HandlerTable | 业务回调表；键/API 为 ComponentId；WidgetId 仅为 WidgetTree 内部同型别名（[#10](decisions.md#d10)、[#101](decisions.md#d101)） |
| State / Computed / Effect | 响应式原语；Effect 禁止直接改 UI；周期可用 **Timer API**（#132） |
| TimerHandle | #132：`run_after` / `run_interval` 返回值；cancel/drop 自动 unregister |
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
| measure | **设计**：`measure(constraints) -> Size`（[#29](decisions.md#d29)、[#103](decisions.md#d103)） |
| preferred_size | 旧测量入口，已由 `measure(constraints) -> Size` 取代（[#103](decisions.md#d103)） |
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
| AnimationRegistry | 动画帧驱动（设计） |
| FontService / ImageService | 字体与图像资源服务 |

## 平台

| 术语 | 含义 |
|------|------|
| Platform | OS 能力聚合 trait |
| Platform traits | `native::traits` 对外接口集合 |
| FakePlatform | 测试用内存 Platform |
| FakeEventSource | 注入 UiEvent 的 FIFO 队列 |
| EventBus | UiEvent 发布订阅 |
| PresentDamage | 物理像素上屏 damage |

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
