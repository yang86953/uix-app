# UIX 绘图层优化方案

> 文档版本：2026-07-04  
> 状态：**分阶段执行手册**（完整解决全部已知问题）  
> 关联文档：[ARCHITECTURE.md](../ARCHITECTURE.md)  
> **执行方式**：按 Phase 0 → 8 顺序推进，每 Phase 结束须通过验收再进入下一 Phase。

---

## 一、设计原则

### 1.1 渲染调度原则

绘图层优化的核心目标可以概括为三条不变量：

| 不变量 | 含义 |
|--------|------|
| **0 帧渲染** | 无数据变更时，零 layout、零 paint、零 present |
| **局部渲染** | 有数据变更时，只处理受影响的最小子树与最小屏幕区域 |
| **必要且正确** | 每个阶段由明确的 invalidation 触发，不靠兜底、猜测或 force 路径 |

> **渲染是数据变更的副作用，不是时间驱动的循环。**

动画、滚动惯性属于「数据在变」（时间驱动的 State），应当触发渲染；静止 UI 不应产生任何帧成本。

### 1.2 能力归属原则

**凡与「如何把内容变成像素并提交到屏幕」相关的能力，归属 `draw` 域（绘图层）。**

- UI 层（`ui` 域）只负责：Widget 声明、布局、事件、主题令牌消费
- Platform 层（`native` 域）只负责：OS 窗口、原始 surface 句柄、系统级 present 原语
- 绘图层拥有：invalidation 调度、合成、DisplayList、帧生命周期、文本光栅化编排、damage 计算

不允许绘图层能力长期散落在 UI event_loop 或 WidgetTree 中。

### 1.3 后端可插拔原则

**渲染后端（CPU / GPU / Null）应可运行时动态切换；后端只实现差异部分，逻辑层共享一套代码。**

| 层次 | 职责 | 切换影响 |
|------|------|----------|
| **RenderPipeline**（共享） | invalidation、合成、DisplayList、帧策略、damage | 无 |
| **RenderBackend**（差异） | surface 分配、像素写入、present/swap、能力声明 | 可热切换 |
| **Rasterizer**（共享/可选加速） | 路径填充、渐变、阴影等算法 | GPU 可覆盖热点 |

目标：`SoftwareEngine` 与 `GpuEngine` 不再各自复制 `begin_frame` / 裁剪 / 清除策略，而是共用 `RenderPipeline`，仅注入不同的 `RenderBackend` 实现。

---

## 二、当前架构概览

### 2.1 分层与数据流

```text
app 层          窗口生命周期、CLI、DI
ui 层           WidgetTree + LayerTree + event_loop
graphics 层     GraphicsEngine + Canvas2D + FrameGraph + Rasterizer
platform 层     IPresenter、IEventLoop、OS 抽象
```

当前每帧渲染路径（简化）：

```text
OS 事件
  → WidgetTree.dispatch_event()
  → WidgetTree.update(dt)          // 动画/滚动推进，收集 dirty_region
  → WidgetTree.layout()            // dirty_traverse 脏子树
  → scroll_region memmove          // 滚动像素移动
  → FrameGraph.compile()           // Pass 裁剪
  → begin_frame(DirtyRects)
  → LayerTree.render()             // 空间 + 脏剪枝，widget.render() 直写 Canvas2D
  → begin_frame(Overlay)
  → LayerTree.render_overlays()    // post_render
  → IPresenter.present(damage)
  → WidgetTree.reset_dirty()
```

### 2.2 已具备的能力

- **多矩形脏区域**（`DirtyRegion`，超过 16 个矩形合并为 bounds）
- **滚动 memmove**（`canvas.scroll_region` + strip 重绘）
- **Layout 脏遍历**（`dirty_traverse`，非 full_frame 时 O(脏子树)）
- **LayerTree 空间剪枝**（frame 与 dirty_region 无交集时跳过）
- **FrameGraph Pass 裁剪**（输入未变 + 输出无消费 → cull）
- **RepaintBoundary 结构**（Picture 节点 + 离屏句柄复用逻辑）

### 2.3 采用的渲染范式（现状）

当前系统**同时混合**了四种范式，但未通过统一中间表示（IR）对齐：

| 范式 | 体现 | 实际运行状态 |
|------|------|--------------|
| 立即模式（IMR） | `WidgetRender::render()` 直接写 `Canvas2D` | ✅ 主路径 |
| Retained 合成 | `LayerTree` Picture 离屏缓存 | ❌ 缓存路径未接通 |
| FrameGraph 编排 | Pass culling、资源版本 | ⚠️ 仅 2 Pass，常被绕过 |
| 脏矩形增量 | `DirtyRegion` + 空间剪枝 | ⚠️ 与坐标系/合成层不一致 |

---

## 三、存在的问题

以下问题按严重程度排列，分为**架构层**与**实现层**。

### 3.1 【P0 架构】缺少统一的 Invalidation 模型

**现象**

