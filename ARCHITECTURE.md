# UIX 架构设计

> 最后更新: 2026-07-03
> 本文档是项目的实时架构地图，随代码变更同步更新。

---

## 一、项目概览

UIX 是一个 Rust 原生跨平台 UI 框架，采用组合优先（composition-over-inheritance）架构。支持 Windows（Win32）和 Linux（Wayland），纯 CPU 软件渲染 + 可选的 GPU 渲染后端。

---

## 二、Workspace 结构

```
uix workspace                   编译期边界（6 workspace crate + 1 proc-macro crate）
├── platform/   (uix-platform)  OS 抽象层
├── graphics/   (uix-graphics)  2D 渲染引擎
├── ui/         (uix-ui)        Widget 框架 + 响应式状态
│   ├── macros/ (uix-macros)    proc-macro crate（编译期独立，无运行时依赖）
├── app/        (uix-app)       应用入口 + 窗口生命周期 + DI
├── demo/       (uix-demo)      演示二进制
└── uix         (根 crate)      聚合重导出层
```

### 依赖拓扑

```
uix (根 crate)
 ├── re-exports → uix_app, uix_graphics, uix_platform, uix_ui
 │   └── 同时 re-export 宏：ui!, define_widget!, tree!
 │
 ├── uix_app ──→ uix_graphics, uix_platform, uix_ui
 │
 ├── uix_ui ──→ uix_graphics, uix_platform, uix_macros
 │
 ├── uix_graphics ──→ uix_platform
 │
 └── uix_platform（无内部 crate 依赖）

uix_macros（编译期 proc-macro，无运行时依赖）
uix_demo ──→ uix, uix_app, uix_graphics, uix_platform, uix_ui
```

### 各 Crate 职责

| Crate | 职责 | 核心导出 |
|-------|------|---------|
| **uix-platform** | OS 抽象：Win32/Wayland 窗口、事件、文件服务、日志、通知、设置、诊断 | `Platform`, `IPresenter`, `IEventLoop`, `IWindowProperties`, `Error`, `EventBus`, `UiEvent`, `Point/Size/Rect/EdgeInsets` |
| **uix-graphics** | 2D 渲染：软件引擎 + FrameGraph + 路径/字体/颜色 + 空间坐标系统 | `GraphicsEngine`, `Canvas2D`, `RenderingBackend`, `Color`, `Path`, `FrameGraph`, `FontService`, `SpatialContext` |
| **uix-ui** | Widget 框架：60+ 组件 + Flexbox/Grid 布局 + 主题/动画 + 响应式状态 + LayerTree | `WidgetComponent`, `WidgetLayout`, `WidgetRender`, `WidgetEventHandler`, `WidgetLifecycle`, `State`/`Computed`/`Effect`, `Animation`, `TokenProvider`, `LayoutEngine`, `WidgetTree` |
| **uix-app** | 应用入口：窗口管理 + CLI 解析 + DI 容器 | `App`, `AppMode`, `Window`, `Cli`, `Container` |
| **uix-macros** | proc-macro：声明式 UI 宏 | 无运行时导出，编译期提供 `ui!`, `define_widget!`, `tree!` |
| **uix（根）** | 聚合重导出 | `pub use uix_app as app; pub use uix_ui as ui;` + 宏重导出 |

---

## 三、核心架构

### 3.1 分层架构

