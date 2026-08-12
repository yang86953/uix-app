# UIX Lang Demo

这个项目展示当前可运行的 uix-lang 文件入口：`src/main.uix` 声明界面、组件私有状态与
点击事件，`src/main.rs` 在编译期调用公开 `uix!` 宏，并继续持有应用与窗口生命周期。

## 运行

从仓库根目录执行：

```powershell
cargo run --release --manifest-path demo/Cargo.toml --bin uix-lang-demo
```

Windows 使用 D3D11；Linux/Wayland 构建使用 EGL OpenGL ES。

## 当前边界

`uix!` 当前生成 `ViewNode`，不启动应用。目标设计中的 `<App>` 根标签尚未作为可运行入口，
因此本项目由 Rust 薄入口组装 `App`，没有引入运行时解释器或第二套窗口所有者。
