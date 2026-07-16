# UIX

Rust **跨平台 App 框架**。入口：`use uix::prelude::*;`。

声明式 UI + 原生绘制 + 零闲置。全栈指 UI + 应用壳 + 本地 Settings，**不含**网络 / 同步。应用 API 为 Rust 门面，无独立 UI DSL。

> **当前**：Windows P6 基线已闭合；Linux / macOS 源码在树，移动端未做。默认 Vulkan 图形，结构化 adapter 诊断与失败 typed。→ [`产品`](docs/产品.md) · [`使用`](docs/使用.md) · [`架构`](docs/架构.md) · [`活跃缺口`](docs/进度.md)

## 快速开始

```bash
cargo run --bin uix-demo              # GUI（11 页 80+ 组件）
cargo run --bin uix-demo -- --cli     # CLI
```

演示 → [`demo/README.md`](demo/README.md)

## 示例

```rust
use uix::prelude::*;

fn main() {
    App::new()
        .title("Hello UIX")
        .size(400, 300)
        .root(|| label("Hello, world!").font_size(32.0))
        .run();
}
```

带交互的计数器：

```rust
use uix::prelude::*;

fn main() {
    let count = State::new(0);

    App::new()
        .title("计数器")
        .size(360, 200)
        .root(move || {
            column((
                count.map_text(|n| format!("{n}")).font_size(48.0),
                row((
                    button("-1").on_click(&count, |c| c.update(|v| *v -= 1)),
                    button("+1").primary().on_click(&count, |c| c.update(|v| *v += 1)),
                )).gap(8.0),
            ))
            .gap(16.0)
            .padding(24.0)
        })
        .run();
}
```

主题 / 多窗 / 浮层 / Settings / IME / 图表等 → [`docs/使用.md`](docs/使用.md)。

## 功能域

| 域 | 路径 | 用途 |
|----|------|------|
| 入口 | `uix::prelude::*` | App、View、State、组件 |
| core | `uix::core::*` | 错误、几何、日志、DI |
| native | `uix::native::*` | 窗口、事件、IME（仅 traits） |
| draw | `uix::draw::*` | 图形引擎、Vulkan/D3D11/Software、颜色、字体 |
| ui | `uix::ui::*` | 组件（80+）、布局、主题、动画 |
| app | `uix::app::*` | App、Window、CLI、多窗、Agent Bridge |
| data | `uix::data::*` | SettingsService（opt-in KV） |

## 图形

默认 Vulkan；失败按 typed 原因在同 API 内有界恢复，耗尽后回 Software(GDI)，仅显式 `Auto` 可跨 GPU API probe。失败终态 `TerminalFailure`，无静默换 API、无 busy retry。空闲真休眠；Windows P6 遮挡基线已闭合。