```
┌─────────────────────────────────────────────────────┐
│  app 层（uix_app）                                   │
│  应用入口 + 窗口生命周期 + CLI + DI 容器              │
│  App { mode, window, cli, container }                │
└──────────────────┬──────────────────────────────────┘
                   │ 依赖 ui/graphics/platform 接口
                   ▼
┌─────────────────────────────────────────────────────┐
│  ui 层（uix_ui）                                     │
│  Widget 框架                                         │
│  ├─ WidgetTree — 组件树（组合/生命周期/事件分发）      │
│  ├─ LayerTree  — 合成树（Picture/ClipRect/Direct）   │
│  ├─ RenderLoop — 渲染事件循环（增量管线）             │
│  ├─ RenderContext — 渲染上下文（2D/3D 双路径）        │
│  ├─ State/Computed/Effect — 响应式状态               │
│  ├─ 60+ 内置 Widget 组件                             │
│  ├─ FlexLayout / GridLayout 布局引擎                 │
│  ├─ Animation 动画系统                               │
│  └─ TokenProvider — Ant Design 5 设计令牌体系         │
└──────────────────┬──────────────────────────────────┘
                   │ 通过 Canvas2D / GraphicsEngine 绘制
                   ▼
┌─────────────────────────────────────────────────────┐
│  graphics 层（uix_graphics）                          │
│  2D 渲染引擎                                         │
│  ├─ GraphicsEngine — 帧生命周期（begin/end frame）    │
│  ├─ Canvas2D — 2D 绘制原语（fill_rect/fill_circle/…）│
│  ├─ RenderingBackend — 像素存储/呈现                  │
│  ├─ SoftwareEngine (CPU) / 可扩展 GpuEngine           │
│  ├─ FrameGraph — 帧图 Pass 编排 + 资源管理            │
│  ├─ Rasterizer — 纯函数光栅化（fill/stroke/blit）     │
│  ├─ FontService — 字体加载/排版/光栅化                │
│  ├─ SpatialContext — 3D 空间坐标系统                  │
│  └─ 增量渲染：scroll_region + DirtyRects + Pass culling│
└──────────────────┬──────────────────────────────────┘
                   │ 依赖 OS 能力
                   ▼
┌─────────────────────────────────────────────────────┐
│  platform 层（uix_platform）                          │
│  OS 抽象层                                           │
│  ├─ Platform trait — 统一平台接口                     │
│  ├─ IPresenter — 像素呈现（Win32 GDI / Wayland SHM） │
│  ├─ IEventLoop — 事件循环                            │
│  ├─ IWindowProperties — 窗口属性                     │
│  ├─ IGraphicsContext — GPU 上下文（EGL）              │
│  ├─ Error/EventBus/诊断/日志/文件服务/通知/设置       │
│  ├─ Win32 实现（windows 模块）                        │
│  └─ Wayland 实现（linux 模块）                        │
└─────────────────────────────────────────────────────┘
```

### 3.2 核心数据流

#### 渲染管线

```
WidgetTree.update(dt)
  ├─ on_update() 推进动画/滚动
  ├─ dirty_rect() 计算变化区域 → 加入 dirty_region
  ├─ scroll_delta() 收集滚动增量
  └─ 返回 keep_polling（有动画/滚动进行中）
      ↓
WidgetTree.layout() —— 仅 dirty_traverse 遍历脏子树
      ↓
Geometry Pass:
  ├─ drain_scroll_deltas() → canvas.scroll_region() 移动已有像素
  ├─ begin_frame(DirtyRects) — 只清除脏区域
  ├─ LayerTree.render() — 裁剪到脏区域，只绘制相交 widget
  └─ end_frame()
      ↓
Overlay Pass:
  ├─ begin_frame(Overlay) — 不清除，叠加绘制
  └─ LayerTree.render_overlays() — 仅脏区域内 post_render
      ↓
tree.reset_dirty() → IPresenter.present() → 屏幕显示
```

#### 事件流

```
OS 事件 → IEventLoop 轮询 → pending_events 队列
                              ↓
WidgetTree.dispatch_event(ev) → 命中测试 → WidgetEventHandler.on_event()
                              ↓
                     EventBus.publish(ev) → 外部订阅者
```

### 3.3 架构决策记录

| 决策 | 方案 | 原因 |
|------|------|------|
| API 外观模式 | 每个 crate 有 `api/traits.rs` + `api/types.rs`，lib.rs `pub use api::*` | 内部模块重构不影响外部使用者；编译器错误信息更清晰 |
| Widget 能力位 | `WidgetCapabilities` 位标记 + `WidgetComponent` 上转型 | 避免上帝接口，按需实现 Layout/Render/Event/Lifecycle |
| LayerTree 在 ui 层 | 从 graphics 迁入，作为 WidgetTree ↔ GraphicsEngine 桥接 | 避免循环依赖，Picture 离屏缓冲生命周期统一管理 |
| 增量渲染优先 | scroll_region + DirtyRects + FrameGraph Pass culling | 非全帧重绘，CPU 渲染性能关键 |
| State thread-safe | `Arc<RwLock<T>>` + thread_local 依赖追踪 | Computed 自动追踪依赖，支持跨线程 |
| 2D 零成本 + 3D 按需 | RenderContext 同时提供直接 Canvas2D 调用和 SpatialContext 投影 | 2D 场景无开销，3D 场景按需启用 |
| 平台工厂函数 | `create_platform()` 编译期条件编译选择实现 | 调用方不需要关心平台分支 |

