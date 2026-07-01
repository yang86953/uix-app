# UIX — Rust Native UI Framework

A modular, cross-platform native UI framework for Rust with a composition-over-inheritance architecture. Supports **Linux (Wayland)** and **Windows (Win32)**.

> **Current status**: ~85% complete — core architecture stable, 54+ widgets, full software renderer.
> Recent: Incremental rendering pipeline — pixels-only scroll (`scroll_region`), dirty-rect-only redraw (`DirtyRects`), FrameGraph pass culling.
> See [docs/graphics-engine-redesign.md](docs/graphics-engine-redesign.md) for detailed design documentation.

## Design Principles

- **Trait-based composition** — Platform, Widget, GraphicsEngine as injectable traits
- **Composition over inheritance** — No base classes, no trait hierarchies simulating OOP
- **Owned widget tree** — Two-phase construction with explicit lifecycle
- **Incremental rendering** — Only redraw what changed: dirty-rect culling, pixel-scroll (`scroll_region`), FrameGraph pass culling
- **Pure software rendering** — CPU-based 2D rasterizer with SDF text, shadows, gradients
- **Reactive state management** — `State<T>` / `Computed<T>` with dependency tracking
- **DI container** — Type-erased service registry for dependency injection
- **Diagnostics-first** — Structured logging, retry policies, circuit breaker, error collection

## Quick Start

```bash
# Run the GUI demo (Ant Design 5 dashboard)
cargo run --bin uix-demo

# Run the CLI demo (subsystem feature showcase)
cargo run --bin uix-demo -- --cli

# Run tests
cargo test
```

## Architecture

UIX 采用 5 workspace crate 编译期隔离 + proc-macro crate 的模块化架构。

### Workspace Crates

```
uix workspace                   编译期边界
├── platform/ (uix-platform)   OS 抽象层 — Win32 / Wayland / 文件服务 / 通知 / 设置
├── graphics/ (uix-graphics)   2D 渲染引擎 + 帧图 + 路径 + 字体 + 布局类型
├── ui/        (uix-ui)        Widget 框架 — 54+ 组件 + 主题 + 动画 + 状态管理
├── ui/macros/ (uix-macros)    proc-macro (ui! / define_widget! / tree!)
├── app/       (uix-app)       应用入口 — 窗口生命周期 + CLI + DI
├── uix        (根 crate)      聚合重导出层（pub use uix_app / uix_graphics / …）
└── demo                       演示二进制
```

### 层依赖

```
app  uix  ──→ ui ──→ graphics ──→ platform
  │  ├──→ platform (factory: create_platform / create_gpu_context)
  │  ├──→ graphics (RenderContext, LayerTree 依赖 GraphicsEngine)
  │  └──→ app      (Window → Platform, Cli, Container)

ui/macros 编译期 proc-macro，无运行时依赖。
```

### 各 Crate 职责

| Crate | 职责 | 主要导出 |
|-------|------|----------|
| **uix-platform** | OS 抽象：Win32/Wayland 窗口、事件、文件、日志、通知、设置 | `Platform`, `PlatformWindow`, `Error`, `EventBus`, `Point/Size/Rect`, 子系统 trait 簇 |
| **uix-graphics** | 2D 渲染：软件引擎 + GPU 引擎 + 帧图 + 路径 + 字体/文本 + 颜色 | `GraphicsEngine`, `Color`, `Canvas2D`, `FrameGraph`, `Path`, `TextBackend` |
| **uix-ui** | Widget 框架：54+ 组件 + Flexbox/Grid 布局 + 主题 + 动画 + 响应式状态 | `Widget`, `State`/`Computed`/`Effect`, `Animation`, `Theme`, 全部组件 |
| **uix-app** | 应用入口：窗口管理 + CLI 解析 + DI 容器 | `App`, `Window`, `Cli`, `Container` |
| **uix（根）** | 聚合重导出，用户 `use uix::*` 即可使用全部 | `app`, `graphics`, `platform`, `ui` |

## Platform Support

| Platform | Backend | Status |
|----------|---------|--------|
| Windows | Win32 API via `windows` crate | ✅ Complete (GDI DIB presentation) |
| Linux | Wayland via `wayland-client` 0.29 | ✅ Working (SHM buffer) |
| macOS | - | ❌ Not yet supported |

## Widget Library (54+ Components)

### Basic ($\checkmark$ 10)
`Button` `Card` `Container` `Divider` `Icon` `Image` `Label` `Space` `Spin` `Typography`

### Form ($\checkmark$ 10)
`AutoComplete` `Checkbox` `ColorPicker` `Dropdown` `Form` `Input` `Radio` `Rate` `Select` `Segmented` `Slider` `Switch`

### Navigation ($\checkmark$ 8)
`Breadcrumb` `Menu` `Nav` `Pagination` `Steps` `Tabs` `Timeline` `Tree` `TreeSelect`

### Data Display ($\checkmark$ 14)
`Avatar` `Calendar` `Chart`(Bar, Line, Pie) `Descriptions` `Grid` `List` `Progress` `Result` `Skeleton` `Table` `Tag` `Empty`

### Feedback ($\checkmark$ 7)
`Alert` `Drawer` `Message` `Modal` `Notification` `Popconfirm` `Popover` `Tooltip`

### Others ($\checkmark$ 5)
`Collapse` `FloatButton` `ScrollView` `Badge` `Misc`

## Core Features

### 🎨 Declarative UI with `ui!` Macro
```rust
use uix::ui;

let node = ui! {
    <Container direction=Row gap=8.0>
        <Button variant="primary" on_click={|| println!("clicked")}>Click</Button>
        <Label color=#666>Description</Label>
    </Container>
};
```

