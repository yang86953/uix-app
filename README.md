# UIX

Rust 跨平台原生桌面 UI 框架（Windows / Linux）。`use uix::prelude::*;` 为推荐入口。

## 核心理念

> **用最少资源，做最好效果。**

这是 UIX **最重要的一条规则**（[#105](docs/decisions.md#d105)，Demand-Driven Zero Idle Work）：有触发才工作，无 pending 则休眠；变化尽量窄，效果不妥协。统领六域依赖、主循环、渲染、事件与 API 设计——冲突时 **以本规则为准**。

**零维护**（[#130](docs/decisions.md#d130)）：Picture、Registry 托管、标脏、Theme 由框架自动；App 用 State/View + **Timer API**（[#132](docs/decisions.md#d132)）、**`post_to_ui`**（[#133](docs/decisions.md#d133)）与 **`on_start`**（[#140](docs/decisions.md#d140)）注入运行中句柄。

细则 → [`docs/areas/systems/demand-driven.md`](docs/areas/systems/demand-driven.md) · [`AGENTS.md`](AGENTS.md)

> **面向 AI Agent**：编码前必读 [`AGENTS.md`](AGENTS.md)（硬约束 + checklist）；架构导航 [`docs/areas/architecture.md`](docs/areas/architecture.md)。

## 快速开始

```bash
cargo run --bin uix-demo              # GUI 多页演示（默认）
cargo run --bin uix-demo -- --cli     # CLI
RUST_LOG=debug cargo run --bin uix-demo
cargo test
```

Linux GUI 需 Wayland 会话。

演示模式、页面说明与 CLI 项 → [`demo/README.md`](demo/README.md)

| 模式 | 命令 | 说明 |
|------|------|------|
| GUI（默认） | `cargo run --bin uix-demo` | 11 页多页应用：Gallery 覆盖矩阵、App Timer/Theme、全部内置 Widget |
| CLI | `--cli` | 各功能域 API 无 GUI 演示 |

公开 API 清单 → [`docs/areas/systems/public-api.md`](docs/areas/systems/public-api.md) · 设计文档 → [`docs/areas/architecture.md`](docs/areas/architecture.md) · 变更记录 → [`CHANGELOG.md`](CHANGELOG.md)

## 示例

```rust
use uix::prelude::*;

fn main() {
    let count = State::new(0);
    let label_count = count.clone();
    let button_count = count.clone();

    App::new()
        .title("UIX Counter")
        .size(420, 260)
        .root(move || {
            let label_count = label_count.clone();
            let button_count = button_count.clone();
            column([
                dynamic_label(move || format!("当前值: {}", label_count.get())).font_size(24.0),
                button("+1").primary().on_click(move || {
                    button_count.set(button_count.get() + 1);
                }),
            ])
            .gap(12.0)
            .padding(16.0)
        })
        .run();
}
```

## 功能域

| 域 | 路径 | 用途 |
|----|------|------|
| 入口 | `uix::prelude::*` | 应用、View DSL、常用组件、事件、handle 与 State |
| core | `uix::core::*` | 错误、几何、日志 |
| native | `uix::native::*` | 窗口、事件（仅 traits 对外） |
| draw | `uix::draw::*` | 引擎、颜色、字体 |
| ui | `uix::ui::*` | 组件、布局、主题、View DSL |
| app | `uix::app::*` | App、Window、CLI |
| data | `uix::data::*` | SettingsService |

架构全貌 → [`docs/areas/architecture.md`](docs/areas/architecture.md) · 最高规则 → [`AGENTS.md`](AGENTS.md)
