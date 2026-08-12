# UIX 演示工作区

`demo/` 是独立的多项目目录（Cargo workspace），按运行模式与界面描述方式拆分为三个项目：

```text
demo/
├── Cargo.toml          # 工作区清单（members: gui-demo, uix-lang-demo, cli-demo）
├── gui-demo/           # GUI 多页演示
│   ├── Cargo.toml      # package: uix-gui-demo；bin: uix-demo
│   └── src/
│       ├── main.rs     # 薄入口：GUI 启动参数与测试入口解析
│       ├── common/     # 页面构建器、展示与自定义 widget 共享辅助
│       ├── demos/      # 12 个内容页 + 组件真窗测试支撑
│       └── gui/        # 多页 GUI 壳层：分组侧边栏、Timer、动画 time
├── uix-lang-demo/      # uix-lang GUI 演示
│   ├── Cargo.toml      # package/bin: uix-lang-demo
│   ├── README.md       # 运行方式与 Rust/uix-lang 边界
│   └── src/
│       ├── main.rs     # Rust 应用组合根与窗口生命周期
│       └── main.uix    # uix-lang 界面、私有状态与事件
└── cli-demo/           # CLI 功能域演示
    ├── Cargo.toml      # package/bin: uix-cli-demo
    └── src/
        ├── main.rs     # 薄入口
        └── cli/        # core/draw/ui/app/data 功能域演示
```

## 运行与测试入口

| 模式 | 命令 | 说明 |
|------|------|------|
| **GUI（默认）** | `cargo run --manifest-path demo/Cargo.toml --bin uix-demo` | 12 页多页应用：80+ Widget + App / Provider 能力全景 |
| **uix-lang GUI** | `cargo run --release --manifest-path demo/Cargo.toml --bin uix-lang-demo` | `.uix` 文件入口、组件私有状态与点击事件 |
| **GUI + 系统主题** | `... --bin uix-demo -- --follow-system-theme` | 显式 opt-in 系统主题初始值与 live 变化 |
| **图形恢复测试支撑** | `... --features test-harness --bin uix-demo -- --test-graphics-recovery` | 供自动测试注入下一次真实绘制帧 typed 失败并检查恢复后交互 |
| **组件真窗测试支撑** | `... --features "test-harness agent-control" --bin uix-demo -- --test-components` | 供自动测试隔离展示当前公开组件及适用状态 |
| **GUI + Agent Bridge** | `... --features agent-control --bin uix-demo -- --agent-control` | 显式启用本机 `uix.agent.v1` 端点 |
| **CLI** | `cargo run --manifest-path demo/Cargo.toml --bin uix-cli-demo` | `core` / `draw` / `ui` / `app` / `data` 无 GUI API |

三个项目共享 `demo/target/` 构建目录；根目录 `cargo` 命令不包含本工作区，需要显式
`--manifest-path demo/Cargo.toml`（或进入 `demo/` 目录执行）。

uix-lang 只负责在编译期生成 `ViewNode`。`uix-lang-demo/src/main.rs` 继续作为 Rust
组合根持有 `App`、窗口配置与运行生命周期；当前没有把目标设计中的 `<App>` 标签当作
已实现入口。
