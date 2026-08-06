# UIX 演示工作区

`demo/` 是独立的多项目目录（Cargo workspace），按运行模式拆分为两个项目：

```text
demo/
├── Cargo.toml          # 工作区清单（members: gui-demo, cli-demo）
├── gui-demo/           # GUI 多页演示
│   ├── Cargo.toml      # package: uix-gui-demo；bin: uix-demo
│   └── src/
│       ├── main.rs     # 薄入口：GUI 启动参数解析与验收模式分发
│       ├── common/     # 页面构建器、展示与自定义 widget 共享辅助
│       ├── demos/      # 12 个内容页 + 逐组件视觉验收库存
│       └── gui/        # 多页 GUI 壳层：分组侧边栏、Timer、动画 time
└── cli-demo/           # CLI 功能域演示
    ├── Cargo.toml      # package/bin: uix-cli-demo
    └── src/
        ├── main.rs     # 薄入口
        └── cli/        # core/draw/ui/app/data 功能域演示
```

## 运行模式

| 模式 | 命令 | 说明 |
|------|------|------|
| **GUI（默认）** | `cargo run --manifest-path demo/Cargo.toml --bin uix-demo` | 12 页多页应用：80+ Widget + App / Provider 能力全景 |
| **GUI + 系统主题** | `... --bin uix-demo -- --follow-system-theme` | 显式 opt-in 系统主题初始值与 live 变化 |
| **GUI + 图形恢复验收** | `... --features test-harness --bin uix-demo -- --graphics-recovery-acceptance` | 下一次真实绘制帧 typed 失败，恢复后继续交互 |
| **GUI + 逐组件视觉验收** | `... --features "test-harness agent-control" --bin uix-demo -- --component-qa` | 隔离展示当前公开组件库存及其适用状态，供真实窗口自动取证 |
| **GUI + Agent Bridge** | `... --features agent-control --bin uix-demo -- --agent-control` | 显式启用本机 `uix.agent.v1` 端点 |
| **CLI** | `cargo run --manifest-path demo/Cargo.toml --bin uix-cli-demo` | `core` / `draw` / `ui` / `app` / `data` 无 GUI API |

两个项目共享 `demo/target/` 构建目录；根目录 `cargo` 命令不包含本工作区，需要显式
`--manifest-path demo/Cargo.toml`（或进入 `demo/` 目录执行）。
