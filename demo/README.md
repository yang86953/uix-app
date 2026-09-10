# UIX 全组件演示

`demo/Cargo.toml` 是独立 Cargo workspace，成员只有 `uix-lang-demo`。
同目录的 [task-list](task-list/README.md) 使用自己的独立 workspace，不在该成员列表中。主演示的应用外壳、12 个页面、
全部已登记组件、样式、展示数据、界面状态与局部交互均由 `.uix` 文件声明；Rust 薄入口只
持有进程参数、平台 UI 线程、字体、可选验收能力、Form 业务提交回调和最终应用生命周期。

```text
demo/
├── Cargo.toml
└── uix-lang-demo/
    ├── Cargo.toml
    ├── README.md
    └── src/
        ├── main.rs
        ├── main.uix
        └── pages/
            ├── start.uix
            ├── core.uix
            ├── extended.uix
            └── reference.uix
```

## 运行

从仓库根目录执行：

```powershell
cargo run --release --manifest-path demo/Cargo.toml --bin uix-lang-demo
```

根目录 `cargo` 命令不包含本工作区，需要显式指定 `demo/Cargo.toml`。

UIX Lang 在编译期把导入闭包转换为 Rust；正式产物不携带解释器。`uix_app!` 返回尚未运行
的 `App` builder，因此应用启动、窗口生命周期与平台资源仍由 Rust Application System 拥有。
