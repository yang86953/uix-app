# UIX

UIX 是一个用 Rust 编写的跨平台原生桌面 UI 框架，用于在 Windows 和 Linux 上提供一致的组件模型、渲染管线、输入事件与应用生命周期。

版本 `0.1.0`，处于积极开发阶段。项目按六大功能域组织：`core` / `native` / `draw` / `ui` / `app` / `data`。平台差异只封装在 `native` 域，上层业务代码不写平台分支。

## 快速入口

| 你想做什么 | 先看 |
|------------|------|
| 运行 demo、了解公共 API | 本文档 |
| 理解系统设计 | [`docs/Main.md`](docs/Main.md) |
| 查询设计决策与废止关系 | [`docs/decisions.md`](docs/decisions.md) |
| 写代码前确认规则与源码边界 | [`AGENTS.md`](AGENTS.md) |

推荐应用侧入口：

```rust
use uix::prelude::*;
```

## 核心特性

- **组件化 Widget 体系**：组件封装数据、行为与生命周期，通过 trait 对外暴露能力。
- **声明式 UI**：提供 `define_widget!`、`tree!` 宏，以及 `ui::view` 简化 API（`column`、`row`、`button`、`dynamic_label` 等）。
- **响应式状态**：`State<T>` 可驱动自动失效；`Computed` / `Effect` 用于派生状态与帧末副作用。
- **布局系统**：支持 Flex、Grid、间距、对齐、伸缩与嵌套布局。
- **增量渲染**：DirtyRects、PresentDamage、像素滚动与局部重绘，避免无意义 present。
- **主题与组件库**：Ant Design 5 风格设计令牌，内置常用 Widget。
- **平台抽象**：窗口、事件循环、呈现器、日志、文件、通知与配置持久化通过 trait 抽象。

## 平台支持

| 平台 | 状态 | 呈现路径 |
|------|------|--------------|
| Windows | 支持 | CPU GDI DIB，GPU 能力按实现可选 |
| Linux | 支持 | Wayland SHM，可选 EGL/GLES |
| macOS | 未支持 | 暂无 backend |

跨平台约束见 [`AGENTS.md`](AGENTS.md)：平台条件编译只允许出现在 `src/native/backends/` 与 `src/native/factory.rs`。

## 快速开始

前置要求：Rust 工具链；Linux 运行 GUI demo 需要可用 Wayland 会话。

```bash
# 完整 GUI 演示
cargo run --bin uix-demo

# 简化 View API 演示
cargo run --bin uix-demo -- --simple

# CLI 演示
cargo run --bin uix-demo -- --cli
```

调试日志：

```bash
RUST_LOG=debug cargo run --bin uix-demo
```

```powershell
$env:RUST_LOG = "debug"; cargo run --bin uix-demo
```

常用验证入口：

```bash
cargo test
cargo test --features test-harness
```

如果工作树或依赖文件被改动，请先以 `cargo metadata`、`Cargo.toml` 和 `git status` 的结果为准。

## 设计权威

项目以文档为设计权威；源码若不一致，应按文档重构，不保留兼容分支。

| 文档 | 说明 |
|------|------|
| [`docs/Main.md`](docs/Main.md) | 系统边界、设计正文、阅读顺序 |
| [`docs/decisions.md`](docs/decisions.md) | 已定稿的取舍、废止关系和追加编号 |
| [`AGENTS.md`](AGENTS.md) | 协作规则、功能域和跨平台硬约束 |

## 基础用法

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

更底层的能力可以按功能域精确导入：

```rust
use uix::app::App;
use uix::core::{Error, Point};
use uix::draw::Color;
use uix::ui::{define_widget, WidgetComponent, WidgetNode};
```

## 模块结构

```text
uix-app/
├── Cargo.toml              # uix crate + uix-demo binary
├── src/
│   ├── core/               # 错误、几何、日志、诊断
│   ├── native/             # 平台能力（traits / backends / services）
│   ├── draw/               # 绘制引擎、光栅化、字体、图片、合成
│   ├── ui/                 # 组件、布局、主题、View DSL
│   ├── app/                # 应用生命周期、主循环、窗口、CLI、DI
│   ├── data/               # 配置持久化
│   ├── lib.rs              # crate 根
│   └── prelude.rs          # 统一对外入口
├── demo/                   # uix-demo 演示
├── docs/                   # 系统设计文档与决策台账
├── assets/                 # 字体、图片等资源
├── AGENTS.md               # 维护规则与架构硬约束
└── README.md               # 本文档
```

依赖方向：

```text
core ← native ← draw ← ui ← app
  ↑      ↑               ↑
  └──────┴─── data ──────┘
```

| 功能域 | 导入路径 | 典型用途 |
|--------|----------|----------|
| 统一入口 | `uix::prelude::*` | 应用开发 |
| 基础设施 | `uix::core::*` | 错误、几何、日志 |
| 平台能力 | `uix::native::*` | 窗口、事件、文件 |
| 绘制 | `uix::draw::*` | 引擎、颜色、字体 |
| 界面 | `uix::ui::*` | 组件、布局、主题 |
| 应用 | `uix::app::*` | App、Window、CLI |
| 数据 | `uix::data::*` | SettingsService |

## 开发约束摘要

- 上层只依赖 `native::traits`，不直接使用 `native::backends::*`。
- `#[cfg(windows/unix)]` 只允许出现在 `src/native/backends/` 与 `src/native/factory.rs`。
- 组件通过 trait 暴露能力，不依赖其他组件内部细节。
- 新设计先补 `docs/decisions.md` 与对应 `docs/systems/*.md`；若涉及源码边界或硬约束，同步 `AGENTS.md`。

完整规则见 [`AGENTS.md`](AGENTS.md)。