- 脏标记分散在 `WidgetTree.dirty_region`、`dirty_nodes`、`subtree_dirty` 三处
- `State::set/update` 触发全局 dirty 回调，无法精确到 widget / 区域
- `keep_polling`、`mark_resource_dirty` 等宽口径逻辑架空 0 帧优化

**后果**

- 静止 UI 仍可能因 polling 标志进入渲染路径
- 小范围 State 变更可能扩大为全帧或过大 dirty bounds
- 正确性依赖 `dirty_bounds` 间隙修正、`mark_full_frame_dirty` 等补丁

**根因**

Invalidation（什么变了）与 Rendering（怎么画）与 Compositing（怎么合成）是三套独立设计，没有通过 DisplayList / RenderObject 层对齐。

---

### 3.2 【P0 架构】立即模式与 Retained 合成脱节

**现象**

`LayerTree` 的 Picture 节点在脏时回退主缓冲直接渲染，干净时 blit 被注释：

```text
ui/src/render/layer.rs
  render_picture_dirty() → render_widget_and_children_direct()  // 始终 direct
  blit_image / blit_offscreen                                   // TODO v2，未启用
```

**后果**

- `is_repaint_boundary()` 几乎无性能收益
- Chart、RichText、Table 等复杂子树无法在 boundary 内缓存
- LayerTree 与 WidgetTree 形成两棵平行树，维护成本高、收益低

**根因**

Widget 直接写像素，没有可录制、可缓存、可 diff 的绘制中间表示。

---

### 3.3 【P0 架构】坐标空间分裂

**现象**

| 坐标空间 | 用途 |
|----------|------|
| 绝对 screen frame | layout 后的 widget frame |
| content 坐标 | ScrollView 子节点 frame |
| viewport 坐标 | dirty_region、scroll_region |
| canvas translate | LayerTree 滚动绘制 hack |

ScrollView 场景需 `force_render` 绕过空间剪枝：

```text
// LayerTree：子节点 frame 在 content 坐标，dirty_region 在 viewport 坐标
let child_force = scroll_off.is_some() || force_render;
```

**后果**

- 滚动时增量剪枝失效，视口内子树趋向全量重绘
- 坐标转换逻辑散落在 LayerTree、ScrollView、event_loop
- 难以保证 hit test / paint / dirty 三者一致

---

### 3.4 【P1 架构】职责分散，三处决策「画什么」

| 模块 | 承担的渲染相关职责 |
|------|-------------------|
| `WidgetTree` | layout、dirty_region、scroll_delta、动画 update、事件 |
| `LayerTree` | 结构重建、脏传播、遍历渲染、ScrollView downcast |
| `event_loop` | Pass 策略、FrameGraph、scroll memmove、present damage |

**后果**

- 任何优化需跨三处协调
- `tree_version` 变化触发 LayerTree 全量 rebuild
- 难以单独测试「给定 invalidation → 预期 damage」

---

### 3.5 【P1 设计】render / post_render 泄漏 Pass 语义

Geometry Pass 会 clear 画布，Overlay Pass 不会。Dropdown、Notification 等组件被迫使用 `post_render` 绘制本应在合成层处理的内容。

**后果**

- Widget 作者需理解底层 Pass 语义
- 两趟遍历（render + post_render）增加复杂度和漏渲风险
- Overlay Pass 的 clip 与 Geometry 不一致时需额外补丁（如滚动视口 frame 纳入 overlay rects）

---

### 3.6 【P1 设计】FrameGraph 过度设计、利用不足

FrameGraph 设计目标是 GPU 多 Pass 编排，当前实际：

- 仅 Geometry + Overlay 两个 Pass，硬编码在 `event_loop`
- 每帧 `mark_resource_dirty(main_color_res)` 防止误裁剪
- `FrameGraph.execute()` 未使用，Pass 体在 event_loop 内联执行
- 资源注册表仅有 `main_color` 一个有效资源

**后果**

- 概念重量与实际收益不匹配
- 「0 Frame Cost」在交互/动画场景下频繁失效
- 增加新人理解成本

---

### 3.7 【P2 实现】动画路径强制全树遍历

`WidgetTree.update()` 使用 `traverse()` 而非 `dirty_traverse()`，否则动画在 `reset_dirty()` 后冻结。

**后果**

- 有动画/惯性滚动时，每帧 O(可见节点) 扫描
- 大树上 animation tick 成本与节点总数挂钩，无法收敛到 O(动画节点)

---

### 3.8 【P2 实现】Present damage 精度不足

最终 `present` 使用 `DirtyRegion.bounds()` 的单矩形 union，非多矩形 damage；16+ 脏矩形合并后丢失精度。

**后果**

- OS 层无法做更细粒度局部提交
- 多区域小变更可能扩大为一个大 damage rect

---

### 3.9 【P2 实现】Graphics 层边界模糊

- `Canvas2D` 为 fat trait，大量 default impl 委托 rasterizer
- `SpatialContext`（3D）与 2D 路径并存，widget 选用不一致
- `DirtyRegion`（2D）与 `DirtyRegion3D`（spatial）并存
- `GraphicsEngine::create_offscreen` 默认 no-op，离屏能力未形成契约

---

### 3.10 【P0 架构】绘图层能力散落在 UI / Platform

