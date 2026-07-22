# UIX

Rust **跨平台 App 框架**。入口：`use uix::prelude::*;`。

声明式 UI + 原生绘制 + 零闲置。全栈指 UI + 应用壳 + 本地 Settings，**不含**网络 / 同步。应用 API 为 Rust 门面，无独立 UI DSL。

> **当前**：Windows P6 功能基线已闭合，`0.0.1` 范围内 110 项已完成工作树定向实现与行为验证，但尚未绑定 clean 冻结候选、候选全量门禁和逐项真窗 / 等价证据，因此版本仍未发布；Linux / macOS 源码在树，移动端未做。→ [`产品`](docs/产品.md) · [`使用`](docs/使用.md) · [`架构`](docs/架构.md) · [`0.0.1 计划`](docs/进度.md#001-首发计划) · [`变更记录`](CHANGELOG.md)

## 授权与交付

Copyright © 2026 `yangyanhui`. UIX 是**闭源、仅限获授权内部使用**的软件，不向 crates.io 发布，也不对外提供源码或二进制制品；完整条款见 [`LICENSE`](LICENSE)。捆绑的 Lucide 字体保留其 ISC / Feather MIT 第三方声明，且不改变 UIX 的专有性质，详见 [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md)。`0.0.1` 的冻结载体是 Windows 11 24H2 x64 内部 ZIP，包含 release Demo、内部 `.crate`、UIX 许可、第三方声明与校验清单；候选打包入口为 [`scripts/build_internal_release.ps1`](scripts/build_internal_release.ps1)，默认拒绝脏工作树。

## 快速开始

```bash
cargo run --bin uix-demo              # GUI（12 页、92 项组件 QA 库存）
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
| draw | `uix::draw::*` | 统一 wgpu GPU backend、Software、颜色、字体 |
| ui | `uix::ui::*` | 组件、布局、主题、动画 |
| app | `uix::app::*` | App、Window、CLI、多窗、Agent Bridge |
| data | `uix::data::*` | SettingsService（opt-in KV） |

## 图形

默认 `Auto`（Windows 优先探测 Vulkan）；失败按 typed 原因在同 API 内有界恢复，耗尽后回 Software(GDI)，仅 `Auto` 可跨 GPU API probe。失败终态 `TerminalFailure`，无静默换 API、无 busy retry。空闲真休眠；Windows P6 遮挡基线已闭合。
