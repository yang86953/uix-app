# UIX — Rust Native UI Framework

Modular UI framework for Windows, built in Rust with a composition-over-inheritance architecture.

## Crates

| Crate | Description |
|-------|-------------|
| `uix-graphics` | 2D rendering abstractions, geometry, color, software engine |
| `uix-platform` | Windows platform layer (Win32), event loop, window management |
| `uix-ui` | Widget tree, layout, rendering, state management |
| `uix-services` | File service, settings, middleware |
| `uix-diag` | Diagnostics: error collection, logging, retry/circuit breaker |
| `uix-app` | Application entry point, DI container, CLI/GUI modes |
| `demo` | GUI demo application |

## Quick Start

```bash
cargo run --bin uix-demo
```

## Architecture

- **Trait-based composition**: Platform, Widget, GraphicsEngine defined as traits with injected implementations
- **DI container**: Type-erased service registry for dependency injection
- **Widget tree**: Owned-tree model with layout/render phases
- **Diagnostics first**: Built-in error collection, structured logging, retry policies

