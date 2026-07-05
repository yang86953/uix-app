# UIX 架构设计

> 最后更新: 2026-07-05（渲染机制审查与完善计划）
> 本文档是项目的实时架构地图，随代码变更同步更新。

---

## 一、定位

UIX 是一个**跨平台原生桌面应用开发框架**（Rust，单一 crate `uix`）。组织原则是：

- **按功能域划分模块** — 模块名对应开发者「在做什么」，而非技术实现层
- **平台差异只封装在 `native` 域** — 其余代码在每个平台上完全一致

---

## 二、Workspace 结构

```text
uix-app/
├── Cargo.toml          # workspace 根 + uix crate
├── src/                # uix 框架源码（六大功能域）
├── demo/               # uix-demo 演示二进制
├── tests/              # 集成测试（按功能域命名）
├── ARCHITECTURE.md     # 本文档
├── README.md           # 快速上手
└── AGENTS.md           # 编码规范
```

### 依赖拓扑

```text
demo ──→ uix
```

---

## 三、六大功能域（当前架构）

```text
uix/src/
│
├── core/                  # 基础设施 — 与 OS / UI 无关
│   ├── error/             # Error, Errc, Result
│   ├── geometry/          # Point, Size, Rect, EdgeInsets
│   ├── log/               # Logger, Sink
│   └── diagnostic/        # 收集、恢复、中间件
│
├── native/                # 平台能力 — 唯一允许平台差异的域
│   ├── traits/            # 公开契约（Platform, IEventLoop, UiEvent…）
│   ├── shared/            # 跨平台共享逻辑（WindowOps, OsEventSource…）
│   ├── backends/          # 平台实现（windows/ + linux/），#[cfg] 只在此处
│   ├── factory.rs         # create_platform(), create_gpu_context()
│   ├── services/          # FileService, NotificationService
│   ├── presenter.rs       # 呈现器编排
│   └── test_harness/      # FakePlatform — 无真实 OS 也能测试上层
│
├── draw/                  # 绘制能力 — 内容如何变成像素
│   ├── traits/            # GraphicsEngine, Canvas2D, TextBackend
│   ├── engine/            # SoftwareEngine, GpuEngine, NullEngine
│   ├── backend/           # Cpu / Gpu / Null 后端
│   ├── rasterizer/        # 光栅化算法
│   ├── font/              # 字体加载与文本布局
│   ├── image/             # 图片解码、路径缓存、BitmapHandle
│   ├── compositor/        # LayerTree, ScenePaint, Picture 离屏
│   ├── render_object/     # DisplayList 缓存与重放
│   ├── painting/          # PaintContext, DisplayList, ThemeSnapshot
│   ├── pipeline/          # FrameRenderer, InvalidationQueue, 帧调度
│   ├── primitives/        # Color, Path, 描边, DirtyRegion
│   └── spatial/           # 坐标变换、viewport 映射
│
├── ui/                    # 界面能力 — 用户看到和交互的一切
│   ├── traits/            # WidgetComponent, WidgetLayout, WidgetRender…
│   ├── core/              # WidgetTree, 事件分发, 命中测试
│   ├── foundation/        # State, Style, 配置与虚拟滚动
│   ├── layout/            # Flex, Grid
│   ├── theme/             # DesignTokens, Theme（Ant Design 5）
│   ├── animation/         # Animation, Transition, Easing
│   ├── managers/          # 焦点、拖拽、事件管理等
│   ├── widgets/           # 内置组件库
│   ├── view/              # 声明式 DSL（column, row, label…）
│   └── macros.rs          # define_widget!, tree!
│
├── app/                   # 应用能力 — 组装各域成为可运行应用
│   ├── shell/             # App 生命周期、CLI、DI
│   ├── event_loop/        # 主事件循环
│   ├── bridge/            # UI ↔ Draw 桥接（ScenePaint 实现、trait 适配）
│   └── window/            # 应用级窗口管理
│
├── data/                  # 数据能力
│   └── settings/          # 键值持久化（SettingsService）
│
├── lib.rs                 # crate 根，声明六大功能域
└── prelude.rs             # 统一对外入口
```

