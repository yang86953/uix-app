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
│       ├── main.uix    # uix-lang 界面：12 个页面组件、全部已注册标签、私有状态与事件
│       ├── multi_window.rs # Rust 多窗口组合边界：跨窗主题联动验收
│       ├── graphics_recovery.rs  # test-harness 图形恢复验收组合模块
│       └── graphics_recovery.uix # 独立图形恢复声明页与稳定自动化标识
├── badge-visual/       # Badge 零/单子装饰器真窗验收
├── selectable-list-visual/ # SelectableList 稳定 id 受控选择真窗验收
├── collapse-visual/    # Collapse 稳定 key 与手风琴真窗验收
├── popconfirm-visual/  # Popconfirm 单 trigger 真窗验收
├── upload-visual/      # Upload 受控文件队列真窗验收
├── backdrop-visual/    # 浮层背景模糊真窗验收
├── rich-text-visual/   # RichText 主题分隔线真窗验收
├── rich-text-image-visual/ # RichText 内联图片真窗验收
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
| **uix-lang 图形恢复验收** | `cargo run --release --manifest-path demo/Cargo.toml --features test-harness --bin uix-lang-demo -- --test-graphics-recovery` | 独立声明页、设备/表面故障注入与恢复后交互 |
| **Badge 真窗验收** | `cargo run --release --manifest-path demo/Cargo.toml --bin badge-visual` | 零子兼容、Icon / Avatar / Button 单子装饰与动态 count / dot |
| **SelectableList 真窗验收** | `cargo run --release --manifest-path demo/Cargo.toml --bin selectable-list-visual` | 空选中、稳定 id 写回、指针与键盘活动高亮 |
| **Collapse 真窗验收** | `cargo run --release --manifest-path demo/Cargo.toml --bin collapse-visual` | 稳定 key、多开/手风琴写回与键盘切换 |
| **Popconfirm 真窗验收** | `cargo run --release --manifest-path demo/Cargo.toml --bin popconfirm-visual` | 单 trigger 悬停/点击与关闭路径 |
| **Upload 真窗验收** | `cargo run --release --manifest-path demo/Cargo.toml --bin upload-visual` | 受控文件队列、扩展名/数量/大小门禁 |
| **Backdrop 真窗验收** | `cargo run --release --manifest-path demo/Cargo.toml --bin backdrop-visual` | 浮层背景模糊策略与快照重建 |
| **RichText 真窗验收** | `cargo run --release --manifest-path demo/Cargo.toml --bin rich-text-visual` | Markdown 主题分隔线解析、显式段混排与选择边界 |
| **RichText 内联图片验收** | `cargo run --release --manifest-path demo/Cargo.toml --bin rich-text-image-visual` | 图片段生命周期、宽度/高度/圆角/fallback 与快照事实 |
| **CLI** | `cargo run --manifest-path demo/Cargo.toml --bin uix-cli-demo` | `core` / `draw` / `ui` / `app` / `data` 无 GUI API |

各项目共享 `demo/target/` 构建目录；根目录 `cargo` 命令不包含本工作区，需要显式
`--manifest-path demo/Cargo.toml`（或进入 `demo/` 目录执行）。

uix-lang 只负责在编译期把 `.uix` 文档转换为 Rust：`uix!` 生成 `ViewNode`，
`uix_app!` 消费 `<App>` 根并返回尚未运行的 `App` builder。`uix-lang-demo/src/main.rs`
继续作为 Rust 组合根持有窗口配置、启动钩子与最终 `.run()`；`<App>` 标签已实现，不再
是目标设计。