**现象**

本应在绘图层统一编排的能力，当前分散在多个 crate：

| 能力 | 当前位置 | 应在 |
|------|----------|------|
| 图层合成（LayerTree） | `ui/src/render/layer.rs` | `graphics/compositor/` |
| 绘制上下文（RenderContext） | `ui/src/render/context.rs` | `graphics/painting/` |
| 文本绘制编排（TextRenderService） | `ui/src/render/text.rs` | `graphics/text/` |
| 帧渲染调度（Pass 策略、damage） | `ui/src/render/event_loop.rs` | `graphics/pipeline/` |
| 脏区域收集 | `WidgetTree` + `graphics::DirtyRegion` | `graphics/pipeline/invalidation/` |
| 滚动 memmove 时机 | `event_loop` + `WidgetTree` | `graphics/pipeline/` |
| Present damage 计算 | `event_loop` | `graphics/pipeline/` |
| 调试绘制 | `ui/src/render/debug.rs` | `graphics/debug/`（feature gate） |

**后果**

- UI 层直接持有 `GraphicsEngine` 并驱动帧生命周期，违反分层
- 绘图层优化必须修改 UI event_loop，改动面大、易回归
- 无法独立测试「给定 invalidation → 输出 pixels + damage」
- 切换渲染后端时，UI 层代码也要跟着分支

---

### 3.11 【P0 架构】后端重复实现、无法动态切换

**现象**

- `App::run` 硬编码 `SoftwareEngine`，无运行时后端选择
- `SoftwareEngine` 与 `GpuEngine` 各自实现完整 `GraphicsEngine`，帧逻辑重复：

```text
SoftwareEngine.begin_frame  → 裁剪 + clear_rect + DirtyRects 策略
GpuEngine.begin_frame       → 全帧 glClear（不支持 partial_redraw）
```

- `GpuCanvas2D` 内嵌 `CpuCanvas2D` 作为 soft_fallback，GPU/CPU 绘制路径混杂在同一 struct
- `GraphicsCapabilities` 仅声明 `partial_redraw: bool`，pipeline 在 UI 层做分支，而非后端透明降级

**后果**

- 新增后端（如 Metal/Vulkan）需复制整套帧逻辑
- GPU 后端无法复用 CPU pipeline 的 invalidation / damage 语义
- 运行时切换后端（如设置页「性能模式」）不可行
- 同一帧内 CPU fallback 与 GPU 路径状态不一致，难以保证视觉一致

---

## 四、能力归属重组

### 4.1 目标 crate 边界

```text
ui 域
  ├── Widget / Layout / Event / State / Theme（令牌）
  └── 通过 RenderSession 提交 PaintCommand，不直接持有 Canvas2D

draw 域（绘图层 — 完整引擎）
  ├── pipeline/          帧调度、InvalidationQueue、0 帧判定
  ├── compositor/        Layer 树、Picture 缓存、scroll transform
  ├── painting/          PaintContext、DisplayList、录制与 replay
  ├── text/              TextLayout + glyph blit 编排
  ├── surface/           主缓冲 + 离屏缓冲管理
  ├── backend/           RenderBackend trait + CPU/GPU/Null 实现
  ├── rasterizer/        共享光栅化算法（现有）
  └── api/               对外稳定契约

native 域
  ├── IEventLoop / 窗口
  └── ISurfacePresenter   仅接收 pixels + damage，不含渲染策略
```

### 4.2 从 UI 层迁入 graphics 的模块

| 模块 | 迁移目标 | 迁移方式 |
|------|----------|----------|
| `ui/render/layer.rs` | `graphics/compositor/layer_tree.rs` | 去除 WidgetTree 直接依赖，改为 `PaintNode` / callback |
| `ui/render/context.rs` | `graphics/painting/paint_context.rs` | 剥离 `TokenProvider`，主题色由 UI 注入 |
| `ui/render/text.rs` | `graphics/text/render.rs` | 与 FontService 合并编排 |
| `ui/render/event_loop.rs` 渲染段 | `graphics/pipeline/render_loop.rs` | event_loop 只留 OS 事件 + Widget 调度 |
| `ui/render/debug.rs` | `graphics/debug/` | `feature = "debug-overlay"` |
| `WidgetTree.dirty_*` | `graphics/pipeline/invalidation/` | UI 通过 `RenderSession::invalidate()` 上报 |

### 4.3 UI 层保留的内容

| 保留 | 原因 |
|------|------|
| `WidgetRender::paint()` | Widget 是 paint 指令的生产者 |
| `WidgetTree.layout()` | 布局属于 UI 语义 |
| `run_widget_loop` 事件段 | OS 事件 → Widget 分发 |
| `TokenProvider` | 设计令牌属于 UI 主题域 |

### 4.4 迁移后的 UI → Graphics 接口

