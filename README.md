# UIX — Rust Native UI Framework

A modular, cross-platform native UI framework for Rust with a composition-over-inheritance architecture. Supports **Linux (Wayland)** and **Windows (Win32)**.

## Design Principles

- **Trait-based composition** — Platform, Widget, GraphicsEngine as injectable traits
- **Composition over inheritance** — No base classes, no trait hierarchies simulating OOP
- **Owned widget tree** — Two-phase construction with explicit lifecycle
- **Pure software rendering** — CPU-based 2D rasterizer with SDF text, shadows, gradients
- **Reactive state management** — `State<T>` / `Computed<T>` with dependency tracking
- **DI container** — Type-erased service registry for dependency injection
- **Diagnostics-first** — Structured logging, retry policies, circuit breaker, error collection

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

## Quick Start

```bash
# Run the GUI demo (Wayland on Linux, Win32 on Windows)
cargo run --bin uix-demo

# Run the CLI demo
cargo run --bin uix-demo -- --cli
```

## Platform Support

| Platform | Backend | Status |
|----------|---------|--------|
| Linux (Wayland) | `wayland-client` 0.29 | ✅ Working (SHM buffer rendering) |
| Windows | Win32 API via `windows` crate | ✅ Working (GDI DIB presentation) |
| macOS | - | ❌ Not yet supported |

## Module Layout

| Module | Path | Description |
|--------|------|-------------|
| `uix::app` | `src/app/` | App lifecycle, Window, CLI, DI container |
| `uix::base` | `src/base/` | Foundation geometry (Point, Rect, Size, EdgeInsets) |
| `uix::graphics` | `src/graphics/` | GraphicsEngine trait, SoftwareEngine, layout, types |
| `uix::platform` | `src/platform/` | Platform abstraction, Win32 + Wayland backends |
| `uix::ui` | `src/ui/` | Widget tree, 11 widget types, theme, animation, managers |
| `uix::services` | `src/services/` | File service, settings, middleware pipeline |
| `uix::diag` | `src/diag/` | Error types, structured logging, recovery, collector |

## Widget Library

- **Button** — Primary, Default, Dashed, Text, Link, Disabled variants
- **Card** — Elevation levels, hoverable, custom children
- **Container** — Flexbox container with direction, justify, align, padding, gap
- **Divider** — Horizontal/vertical, with text label
- **Grid** — Multi-column grid layout
- **Input** — Text input with placeholder, sizes
- **Label** — Text label with color and font size
- **Modal** — Overlay dialog with backdrop
- **Progress** — Progress bar with percentage
- **Space** — Spacing component with configurable gap
- **Tabs** — Tab-based content switching

## State Management

```rust
let count = State::new(0);
count.watch(|v| println!("count = {}", v));
count.set(1);

let sum = Computed::new(|| a.get() + b.get());
```

## Theming

UIX ships with Ant Design 5 design tokens (light and dark themes):

```rust
let tokens = DesignTokens::antd_light();
// or DesignTokens::antd_dark()
```
