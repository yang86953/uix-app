# 渲染系统

← [Main](../architecture.md) · 系统 **#7** · 功能域：`draw` · `app`（ScenePaint 桥接）

> ScenePaint 契约绘制；局部重绘上屏。不知具体组件类型。**须符合** [按需零闲置](demand-driven.md) L0/L1/L2（#105、#107、#122、#129）。

## 索引

| 主题 | 章节 | 决策 |
|------|------|------|
| 引擎 | [引擎](#引擎) | #59 #70 #163 |
| 多图形 API | [多图形 API](#多图形-api) | #162 #163 #164 #168 #169 |
| 场景与合成 | [场景与合成](#场景与合成) | #82 #122 #129 |
| 失效队列 | [管线与失效](#管线与失效) | #86 #87 #122 #129 |
| 帧渲染 | [FrameRenderer](#framerenderer) | #59 |
| 资源服务 | [字体与图像](#字体与图像) | #42 #65 |
| 空间几何 | [空间几何](#空间几何) | #70 |
| 动画帧 | [动画帧](#动画帧) | #83 |

**关联**：[application](application.md) · [component](component.md) · [theme-style](theme-style.md) · [foundation](foundation.md) · [platform](platform.md) · [demand-driven](demand-driven.md)

---

## 引擎

> GPU 选型与 probe 流程 → [graphics-backend-pluggable · 架构总览](graphics-backend-pluggable.md#架构总览) · [核心抽象](graphics-backend-pluggable.md#核心抽象)。下文列 **draw 侧 engine 种类**。

### 选择与回退（#59）

> 引擎由 `RasterMode` × `PresentMode` 表驱动装配（[#168](../../decisions.md#d168) [#169](../../decisions.md#d169)）— 见 [graphics-backend-pluggable · 可组合渲染轴](graphics-backend-pluggable.md#可组合渲染轴)。

| Engine | Raster × Present | 光栅组件 | Present 组件 | 备注 |
|--------|------------------|----------|--------------|------|
| **GpuEngine** | `GpuNative` × `Swapchain` | `RenderBackendRegistry` → `GpuBackend` / `D3d11Backend` | `swap_buffers` / swapchain | OpenGL ES ✅；D3D11 ✅（原生 fill/stroke/glyph atlas + soft blit） |
| **PresentUploadEngine** | `Cpu` × `PixelUpload` | `CpuBackend` | `IGraphicsContext::present(PixelBuffer)` | Vulkan / Metal ✅ |
| **SoftwareEngine** | `Cpu` × `CpuPresenter` | `CpuBackend` | `IPresenter` | GPU probe 全失败回退 |
| **NullEngine** | — | 无 | 无 | 测试 |

`App::run_gui` 经 `create_preferred_engine` 完成 probe + 按 caps 轴组合装配；绘图层裸 `RenderSession::new(BackendKind::Auto)` 在没有平台 GPU context 注入时等价于 `Cpu`；需要 GPU 时须先绑定平台图形上下文再切换 `BackendKind::Gpu`。

### GraphicsEngine 契约

`begin_frame(UpdateStrategy)` → 绘制 → `end_frame(&DamageRegion)` → `RenderOutcome`。

| UpdateStrategy | 含义 |
|----------------|------|
| `FullRedraw` | 全帧清除重绘 |
| `DirtyRects(rects)` | 局部 clip + 重绘 |

后端不支持 partial → `normalize_strategy` 降级 Full（`pipeline/frame.rs`）。

### Present 规则

| 条件 | 行为 |
|------|------|
| Paint/Composite 脏 + 非 Idle | `Present(damage)` → platform presenter |
| 仅 Layout 脏 | 不 render、不 present |
| 无脏区 | `RenderOutcome::Idle`，跳过 present |

---

<a id="多图形-api"></a>

## 多图形 API

> **设计目标**（#162）：在 `native::traits` / `draw::traits` 稳定契约之下，支持 **多种底层图形 API**；`app` / `draw` / `ui` **不得**依赖具体 API（OpenGL、Vulkan、D3D、Metal 等），仅通过 trait 与工厂选型。
>
> **可插拔 registry**（#163、#164）与架构原则 → [graphics-backend-pluggable.md](graphics-backend-pluggable.md#图形-api-架构原则)。初始化数据流见 [架构总览](graphics-backend-pluggable.md#架构总览)。

### 分层抽象

> **#168 · #169**：下列类型为 **正交组件**；Engine 类 = `RasterMode` × `PresentMode` 的组装结果。详见 [可组合渲染轴](graphics-backend-pluggable.md#可组合渲染轴)。

| 层 | 类型 | 职责 | 上层可见 |
|----|------|------|----------|
| **native** | `IGraphicsContext` | 图形 API peer：surface、`present(PresentFrame)`、DPR | draw bootstrap；app **不**直接持有 |
| **draw** | `RenderBackend` + `CpuBackend` / `GpuBackend` | **光栅**组件（CPU / GPU 对等） | 引擎内部 |
| **draw** | `PresentUploadEngine` / `GpuEngine` / `SoftwareEngine` | 按轴组合装配的 engine | `app` FrameRenderer |
| **native** | `IPresenter` | CPU 像素上屏（GDI / SHM / CALayer） | `SoftwareEngine` 回退 |
| **draw** | `GraphicsEngine` | `begin_frame` / `end_frame` / `UpdateStrategy` | `app` FrameRenderer |
| **draw** | `BackendKind` | `Cpu` / `Gpu` / `Auto` / `Null`；**不**暴露具体 GPU API | 引擎内部 / 测试 |
| **native** | `GraphicsBackend` | API 身份枚举（诊断 / opt-in） | factory；**非** prelude 稳定 API |
| **native** | `RasterMode` / `PresentMode` | 光栅轴 / present 轴；`GraphicsContextCaps` 字段 | factory / engine 装配 |

`BackendKind::Gpu` 表示「尝试 GPU 路径」；具体 API 由 registry probe；**光栅与 present 组合**由 `caps().raster` × `caps().present` 决定。

### 候选 API 与平台矩阵

> 下表为 **各 OS 编译收录**；**Auto 顺序**由 registry 表驱动。非上层架构轴。Auto 链与正交轴摘要 → [graphics-backend-pluggable · 核心抽象](graphics-backend-pluggable.md#核心抽象)。

| 平台 | 主选 | 次选 | **当前已实现** | CPU 回退 |
|------|------|------|----------------|----------|
| **Windows** | Direct3D 11 | OpenGL ES（WGL） | ✅ D3D11（`GpuNative` × `Swapchain`，原生 fill/stroke/glyph）+ OpenGL ES（`GpuNative` × `Swapchain`，WGL） | ✅ SoftwareEngine（GDI） |
| **Linux** | Vulkan | OpenGL ES（EGL） | ✅ Vulkan（`Cpu` × `PixelUpload`）+ OpenGL ES（`GpuNative` × `Swapchain`，EGL / Wayland） | ✅ SoftwareEngine（SHM） |
| **macOS** | Metal | — | ✅ Metal（`Cpu` × `PixelUpload`，feature `metal`）+ AppKit SoftwareEngine bootstrap | ✅ SoftwareEngine bootstrap |
| **Web**（远期） | WebGPU | — | ❌ | — |

图例：**✅ 已实现** · **规划** 为 backlog，见 [implementation · P6 生产级框架](../implementation.md#p6-生产级框架)。

### 选型与回退链（#162）

选型在 **窗口 / 引擎初始化时一次性完成**；**禁止**每帧探测或切换 API（#105）。probe 流程、Auto 链、配置优先级 → [graphics-backend-pluggable · 核心抽象](graphics-backend-pluggable.md#核心抽象) · [配置入口](#配置入口-p65-已落地)。D3D12 context 与 Metal **native raster** → [implementation · P6](../implementation.md#p6-生产级框架)。

### draw 域约束

- `draw` **不** `use native::backends::*` 或 `native::graphics::*`；仅 `IGraphicsContext` trait object。
- `GpuBackend` / `canvas_2d` 通过 `get_proc_address` 加载 GL 函数；非 GL **GPU 光栅** backend 经 `RenderBackendRegistry` 注册（OpenGL ES ✅；D3D11 ✅ 原生 fill/stroke/glyph/gradient/path/shadow + soft blit）。Vulkan/Metal 当前 caps 为 `Cpu` × `PixelUpload`；**下一代码优先**：D3D11 复杂 path；随后 Metal / D3D12。**非**「这些 API 只能 CPU 光栅」（#168）。
- `ScenePaint`、LayerTree、InvalidationQueue **与** GPU API 无关；局部重绘 damage 几何仍来自 `core::damage`。

### 配置入口（P6.5 已落地）

| 入口 | 说明 |
|------|------|
| App builder | `.graphics_backend(GraphicsBackend::Auto)` — 显式 builder 优先级最高 |
| 环境变量 | `UIX_GRAPHICS_BACKEND=vulkan` — 开发 / CI 覆盖；仅初始化时读取 |
| Settings | `graphics_backend` 或 `uix.graphics_backend`；仅 `.settings(path)` opt-in load 后参与选型 |
| 运行时只读 | factory 记录最终 `GraphicsBackend`；**不可**热切换 |

优先级：App builder > 环境变量 > Settings > `Auto`。无效值记录 warning 后继续读取下一来源；选型仍只在窗口 / 引擎初始化时完成一次。

公开 API 形状见 [public-api](public-api.md) 与 [#162](../../decisions.md#d162)。

**关联**：[platform · 窗口与呈现](platform.md#窗口与呈现) · [platform · 工厂与后端](platform.md#工厂与后端) · [implementation · P6](../implementation.md#p6-生产级框架)

---

<a id="场景与合成"></a>

## 场景与合成

### ScenePaint

draw 层 trait（`compositor/scene_paint.rs`）；**不依赖 ui**。app 层 `WidgetTree` 实现。

Compositor 只通过 ScenePaint 读树：frame、children、clip、scroll、dirty、paint callback。

### LayerTree

结构同步（`tree_version` 变化时 rebuild）：

| LayerNode | 条件 | 渲染 |
|-----------|------|------|
| **Picture** | `PicturePolicy::Eligible` **且** `node_count≥8` **且** `est_pixels≥65536`（#122、#129） | 离屏光栅化 → blit |
| **ClipRect** | `children_clip` 存在 | Content → clip+scroll → children → AfterChildren |
| **Direct** | 不满足 Picture 条件 | Content → children |

Picture 由框架 **自动推断** Policy（metadata + runtime 合并 #136）+ 自适应阈值；**无**调用方黑名单。当前由 Container/Grid/Space/Layout shell、Breadcrumb/Form shell 与 Divider/Icon/Avatar/Badge、Empty/Tag/Descriptions/Result/Alert/Timeline/Skeleton/List/Chart/QRCode/Watermark 等静态高收益组件声明 Eligible，实际启用仍受运行时信号、node_count 与像素阈值约束。详见 [demand-driven · PicturePolicy](demand-driven.md#picturepolicy-自动推断122129) · [component · PicturePolicy 元数据](component.md#picturepolicy-元数据122)。

Build 时子节点按 `z_index` 排序。旧 Picture offscreen 按 node_id+bounds 复用。

### RenderObjectTree

并行 DisplayList 缓存（`render_object/tree.rs`）：

- 首帧后 Content pass 可 record+replay DisplayList
- `sync(scene)` 刷新 dirty 节点
- 与 LayerTree 协同：`paint_content` 走 cache，AfterChildren 仍 live paint

### Picture 离屏（#82）

```text
ensure_offscreen(w,h)
→ record DisplayList (或 replay cache)
→ render Direct/ClipRect 子树进 offscreen
→ blit 到屏幕 bounds
```

`MAX_OFFSCREEN_RETRY = 8`；失败回退 Direct 绘制。

### 视口裁剪

`viewport_transform.rs`：累积 scroll offset → content rect 投影到屏幕 → 与 dirty_region 求交决定是否绘制。

---

<a id="管线与失效"></a>

## 管线与失效

### Invalidation 类型

```rust
enum Invalidation {
    Layout(NodeId),
    Paint { id: NodeId, rect: Option<Rect> },  // None = full frame
    Composite { rect: Rect, scroll: Option<ScrollDelta> },
}
```

### InvalidationQueue 行为

| 操作 | 效果 |
|------|------|
| `push(Paint)` | 同 node 矩形 merge |
| `has_layout()` | 是否需 layout  pass |
| `has_paint_or_composite()` | 是否需 render pass |
| `dirty_region()` | → `DirtyRegion`（Paint + Composite rects） |
| `clear()` | 帧末 `reset_invalidation` |

WidgetTree（`tree_dirty.rs`）共享 `InvalidationQueueHandle`；State 绑定通过 `invalidate_paint_handle` 跨线程安全入口（仍主线程消费）。

### Damage 流转

```text
InvalidationQueue.dirty_region()
    → FrameRenderer.compute_damage()
    → DamageRegion (padded partial rects)
    → begin_frame(D DirtyRects)
    → end_frame → PresentDamage
    → IPresenter / IGraphicsContext
```

几何类型定义在 `core::damage`（见 [foundation](foundation.md)）。

> **实现注记**：`Invalidation::Composite` 与 `FrameRenderer` 的 scroll_region **memmove** API（`tree.drain_scroll_region_move`、`Canvas2D::scroll_region`）已存在；Wheel、键盘、滚动条拖拽与程序化 ScrollView 滚动已写入 Composite exposed strip，并避免整 viewport Paint invalidate；新增滚动来源须复用该路径。

---

## FrameRenderer

`render_frame(engine, scene: &ScenePaint, input)` 顺序：

```text
1. 归一化 dirty（首帧 / full_frame / 无 partial 能力 → full；首帧后空 dirty 且无 Composite → Idle）
2. tree_version 变 → LayerTree.build + sweep_orphaned_offscreens
3. LayerTree.update_dirty(scene)
4. RenderObjectTree.sync(scene)
5. scroll_region memmove（如有 Composite scroll）
6. compute_damage → UpdateStrategy
7. engine.begin_frame → Idle? early exit
8. LayerTree.render (Content + focus ring + debug HUD)
9. engine.end_frame(damage)
10. FrameRenderOutput { outcome: Present(damage), inv_source, tree_version }
```

Debug：`Ctrl+Shift+D` 切换 debug_mode（`UIX_DEBUG` 启动即开）。
- **边框**：仅绘制指针下的 hover 祖先链；尺寸标签只贴最深命中节点（避免满屏彩框与叠标签）。
- **标脏**：debug 下 `PointerMove` 仅在 `hit_test` 目标变化时全帧标脏（非每 move）。
- **HUD**：右上角可读计数 + `toggle: Ctrl+Shift+D` 提示。
不用 F12：Windows 调试器下 F12 会触发系统 `DebugBreak`（`DbgBreakPoint`），与应用无关。
`LayerTree` 绘制前须 `PaintContext::set_debug_mode(true)`，否则边框路径 no-op。

> **实现注记**：`FrameRenderer` 已在 `rendered_first && dirty_region.is_empty() && scroll_move.is_none()` 时直接返回 `RenderOutcome::Idle`，避免空 dirty 被提升为全帧 present；首帧仍强制 full redraw。

---

<a id="字体与图像"></a>

## 字体与图像

| 服务 | 职责 |
|------|------|
| **FontService** | 系统字体加载（#65）、glyph 光栅化、TextLayout；缺字 tofu（`TOFU_GLYPH_ID`）；命中/光标按 **字符下标** |
| **ImageService** | 解码、BitmapHandle / ImageSlot 管理 |

文本绘制：`TextBackend` trait；ColorValue + TypographyToken 在 render 时 resolve。

Bitmap 字体（内置）用于 debug / 回退；正常路径走系统字体栈。

---

<a id="空间几何"></a>

## 空间几何

| 模块 | 用途 |
|------|------|
| `core::{Point, Size, Rect, EdgeInsets}` | 2D 逻辑坐标 |
| `draw::spatial` | PhysicalBox、Mat4、AABB3D、Ray3D — 3D 命中与变换 |
| `scale_factor` | HiDPI（#70）：布局/绘制逻辑 px；present 前 scale 到物理像素 |

禁止在 draw/ui 重复定义几何类型（AGENTS.md）。

---

<a id="动画帧"></a>

## 动画帧

设计（#83）：`AnimationRegistry` 持有 active node 集合；主循环在 `tick_effects` **之前**调用 `tree.update(dt)`，由各 widget 推进 `Animatable`（见 [component · 动画](component.md#动画)、[application · 主循环](application.md#主循环)）。

> **实现注记**：`WidgetAnimation` 能力与 `tree.update(dt)` 已接入；动画更新会按 widget 的 `dirty_bounds` 做窄 Paint 标脏，单窗与副窗 event loop 在仍有动画时以 `Animation(id)` 登记下一帧 Registry deadline，due 帧只推进到期 id。内置 Animation 源见 [component · 动画](component.md#动画)；多窗运行期 frame drain 已接。

---

## draw 域模块图

```text
draw/
├── traits/          GraphicsEngine, Canvas2D, TextBackend
├── engine/          bootstrap.rs（GPU probe）、factory.rs（profile 分派）、SoftwareEngine (cpu)
├── backend/         RenderBackend 抽象、registry.rs（backend 配对）
├── gpu_engine/      GpuEngine
├── pipeline/        InvalidationQueue, FrameRenderer, AnimationRegistry
├── compositor/      ScenePaint, LayerTree, Picture, viewport_transform
├── render_object/   DisplayList cache
├── font/            FontService, text backends
├── image/           ImageService
├── painting/        PaintContext, ThemeTokens
└── spatial/         Mat4, PhysicalBox
```

`draw` **不依赖** `ui` 或 `app`；ScenePaint 是唯一向上暴露的树只读接口。