---

## 四、模块详解

### 4.1 platform 层

- **位置**: `platform/src/`
- **职责**: 封装 OS 差异，暴露统一抽象接口
- **核心 trait**:
  - `Platform` — 平台入口（创建窗口/事件循环/文件服务）
  - `IPresenter` — CPU 像素呈现（present/resize）
  - `IGraphicsContext` — GPU 图形上下文（EGL/GL）
  - `IWindowProperties` — 窗口属性（尺寸/最小最大/原生句柄）
  - `IEventLoop` — 事件循环
- **核心类型**: `Error`（分级错误码 + 上下文链）、`EventBus`（发布-订阅）、`UiEvent`
- **模块**:
  - `error.rs` — 错误码与 Error 结构体
  - `event.rs` / `event_bus.rs` — 事件类型与发布-订阅总线
  - `geometry.rs` — Point/Size/Rect/EdgeInsets
  - `log/` — 日志基础设施
  - `presenter.rs` — 像素呈现器接口（CPU/GPU）
  - `types/` — 平台层数据类型（key/input/console/display/system/status）
  - `shared/` — 跨平台共享实现 + 窗口/事件 API trait
  - `diagnostic/` — 错误收集/崩溃处理/恢复策略
  - `file_service.rs` / `notification.rs` / `settings.rs` — 业务服务（原独立 crate 迁入）
  - `windows/` — Win32 实现（GDI DIB 呈现）
  - `linux/` — Wayland 实现（libwayland-client + EGL/GLES 后端）
- **依赖 →**: 无内部 crate 依赖
- **被依赖 ←**: graphics, ui, app

### 4.2 graphics 层

- **位置**: `graphics/src/`
- **职责**: 2D 渲染引擎、帧图编排、空间坐标系统
- **核心 trait**:
  - `GraphicsEngine` — 引擎生命周期（initialize/shutdown/begin_frame/end_frame）+ 离屏管理
  - `Canvas2D` — 2D 绘制原语（fill_rect/fill_circle/fill_path/draw_text/blit 等）
  - `RenderingBackend` — 像素缓冲管理（surface_size/pixels/clear_rect/present/copy_region）
- **核心类型**: `Color`, `Path`, `UpdateStrategy`（FullRedraw/DirtyRects/Overlay）, `RenderOutcome`（Idle/Present）
- **模块**:
  - `engine/cpu/` — SoftwareEngine（CPU 光栅化实现）+ Canvas2D 实现
  - `engine/cpu/software.rs` — SoftwareEngine 主入口
  - `engine/cpu/canvas_2d.rs` — Canvas2D trait 的 CPU 实现
  - `engine/cpu/raster_renderer.rs` — CPU 光栅化核心（填色/描边/渐变）
  - `engine/cpu/pixel_surface.rs` — 像素表面管理
  - `engine/cpu/noop_canvas_2d.rs` — 空操作 Canvas2D（用于测试/非活跃帧）
  - `frame_graph/` — FrameGraph Pass 编排、编译、资源管理、Pass culling
  - `rasterizer/` — 纯函数光栅化器（fill/stroke/blit/形状）
  - `spatial/` — 空间坐标系统（Mat4/Vec3/Vec4/AABB3D/Ray3D/PhysicalUnit/SpatialContext）
  - `text_backends/` — 文本渲染后端
  - `traits/` — `GraphicsEngine`, `Canvas2D`, `RenderingBackend` 定义
  - `api/` — 稳定公开 API（traits + types re-export）
  - `font_service.rs` — 字体加载/排版/glyph 光栅化
  - `color.rs` — Color 类型
  - `path.rs` — 路径构造（move_to/line_to/quad_to/cubic_to/close）
  - `stroker.rs` — 路径描边计算
  - `flattener.rs` — 曲线展平
  - `blur.rs` — 模糊效果
  - `types.rs` — 值类型（BlendMode/FillRule/FontHandle/ImageHandle 等）
  - `null_engine.rs` — 空引擎（用于无渲染环境）
- **依赖 →**: uix-platform
- **被依赖 ←**: ui, app

### 4.3 ui 层

