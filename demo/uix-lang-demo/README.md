# UIX Lang 全组件演示

这是仓库唯一 demo。`src/main.uix` 声明 1200×800 应用外壳并递归导入 12 个页面；全部已
登记组件、样式、展示数据、界面状态和局部交互都使用 uix-lang 语法。`src/main.rs` 不构造
任何演示 View 或展示数据，只保留 UIX Lang 规范明确排除的进程参数、平台 UI 线程、字体、
可选 Agent 控制、Form 业务提交回调与 `App::run()` 生命周期。

## 运行

从仓库根目录执行：

```powershell
cargo run --release --manifest-path demo/Cargo.toml --bin uix-lang-demo
```

需要启用本机 Agent 控制时，同时打开 Cargo feature 与运行参数：

```powershell
cargo run --release --manifest-path demo/Cargo.toml --features agent-control --bin uix-lang-demo -- --agent-control
```

普通启动不会发布 Agent 端点；只传参数但未启用 feature 会在创建窗口前以退出码 2 失败。

Windows 与 Linux 默认首选 Vulkan；Windows 保留 D3D11、Linux/Wayland 保留 EGL OpenGL ES 兼容回退。入口统一经
`uix::platform::run_on_ui_thread` 运行，并在窗口创建前安装仓库内固定的 CJK 字体。

## 声明范围

- `main.uix`：主题、样式类、自定义 Widget、应用壳、标题栏、侧栏和页面切换状态。
- `pages/start.uix`：首页与 uix-lang 状态、主题、action、条件链能力。
- `pages/core.uix`：通用、布局、导航组件和语言面虚拟列表数据。
- `pages/extended.uix`：输入、数据展示、反馈、十二类图表及其他组件。
- `pages/reference.uix`：框架能力和覆盖清单。

正式产物不携带 UIX Lang 解释器；`.uix` 导入闭包在编译期生成现有 Rust `App` 与
`ViewNode`。业务规则、I/O、多窗口编排和应用生命周期仍属于 Rust Application System。