### 功能域职责

| 功能域 | 一句话 | 开发者关心的事 |
|--------|--------|---------------|
| **core** | 全框架共享基础 | 错误处理、几何、日志、诊断 |
| **native** | 与操作系统打交道 | 窗口、事件、输入、文件、通知 |
| **draw** | 内容变成像素 | 2D 引擎、字体、光栅化、合成 |
| **ui** | 界面与交互 | 组件、布局、主题、动画、响应式状态 |
| **app** | 应用怎么跑起来 | 启动、主循环、窗口、CLI、DI |
| **data** | 数据从哪来存哪去 | 配置持久化 |

### 依赖方向（严格单向）

```text
core ← native ← draw ← ui ← app
  ↑      ↑               ↑
  └──────┴─── data ──────┘
```

| 规则 | 说明 |
|------|------|
| 上层依赖下层 | `ui` 依赖 `draw` + `core`，不反向 |
| 平台隔离 | `draw` / `ui` / `app` / `data` / `core` 只依赖 `native::traits` |
| 禁止泄漏 | 上层不得 `use native::backends::*`，不得出现 `#[cfg(windows)]` |

### 对外入口

| 场景 | 导入 |
|------|------|
| 90% 应用开发 | `use uix::prelude::*;` |
| 按域精确导入 | `uix::core::*` / `native::*` / `draw::*` / `ui::*` / `app::*` / `data::*` |

`prelude` 重导出：`App`、`State`、`column`/`row`/`button` 等 View DSL、常用组件、`Error`/`Point`/`Color` 等基础类型。

---

## 四、跨平台设计（硬约束）

### 4.1 平台分支只存在于 `native/backends/` 与 `native/factory.rs`

```rust
// ✅ 允许 — native/backends/mod.rs 或 native/factory.rs
#[cfg(windows)]
pub mod windows;
#[cfg(all(unix, not(target_os = "macos")))]
pub mod linux;

// ❌ 禁止 — draw / ui / app / data / core 任何地方
#[cfg(windows)]
fn handle_click() { ... }
```

### 4.2 能力差异用 trait 表达，不用条件编译泄漏

平台能力不同（如 Windows 无 GPU 上下文）时，在 `native` 内部处理，向上返回 `Result` 或能力查询：

```rust
// native/factory.rs
pub fn create_gpu_context(...) -> Result<Box<dyn IGraphicsContext>, Error> {
    // Linux: 返回 EglContext
    // 其他:  返回 Err — 上层据此选择 SoftwareEngine，不写 cfg
}
```

### 4.3 统一类型体系，禁止 OS 原生概念泄漏

| 统一类型（native/traits） | 禁止出现在上层 |
|--------------------------|---------------|
| `UiEvent`, `KeyCode`, `MouseButton` | `WM_LBUTTONDOWN`, `wl_pointer` |
| `PlatformWindow` trait | `HWND`, `wl_surface` |
| `IPresenter` | `BitBlt`, `wl_shm_pool` |
| `Error`, `Errc` | `GetLastError()`, `errno` |

### 4.4 native 域三层抹平机制

```text
native/traits     统一契约 — 上层唯一可见的 native 接口
native/shared     共享逻辑 — 平台只实现最小钩子，其余一份代码
native/backends/  平台实现 — #[cfg] 只在此处
```

**shared 层关键模式**：

| 模式 | 平台必须实现 | 自动获得 |
|------|-------------|---------|
| `WindowOps` | 6 个方法（show/hide/close/title/size/handle） | `PlatformWindow` + `IWindowProperties` + `INativeHandle` |
| `OsEventSource` | 3 个方法（dispatch_pending/blocking/next_event） | `IEventLoop`（poll/wait/wait_timeout） |
| `FileSystemCore<P>` | `SpecialDirProvider` | `IFileSystem` |