```rust
/// UI 层持有的绘图层会话，不包含后端细节。
pub struct RenderSession {
    // 内部：pipeline + compositor + backend handle
}

impl RenderSession {
    /// 上报失效（唯一渲染触发入口）。
    pub fn invalidate(&mut self, inv: Invalidation);

    /// 执行一帧渲染；无 invalidation 时返回 Idle。
    pub fn render_frame(&mut self, paint_callbacks: &PaintCallbacks) -> RenderOutcome;

    /// 运行时切换后端（可选，见 §5.4）。
    pub fn set_backend(&mut self, backend: BackendKind) -> Result<(), Error>;

    /// 查询当前后端能力。
    pub fn capabilities(&self) -> BackendCapabilities;
}
```

Widget 绘制不再接收 `&mut Canvas2D`，而是 `&mut PaintContext`（录制 DisplayList ops）。

---

## 五、可切换后端架构

### 5.1 分层模型

```text
                    ┌─────────────────────────────────┐
                    │         RenderPipeline          │
                    │  (共享逻辑 — 与后端无关)         │
                    │  · InvalidationQueue            │
                    │  · 0 帧判定                     │
                    │  · DisplayList 录制/replay      │
                    │  · Compositor / LayerTree       │
                    │  · scroll memmove 调度          │
                    │  · damage region 计算           │
                    │  · Geometry / Overlay Pass      │
                    └───────────────┬─────────────────┘
                                    │ 委托
                    ┌───────────────▼─────────────────┐
                    │       RenderBackend trait       │
                    │  (差异部分 — 可替换实现)         │
                    └───────────────┬─────────────────┘
            ┌───────────────────────┼───────────────────────┐
            ▼                       ▼                       ▼
     CpuBackend              GpuBackend              NullBackend
     · PixelSurface          · GL/Vulkan texture     · 测试桩
     · CPU rasterizer        · GPU shader 路径
     · IPresenter.present    · swap_buffers
```

**关键约束**：`begin_frame` / `end_frame` 的**策略逻辑**（DirtyRects 裁剪、clear 范围、0 帧判定）在 `RenderPipeline`，不在各 Backend。

### 5.2 RenderBackend trait（仅差异部分）

```rust
/// 渲染后端 — 只负责 surface 与像素提交，不含帧调度逻辑。
pub trait RenderBackend: Send {
    /// 后端标识与能力。
    fn kind(&self) -> BackendKind;
    fn capabilities(&self) -> BackendCapabilities;

    /// Surface 生命周期。
    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error>;
    fn shutdown(&mut self);

    /// 获取可绘制 surface（主缓冲）。
    fn surface(&mut self) -> &mut dyn DrawSurface;

    /// 离屏 surface 池。
    fn create_offscreen(&mut self, w: i32, h: i32) -> Option<OffscreenHandle>;
    fn destroy_offscreen(&mut self, handle: OffscreenHandle);
    fn offscreen(&mut self, handle: &OffscreenHandle) -> Option<&mut dyn DrawSurface>;

    /// 将 surface 内容提交到屏幕。
    fn present(&mut self, damage: &DamageRegion) -> Result<(), Error>;
}

/// 可绘制 surface — Backend 提供，Pipeline 通过 DrawContext 写入。
pub trait DrawSurface {
    fn size(&self) -> Size;
    fn clear_rect(&mut self, rect: Rect, color: Color);
    fn copy_region(&mut self, src: Rect, dst: Point);   // scroll memmove
    fn canvas(&mut self) -> &mut dyn Canvas2D;           // 或 DisplayListExecutor
}
```

### 5.3 共享逻辑清单（不得在后端重复实现）

| 逻辑 | 所在模块 | 说明 |
|------|----------|------|
| InvalidationQueue 合并 | `pipeline/invalidation.rs` | 所有后端共用 |
| 0 帧判定 | `pipeline/frame.rs` | 队列空 → Idle |
| DirtyRects → clip 区域 | `pipeline/frame.rs` | 后端只执行 `surface.clear_rect` |
| DisplayList 录制与 replay | `painting/display_list.rs` | 后端无关 |
| LayerTree 合成顺序 | `compositor/` | 后端无关 |
| Picture 缓存命中/失效 | `compositor/picture.rs` | 后端无关 |
| scroll memmove 调度 | `pipeline/scroll.rs` | 调用 `surface.copy_region` |
| damage region 合并 | `pipeline/damage.rs` | present 前统一计算 |
| 文本 layout | `text/` + FontService | 后端无关 |
| glyph blit | 共享 rasterizer 或 backend 加速 | 见 §5.5 |

### 5.4 后端差异清单（仅 Backend 实现）

| 差异 | CpuBackend | GpuBackend | NullBackend |
|------|------------|------------|-------------|
| 像素存储 | `Vec<u32>` | GPU texture | 无 |
| 路径填充 | CPU rasterizer | GPU shader / compute | no-op |
| 圆角矩形 | CPU | GPU instanced quads | no-op |
| 渐变/阴影 | CPU | GPU 或回退 CPU | no-op |
| present | `IPresenter::present(pixels, damage)` | `swap_buffers` | 丢弃 |
| partial_redraw | ✅ 原生支持 | 可选（damage 区域 glScissor） | N/A |
| 离屏缓冲 | `CpuCanvas2D` 池 | GPU FBO 池 | 空句柄 |

