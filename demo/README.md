# UIX 演示工作区

`demo/` 是独立的多项目目录（Cargo workspace），以 uix-lang 声明式演示为主：

```text
demo/
├── Cargo.toml          # 工作区清单（主演示、CLI 与独立真窗验收程序）
├── uix-lang-demo/      # uix-lang GUI 演示（仓库主演示）
│   ├── Cargo.toml      # package/bin: uix-lang-demo
│   ├── README.md       # 运行方式与 Rust/uix-lang 边界
│   └── src/
│       ├── main.rs     # Rust 应用组合根：窗口生命周期、类型化数据与语言面之外的状态
│       └── main.uix    # uix-lang 界面：12 个页面组件、全部已注册标签、私有状态与事件
├── badge-visual/       # Badge 零/单子装饰器真窗验收
├── selectable-list-visual/ # SelectableList 稳定 id 受控选择真窗验收
├── popconfirm-visual/  # Popconfirm 单 trigger 真窗验收
└── cli-demo/           # CLI 功能域演示
    ├── Cargo.toml      # package/bin: uix-cli-demo
    └── src/
        ├── main.rs     # 薄入口
        └── cli/        # core/draw/ui/app/data 功能域演示
```

## 运行与测试入口

| 模式 | 命令 | 说明 |
|------|------|------|
| **uix-lang GUI（主演示）** | `cargo run --release --manifest-path demo/Cargo.toml --bin uix-lang-demo` | `.uix` 文件入口、12 个页面组件、私有/类型化状态与全部已注册标签 |
| **Badge 真窗验收** | `cargo run --release --manifest-path demo/Cargo.toml --bin badge-visual` | 零子兼容、Icon / Avatar / Button 单子装饰与动态 count / dot |
| **SelectableList 真窗验收** | `cargo run --release --manifest-path demo/Cargo.toml --bin selectable-list-visual` | 空选中、稳定 id 写回、指针与键盘活动高亮 |
| **CLI** | `cargo run --manifest-path demo/Cargo.toml --bin uix-cli-demo` | `core` / `draw` / `ui` / `app` / `data` 无 GUI API |

各项目共享 `demo/target/` 构建目录；根目录 `cargo` 命令不包含本工作区，需要显式
`--manifest-path demo/Cargo.toml`（或进入 `demo/` 目录执行）。

uix-lang 只负责在编译期生成 `ViewNode`。`uix-lang-demo/src/main.rs` 继续作为 Rust
组合根持有 `App`、窗口配置与运行生命周期；当前没有把目标设计中的 `<App>` 标签当作
已实现入口。
