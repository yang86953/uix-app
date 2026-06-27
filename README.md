# UIX — Rust Native UI Framework

A modular, cross-platform native UI framework for Rust with a composition-over-inheritance architecture. Supports **Linux (Wayland)** and **Windows (Win32)**.

> **Current status**: ~80% complete — core architecture stable, 54+ widgets, full software renderer.
> See [ARCHITECTURE.md](ARCHITECTURE.md) for detailed design documentation.

## Design Principles

- **Trait-based composition** — Platform, Widget, GraphicsEngine as injectable traits
- **Composition over inheritance** — No base classes, no trait hierarchies simulating OOP
- **Owned widget tree** — Two-phase construction with explicit lifecycle
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

UIX 采用 7 层架构 + workspace crates 编译期保障。

### Workspace Crates

```
uix workspace             编译期边界
├── uix-core/   (L0)     基础类型，零依赖
├── uix-diag/   (L1)     诊断，仅依赖 core
├── ui/macros/           proc-macro (ui! macro)
├── uix/        (L2-L6)  主 crate
│   ├── services/ (L2)   文件/设置/中间件/通知
│   ├── platform/ (L3)   Win32 / Wayland
│   ├── graphics/ (L4)   渲染引擎 + LayerTree 桥接
│   ├── ui/       (L5)   Widget 框架
│   └── app/      (L6)   应用入口
└── demo/               演示二进制
```

### 层依赖

```
L6  App  ──→ UI ──→ Graphics ──→ Diag ──→ Core
                 ↘ Platform  ──→ Diag ──→ Core
                    Services ──→ Diag ──→ Core
```

| 层 | crate | 职责 | 依赖 |
|-----|-------|------|------|
| **6 — App** | `uix` | 入口、窗口、CLI、DI | 所有下层 |
| **5 — UI** | `uix` | Widget 树、布局、主题、状态 | Graphics, Core |
| **4 — Graphics** | `uix` | 渲染引擎、LayerTree | Diag, Core |
| **3 — Platform** | `uix` | Win32/Wayland 抽象 | Diag, Core |
| **2 — Services** | `uix` | 文件、设置、中间件、通知 | Diag, Core |
| **1 — Diagnostics** | `uix-diag` | 错误、日志、恢复 | Core |
| **0 — Core** | `uix-core` | Point、Rect、Color 等 | 无 |

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
uix-app/
├── src/
│   ├── app/          # Application layer
│   ├── base/         # Foundation types
│   ├── diag/         # Diagnostics (error, log, recovery)
│   ├── graphics/     # Graphics engine + layout
│   ├── platform/     # OS abstraction (windows/, linux/)
│   ├── services/     # Business services
│   ├── ui/           # Widget framework
│   │   ├── layout/   # Flexbox + Grid engine
│   │   ├── managers/ # 10-manager system
│   │   ├── theme/    # Design token system
│   │   └── widgets/  # 54+ components
│   ├── demos/        # GUI + CLI demos
│   ├── lib.rs        # Crate root
│   └── main.rs       # Binary entry
├── ui/macros/       # proc-macro crate (ui! macro) 内嵌于 ui
├── tests/            # Integration tests
├── assets/           # Fonts, resources
└── Cargo.toml
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

Current test count: **266 unit tests** + 16 doc-tests (all passing).

## Building

```bash
# Debug build
cargo build

# Release build
cargo build --release
```

## Planned Improvements

### 🔮 中长期
- **进一步 crate 拆分** — 待解决 graphics ↔ ui 循环依赖后，将 platform/graphics/ui/app 拆为独立 crate
- **GPU 渲染后端** — Direct2D / Vulkan 支持
- **macOS 支持** — 通过 AppKit 桥接
- **平台 FFI 迁移** — 完全替换本地 `extern` 声明为 `windows` crate

## Contributing

See [AGENTS.md](AGENTS.md) for project rules and conventions.

Key guidelines:
- `#![deny(clippy::unwrap_used)]` — no unwrap/expect in production code
- Each Rust file ≤ 900 lines
- Chinese comments for internal documentation
- CodeGraph for code analysis before refactoring