### 5.5 后端切换策略

**运行时切换**（用户可选「性能模式」）：

```text
1. drain 当前 invalidation 队列（或强制全帧）
2. backend.shutdown() — 释放旧 surface
3. backend = create(new_kind)
4. backend.resize(w, h)
5. invalidate(FullFrame) — 下一帧全量重绘
6. Picture 缓存全部失效（bounds 不变但像素格式可能变）
```

**编译期可选后端**（减少二进制体积）：

```toml
[features]
default = ["backend-cpu"]
backend-cpu = []
backend-gpu = ["dep:glow"]
backend-null = []   # 测试
```

**GPU 回退 CPU**（单帧内）：

- 不推荐 `GpuCanvas2D` 内嵌 `CpuCanvas2D` 的混合 struct
- 推荐：未实现的 draw op 在 DisplayList replay 时路由到 `SharedRasterizer`（CPU），结果 blit 到 GPU surface
- 长期：逐步将热点 op 迁移到 GPU shader，缩小 CPU 回退面

### 5.6 与现有 GraphicsEngine 的关系

| 现状 | 目标 |
|------|------|
| `GraphicsEngine` trait 混合帧逻辑 + 绘制 + 离屏 | 拆分为 `RenderPipeline` + `RenderBackend` |
| `SoftwareEngine` 实现全部 | `CpuBackend` 仅 surface + present |
| `GpuEngine` 实现全部 | `GpuBackend` 仅 GL surface + swap |
| UI 持有 `&mut dyn GraphicsEngine` | UI 持有 `RenderSession` |
| `App` 硬编码 SoftwareEngine | `RenderSession::new(BackendKind::Auto)` |

过渡期可保留 `GraphicsEngine` 作为 `RenderSession` 内部适配，对外逐步废弃。

### 5.7 后端能力协商

Pipeline 根据 `BackendCapabilities` 自动降级，UI 层不分支：

```rust
pub struct BackendCapabilities {
    pub partial_redraw: bool,       // 不支持则 pipeline 自动 expand damage 为全帧
    pub offscreen: bool,            // 不支持则 Picture 缓存降级为 direct paint
    pub scroll_memmove: bool,       // 不支持则 scroll 退化为全量 strip repaint
    pub presentation: PresentationMode,
}
```

---

## 六、目标架构：Demand-Driven Rendering

### 6.1 核心模型

```text
数据/State 变更
  → Invalidation（Layout / Paint / Composite）
  → Scheduler 合并队列
  → 队列为空？ → RenderOutcome::Idle（0 帧，阻塞等 OS 事件）
  → Layout Pass（仅脏子树）
  → Paint Pass（仅脏节点 + dirty rect clip）
  → Composite / Present（仅 damage 区域）
```

### 6.2 Invalidation 类型

```rust
/// 渲染失效信号——所有渲染触发的唯一入口。
enum Invalidation {
    /// 布局可能变化（尺寸、位置、子树结构）。
    Layout(WidgetId),

    /// 视觉变化，布局不变。
    Paint {
        id: WidgetId,
        /// None 表示整个 widget 的 dirty_rect。
        rect: Option<Rect>,
    },

    /// 合成层操作（如 scroll memmove 后的 exposed strip）。
    Composite {
        rect: Rect,
        /// 可选：memmove 参数，在 Paint 之前执行。
        scroll: Option<ScrollDelta>,
    },
}
```

**触发规则（示例）**

| 变更源 | Invalidation |
|--------|--------------|
| `State::set`（绑定 widget） | `Paint { id, rect }` |
| resize / 子节点增删 | `Layout(root)` → 向下传播 |
| 动画 tick | `Paint { id, dirty_rect }` |
| scroll delta | `Composite { strip, scroll }` |
| 主题切换 / 树重建 | `Layout` + 全屏 `Paint` |

### 6.3 调度器职责

```text
InvalidationQueue
  ├── 合并同一 widget 的重复 Paint
  ├── Layout 优先于 Paint（Layout 后重算 paint rect）
  ├── 合并 Paint rect → DirtyRegion（multi-rect，保留精度）
  └── is_empty() → 0 帧，不进入渲染管线
```

### 6.4 三阶段严格按需

| 阶段 | 进入条件 | 工作范围 |
|------|----------|----------|
| **Layout** | 队列含 `Layout` | `dirty_traverse` 脏子树 |
| **Paint** | 队列含 `Paint` 或 Layout 导致 frame 变化 | 脏节点 + clip 到 dirty rect |
| **Present** | 有像素写入 | 仅 damage region（支持 multi-rect） |

每阶段入口自检：

```text
□ invalidation 队列是否为空？     空 → 0 帧
□ 是否有 Layout invalidation？   无 → 跳过 layout
□ 是否有 Paint invalidation？    无 → 跳过 paint
□ damage region 是否为空？       空 → 跳过 present
```

### 6.5 目标分层（整合能力归属 + 可切换后端）

