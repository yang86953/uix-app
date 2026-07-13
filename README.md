# UIX

Rust **跨平台 App 框架**（桌面 + 移动端愿景）。入口：`use uix::prelude::*;`。

声明式 UI + 原生绘制 + 零闲置。全栈指 UI + 应用壳 + 本地 Settings，**不含**网络 / 同步。应用 API 为 Rust 门面，现阶段无独立 UI DSL。

> **当前**：桌面 Win/Linux/macOS 源码路径已在树中；**Windows 优先**生产可用（P6 未闭合）；移动端未做。→ [`产品`](docs/产品.md) · [`使用`](docs/使用.md) · [`架构`](docs/架构.md)

## 快速开始

```bash
cargo run --bin uix-demo              # GUI
cargo run --bin uix-demo -- --cli     # CLI
cargo test
```

演示 → [`demo/README.md`](demo/README.md)

## 示例

```rust
use uix::prelude::*;

fn main() {
    let count = State::new(0);

    App::new()
        .title("UIX Counter")
        .size(420, 260)
        .root(move || {
            column((
                count.map_text(|n| format!("当前值: {n}")).font_size(24.0),
                button("+1").primary().on_click(&count, |c| {
                    c.update(|v| *v += 1);
                }),
            ))
            .gap(12.0)
            .padding(16.0)
        })
        .run();
}
```

主题 / 多窗 / 浮层 / Settings 等 → [`docs/使用.md`](docs/使用.md)。

## 功能域

| 域 | 路径 | 用途 |
|----|------|------|
| 入口 | `uix::prelude::*` | App、View、组件、State |
| core | `uix::core::*` | 错误、几何、日志 |
| native | `uix::native::*` | 窗口、事件（仅 traits） |
| draw | `uix::draw::*` | 引擎、颜色、字体 |
| ui | `uix::ui::*` | 组件、布局、主题、View |
| app | `uix::app::*` | App、Window、CLI |
| data | `uix::data::*` | SettingsService |
