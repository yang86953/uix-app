# UIX

Rust **跨平台（桌面 + 移动端）全栈 App 开发框架**。入口：`use uix::prelude::*;`。

**Web 式声明 UI + 原生性能 + 零闲置 + 全栈**（[#105](docs/决策.md#d105)）。应用作者 API 继续打磨 Rust 门面，现阶段不另起 UI DSL（[#179](docs/决策.md#d179)）。

> **当前**：桌面 Win/Linux/macOS 已编码；**Windows 优先**（[#167](docs/决策.md#d167)）；移动端未实现。愿景 → [`产品`](docs/产品.md) · 上手 → [`架构 · 应用作者入口`](docs/架构.md#应用作者入口) · 硬约束 → [`架构 · #105`](docs/架构.md#核心理念最高规则-105)。

## 快速开始

```bash
cargo run --bin uix-demo              # GUI（默认）
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
                    c.set(c.get() + 1);
                }),
            ))
            .gap(12.0)
            .padding(16.0)
        })
        .run();
}
```

## 功能域

| 域 | 路径 | 用途 |
|----|------|------|
| 入口 | `uix::prelude::*` | App、View、组件、事件、State |
| core | `uix::core::*` | 错误、几何、日志 |
| native | `uix::native::*` | 窗口、事件（仅 traits） |
| draw | `uix::draw::*` | 引擎、颜色、字体 |
| ui | `uix::ui::*` | 组件、布局、主题、View |
| app | `uix::app::*` | App、Window、CLI |
| data | `uix::data::*` | SettingsService |
