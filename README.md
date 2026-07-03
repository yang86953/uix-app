# UIX

UIX 是一个用 Rust 编写的原生桌面 UI 框架，目标是在 Windows 和 Linux 上提供一致的组件模型、渲染管线和应用生命周期。

当前项目处于 `0.1.0` 开发阶段，核心方向是组件化架构、响应式状态、增量渲染和跨平台抽象。平台差异只封装在 `uix-platform` 内部，上层业务与 UI 代码不直接依赖具体 OS 实现。

## 核心特性

- **组件化 Widget 体系**：每个组件封装数据、行为和生命周期，通过 trait 接口组合，而不是继承层级。
- **声明式 UI 构建**：提供 `ui!`、`define_widget!`、`tree!` 宏，以及面向应用层的 `view` 简化 API。
- **响应式状态**：`State<T>`、`Computed<T>`、`Effect` 自动追踪依赖并触发局部更新。
- **布局系统**：支持 Flexbox、Grid、间距、对齐、伸缩和嵌套布局。
- **增量渲染**：基于 DirtyRects、像素滚动和 FrameGraph pass 裁剪，只重绘变化区域。
- **主题与组件库**：内置 Ant Design 5 风格设计令牌和 60+ 常用 Widget。
- **平台抽象**：统一封装窗口、事件循环、呈现器、日志、文件、通知和设置服务。

## 快速开始

前置要求：安装 Rust 工具链，并在支持的平台上运行。Linux 需要可用的 Wayland 会话。

```bash
# 完整 GUI 演示
cargo run --bin uix-demo

# 简化 API 演示
cargo run --bin uix-demo -- --simple

# CLI 演示
cargo run --bin uix-demo -- --cli
```

设置日志级别：

```bash
RUST_LOG=debug cargo run --bin uix-demo
```

PowerShell：

```powershell
$env:RUST_LOG = "debug"; cargo run --bin uix-demo
```

## 基础用法

应用层可以优先使用 `uix::ui::view` 简化 API：

```rust
use uix::ui::view::*;
use uix::ui::{App, State};

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

更底层的 Widget 能力可以通过 `uix::ui::*`、`define_widget!` 和 `tree!` 直接使用。

## Workspace 结构

```text
uix-app/
├── Cargo.toml              # workspace 根与聚合 crate: uix
├── src/                    # 根 crate 重导出层
├── platform/               # uix-platform: OS 抽象、事件、呈现、日志、服务
├── graphics/               # uix-graphics: 2D 渲染、FrameGraph、字体、空间坐标
├── ui/                     # uix-ui: Widget、布局、状态、主题、动画、组件库
│   └── macros/             # uix-macros: ui!/define_widget!/tree!
├── app/                    # uix-app: 应用入口、窗口生命周期、CLI、DI
├── demo/                   # uix-demo: GUI、CLI、简化 API 演示
├── ARCHITECTURE.md         # 架构设计与模块详解
└── AGENTS.md               # 项目编码规范与设计约束
```

依赖方向保持单向：

```text
uix -> app -> ui -> graphics -> platform
       └──> ui -> graphics -> platform
```

`uix-macros` 是编译期宏 crate，不参与运行时依赖链。

## 平台支持

| 平台 | 后端 | 状态 |
|------|------|------|
| Windows | Win32 API + GDI DIB 呈现 | 支持 |
| Linux | Wayland SHM buffer，可选 EGL/GLES | 支持 |
| macOS | 暂无 | 不支持 |

## 开发校验

```bash
cargo fmt --all
cargo clippy --workspace --all-targets
cargo test --workspace
```

workspace lint 已禁止 `unwrap()` 和 `expect()` 进入生产代码。更多架构约束和编码规则见 [`AGENTS.md`](AGENTS.md)。

## 文档

- [`ARCHITECTURE.md`](ARCHITECTURE.md)：实时架构地图、分层关系、数据流和关键设计决策。
- [`AGENTS.md`](AGENTS.md)：组件化设计原则、Fail Fast、可测性和编码硬约束。

## License

MIT