```text
ui 域
  ├── Widget / Layout / Event / State / Theme
  └── RenderSession::invalidate + paint callbacks

draw 域
  ├── pipeline/          InvalidationQueue、帧调度、0 帧、scroll、damage
  ├── compositor/        LayerTree、Picture 缓存、Transform
  ├── painting/          PaintContext、DisplayList
  ├── text/              TextRenderService（从 ui 迁入）
  ├── backend/           RenderBackend trait
  │   ├── cpu/           CpuBackend + PixelSurface
  │   ├── gpu/           GpuBackend + shaders
  │   └── null/          NullBackend
  ├── rasterizer/        共享算法（CPU 默认，GPU 可选覆盖）
  └── api/               RenderSession、RenderOutcome 公开契约

native 域
  └── ISurfacePresenter  原始 present 原语
```

### 6.6 与现有模块的映射

| 现有 | 目标态 | 处置 |
|------|--------|------|
| `ui/render/*` | `graphics/{pipeline,compositor,painting,text}/` | **迁入** graphics |
| `WidgetTree.dirty_*` | `pipeline/invalidation/` | 迁入，UI 只上报 |
| `LayerTree` Picture | `compositor/picture.rs` | 迁入 + 接通 blit |
| `SoftwareEngine` / `GpuEngine` | `RenderPipeline` + `CpuBackend` / `GpuBackend` | **拆分** |
| `GraphicsEngine` trait | `RenderSession` + `RenderBackend` | 过渡适配后废弃 |
| `FrameGraph` | 删除或 `backend/gpu/pass_graph.rs` | GPU 专用 |
| `keep_polling` | `AnimationRegistry` | 移除 |
| `render` / `post_render` | 统一 `paint()` + compositor z-order | 废弃 post_render |

---

## 七、分阶段完整解决方案

> **目标**：分阶段交付，**不遗留已知架构债**——§3.1–§3.11 在 **Phase 8** 结束时全部关闭。  
> **原则**：每 Phase 可独立合并、可验收；Phase 结束须 `cargo test --workspace` 通过再进入下一 Phase。

### 7.1 问题覆盖矩阵

| 问题 | 描述 | 关闭 Phase |
|------|------|------------|
| §3.1 | 缺少统一 Invalidation 模型 | **Phase 2** + **Phase 6** |
| §3.2 | IMR 与 Retained 合成脱节 | **Phase 4** + **Phase 7** |
| §3.3 | 坐标空间分裂 | **Phase 5** |
| §3.4 | 职责分散（三处决策） | **Phase 3** |
| §3.5 | render/post_render 泄漏 Pass | **Phase 7** |
| §3.6 | FrameGraph 过度设计 | **Phase 8** |
| §3.7 | 动画全树 traverse | **Phase 2** |
| §3.8 | Present damage 精度不足 | **Phase 8** |
| §3.9 | Graphics 层边界模糊 | **Phase 1** + **Phase 8** |
| §3.10 | 能力散落在 UI | **Phase 3** |
| §3.11 | 后端重复、不可切换 | **Phase 1** + **Phase 8** |

### 7.2 已锁定选型（全程不变）

| 项 | 决策 |
|----|------|
| 架构 | Demand-Driven Rendering + 能力归属 graphics + 可插拔后端 |
| 依赖 | draw 禁止依赖 ui 域；`ScenePaint` trait 解耦 |
| 后端 | `RenderPipeline`（共享）+ `RenderBackend`（差异） |
| 主题 | `ThemeSnapshot` 每帧注入 |
| Widget API | Phase 1–6 保留 `render/post_render`；**Phase 7** 合并为 `paint` |
| DisplayList | **Phase 7** 完整落地（非可选） |
| FrameGraph | Phase 2 从 UI 移除；**Phase 8** 删除源码 |
| RenderObject | **Phase 9 可选**，不在必需范围 |

### 7.3 目标目录结构（Phase 8 完成态）

```text
graphics/src/
├── pipeline/          # invalidation, frame, damage, scroll, session, animation_registry
├── backend/           # traits, cpu, gpu, null
├── compositor/        # scene, layer_tree, viewport_transform
├── painting/          # context, theme, display_list
├── text/              # render
├── debug/             # overlay（feature gate）
└── api/               # RenderSession, BackendKind

ui/src/render/
├── event_loop.rs      # OS 事件 + Widget 调度
└── scene_paint.rs     # impl ScenePaint for WidgetTree
```

### 7.4 Phase 0：度量基线（3–5 天）

- [ ] 统计钩子：`layout_calls`, `paint_calls`, `present_calls`, `idle_frames`
- [ ] 基准测试：静止 0 帧 / hover 局部 / scroll strip / 10 动画 10 节点
- [ ] Debug overlay 显示 invalidation 来源

**验收**：基线测试入库 CI（未达标项标 `#[ignore]` + TODO Phase N）

---

### 7.5 Phase 1：Pipeline + 可切换后端（1–2 周）

**关闭**：§3.11、§3.9（部分）