- **位置**: `ui/src/`
- **职责**: Widget 框架、组件库、布局、动画、响应式状态、主题
- **核心 trait**:
  - `WidgetComponent` — 组件核心标识（capabilities + 上转型 + build）
  - `WidgetLayout` — 布局（preferred_size/flex_grow/layout_children）
  - `WidgetRender` — 渲染（render/post_render/dirty_rect）
  - `WidgetEventHandler` — 事件（on_event/hit_test/scroll_delta）
  - `WidgetLifecycle` — 生命周期（on_init/on_mount/on_unmount/on_update）
  - `TokenProvider` — 设计令牌（IColorTokens/ITypographyTokens/ISpacingTokens/IBoxShadowTokens 聚合）
  - `LayoutEngine` — 布局引擎（FlexLayout/GridLayout 实现此 trait）
  - `TextRenderer` — 文本渲染服务
  - `DebugRenderer` — 调试渲染服务
- **核心类型**: `WidgetNode`, `BoxedWidget`, `WidgetTree`, `State<T>`, `Computed<T>`, `Effect`, `RenderContext`, `Style`, `WidgetCapabilities`, `WidgetEvent`, `EventResult`
- **模块**:
  - `widget/` — Widget trait 定义、BoxedWidget、WidgetTree、WidgetNode、事件类型
  - `widgets/` — 60+ 内置组件（见第四节末完整列表）
  - `layout/` — FlexLayout + GridLayout 引擎
  - `animation/` — 动画系统（Animatable trait + Easing + Animation + Driver）
  - `theme/` — 设计令牌体系（Ant Design 5 风格，支持亮/暗切换）
  - `state.rs` — 响应式状态（State/Computed/Effect + 自动依赖追踪）
  - `render_context.rs` — 渲染上下文（持有 Canvas2D + SpatialContext + TextRenderService）
  - `render_loop.rs` — 渲染事件循环（增量管线编排）
  - `layer.rs` — LayerTree 合成树（Picture/ClipRect/Direct 节点）
  - `style.rs` — Style 结构体（布局/视觉/文本属性集合）
  - `macros.rs` — `ui!`, `define_widget!`, `tree!` 宏的运行时部分
  - `text_render.rs` — TextRenderService 实现
  - `debug_render.rs` — 调试绘制边框/布局线
  - `managers/` — EventManager/FocusManager/DragManager/StateManager 等
  - `virtual_scroll.rs` — 虚拟滚动支持
  - `children.rs` — 子节点管理
  - `config_provider.rs` — 配置提供者
  - `context.rs` — 上下文类型
  - `clipboard.rs` — 剪贴板支持
  - `focus_trap.rs` — 焦点陷阱
  - `locale.rs` — 国际化
  - `widget_builder.rs` — WidgetBuilder 构建器
  - `macros/` （子 crate） — proc-macro crate（`ui!`, `define_widget!`, `tree!` 的编译期部分）
- **依赖 →**: uix-graphics, uix-platform, uix-macros
- **被依赖 ←**: app

### 4.4 app 层

- **位置**: `app/src/`
- **职责**: 应用入口、窗口生命周期、CLI 解析、DI 容器
- **核心类型**: `App`, `AppMode`（GUI/CLI）, `Window`, `Cli`, `Container`
  - `App` — 应用配置入口（mode/window/cli/container/exit_code）
  - `Window` — 窗口生命周期管理（调用 uix_platform 创建平台窗口）
  - `Cli` — CLI 命令注册与执行
  - `Container` — 类型擦除 DI 容器（singleton 注册/解析）
- **约束**: app 层只负责应用配置和入口，不管理渲染引擎/字体/主题/渲染循环（这些归 ui 层）
- **依赖 →**: uix-graphics, uix-platform, uix-ui
- **被依赖 ←**: demo, uix（根 crate）

### 4.5 内置 Widget 组件（60+）

| 类别 | 组件 |
|------|------|
| Basic (10) | Button, Card, Container, Divider, Icon, Image, Label, Space, Spin, Typography |
| Form (12) | AutoComplete, Checkbox, ColorPicker, DatePicker, Dropdown, Form, Input, InputNumber, Radio, Rate, Select, Segmented, Slider, Switch, TimePicker |
| Navigation (8) | Breadcrumb, Menu, Nav, Pagination, Steps, Tabs, Timeline, Tree, TreeSelect |
| Data Display (14) | Avatar, Calendar, Chart(Bar/Line/Pie), Descriptions, Grid, List, Progress, Result, Skeleton, Table, Tag, Empty |
| Feedback (7) | Alert, Drawer, Message, Modal, Notification, Popconfirm, Popover, Tooltip |
| Others (12) | Collapse, FloatButton, ScrollView, Badge, Misc, Affix, Anchor, BackTop, Carousel, Cascader, Mentions, Splitter, SelectableList, ThemeToggle |

