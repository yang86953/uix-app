# 变更记录

这里记录 UIX 闭源版本中已经发生、会影响使用方、交付物或验证方式的变化。变更条目不表示版本已发布，也不承担任务、负责人、阻塞和实时完成度管理；`0.0.1` 的发布与交付事实见[交付与许可](docs/产品/交付与许可.md)。

## 0.0.1（未发布）

### 首发能力

- Windows 与 Linux 均纳入 `0.0.1` 首发平台范围；两平台统一以 Vulkan GPU-native swapchain 为首选，Windows 保留 D3D11、Linux/Wayland 保留 EGL OpenGL ES 兼容回退。
- 提供 Rust 声明式 View、State / Computed、应用壳、本地 Settings 与桌面组件体系。
- `demo/` 精简为只包含 `uix-lang-demo` 的独立工作区；主演示以 uix-lang 声明 12 个页面、全部已登记组件、展示数据、界面状态与局部交互，Rust 薄入口只保留应用生命周期、平台资源和 Form 业务提交回调。
- 编译产物配置入仓：根包与 demo 工作区显式声明 release/dev/test profile（体积优先 `opt-level="s"`、fat LTO、符号表全剥离、`codegen-units=1`），不再依赖机器级 cargo 配置；release 的过程宏与构建脚本单独提速编译。
- 默认 feature 集同时启用 `vulkan`、`d3d11` 与 `opengles`；自动选择按 registry 优先级使用 Vulkan（100），Windows 依次保留 D3D11（30）与 OpenGL ES（10），Linux 保留 OpenGL ES（10），有界 GPU 恢复耗尽后再整体回退 Software。
- 支持多窗口、IME、主题、定时与动画、浮层、结构化语义快照及显式启用的本机 Agent 开发预览。
- 提供唯一的 `uix-lang-demo` 全组件演示；原 CLI 与 8 个独立真窗 demo 已移除，对应组件统一在主演示中以 uix-lang 声明。
- uix-lang 能力扩展：组件私有 state 支持类型注解（`u32` / `usize` / `f32` / `i32`）；受控组件与双向绑定属性（`open` / `value` / `checked` / `current` 等句柄位）可直接绑定组件私有 state；数据类属性按值 clone 消费，同一绑定可被多个标签复用；`uix-lang-demo` 对齐 API GUI Demo 全部已注册标签，并按页拆分为 12 个声明式页面组件。
- uix-lang 图表声明补齐类型化气泡数据、基础坐标图多系列、共同高级配置、热力图自定义色阶、四类高级坐标图参考线、仪表盘格式化器与自定义混合系列；集合与回调在生成代码中保持精确公开类型并复用既有运行时绘制契约。
- uix-lang Tabs 登记 `tabPosition` 与 `scrollable`，支持四向标签栏和沿主轴溢出滚动；生成层只传递声明配置，面板子树、具体几何与滚动偏移仍由运行时 Tabs 独占。
- uix-lang Steps 登记 `clickable` 与 `dot`，可声明用户切换策略和圆点样式，同时保持调用方 `State<usize>` 为当前步骤的唯一业务状态来源。
- uix-lang Pagination 登记 `showTotal`、`showSizeChanger`、`simple`、`showJumper` 与 `pageSizeOptions`，显式显示策略可覆盖绑定默认，同时保留分页运行时的交互与归一化所有权。
- uix-lang Anchor 登记 `showInk`，可声明活动锚点指示线绘制策略，同时保持运行时选择/位置缓存与应用滚动所有权边界不变。
- uix-lang Breadcrumb 登记 `separator` 与 `maxItems`，可声明路径分隔文本和折叠阈值，同时保持运行时选择、溢出菜单与测量/命中生命周期不变。
- uix-lang Menu 登记 `mode="inline"`；内联模式固定垂直并始终展开完整递归子树，即使同时声明 `collapsible` 也不会改写调用方 `openKeys`。
- 所有产品与 Demo 图标统一走 `Icon` 组件管线；内置紧凑槽位复用 `Icon::paint_in_frame`，不再以 Unicode、ASCII、本地化字符或独立手绘形状充当图标。
- 捆绑字体固定为 `lucide-static` 1.17.0，并随内部包保留 ISC 与 Feather 派生图标 MIT 声明。

### 2026-08-14 功能扩展