---

## 五、核心数据流

### 5.1 端到端链路

```text
OS 事件 / 动画 tick
  → native/backends（平台特有分发）
  → native/traits::UiEvent（统一格式）
  → app::run_widget_loop()
       ├─ map_ui_event() → WidgetTree::dispatch_event()  （输入、hover 失效）
       ├─ WidgetTree::update(dt)                         （动画 tick → Paint 失效）
       ├─ WidgetTree::layout()                           （Layout 失效子树）
       └─ draw::FrameRenderer::render_frame()
            ├─ WidgetTree as ScenePaint（app/bridge）
            ├─ LayerTree 合成 + RenderObject DisplayList 缓存
            ├─ GraphicsEngine begin_frame / end_frame
            └─ IPresenter::present() → 屏幕
  → 帧末 WidgetTree::reset_dirty()                       （清空 InvalidationQueue）
```

从 `app` 往下，没有任何一步需要知道当前是 Windows 还是 Linux。

### 5.2 渲染管线（当前实现）

主循环在 `app/event_loop/event_loop.rs`，单帧顺序如下：

```text
1. 收集 OS 事件 → dispatch_event（hover/click 等标记 Paint 失效）
2. update(dt)     → 动画节点 tick，push Invalidation::Paint / 注册 AnimationRegistry
3. layout()       → 仅 layout_traverse() 覆盖的子树；末步 bind_reactive_widget_states()
4. need_render?   → window_visible && (!rendered_first || has_render_work())
5. render_frame() → draw/pipeline/FrameRenderer
6. reset_dirty()  → 清空 InvalidationQueue（含已消费的 Layout 项）
7. present        → engine 像素缓冲 → IPresenter（PresentDamage）
```

**F12 调试 HUD（`RenderMetrics`）**

按 F12 切换 overlay 时，`event_loop` 会 `mark_full_frame_dirty()` 触发重绘。HUD 字段来自 `draw/pipeline/metrics.rs`：

| 字段 | 含义 |
|------|------|
| `layout_calls` | 本会话累计 `WidgetTree::layout()` 次数 |
| `paint_calls` | 累计 `FrameRenderer::render_frame` 调用（含 Idle 短路前的 paint 路径进入） |
| `present_calls` | 累计 `RenderOutcome::Present` 次数（实际上屏或 NullEngine 等价值 present） |
| `idle_frames` | 累计 `RenderOutcome::Idle` 帧数 |
| `last_invalidation` | 上一帧 present/idle 的触发来源：`first_frame` / `dirty_region` / `animation` / `layout_event` / `idle` |

**GPU partial redraw（R4-3）**

`BackendCapabilities::gpu()` 声明 `partial_redraw: true`。`FrameRenderer` 对 GPU 与 CPU 共用 `DirtyRects` 策略；`GpuCanvas2D::clear_rect_raw` 通过 GL scissor 局部清屏；`GpuEngine::end_frame(present_damage)` 将 `compute_damage` 结果传给 `GpuBackend::present`（当前 `IGraphicsContext::swap_buffers` 仍全屏交换，damage 已贯通供后续 native 优化）。

**`UpdateStrategy::Overlay` 说明（R4-2 决策）**

当前主路径为**单 Pass**（Content + AfterChildren 同帧绘制）。`Overlay` 策略保留于 trait 层供后端扩展，但在 `frame::normalize_strategy` 中对不支持 partial 的后端会降级为 `FullRedraw`；`begin_frame` 在 `Overlay` 模式下**不清屏**，仅裁剪 dirty rects。新代码应优先使用 `DirtyRects` + Composite scroll strip，而非依赖双 Pass overlay。

`FrameRenderer::render_frame` 内部：