### 📐 Flexbox + Grid Layout
```rust
let result = compute_flex_layout(&FlexInput {
    direction: FlexDirection::Row,
    gap: 8.0,
    padding: EdgeInsets::uniform(16.0),
    container: Rect::new(0.0, 0.0, 400.0, 300.0),
    children: vec![FlexChild { flex_grow: 1.0, .. }, FlexChild { flex_grow: 2.0, .. }],
    ..Default::default()
});
```

### 🔄 Reactive State with Auto-Tracking
```rust
let count = State::new(0);
count.watch(|v| println!("count = {}", v));
count.set(1);

// Computed 自动追踪依赖
let a = State::new(1);
let b = State::new(2);
let sum = Computed::new(move || a.get() + b.get());
a.set(10);
assert_eq!(sum.get(), 12);  // 自动重新计算

// Effect — 响应式副作用
let eff = Effect::new(|| println!("count = {}", count.get()));
count.set(42);  // eff.tick() 返回 true
```

### 🎯 Incremental Rendering — 只在变动处绘制

ScrollView 滚动时无需全帧重绘：

```
wheel → velocity → on_update → dirty_rect(strip) + scroll_delta(dx,dy)
  ↓
drain_scroll_deltas → canvas.scroll_region(viewport, dx, dy)  // memmove 现有像素
  ↓
begin_frame(DirtyRects)  // 只清除 strip，不清全帧
  ↓
render()  // clip 到 strip，只重绘新暴露区域
```

| 机制 | 效果 |
|------|------|
| `Canvas2D::scroll_region` | 像素级 memmove，O(偏移量×视口宽)，非 O(全帧) |
| `UpdateStrategy::DirtyRects` | 只清除变化区域，不清全帧 |
| `Widget::dirty_rect` | 每个 widget 精确计算自身变化区域 |
| `FrameGraph` culling | 输入未变+输出无消费→跳过 Pass，`zero_frame_cost` 休眠 |
| `old_dirty_rect` 动画快照 | 动画前后脏区域对比，无视觉残留 |

### 🧩 Widget Trait Composition
Widget 行为拆分为四个维度，可按需使用：
```rust
fn render_only(w: &impl WidgetRender, ctx: &mut RenderContext) {
    w.render(w.frame(), ctx, tree);
}
fn get_layout(w: &impl WidgetLayout) -> f32 {
    w.preferred_size(None).h
}
// 子 trait 通过 blanket impl 从 Widget 自动派生
```

### 🎯 Theme System (Ant Design 5)
```rust
let light = DesignTokens::antd_light();
let dark = DesignTokens::antd_dark();

// Runtime theme switching via DynTokens
let dyn_tokens = Arc::new(DynTokens::new(light));
dyn_tokens.set_mode(true); // switch to dark
```

### 📦 DI Container
```rust
let mut container = Container::new();
container.singleton(database_pool);
container.singleton(config);

let pool = container.resolve::<DatabasePool>();
```

## Project Structure

```
uix-app/                        # workspace 根（Cargo.toml）
├── lib.rs                      # crate 根入口 — 聚合重导出
├── src/lib.rs                  # crate 源（pub use uix_* as *）
├── platform/                   # uix-platform crate
│   ├── src/                    # error, geometry, event, event_bus, log,
│   │                           # presenter, types, shared, diagnostic,
│   │                           # file_service, notification, settings
│   │                           # windows/ (Win32), linux/ (Wayland)
│   └── tests/geometry.rs
├── graphics/                   # uix-graphics crate
│   ├── src/                    # engine/, frame_graph/, gpu_engine/,
│   │                           # rasterizer/, text_backends/, traits/
│   │                           # api, bitmap_font, blur, color, flattener,
│   │                           # font_service, null_engine, path, stroker,
│   │                           # text_backend, types
│   └── tests/integration.rs
├── ui/                         # uix-ui crate
│   ├── src/                    # animation/, layout/, managers/, theme/,
│   │   │                       # widget/, widgets/ (54+ components)
│   │   │                       # api, children, clipboard, config_provider,
│   │   │                       # context, focus_trap, layer, locale, macros,
│   │   │                       # render_context, render_loop, state, style,
│   │   │                       # virtual_scroll, widget_builder
│   ├── macros/                 # uix-macros proc-macro crate (ui! macro)
│   └── tests/                  # animation, core, layout, theme, widgets
├── app/                        # uix-app crate
│   ├── src/                    # application, cli, di, window, api
│   └── tests/cli_and_di.rs
├── demo/                       # 演示二进制
│   └── src/main.rs
├── assets/fonts/               # 字体资源（lucide.ttf）
├── docs/                       # 设计文档
├── AGENTS.md                   # 项目规则
└── README.md                   # 本文件
```

## Testing

```bash
# Run all tests
cargo test

# Run specific module tests
cargo test ui::layout
cargo test graphics::types
cargo test ui::state  # 响应式状态测试
```

Current test count: **332 unit tests** across 6 crates (all passing on Linux/Wayland; Windows 部分平台相关测试略少).

## Building

```bash
# Debug build
cargo build

# Release build
cargo build --release
```

## Planned Improvements

### 🔮 中长期
- **GPU 渲染后端** — Direct2D / Vulkan 支持
- **macOS 支持** — 通过 AppKit 桥接
- **平台 FFI 迁移** — 完全替换本地 `extern` 声明为 `windows` crate
- **文档生成** — 基于代码分析自动生成 API 引用文档

## Contributing

See [AGENTS.md](AGENTS.md) for project rules and conventions.

Key guidelines:
- `#![deny(clippy::unwrap_used)]` — no unwrap/expect in production code
- Each Rust file ≤ 900 lines
- Chinese comments for internal documentation
- CodeGraph for code analysis before refactoring