- Windows 系统通知接通：完成通知身份（AUMID）与发送契约（`NotificationService`），并提供真发送运行时验收；Linux（notify-send）与 macOS（osascript）经平台命令发送。
- 富文本内联图片完整生命周期：解析边界、图片资源后台解码轮询、绘制与无障碍快照事实，随 RichText 一起交付。
- 浮层背景模糊：Modal / Drawer 等浮层接入背景模糊策略与区域失效、快照重建，含真窗验收。
- Popconfirm 组合触发器：多触发器组合支持（父级测量、键盘释放捕获与回归覆盖）。

### 2026-08-15 功能扩展

- RichText 主题分隔线与内联图片能力统一进入 uix-lang 全组件主演示，不再维护独立真窗 demo。

### 2026-08-15 工程与验证范围

- `cargo test` 只编译运行公开 API 契约测试（`app/core/data/diagnostics/draw/native/ui` 的 `tests/*_public_api.rs`）：移除 38 个非 API 集成测试（`flex_overflow_*`、`uix_lang_*_windows`、`usage_*`、`semantic_actions`、`layout_invariants`、`tessellator_contract` 等）与未被引用的 `tests/unit` 死代码，同步清理 `platform_notification_runtime` 的 `[test]` 表声明；设备丢失/表面丢失真窗注入验收由 demo 的 `--test-graphics-recovery` 页面承担，真窗视觉与通知运行时验收随精简移除。

### 2026-08-17 工程与验证范围

- ui System 的私有组件运行时 Module 从 `component` 更名为 `widget_runtime`，同步迁移内部限定路径与架构文档；公开 `component!` 宏、Component 层类型和 ui 根门面保持不变。
- 盒阴影定义、Rust 样式链与 UIX Lang `boxShadow` 支持可选正负 spread；既有四参数构造器与四段语言语法继续保持零 spread。

### 2026-08-27 工程与验证范围

- 测试面收敛为公开 API 契约：只保留 `tests/*_public_api.rs`（38 个消费者目标），移除非 API 集成测试（分配合同、渲染合同、profile 与回归二进制）、Python 合同脚本与整层 `tests/unit` 单元测试；src 内对应的 `#[path]` 单元挂载全部解除，内联单元测试保留。
- 寄存在 `tests/support` 的特性门控支撑代码回归 crate 语义：`draw` / `native` / `ui` test harness 原样保留供 `test-harness` feature 与公开 API 测试使用；GPU parity harness 从 `tests/unit` 迁移至 `tests/support/native/gpu_parity/` 并保持原挂载语义。
- Cargo.toml 移除已删除目标的 `[[test]]` 声明（4 个 GPU parity 目标与动画分配合同目标）；相关性能决策记录中引用已删套件的命令按登记时点存档处理，不作为现行入口（见性能决策记录索引的使用限制）。
- 项目动态待办停用 Gitea Issue 管理：完成度与任务判定以仓库内文档（docs/）和实际代码为准；既有文字中对 Gitea Issue 的引用按撰写时点存档处理，不作为现行入口。

### 2026-08-27 uix-lang 语言与工具链

- `<Widget>` 成员支持 `@props` / `@state` / `@computed` / `@actions` 块级声明，与字符串属性形式解析等价；同一成员双写两种形式被编译期拒绝。
- 组件专属状态值类型（`CascaderValue`、`Vec<UploadFile>`）从核心白名单迁入 `UiProjectionSchema` 按拥有组件登记并挂接 Cargo capability 门禁；未启用对应 feature 时在生成 Rust 前给出定向诊断（UIX2000 / UIX2001），不再延迟到 rustc 报公开符号缺失。
- `uix scaffold visual <rust文件> <StructName>` 从同模块 Rust 结构体生成可编译的 `<Visual>` 声明骨架：已知标量类型给占位初值，复合类型输出 TODO 提示，消除 `.uix` 与 Rust 双侧手写字段搬运。
- 公开宏文件入口补全消费者证据：`uix!` 递归导入闭包、`uix_app!` 未运行 builder 与 `uix_items!` Visual 静态项由新增的 `tests/uix_lang_public_api.rs` 消费并断言单一静态地址；`uix scaffold visual` 骨架输出增加锁定单测。
- 新增消费者层门禁证据：`tests/uix_lang_gate_public_api.rs` 锁定公开 check 入口的稳定诊断码、阶段、行列与文案快照；工作区新成员 `uix-lang-gate-consumer` 在真实消费者构建中复现双写拒绝与 `CascaderValue` 值类型门禁的 rustc 级定向诊断（输出行携带码、阶段与主 SourceSpan）。