```text
InvalidationQueue.dirty_region()
    ↓ 首帧 / 全帧 / 空 region / 后端不支持 partial → DirtyRegion::full()
tree_version 变化?
    ↓ 是 → LayerTree.build() + sweep_orphaned_offscreens()
LayerTree.update_dirty(scene)
RenderObjectTree.sync(scene)
scroll_move? → engine.canvas_2d().scroll_region()
begin_frame(FullRedraw | DirtyRects)
LayerTree.render()          ← 单 Pass：Content + AfterChildren + 焦点环
    ├─ 首帧：绕过 DisplayList 缓存（render_objects = None）
    └─ 后续帧：RenderObjectTree.try_replay / paint_content
end_frame(present_damage)   ← GPU 路径在此 present；CPU 由 event_loop 提交像素
→ RenderOutcome::Present(damage)
```

**关键组件**

| 组件 | 路径 | 职责 |
|------|------|------|
| `InvalidationQueue` | `draw/pipeline/invalidation.rs` | Layout / Paint / Composite 统一失效入口 |
| `WidgetTree` | `ui/core/widget/` | 布局、事件、失效上报；实现 `ScenePaint` 于 `app/bridge` |
| `FrameRenderer` | `draw/pipeline/render_frame.rs` | 帧调度、damage 计算、首帧策略 |
| `LayerTree` | `draw/compositor/layer_tree.rs` | Direct / ClipRect / Picture 合成 |
| `RenderObjectTree` | `draw/render_object/` |  per-widget DisplayList 录制与重放 |
| `ImageService` | `draw/image/` | 解码缓存；`PaintContext::draw_image` 绘制 |

**失效语义**

| 类型 | 触发方 | 消费方 | 是否驱动 present |
|------|--------|--------|------------------|
| `Layout(id)` | 结构/尺寸变化 | `layout()` | 否（仅 `has_layout()` 触发布局） |
| `Paint { id, rect }` | State、hover、动画、手动 mark | `render_frame` | 是 |
| `Composite { rect, scroll }` | 滚动 memmove | `scroll_region` + render | 是 |

**响应式 State 绑定（当前能力边界）**

| 路径 | 是否自动 Paint 失效 | 说明 |
|------|---------------------|------|
| `dynamic_label` + `State::get`（含 View 外 State） | ✅ | layout 后闭包探测 + 构造期 capture |
| `State::bind_paint_invalidation` 手动绑定 | ✅ | 精确 rect |
| View 构建期 `State::new` + orphan 兜底 | ✅ | `capture_view` + `bind_orphan_pending_states` |
| `Computed` / `Effect` | ✅ | R2-2 layout 探测 + R2-3 `tick_effects` |

