# UIX 架构地图

> 最后更新：2026-07-05  
> **用途**：供协作者和 AI 快速理解定稿架构、源码边界与常用入口。源码若与本文不一致，按本文与 `docs/` 重构。  
> 上手指南 → [`README.md`](README.md) · 设计文档 → [`docs/Main.md`](docs/Main.md) · 维护规则 → [`AGENTS.md`](AGENTS.md)

---

## 1. 项目概览

| 项 | 设计 |
|----|----------|
| 名称 | UIX — Rust 跨平台原生桌面 UI 框架 |
| 版本 | `0.1.0`，积极开发中 |
| 推荐入口 | `use uix::prelude::*;` |
| 主要源码 | 根 crate 的 `src/` |
| 演示入口 | `demo/` 中的 `uix-demo` |
| 设计组织 | 设计按系统写；代码按功能域实现 |

UIX 在 Windows 与 Linux 上提供一致的组件模型、渲染管线、输入事件和应用生命周期。平台差异只能封装在 `native` 域，上层通过 trait 与 `Result` 感知能力差异。

---

## 2. 功能域地图

依赖方向保持严格单向：

```text
core ← native ← draw ← ui ← app
  ↑      ↑               ↑
  └──────┴─── data ──────┘
```

| 域 | 路径 | 职责 |
|----|------|------|
| core | `src/core/` | 错误、几何、日志、诊断 |
| native | `src/native/` | 平台能力：窗口、事件、输入、文件、通知、呈现 |
| draw | `src/draw/` | 绘制引擎、光栅化、字体、图片、合成、帧管线 |
| ui | `src/ui/` | Widget、布局、主题、动画、View DSL、响应式状态 |
| app | `src/app/` | 应用生命周期、主循环、窗口、CLI、DI、桥接层 |
| data | `src/data/` | 配置持久化 |

关键桥接：

| 链路 | 入口 |
|------|------|
| 组件树 → 渲染场景 | `src/app/bridge/scene_paint.rs` |
| 主循环 → 帧渲染 | `src/app/event_loop/event_loop.rs` → `src/draw/pipeline/render_frame.rs` |
| 平台实现选择 | `src/native/factory.rs` |
| 对外常用符号 | `src/prelude.rs` |

---

## 3. 平台支持

| 平台 | 路径 | 呈现方式 |
|------|----------|----------|
| Windows | `src/native/backends/windows/` | CPU GDI DIB，GPU 能力按实现可选 |
| Linux | `src/native/backends/linux/` | Wayland SHM，可选 EGL/GLES |
| macOS | 无后端 | 未支持 |

硬约束见 [`AGENTS.md`](AGENTS.md)：`#[cfg(windows/unix)]` 只允许出现在 `src/native/backends/` 与 `src/native/factory.rs`。

---

## 4. 能力蓝图

### 框架基础

| 能力 | 要求 |
|------|------|
| 六大功能域结构 | 按 `core / native / draw / ui / app / data` 组织 |
| `prelude` 统一入口 | 应用侧推荐 `use uix::prelude::*;` |
| native trait + backend 隔离 | 上层只依赖 `native::traits` |
| FakePlatform / test harness | 用于平台与渲染断言 |

### UI 与响应式

| 能力 | 关键路径 |
|------|----------|
| View DSL：`column`、`row`、`button`、`dynamic_label` 等 | `src/ui/view/` |
| `State<T>` 驱动 Paint 失效 | `src/ui/foundation/state.rs` |
| `Computed` / `Effect` | `src/ui/foundation/state.rs`，主循环帧末调度 |
| 内置 Widget | `src/ui/widgets/` |
| Ant Design 风格 token / theme | `src/ui/theme/` |

### 渲染

| 能力 | 关键路径 |
|------|----------|
| CPU 软件渲染 | `src/draw/engine/cpu/` |
| GPU 画布入口 | `src/draw/gpu_engine/` |
| DirtyRects / PresentDamage | `src/draw/pipeline/` |
| DisplayList / LayerTree | `src/draw/painting/`，`src/draw/compositor/` |
| 图片懒加载与解码 | `src/draw/image/` |
| F12 调试 HUD / RenderMetrics | `src/draw/debug/`，`src/draw/pipeline/metrics.rs` |

---

## 5. 定稿设计要点