- [ ] `backend/traits.rs`：`RenderBackend`, `DrawSurface`, `BackendKind`
- [ ] `backend/{cpu,gpu,null}.rs`
- [ ] `pipeline/frame.rs`：从 `SoftwareEngine.begin_frame` **唯一**抽出 clip/clear/Idle
- [ ] `pipeline/session.rs` 骨架
- [ ] `SoftwareEngine`/`GpuEngine` 瘦身为 backend；`GraphicsEngine` 委托 session
- [ ] `BackendKind::Cpu | Gpu | Auto` + `set_backend()`（切换 → FullFrame）
- [ ] `!partial_redraw` 时 Pipeline 自动 FullRedraw

**本 Phase 不改**：LayerTree 位置、WidgetTree dirty、event_loop 结构

**验收**：CPU/GPU 共用一份 `frame.rs`；NullBackend 单测；demo 可切换后端

---

### 7.6 Phase 2：Invalidation + 0 帧 + 动画（1–2 周）

**关闭**：§3.1（核心）、§3.7

- [ ] `pipeline/invalidation.rs` + `animation_registry.rs`
- [ ] `WidgetTree.bind_invalidation` → `InvalidationQueue`
- [ ] 移除 `keep_polling → need_render`
- [ ] `update` 仅 tick AnimationRegistry
- [ ] **event_loop 删除 FrameGraph**（UI 路径不再使用）

**验收**：静止窗口 `present_calls == 0`；动画结束恢复 0 帧

---

### 7.7 Phase 3：能力迁入 graphics（2 周）

**关闭**：§3.10、§3.4

- [ ] `ScenePaint` trait + `ui/scene_paint.rs`
- [ ] 迁移 context / text / debug / layer → graphics
- [ ] `RenderSession::render_frame` 接管渲染段；event_loop 瘦身
- [ ] `ThemeSnapshot`；删除 ui 已迁走源文件

**验收**：event_loop 无 Canvas2D/damage/FrameGraph；graphics 可 mock scene 单测

---

### 7.8 Phase 4：Picture 离屏缓存（1–2 周）

**关闭**：§3.2（Retained）

- [ ] Picture 脏→offscreen 栅格化；干净→`blit_offscreen_src`
- [ ] 删除 `render_widget_and_children_direct` 主路径回退
- [ ] RepaintBoundary 集成测试

**验收**：boundary 外变更不触发内部 re-render

---

### 7.9 Phase 5：ScrollView 坐标统一（1–2 周）

**关闭**：§3.3

- [ ] `viewport_transform.rs`；`ScenePaint::scroll_offset`
- [ ] 删除 ScrollView downcast、全部 `force_render`、`dirty_bounds` hack
- [ ] scroll → `Invalidation::Composite { strip, scroll }`

**验收**：滚动 paint ≈ strip 节点；代码库无 `force_render`

---

### 7.10 Phase 6：精确 Invalidation + State（1 周）

**关闭**：§3.1（完整）

- [ ] Layout / Paint invalidation 分离
- [ ] `State` 绑定 `(WidgetId, Invalidation)`
- [ ] 移除 `WidgetTree.dirty.region` 等三件套

**验收**：Counter +1 仅 label damage

---

### 7.11 Phase 7：DisplayList + 消除 post_render（2–3 周）

**关闭**：§3.2（根因）、§3.5

- [ ] `display_list.rs` record-replay
- [ ] Compositor z-order 统一合成
- [ ] 迁移 Dropdown/Notification 等；废弃 `post_render`
- [ ] Picture 缓存 DisplayList

**验收**：无 widget 实现 post_render；overlay 视觉正确

---

### 7.12 Phase 8：Platform + GPU 清理 + 收尾（1–2 周）

**关闭**：§3.6、§3.8、§3.9、§3.11（GPU）

- [ ] platform multi-rect `DamageRegion`
- [ ] 移除 GpuCanvas2D 内嵌 CPU → SharedRasterizer 路由
- [ ] 删除 FrameGraph 源码；删除公开 `GraphicsEngine`
- [ ] 更新 ARCHITECTURE.md / README.md

**验收**：§7.1 矩阵全部关闭；`cargo test --workspace` + demo

---

### 7.13 Phase 9（可选）：RenderObject 树

Widget/Render 长期解耦，4+ 周，不在「完整解决」必需范围。

---

### 7.14 阶段依赖

```text
P0 → P1 → P2 → P3 → P4 → P5 → P6 → P7 → P8 → (P9)
```

**总工期估算**：约 12–16 周（1 人全职）；Phase 0 可与 Phase 1 并行启动测试编写。

| Phase | 周期 | 累计能力 |
|-------|------|----------|
| 0 | 3–5 天 | 可度量回归 |
| 1 | 1–2 周 | 可切换后端、共享帧逻辑 |
| 2 | 1–2 周 | **0 帧** |
| 3 | 2 周 | 能力归属 graphics |
| 4 | 1–2 周 | Picture 缓存 |
| 5 | 1–2 周 | Scroll 正确增量 |
| 6 | 1 周 | 精确 invalidation |
| 7 | 2–3 周 | DisplayList、无 post_render |
| 8 | 1–2 周 | **全部问题关闭** |

---

### 7.15 各 Phase 启动指令（下一会话复制对应段）

