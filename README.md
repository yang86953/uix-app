# UIX

UIX 是一个用 Rust 编写的跨平台原生桌面 UI 框架，在 Windows 和 Linux 上提供一致的组件模型、渲染管线和应用生命周期。

当前版本 `0.1.0`，处于积极开发阶段。框架按**六大功能域**组织（`core` / `native` / `draw` / `ui` / `app` / `data`），平台差异只封装在 `native` 域，上层代码各平台完全一致。

## 核心特性

- **组件化 Widget 体系**：每个组件封装数据、行为和生命周期，通过 trait 接口组合
- **声明式 UI**：`define_widget!`、`tree!` 宏，以及 `ui::view` 简化 API（`column`、`button`、`dynamic_label` 等）
- **响应式状态**：`State<T>`、`Computed<T>`、`Effect` 自动追踪依赖并触发局部更新
- **布局系统**：Flexbox、Grid、间距、对齐、伸缩与嵌套布局
- **增量渲染**：DirtyRects、多矩形 PresentDamage 与像素滚动，只重绘变化区域
- **主题与组件库**：Ant Design 5 风格设计令牌，60+ 内置 Widget
- **平台抽象**：窗口、事件循环、呈现器、日志、文件、通知与配置持久化

## 快速开始

**前置要求**：Rust 工具链；Linux 需可用 Wayland 会话。

```bash
# 完整 GUI 演示
cargo run --bin uix-demo

# 简化 API 演示
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

## 基础用法

推荐通过 `prelude` 统一导入：

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

更底层的能力可通过各功能域精确导入：

```rust
use uix::ui::{define_widget, WidgetComponent, WidgetNode};
use uix::draw::Color;
use uix::core::Error;
use uix::app::App;
```

## 模块结构

```text
uix-app/
├── Cargo.toml              # workspace 根 + uix crate
├── src/
│   ├── core/               # 错误、几何、日志、诊断
│   ├── native/             # 平台能力（traits / backends / services）
│   ├── draw/               # 绘制引擎、光栅化、字体、合成
│   ├── ui/                 # 组件、布局、主题、View DSL
│   ├── app/                # 应用生命周期、主循环、窗口、CLI、DI
│   ├── data/               # 配置持久化
│   ├── lib.rs              # crate 根
│   └── prelude.rs          # 统一对外入口
├── demo/                   # uix-demo 演示
├── tests/                  # 集成测试
├── ARCHITECTURE.md         # 架构地图
├── AGENTS.md               # 编码规范
└── README.md               # 本文档
```

依赖方向：

```text
core ← native ← draw ← ui ← app
  ↑      ↑               ↑
  └──────┴─── data ──────┘
```

| 功能域 | 导入路径 | 典型用途 |
|--------|---------|---------|
| 统一入口 | `uix::prelude::*` | 应用开发 |
| 基础设施 | `uix::core::*` | 错误、几何、日志 |
| 平台能力 | `uix::native::*` | 窗口、事件、文件 |
| 绘制 | `uix::draw::*` | 引擎、颜色、字体 |
| 界面 | `uix::ui::*` | 组件、布局、主题 |
| 应用 | `uix::app::*` | App、Window、CLI |
| 数据 | `uix::data::*` | SettingsService |

## 平台支持

| 平台 | 后端 | 状态 |
|------|------|------|
| Windows | Win32 API + GDI DIB 呈现 | ✅ 支持 |
| Linux | Wayland SHM buffer，可选 EGL/GLES | ✅ 支持 |
| macOS | — | ❌ 暂不支持 |

跨平台规则：`#[cfg(windows/unix)]` 仅允许出现在 `native/backends/` 与 `native/factory.rs`，详见 [`ARCHITECTURE.md`](ARCHITECTURE.md)。

## 开发校验

```bash
cargo fmt --all
cargo clippy --workspace --all-targets
cargo test --workspace
cargo test --features test-harness -p uix   # 含 FakePlatform 的平台集成测试
```

workspace lint 禁止 `unwrap()` / `expect()` 进入生产代码。

## 文档

| 文档 | 内容 |
|------|------|
| [`ARCHITECTURE.md`](ARCHITECTURE.md) | 功能域详解、数据流、跨平台规则、设计决策 |
| [`AGENTS.md`](AGENTS.md) | 组件化原则、Fail Fast、可测性、编码硬约束 |

## License

MIT