> 详见 [十一、渲染机制完善计划](#十一渲染机制完善计划)。

### 5.3 事件流

```text
OS 事件 → IEventLoop → UiEvent → WidgetTree.dispatch_event()
    → 命中测试 → WidgetEventHandler.on_event()
    → EventBus.publish() → 外部订阅者
```

---

## 六、开发者使用指南

### 6.1 按任务选功能域

| 我要做… | 功能域 | 典型 API |
|---------|--------|---------|
| 启动应用 | `app` | `App::new().root(...).run()` |
| 写界面 | `ui` | `column`, `Button`, `State` |
| 自定义组件 | `ui` | `define_widget!`, `WidgetComponent` |
| 调布局 / 主题 | `ui` | `FlexLayout`, `Theme` |
| 自定义绘制 | `draw` | `Canvas2D`, `Color`, `Path` |
| 读文件 / 弹通知 | `native` | `FileService`, `NotificationService` |
| 持久化配置 | `data` | `SettingsService` |
| 错误 / 日志 | `core` | `Error`, `Logger` |
| 90% 场景 | `prelude` | `use uix::prelude::*` |

### 6.2 入口示例

```rust
use uix::prelude::*;

fn main() {
    App::new()
        .title("我的应用")
        .size(1024, 768)
        .root(column([label("Hello")]))
        .run();
}
```

### 6.3 自定义组件（底层）

```rust
use uix::prelude::*;

define_widget! {
    struct Greeting { text: String }

    impl WidgetComponent for Greeting {
        fn build(self, _ctx: &WidgetContext) -> WidgetNode {
            label(self.text).into_node()
        }
    }
}
```

---

## 七、测试布局

### 集成测试（`tests/`）

按六大功能域组织，每个域一个入口二进制：

```text
tests/
├── common/mod.rs       # 共享辅助
├── core.rs             # core 域入口
├── native.rs           # native 域入口
├── draw.rs             # draw 域入口
├── ui.rs               # ui 域入口
├── app.rs              # app 域入口
├── core/               # error, geometry, log, diagnostic
├── native/             # api, event, event_bus, shared, smoke, types, platform_integration
├── draw/               # integration, engine_cpu, bitmap_font, font_service, ...
├── ui/                 # core, layout, theme, animation, widgets, ...
└── app/                # application, window, cli_and_di
```

| 入口 | 覆盖域 | 子模块示例 |
|------|--------|-----------|
| `--test core` | `core` | `error`, `geometry`, `log`, `diagnostic` |
| `--test native` | `native` | `api`, `event`, `smoke`, `platform_integration` |
| `--test draw` | `draw` | `integration`, `engine_cpu`, `render_baseline` |
| `--test ui` | `ui` | `core`, `layout`, `widgets`, `with_native` |
| `--test app` | `app` | `application`, `window`, `cli_and_di` |

集成测试优先 `use uix::prelude::*`，域专用符号再精确导入。

常用命令：

```bash
cargo test -p uix                          # 全部测试（单元 + 集成）
cargo test -p uix --test core              # 单个域集成测试
cargo test --features test-harness -p uix  # 含 FakePlatform 的测试
```

### 单元测试（`src/`）

分布在各模块的 `#[cfg(test)]` 块中，用于测试 `pub(crate)` / 私有实现细节。
**不迁入** `tests/`，因集成测试 crate 无法访问 crate 内部符号。

`demo/` 内的布局冒烟测试保留在 demo crate 的 `#[cfg(test)]` 中。

---

## 八、重构历程（已完成）

| 阶段 | 目标 | 状态 |
|------|------|------|
| **P0 文档** | 确立功能域架构与跨平台规则 | ✅ |
| **P1 统一入口** | 新增 `prelude`；demo 改用 prelude | ✅ |
| **P2 收拢入口** | 合并两个 `App`；`map_ui_event` 去重；消除 `api/` 层 | ✅ |
| **P3 物理搬迁** | `platform/render/widget/runtime/view` → 六大功能域 | ✅ |

### 路径迁移对照（供查阅）

| 当前路径 | 原路径 |
|---------|--------|
| `core/*` | `platform/api/error`, `platform/log`, `platform/diagnostic` |
| `native/*` | `platform/windows`, `platform/linux`, `platform/shared`, `platform/services` |
| `draw/*` | `render/*` |
| `ui/*` | `widget/*`, `view/*` |
| `app/*` | `runtime/*`, `widget/scene/*` |
| `data/settings` | `platform/services/settings.rs` |
| `draw/traits`, `ui/traits` | `api/render/*`, `api/widget/*` |

---

## 九、关键设计决策

| 决策 | 方案 | 原因 |
|------|------|------|
| 组织维度 | 六大功能域（core / native / draw / ui / app / data） | 按开发者任务划分，不按技术层 |
| 跨平台 | `native` 域封装全部差异，上层零 `#[cfg]` | 余下代码每个平台完全一致 |
| 单一 crate | 全部合入 `uix` | 消除跨 crate 边界与镜像层 |
| 契约位置 | 各域 `traits/`，无独立 `api/` 层 | 契约与实现同域，通过 `pub` 控制暴露 |
| Widget 能力位 | `WidgetCapabilities` + 上转型 | 按需实现 Layout / Render / Event / Lifecycle |
| 增量渲染 | scroll_region + DirtyRects + PresentDamage + LayerTree/Picture | CPU 渲染只重绘变化像素；RepaintBoundary 离屏缓存 |
| 图片资源 | `draw/image::ImageService` + `BitmapHandle` | 解码与路径缓存独立于引擎；widget 经 `PaintContext::draw_image` 绘制 |
| draw ↔ ui 解耦 | `ScenePaint` trait + `app/bridge` | draw 不依赖 ui 域 |
| 测试 | `native/test_harness::FakePlatform` | 上层测试不依赖真实 OS |

---

## 十、代码约束

与 [`AGENTS.md`](AGENTS.md) 一致，核心条目：

- `deny(clippy::unwrap_used)`, `deny(clippy::expect_used)`
- 每个 Rust 文件 ≤ 900 行
- 中文注释
- 组合优于继承
- `#[cfg(windows/unix)]` 只允许出现在 `native/backends/` 和 `native/factory.rs`
- 上层模块禁止依赖 `native::backends::*`
- 重构不做兼容层，直接改路径

---

## 十一、渲染机制完善计划

> 基于 2026-07-05 渲染链路审查结论。目标：数据变更可靠上屏、失效粒度可测、文档与实现一致。

### 11.1 现状评估

| 维度 | 状态 | 说明 |
|------|------|------|
| 架构分层 | ✅ 良好 | `ui → app/bridge → draw` 单向依赖，`ScenePaint` 解耦正确 |
| 失效队列 | ✅ 良好 | Layout / Paint / Composite 分离；`has_render_work()` 避免空 render |
| 首帧 / Picture 缓存 | ✅ 良好 | 首帧 bypass DisplayList；子脏传播至 Picture 离屏 |
| 滚动 viewport 坐标 | ✅ 良好 | `viewport_transform` 将 content 映射后再与 dirty 求交 |
| State 自动重绘 | ✅ 已修复（R1） | `dynamic_label` + layout 闭包探测；View 外 State 已支持 |
| 调试 overlay | ✅ 已修复（R1-3） | F12 切换触发 `mark_full_frame_dirty` |
| 局部 damage 优化 | 🔲 待做 | hover / scroll strip 基线测试仍为 TODO |
| 文档同步 | ⚠️ 已修正 | 本节与 5.2 已对齐当前单 Pass 实现 |

**测试基线（审查日）**

```bash
cargo test --features test-harness -p uix --test draw   # 291/291 通过
cargo test --features test-harness -p uix --test ui     # 199/200（baseline_idle_zero_present 待修）
```

---

### 11.2 阶段规划

#### Phase R1 — 正确性修复（P0） ✅ 已完成

| 编号 | 任务 | 状态 |
|------|------|------|
| R1-1 | ViewContext：`capture_view` + layout 闭包探测 State 绑定 | ✅ |
| R1-2 | `bind_reactive_widget_states` 扩展 + `bind_orphan_pending_states` | ✅ |
| R1-3 | F12 调试模式 `mark_full_frame_dirty` | ✅ |
| R1-4 | Image 懒加载成功 push Paint 失效 | ✅ |
| R1-5 | 修复 `baseline_idle_zero_present` | ✅ |

**R1-1 设计要点**

```text
ViewAdapter::build_nodes()
  └─ expand 递归时 with_view_context(widget_id, ||
       set_current_view_dirty_fn(闭包 → invalidate_paint(widget_id))
       构建子 ViewNode / State::new 自动挂接
     )
layout() 末步
  └─ bind_reactive_widget_states()  // 将 Paint rect 从 frame 精确化
```

#### Phase R2 — 帧调度健壮性（P1，1 周）

| 编号 | 任务 | 状态 |
|------|------|------|
| R2-1 | **尊重 begin_frame 返回值** | ✅ |
| R2-2 | **Computed 失效策略** | ✅ paint 绑定 + layout 探测强制重算捕获 State |
| R2-3 | **Effect 与 UI 联动** | ✅ 构建期注册 + `tick_effects()`（event_loop 每帧） |

#### Phase R3 — 局部重绘优化（P2，2–3 周）

| 编号 | 任务 | 涉及模块 | 状态 |
|------|------|----------|------|
| R3-1 | **Hover 局部 damage** | `tree_events.rs`, `render_frame.rs` | ✅ `baseline_hover_partial_damage` |
| R3-2 | **Scroll strip 重绘** | `ScrollView`, `Invalidation::Composite` | ✅ strip + `scroll_region_move` |
| R3-3 | **多动画节点独立失效** | `AnimationRegistry`, `tree_layout.rs` | ✅ `baseline_ten_animations_ten_nodes` |
| R3-4 | **滚动失效策略分级** | `tree_layout.rs`, `tree_events.rs` | ✅ 小 delta Composite；大跳转整视口 Paint |

#### Phase R4 — 合成与缓存深化（P3，按需）

| 编号 | 任务 | 涉及模块 | 状态 |
|------|------|----------|------|
| R4-1 | **DisplayList replay_canvas 补全 DrawImage** | `draw/painting/display_list.rs` | ✅ 可选 `ImageService` 参数 |
| R4-2 | **Overlay Pass 评估** | `draw/pipeline/frame.rs` | ✅ 文档决策：保留 trait，主路径单 Pass |
| R4-3 | **GPU partial redraw 对齐** | `GpuEngine`, `BackendCapabilities` | ✅ |

#### Phase R5 — 测试与可观测性（贯穿）

| 编号 | 任务 | 验收标准 |
|------|------|----------|
| R5-1 | 修复 `tests/native/api.rs` 缺失 geometry import | ✅ |
| R5-2 | 新增 `app/event_loop` 集成测试：State 变更 → present 调用 | ✅ 两帧 FrameRenderer 路径 + NullEngine |
| R5-3 | RenderMetrics HUD 文档化 | ✅ 本文档 5.2 |
| R5-4 | CI 启用 `render_baseline` 非 ignore 项 | ✅ R3 已取消 ignore |

---

### 11.3 依赖关系

```text
R1（State 绑定、F12、Image）
  ↓
R2（begin_frame、Computed/Effect）—— 依赖 R1 的 ViewContext
  ↓
R3（局部 damage）—— 依赖 R2-1 帧调度正确
  ↓
R4（缓存深化）—— 可选，与 R3 并行
R5 —— 贯穿各阶段
```

**建议落地顺序**：R1-3 → R1-1 → R1-2 → R1-4 → R1-5 → R2-1 → R3-1/R3-2 → 其余。

---

### 11.4 开发者指南

**响应式 UI 推荐写法**

1. **计数器 / 动态文本**：`dynamic_label(|| format!("...", state.get()))`；State 可在 View 外创建（README Counter 模式）。
2. **View 内新建 State**：在 `App::root(...)` 或 `ViewAdapter::capture_view` 构建期内调用 `State::new`，由 `DynamicLabel` 或 orphan 兜底绑定。
3. **Computed**：在 `dynamic_label` 中读取；layout 探测期会强制执行一次计算以捕获 State 依赖（R2-2）。
4. **图片**：关键图用 `.slot(handle)` 预加载；路径懒加载首帧成功后自动失效（R1-4）。
5. **调试**：F12 即时切换调试 overlay。

---

### 11.5 完成定义（Definition of Done）

整个完善计划视为完成当且仅当：

- [x] R1 正确性修复（State 绑定、F12、Image、idle 基线）
- [x] R2-1 begin_frame 返回值
- [x] R2-2 / R2-3 Computed 与 Effect 联动
- [x] R3 hover / scroll strip / 多动画基线测试
- [x] R4-1 / R4-2 / R4-3 DisplayList DrawImage、Overlay 决策、GPU partial redraw
- [x] R5-2 event_loop State → present 集成测试
- [x] `cargo test --features test-harness -p uix` 全部通过（含 native）
- [x] F12、hover、scroll 三条基线测试取消 ignore 并通过（R3）
- [x] §11 渲染完善计划 R1–R5 全部落地
