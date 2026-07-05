# UIX 架构设计

> 最后更新: 2026-07-05
> 本文档是项目的实时架构地图，随代码变更同步更新。

---

## 一、定位

UIX 是一个**跨平台原生桌面应用开发框架**（Rust）。组织原则是：

- **按功能域划分模块** — 模块名对应开发者「在做什么」，而非技术实现层
- **平台差异只封装在 `native` 域** — 其余代码在每个平台上完全一致

---

## 二、Workspace 结构

```
uix workspace
├── src/        (uix)     单一框架 crate
├── demo/       (uix-demo) 演示二进制
└── tests/      集成测试
```

### 依赖拓扑

```
demo ──→ uix
```

---

## 三、六大功能域（目标架构）

```
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
│   └── test_harness/      # FakePlatform — 无真实 OS 也能测试上层
│
├── draw/                  # 绘制能力 — 内容如何变成像素
│   ├── traits/            # GraphicsEngine, Canvas2D, TextBackend
│   ├── engine/            # SoftwareEngine, GpuEngine, NullEngine
│   ├── backend/           # Cpu / Gpu / Null 后端
│   ├── rasterizer/        # 光栅化算法
│   ├── font/              # 字体加载与文本布局
│   ├── compositor/        # LayerTree, ScenePaint
│   ├── pipeline/          # 帧调度、无效化
│   ├── primitives/        # Color, Path, 描边
│   └── spatial/           # 坐标变换、脏区域
│
├── ui/                    # 界面能力 — 用户看到和交互的一切
│   ├── traits/            # WidgetComponent, WidgetLayout, WidgetRender…
│   ├── core/              # WidgetTree, 事件分发, 命中测试
│   ├── foundation/        # State, Style, 配置与虚拟滚动
│   ├── layout/            # Flex, Grid
│   ├── theme/             # DesignTokens, Theme（Ant Design 5）
│   ├── animation/         # Animation, Transition, Easing
│   ├── managers/          # 焦点、拖拽、事件管理等
│   ├── widgets/           # 76 个内置组件
│   ├── view/              # 声明式 DSL（column, row, label…）
│   └── macros/            # define_widget!, tree!
│
├── app/                   # 应用能力 — 组装各域成为可运行应用
│   ├── shell/             # App 生命周期、CLI、DI
│   ├── event_loop/        # 主事件循环
│   ├── bridge/            # UI ↔ Draw 桥接（ScenePaint, trait 适配）
│   ├── window/            # 应用级窗口管理
│   └── navigation/        # 【预留】应用级路由 / 页面栈
│
├── data/                  # 数据能力
│   ├── settings/          # 键值持久化
│   ├── store/             # 【预留】应用状态仓库
│   └── sync/              # 【预留】离线 / 同步
│
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
| **data** | 数据从哪来存哪去 | 配置持久化、（未来）状态仓库与同步 |

### 依赖方向（严格单向）

```
core ← native ← draw ← ui ← app
  ↑      ↑               ↑
  └──────┴─── data ──────┘
```

| 规则 | 说明 |
|------|------|
| 上层依赖下层 | `ui` 依赖 `draw` + `core`，不反向 |
| 平台隔离 | `draw` / `ui` / `app` / `data` / `core` 只依赖 `native::traits` |
| 禁止泄漏 | 上层不得 `use native::backends::*`，不得出现 `#[cfg(windows)]` |

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

当前代码已满足：`#[cfg(windows/unix)]` 仅出现在 `native/backends/` 与 `native/factory.rs`，`draw` / `ui` / `app` / `data` / `core` 中为零。

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

```
native/traits     统一契约 — 上层唯一可见的 native 接口
native/shared     共享逻辑 — 平台只实现最小钩子，其余一份代码
native/backends/  平台实现 — #[cfg] 只在此处
```

**shared 层关键模式**（现有代码，迁移时保留）：

| 模式 | 平台必须实现 | 自动获得 |
|------|-------------|---------|
| `WindowOps` | 6 个方法（show/hide/close/title/size/handle） | `PlatformWindow` + `IWindowProperties` + `INativeHandle` |
| `OsEventSource` | 3 个方法（dispatch_pending/blocking/next_event） | `IEventLoop`（poll/wait/wait_timeout） |
| `FileSystemCore<P>` | `SpecialDirProvider` | `IFileSystem` |

---

## 五、核心数据流

### 5.1 端到端链路

```
OS 事件
  → native/backends（平台特有分发）
  → native/traits::UiEvent（统一格式）
  → app::map_ui_event() → WidgetEvent
  → ui::WidgetTree::dispatch_event()
  → ui::WidgetTree::update(dt) → dirty regions
  → ui::WidgetTree::layout()
  → app/bridge::ScenePaint → draw/compositor::LayerTree::render()
  → draw/engine::end_frame()
  → native/traits::IPresenter::present() → 屏幕
```

