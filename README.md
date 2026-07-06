# UIX

Rust 跨平台原生桌面 UI 框架（Windows / Linux）。`use uix::prelude::*;` 为推荐入口。

设计文档：[`docs/Main.md`](docs/Main.md) · 决策 [`docs/decisions.md`](docs/decisions.md) · 规则 [`AGENTS.md`](AGENTS.md)

## 快速开始

```bash
cargo run --bin uix-demo              # GUI
cargo run --bin uix-demo -- --simple  # 简化 View API
cargo run --bin uix-demo -- --cli     # CLI
RUST_LOG=debug cargo run --bin uix-demo
cargo test
```

Linux GUI 需 Wayland 会话。

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
        .root(
            column([
                dynamic_label(move || format!("当前值: {}", label_count.get())).font_size(24.0),
                button("+1").primary().on_click(move || {
                    button_count.set(button_count.get() + 1);
                }),
            ])
            .gap(12.0)
            .padding(16.0),
        )
        .run();
}
```

## 功能域

| 域 | 路径 | 用途 |
|----|------|------|
| 入口 | `uix::prelude::*` | 应用开发 |
| core | `uix::core::*` | 错误、几何、日志 |
| native | `uix::native::*` | 窗口、事件（仅 traits 对外） |
| draw | `uix::draw::*` | 引擎、颜色、字体 |
| ui | `uix::ui::*` | 组件、布局、主题、View DSL |
| app | `uix::app::*` | App、Window、CLI |
| data | `uix::data::*` | SettingsService |

架构约束见 [`AGENTS.md`](AGENTS.md)。
