# UIX 演示程序

```text
demo/src/
├── main.rs          # 薄入口：gui | cli
├── gui/             # 多页 GUI 壳层：分组侧边栏、Timer、动画 time
├── cli/             # core/platform/graphics/ui/app/data CLI
├── demos/           # 12 个内容页
│   ├── home.rs      # 首页：入门 + 快捷导航
│   ├── runtime.rs   # 应用能力（State/Timer/Theme/多窗口/View DSL）
│   ├── general.rs   # 通用 widgets
│   ├── layout.rs    # 布局 / 容器
│   ├── nav.rs       # 导航 + Navigation/NavGroup
│   ├── input.rs     # 输入控件
```

## 运行模式

| 模式 | 命令 | 说明 |
|------|------|------|
| **GUI（默认）** | `cargo run --bin uix-demo` | 12 页多页应用：80+ Widget + App / Provider 能力全景 |
| **GUI + 系统主题** | `--bin uix-demo -- --follow-system-theme` | 显式 opt-in 系统主题初始值与 live 变化 |
| **GUI + 图形恢复验收** | `--features test-harness --bin uix-demo -- --graphics-recovery-acceptance` | 下一次真实绘制帧 typed 失败，恢复后继续交互 |
| **GUI + 逐组件视觉验收** | `--features "test-harness agent-control" --bin uix-demo -- --component-qa` | 隔离展示当前公开组件库存及其适用状态，供真实窗口自动取证 |
| **GUI + Agent Bridge** | `--features agent-control --bin uix-demo -- --agent-control` | 显式启用本机 `uix.agent.v1` 端点 |
| **CLI** | `--cli` | `core` / `platform` / `graphics` / `ui` / `app` / `data` 无 GUI API |