---

## 五、关键设计约定

### 5.1 Widget 能力体系
- Widget 行为拆分为四个独立 trait（Layout/Render/Event/Lifecycle），通过 `WidgetCapabilities` 位标记声明
- `define_widget!` 宏自动生成上转型代码（`as_layout()` / `as_render()` 等）
- BoxedWidget 持有 `dyn WidgetComponent`，在调用时动态向下转型

### 5.2 渲染策略
- 每帧两步：Geometry Pass（清除+绘制）→ Overlay Pass（叠加）
- scroll_region：ScrollView 滚动时 memmove 已有像素，只重绘新暴露区域
- DirtyRects：只清除和重绘变化的矩形区域
- FrameGraph Pass culling：输入未变 + 输出无消费 → 跳过 Pass

### 5.3 状态管理
- `State<T>` — 响应式值（`set()` 触发通知）
- `Computed<T>` — 自动追踪依赖的派生值（惰性求值 + 缓存 + 无效化）
- `Effect` — 响应式副作用（依赖变化时触发）
- 使用 thread_local 依赖追踪，同一线程内 Computed 的 get() 自动注册依赖

### 5.4 LayerTree 合成
- 三种节点：Picture（离屏缓存）、ClipRect（裁剪）、Direct（直接绘制）
- Picture 节点支持嵌套 RepaintBoundary
- build 时按 z_index 预排序子节点，渲染时零排序开销
- 离屏创建失败退避（retry_count 机制）

### 5.5 API 外观模式
- 每个 crate 的 `api/` 模块定义公开契约（trait + 类型）
- 内部模块只保留实现，不对外暴露
- 重构时保持 api 签名不变即可

### 5.6 代码约束
- deny(clippy::unwrap_used), deny(clippy::expect_used)
- 每个 Rust 文件 ≤ 900 行
- 中文注释
- 组合优于继承，无 trait 继承模拟 OOP

---

## 六、外部依赖

| 依赖 | 用途 | 归属 crate |
|------|------|-----------|
| `windows` 0.62 | Win32 API 绑定 | uix-platform |
| `wayland-client` 0.29 | Wayland 协议客户端 | uix-platform |
| `wayland-protocols` 0.29 | Wayland 扩展协议 | uix-platform |
| `libc` | Linux C 库绑定 | uix-platform |
| `glow` | OpenGL 函数加载 | uix-platform, uix-graphics |
| `khronos-egl` | EGL 绑定 | uix-platform |
| `ab_glyph` | 字体加载与 glyph 光栅化 | uix-graphics |
| `syn` / `quote` / `proc-macro2` | proc-macro 基础设施 | uix-macros |

---

## 七、常见架构问题

### Q: 新增一个 widget 怎么做？
1. 在 `ui/src/widgets/` 下新建文件
2. 用 `define_widget!` 宏定义组件，实现需要的 trait（Layout/Render/Event/Lifecycle）
3. 在 `ui/src/widgets/mod.rs` 注册
4. 如果要通过 `ui!` 宏使用，在 `ui/src/macros.rs` 的组件表中注册

### Q: 新增一个平台后端？
1. 在 `platform/src/` 下新建目录（如 `macos/`）
2. 实现 `Platform`、`IPresenter`、`IEventLoop`、`IWindowProperties` 等 trait
3. 在 `platform/src/lib.rs` 的条件编译中添加新平台分支
4. 在 `create_platform()` 工厂函数中添加新平台分支

### Q: 2D 和 3D 绘制怎么选？
- 纯 2D：直接调用 `ctx.fill_rect()` / `ctx.fill_path()` 等 — 零开销
- 需要 3D 变换：通过 `ctx.spatial()` 获取 `SpatialContext`，设置 transform 后绘制

### Q: WidgetTree 和 LayerTree 的区别？
- **WidgetTree**：逻辑树 — 管理组件生命周期、事件分发、布局
- **LayerTree**：渲染树 — 管理离屏缓冲、裁剪、绘制顺序。由 WidgetTree 构建产生
