# UIX

Rust **跨平台 App 框架**。入口：`use uix::prelude::*;`。

声明式 UI + 原生绘制 + 零闲置。全栈指 UI + 应用壳 + 本地 Settings，**不含**网络 / 同步。应用 API 为 Rust 门面，无独立 UI DSL。

> **当前**：Windows 优先（P6 gate 推进中）；Linux / macOS 源码在树；移动端未做。默认 Vulkan 图形，结构化 adapter 诊断与失败 typed。→ [`产品`](docs/产品.md) · [`使用`](docs/使用.md) · [`架构`](docs/架构.md)

## 快速开始

```bash
cargo run --bin uix-demo              # GUI（11 页 80+ 组件）
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

默认 Vulkan → 失败 typed 自动回退：D3D11、OpenGL ES、Software(GDI)。失败终态 `TerminalFailure`，无静默换 API、无 busy retry。空闲真休眠，遮挡可停帧。

## 测试

```bash
cargo test --all-targets                                              # 全量
cargo test --features test-harness                                    # 无窗 TestApp + 选择器 + 语义快照
cargo test --features agent-control --test agent_gui_windows -- --ignored  # 真窗验收（Windows）
```
