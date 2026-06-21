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

```
┌──────────────────────────────────────────────┐
│                   Application                 │
│  (App lifecycle, Window event loop, CLI/DI)   │
├──────────────────────────────────────────────┤
│                      UI                       │
│  (Widget tree, layout, render, state, theme)  │
├───────────────────┬──────────────────────────┤
│     Graphics      │        Platform           │
│  (SoftwareEngine, │  (Win32 / Wayland, input, │
│   frame graph,    │   clipboard, display,     │
│   layout engine)  │   file dialog, timer...)  │
├───────────────────┴──────────────────────────┤
│              Services + Diagnostics           │
│  (File, Settings, Middleware / Error, Log,    │
│   Recovery, Collector)                       │
└──────────────────────────────────────────────┘
```

### Layer Responsibilities

| Layer | Responsibility |
|-------|---------------|
| **6 — App** | Entry point, window lifecycle, CLI routing, DI container |
| **5 — UI** | Widget tree, layout (Flexbox/Grid), theme, animation, state mgmt |
| **4 — Graphics** | 2D rendering engine, FrameGraph, SoftwareEngine, font service |
| **3 — Platform** | OS abstraction: window, events, clipboard, file dialog, console |
| **2 — Services** | File I/O, settings persistence, middleware pipeline |
| **1 — Diagnostics** | Error types, structured logging, recovery policies, collectors |
| **0 — Base** | Foundation types: Point, Rect, Size, EdgeInsets, Color |

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

### 🔄 Reactive State
```rust
let count = State::new(0);
count.watch(|v| println!("count = {}", v));
count.set(1);

let sum = Computed::new(|| a.get() + b.get());
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
├── uix-macros/       # proc-macro crate (ui! macro)
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

### 🔜 近期
- **Workspace crates** — 拆分为 `uix-core`、`uix-diag`、`uix-graphics`、`uix-platform`、`uix-ui`、`uix-services`、`uix-app` 独立 crate，编译期强制层边界
- **Widget trait 拆分** — 将 15 方法的 `Widget` trait 拆为 `WidgetRender`、`WidgetLayout`、`WidgetEvent`、`WidgetLifecycle` 子 trait

### 🔮 中长期
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