从 `app` 往下，没有任何一步需要知道当前是 Windows 还是 Linux。

### 5.2 渲染管线

```
WidgetTree.update(dt) → dirty regions + scroll deltas
    ↓
WidgetTree.layout() → 仅遍历脏子树
    ↓
ScenePaint.scroll_region() → begin_frame(DirtyRects) → LayerTree.render() → end_frame()
    ↓
begin_frame(Overlay) → LayerTree.render_overlays() → end_frame()
    ↓
IPresenter.present() → 屏幕
```

### 5.3 事件流

```
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

### 6.2 目标入口体验

```rust
use uix::prelude::*;

fn main() {
    App::new()
        .title("我的应用")
        .size(1024, 768)
        .root(my_page())
        .run();
}
```

---

## 七、现状 → 目标映射（已完成）

> P3 物理搬迁已完成，下表保留迁移对照供查阅。

| 目标功能域 | 原代码路径 | 状态 |
|-----------|-------------|------|
| `core/error` | `platform/api/error/` | ✅ |
| `core/geometry` | `platform/api/geometry/` | ✅ |
| `core/log` | `platform/log/` | ✅ |
| `core/diagnostic` | `platform/diagnostic/` | ✅ |
| `native/traits` | `platform/api/` | ✅ |
| `native/shared` | `platform/shared/` | ✅ |
| `native/backends/windows` | `platform/windows/` | ✅ |
| `native/backends/linux` | `platform/linux/` | ✅ |
| `native/services` | `platform/services/` | ✅ |
| `native/test_harness` | `platform/test_harness/` | ✅ |
| `draw/*` | `render/*` | ✅ |
| `ui/core` | `widget/core/` | ✅ |
| `ui/foundation/state` | `widget/foundation/state/` | ✅ |
| `ui/foundation/style` | `widget/foundation/style/` | ✅ |
| `ui/widgets` | `widget/widgets/` | ✅ |
| `ui/view` | `view/` | ✅ |
| `app/shell` | `runtime/application.rs` + `view/app.rs` | ✅ |
| `app/event_loop` | `widget/scene/event_loop.rs` | ✅ |
| `app/bridge` | `widget/scene/bridges.rs` + `scene_paint.rs` | ✅ |
| `app/window` | `runtime/window.rs` | ✅ |
| `app/shell/cli` | `runtime/cli.rs` | ✅ |
| `app/shell/di` | `runtime/di.rs` | ✅ |
| `data/settings` | `platform/services/settings.rs` | ✅ |
| `prelude` | — | ✅ |
| `api/*` | `api/` + 各域 re-export | ✅ 已消除 |

### 待清理项

- ~~`api/` 独立镜像层~~（P2 已删除，trait 迁入 `widget/traits/` 与 `render/traits/`）
- ~~`runtime::App` 与 `view::App` 两个同名入口~~（P2 已合并为 `runtime::App`）
- ~~`map_ui_event` 重复实现~~（P2 已去重）

---

## 八、实施路线

| 阶段 | 目标 | 改动范围 |
|------|------|---------|
| **P0 文档** | 确立功能域架构与跨平台规则 | 本文档 ✅ |
| **P1 统一入口** | 新增 `prelude`；demo 改用 prelude；删 render 旧路径 shim | 低 | ✅ |
| **P2 收拢入口** | 合并两个 `App`；`map_ui_event` 去重；消除 `api/` 纯 re-export | 中 | ✅ |
| **P3 物理搬迁** | 按第七章映射表搬迁模块 | 高 | ✅ |

---

## 九、关键设计决策

| 决策 | 方案 | 原因 |
|------|------|------|
| 组织维度 | 六大功能域（core / native / draw / ui / app / data） | 按开发者任务划分，不按技术层 |
| 跨平台 | `native` 域封装全部差异，上层零 `#[cfg]` | 余下代码每个平台完全一致 |
| 单一 crate | 全部合入 `uix` | 消除跨 crate 边界 |
| 契约位置 | 各域 `traits/`，消灭独立 `api/` 层 | 契约与实现同域，通过 `pub` 控制暴露 |
| Widget 能力位 | `WidgetCapabilities` + 上转型 | 按需实现 Layout / Render / Event / Lifecycle |
| 增量渲染 | scroll_region + DirtyRects + PresentDamage | CPU 渲染只重绘变化像素 |
| 测试 | `native/test_harness::FakePlatform` | 上层测试不依赖真实 OS |

---

## 十、代码约束

- `deny(clippy::unwrap_used)`, `deny(clippy::expect_used)`
- 每个 Rust 文件 ≤ 900 行
- 中文注释
- 组合优于继承
- `#[cfg(windows/unix)]` 只允许出现在 `native/backends/` 和 `native/factory.rs`
- 上层模块禁止依赖 `native::backends::*`