### 2026-08-27 缺陷修复

- 声明式透明度动画在文本内容上不生效：View 统一样式与动画绑定写入的 `style.opacity` 只被各组件在背景/边框绘制路径自行消费，文本等其余内容完全丢失衰减。现把节点声明透明度沿声明树进入运行时节点，并在 scene 节点合成通道与 enter/leave 过渡透明度统一叠乘（LayerTree 已有画布级联语义），组件内部不再重复应用；颜色类关键帧/过渡动画由此在同一 Drawing 语义下跨 Windows 与 Linux 表现一致。

### 修复与平台完善

- OpenGL ES 后端按渲染目标修正纵向行序：`opengles` feature 下 Wayland EGL 界面不再上下颠倒、Windows WGL 回读行序规范化，并完成 Linux EGL GUI 路径首次构建验证。
- Wayland 接通自定义标题栏装饰模式：GUI Demo 跨平台启用自定义标题栏，Wayland 通过 xdg-decoration 在客户端与服务端装饰间切换。
- Wayland 程序化 `set_size` 现在同代更新 logical/drawable surface metrics，并向唯一窗口事件管线
  发布定向 resize；可调整大小的顶层窗口不再只更新 geometry/约束却继续呈现旧客户区 buffer。
- Agent 主窗口关闭会在图形与原生资源清理前发布精确 generation 的 closed 事实；终态 wait 回复
  获得有界传输排空窗口，调用方可在 `close_window` 后可靠取得关闭证明而不被 teardown 抢先断流。
- 应用窗口默认启用文件拖放：Windows 通过 `WM_DROPFILES` 交付本地路径，Wayland 完成 data offer、`text/uri-list` 非阻塞读取、逐窗路由及 checked teardown；稳定缺少能力的平台继续创建无拖放窗口。
- macOS 修复 platform 拆分遗留属性：恢复 `MacosAppEvent` 的 `Debug` / `Clone` 派生，并补充源码契约防止回归。
- Windows D3D12 完成生产装配：显式 `d3d12` feature 现在交付完整 GPU-native thin RHI 并注册为优先级 40 的候选；Vulkan 仍保持最高优先级，真实 Windows 公开运行继续单独验收。
- macOS Metal 完成生产装配：显式 `metal` feature 现在通过 CAMetalLayer 交付 GPU-native thin RHI、12 类固定 pipeline、Retina 物理 drawable、失败帧回滚与 swapchain present，并注册为低于 Vulkan 的优先级 90 候选；真实 macOS 运行继续单独验收。
- `Platform::gpu_adapters` 在 D3D11、D3D12、Vulkan 与 Metal 上返回对应原生 API 的 owned 设备描述；OpenGL ES 因缺少无上下文的可移植适配器枚举协议，继续返回明确的 typed `NotImplemented`。

### 发布状态

- 版权归 `yangyanhui` 所有；版本按专有闭源、仅限内部授权使用的方式交付，不对外分发源码或二进制制品。
- 已定义的 Windows 内部交付载体为 Windows 11 24H2 x64 ZIP：release Demo、内部 `.crate`、UIX 专有许可、Lucide / Feather 第三方声明、说明文件、运行时 Demo 图片与 SHA-256 清单；release 使用 Vulkan 首选、D3D11 兼容回退矩阵，并保留 Software 整体回退。
- 已定义 Linux x64 内部候选包：以确定性 `tar.gz` 交付 Vulkan 首选、Wayland/EGL OpenGL ES 兼容回退的主演示、内部 `.crate`、许可与说明载荷，并由独立校验器冻结条目顺序、epoch 时间、数字所有者、权限、gzip header 与逐项 SHA-256。
- `0.0.1` 正式制品只经私有 Gitea Release 分发，不引入发布签名私钥或自动更新器；升级使用已校验的并行版本目录，失败时回切上一份制品与 Settings 备份。
- 项目动态事实源已迁移到 Gitea；任务、负责人、阻塞、环境矩阵与带时点的验证结果统一由 [0.0.1 产品完成父 Issue #1](http://100.79.245.29:3000/admin/uix-app/issues/1) 及其子 Issue 持有。
- 本版本仍未发布；正式发布状态、平台矩阵与带时点的验证结果以 Gitea 发布任务为准。
