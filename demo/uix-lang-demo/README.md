# UIX Lang Demo

这个项目是仓库主演示：`src/main.uix` 声明 1200×800
应用外壳、12 个页面组件、组件私有状态与全部已注册内置标签；`src/main.rs` 在编译期
调用公开 `uix_app!` 宏生成现有 `App` builder，并继续由 Rust 持有应用与窗口生命周期。

## 运行

从仓库根目录执行：

```powershell
cargo run --release --manifest-path demo/Cargo.toml --bin uix-lang-demo
```

需要执行本机 Agent 语义验收时，必须同时打开 Cargo feature 与运行时参数：

```powershell
cargo run --release --manifest-path demo/Cargo.toml --features agent-control --bin uix-lang-demo -- --agent-control
```

普通启动不会发布 Agent 端点；只传 `--agent-control` 但未启用 feature 会在创建窗口前
以退出码 2 返回定向错误。

需要验证设备丢失、表面丢失与恢复后交互时，必须同时打开测试 feature 与运行时参数：

```powershell
cargo run --release --manifest-path demo/Cargo.toml --features test-harness --bin uix-lang-demo -- --test-graphics-recovery
```

需要通过 Agent 语义树自动执行这组操作时，同时启用两项 feature 和两项参数：

```powershell
cargo run --release --manifest-path demo/Cargo.toml --features "agent-control,test-harness" --bin uix-lang-demo -- --agent-control --test-graphics-recovery
```

`src/graphics_recovery.uix` 是独立验收页面；`src/graphics_recovery.rs` 只持有
`on_start` 交付的逐窗 `AppHandle` 并调用公开 test-harness 故障入口。普通主演示不编译该
模块，也不会展示或开放故障按钮。

仓库测试已精简为公开 API 契约测试（`tests/*_public_api.rs`）；设备丢失、表面丢失与
恢复后交互的真窗注入验收由本 demo 的 `--test-graphics-recovery` 页面承担，不再作为
独立 cargo test target 运行。

十二页与已登记组件的浅色 / 深色真实窗口矩阵、多窗口与跨窗主题、系统主题跟随、
派生语义树与前景焦点像素门禁等 Windows 真窗验收同步精简，由公开 API 契约测试与
本 demo 的 `--agent-control` 协议验证共同覆盖。

Windows 使用 D3D11；Linux/Wayland 构建使用 EGL OpenGL ES。入口统一经
`uix::platform::run_on_ui_thread` 运行：Windows 在 8MB 大栈 UI 线程上执行，
承载声明文件展开出的深层 ViewNode 树；其他平台直接在进程主线程执行。

## 对齐范围

- 对齐自定义标题栏、分组侧边栏（入门 / 组件 / 参考）、页头、滚动内容区、状态栏与
  12 个公开页面入口；页头显示 `run_interval` 驱动的 live tick。
- 已注册标签全部在 `.uix` 中展示：通用、布局、输入、数据展示、反馈、导航与框架
  六类内置组件（见覆盖清单页）。
- 声明式状态：12 个页面各自声明为 `<Component>`；双向绑定（输入、勾选、选择、日期、
  分页、步骤、受控 Modal / Drawer）直接使用组件私有 state 与 `setState`，类型化
  state 注解覆盖基础类型（`u32` / `usize` / `f32` / `i32`）、语义类型（Date / Time /
  Color / Point / CascaderValue / HashSet&lt;String&gt; / Option&lt;String&gt; / Vec&lt;String&gt; /
  Vec&lt;UploadFile&gt;）与 record 模型。
- 类型化数据（SelectableItem、CollapsePanel、SelectOption、CascaderOption、TreeNode、
  TimelineItem、DescriptionsItem、BreadcrumbItem、AnchorItem、Step）由语言面数据构造链直接声明：`SelectOption('中国', 'cn')`、
  `CascaderOption('浙江', 'zj').children([...])`、`Step('注册').status('finish')`；数组字面量
  `['female', 'male']` 编译为 vec，同一数据绑定可被多个标签按值 clone 复用。
- 语言能力：props（String / number / bool / State<T> / 回调签名）、样式类继承与内联
  style、If / For 控制流、setTheme 框架内置操作（主题请求通道，无调用方桥接）、
  不可变数组操作与受限闭包、按顺序求值的 computed 派生值、`<Record>` 类型化业务模型
  （`uix_items!` 生成模块级结构体供 Rust 侧回调引用）。数据展示页以 `filter` 派生有效
  展开项，再由后续 computed 展示剩余数量；同页通过默认与具名 `Slot` 复用统一面板容器。
- Rust 图表文档列出的十二类图表标签已全部登记类型化静态映射；
  Transfer、气泡、多系列、高级图表配置及其余图表标签保留 Rust API 边界。SelectableList、Collapse、Badge、Upload、Message、
  Notification 与 Popconfirm 已在主演示中提供声明样例。
- 真窗验收控制面是测试侧能力，不复制到声明式产品界面；主演示通过
  `agent-control` feature 与 `--agent-control` 参数双门禁复用现有 App System 控制面。
- 图形恢复页面通过 `test-harness` feature 与 `--test-graphics-recovery` 参数双门禁独立
  组装；Application System 继续唯一拥有逐窗 `AppHandle`、图形会话与故障注入入口。

## 当前边界

`uix!` 继续只生成 `ViewNode`；本项目使用 `uix_app!` 消费 `<App>` 根并返回尚未运行的现有
`App` builder。窗口标题、尺寸和初始主题来自语言面，`custom_title_bar`、`on_start`、可选
`enable_agent_control` 与最终 `run()` 仍由 Rust 薄入口链式配置，没有引入运行时解释器或
第二套窗口所有者。

Rust 侧只保留三类内容：窗口生命周期状态（页面、计数与秒级 tick）与 App 组合根，以及
由 Rust 构造的私有类型演示数据（VirtualScroll 的 DemoRow 行）。类型化状态（集合、级联
路径、日期、时间、颜色、滚动位置与表单模型）全部由语言面组件私有 state 声明；表单
业务模型由 `<Record>` 声明并经 `uix_items!` 生成模块级结构体，Rust 侧 `submit_profile`
直接引用该类型。

根视图背景：`uix!` 生成的普通 `ViewNode` 仍保持透明语义；当它作为窗口根 View 且没有显式
背景时，`App` 组合根会应用当前主题的 `BgLayout` 默认值。调用方显式设置的背景（包括透明色）
优先，不会被应用默认值覆盖。