| 主题 | 设计 | 参考 |
|------|------|------|
| 事件链路 | `UiEvent → SystemEvent → SemanticEvent / CustomEvent` 全链路覆盖所有输入 | `docs/systems/event.md` |
| 业务绑定 | 业务回调存入 `HandlerTable`，组件 struct 不保存业务闭包 | `docs/systems/component.md` |
| Button API | Button 为纯文本组件，外观由 `StyleSet::button_*()` 预设驱动 | `docs/systems/component.md`, `docs/systems/theme-style.md` |
| 浮层 | `OverlayStack` 统一调度浮层、焦点与事件穿透 | `docs/systems/overlay.md` |
| 多窗口 / AppState | v1 多窗口共享 AppState + Theme，每窗独立组件树 | `docs/systems/application.md` |
| 主题解析 | Style 通过 ColorValue / TypographyToken 延迟解析 | `docs/systems/theme-style.md` |

---

## 6. 一帧数据流

```text
OS 事件 / 动画帧
  → native/backends（平台分发）
  → native/traits::UiEvent（统一事件）
  → app 边界转换为 SystemEvent
  → app 主循环
       ├─ WidgetTree::dispatch_system_event()
       ├─ HandlerTable 派发 SemanticEvent / CustomEvent
       ├─ WidgetTree::update(dt)
       ├─ WidgetTree::layout()
       └─ FrameRenderer::render_frame()
            ├─ WidgetTree as ScenePaint
            ├─ LayerTree / DisplayList / Picture
            ├─ GraphicsEngine begin_frame / end_frame
            └─ IPresenter::present()
  → WidgetTree::reset_dirty()
```

从 `app` 往下，业务代码不需要知道运行在 Windows 还是 Linux。

---

## 7. 失效与 present 语义

| 类型 | 消费方 | 是否驱动 present |
|------|--------|------------------|
| `Layout` | `WidgetTree::layout()` | 否 |
| `Paint { id, rect }` | `FrameRenderer` | 是 |
| `Composite { scroll }` | 滚动条带 / 合成路径 | 是 |

主循环只在有首帧、Paint、Composite 或动画工作时上屏；纯 Layout 失效只更新几何，不提交像素。

---

## 8. 常用验证入口

```bash
cargo test
cargo test --features test-harness
cargo run --bin uix-demo
cargo run --bin uix-demo -- --simple
cargo run --bin uix-demo -- --cli
```

这些命令是项目约定入口。若依赖文件被改动，先以 root `Cargo.toml` 和 `git status` 为准。

---

## 9. 落地约束

| 项 | 说明 |
|----|------|
| macOS | 无 backend |
| 兼容层 | 不保留长期兼容层；源码不一致时按文档重构 |
| 构建入口 | 若 `Cargo.toml` 被截断，先修复 crate 配置再运行 Cargo 命令 |
| Git 状态 | 若出现大量 `D` + 同名 `??`，先确认是否为异常工作树状态，不要误回滚 |

---

## 10. 改 X 先看 Y

| 我要改… | 先看 |
|---------|------|
| 主循环 / present | `src/app/event_loop/event_loop.rs` |
| 帧渲染 / damage | `src/draw/pipeline/render_frame.rs` |
| 失效队列 | `src/draw/pipeline/invalidation.rs` |
| State 绑定 / 响应式失效 | `src/ui/foundation/state.rs`，`src/ui/view/adapter.rs` |
| 滚动 / hover 失效 | `src/ui/core/widget/tree_events.rs`，`src/ui/core/widget/tree_layout.rs` |
| 组件绘制 | `src/ui/widgets/`，`src/draw/painting/paint_context.rs` |
| 平台窗口 / 呈现 | `src/native/backends/`，`src/native/traits/` |
| 设计取舍 | [`docs/decisions.md`](docs/decisions.md) |
| 系统级设计 | [`docs/Main.md`](docs/Main.md) |
| 编码约束 | [`AGENTS.md`](AGENTS.md) |

---

## 11. 文档分工

| 文档 | 什么时候看 |
|------|------------|
| [`README.md`](README.md) | 第一次上手、运行 demo、了解公共 API |
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | 快速定位架构地图与源码边界 |
| [`docs/Main.md`](docs/Main.md) | 理解系统级设计与文档目录 |
| [`docs/decisions.md`](docs/decisions.md) | 查设计决策、废止关系和后续追加编号 |
| [`AGENTS.md`](AGENTS.md) | 写代码前确认架构硬约束 |