```text
请阅读 docs/rendering-engine-optimization.md，执行 Phase N（§7.x）。
仅做该 Phase 范围；Phase 结束 cargo test --workspace；不 commit 除非要求。
draw 禁止依赖 ui 域。
```

| Phase | 章节 |
|-------|------|
| 1 | §7.5 |
| 2 | §7.6 |
| 3 | §7.7 |
| 4 | §7.8 |
| 5 | §7.9 |
| 6 | §7.10 |
| 7 | §7.11 |
| 8 | §7.12 |

---

### 7.16 项目级完成定义（Phase 8）

- [ ] §3.1–§3.11 均有测试关闭记录
- [ ] 0 帧 + 局部渲染 + 可切换后端 + 能力在 graphics
- [ ] 无 FrameGraph / force_render / Picture fallback / GpuCanvas2D⊃CPU

---

## 八、明确不做的事

| 不做 | 原因 |
|------|------|
| 保留 FrameGraph 作为 CPU 2-Pass 编排 | 过度抽象，event_loop 内联更清晰 |
| 继续扩展 `dirty_bounds` 间隙修正 | 掩盖坐标/合成语义错误，应修根因 |
| Widget 直接写 Canvas2D 作为长期方案 | Phase 7 DisplayList 后禁止 |
| 无 invalidation 的 `keep_polling` 渲染 | 违反 0 帧原则 |
| 每个 Backend 各自实现 begin_frame 策略 | 违反「逻辑共享」原则，应只在 RenderPipeline |
| UI 层直接持有 Canvas2D / GraphicsEngine | 违反能力归属原则 |
| GpuCanvas2D 内嵌 CpuCanvas2D 混合 struct | 改用 DisplayList replay 路由到 SharedRasterizer |

---

## 九、风险与权衡

| 风险 | 缓解 |
|------|------|
| 漏 invalidation 导致画面不更新 | Debug 模式显示 invalidation 来源；集成测试覆盖关键交互 |
| Phase 7 DisplayList 迁移工作量大 | Phase 4 Picture 已验证缓存路径；逐 widget 迁移 post_render |
| Phase 3 迁移破坏 UI 编译 | RenderSession 适配层；ui 重导出 graphics 类型 |

---

## 十、已锁定选型

| 项 | 结论 |
|----|------|
| 实施顺序 | **Phase 0 → 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8**（§7.14） |
| 后端切换 | 启动 `BackendKind` + `set_backend()`（Phase 8 完整） |
| GPU 回退 | Phase 8 移除 GpuCanvas2D⊃CPU；此前 Pipeline 全帧降级 |
| DisplayList | **Phase 7 必做**（完整解决 §3.2/§3.5） |
| post_render | **Phase 7 废弃** |
| Present damage | **Phase 8 multi-rect** |
| RenderObject | Phase 9 可选 |

---

## 十一、成功标准

| 场景 | 期望行为 |
|------|----------|
| 窗口打开后无交互 | 0 present，CPU ≈ idle |
| 鼠标移过 Button | 仅 button damage rect repaint |
| Counter +1 | 仅 label 区域 repaint |
| ScrollView 慢速滚动 | memmove + strip paint，非视口全量 |
| RepaintBoundary 内静态 + 外部变更 | boundary 内 blit，不 re-render |
| 10 个独立动画 | update 10 节点，paint 10 个 rect |
| 主题切换 | 一次全帧 repaint（预期行为） |

| 运行时切换 CPU ↔ GPU 后端 | 一次 FullFrame 后正常渲染，无 crash |
| graphics 层独立测试 pipeline | 不依赖 WidgetTree 即可验证 0 帧 / damage |

---

## 十二、参考资料

- 内部：[ARCHITECTURE.md](../ARCHITECTURE.md) — 当前分层与渲染管线
- 内部：[ui/src/render/event_loop.rs](../ui/src/render/event_loop.rs) — 帧循环（渲染段待迁入 graphics）
- 内部：[ui/src/render/layer.rs](../ui/src/render/layer.rs) — LayerTree（待迁入 graphics/compositor）
- 内部：[graphics/src/frame_graph/mod.rs](../graphics/src/frame_graph/mod.rs) — FrameGraph（待删除或 GPU 专用）
- 内部：[graphics/src/engine/cpu/software.rs](../graphics/src/engine/cpu/software.rs) — CPU 引擎（待拆分为 Pipeline + Backend）
- 内部：[graphics/src/gpu_engine/mod.rs](../graphics/src/gpu_engine/mod.rs) — GPU 引擎（待拆分）
- 内部：[ui/src/view/app.rs](../ui/src/view/app.rs) — 当前硬编码 SoftwareEngine
- 外部：Flutter RenderObject / Layer 模型
- 外部：Blink PaintLayer / DisplayItemList

---

## 修订记录

| 日期 | 作者 | 说明 |
|------|------|------|
| 2026-07-03 | — | 初稿，问题梳理 + 分阶段方案 + 待讨论项 |
| 2026-07-04 | — | 改为分阶段完整方案 Phase 0–8；问题覆盖矩阵；删除单会话/归档重复内容 |
