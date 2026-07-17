# UIX 演示程序

> **角色**：能力全景 + 可执行验收场景（多页组件 / Timer / 主题 / 多窗口 / View DSL），**不是**入门 starter。
> 最小应用请复制 [README · 示例](../README.md#示例)；门面速查 → [`使用`](../docs/使用.md)。

## 快速运行

```bash
cargo run --bin uix-demo              # GUI（默认）
cargo run --bin uix-demo -- --follow-system-theme
cargo run --features test-harness --bin uix-demo -- --graphics-recovery-acceptance
cargo run --bin uix-demo -- --cli     # CLI
RUST_LOG=debug cargo run --bin uix-demo
cargo run --features agent-control --bin uix-demo -- --agent-control
```

Linux GUI 需 Wayland。

`--follow-system-theme` 通过公开 `App::follow_system_theme(true)` 让 GUI 初始主题和后续变化由 OS 托管；该模式显示“跟随系统”状态，不再提供会覆盖托管状态的手动主题开关。`--graphics-recovery-acceptance` 仅在 `test-harness` feature 下显示下一帧 typed `DeviceLost` 注入与恢复后交互路径，Demo 仍通过公开 `AppHandle::inject_graphics_device_lost_for_test()` 使用框架。`--agent-control` 仅用于本机开发预览，必须与 `agent-control` feature 同时启用；它会按 [`使用 · Agent Bridge`](../docs/使用指南/Agent%20Bridge.md#agent-bridge-开发预览) 启动当前用户私有端点。这些 GUI 参数都不能与 `--cli` 同时使用。

## 目录结构

```text
demo/src/
├── main.rs          # 薄入口：gui | cli
├── gui/             # 多页 GUI 壳层：分组侧边栏、Timer、动画 time
├── cli/             # core/native/draw/ui/app/data CLI
├── demos/           # 12 个内容页
│   ├── home.rs      # 首页：入门 + 快捷导航
│   ├── runtime.rs   # 应用能力（State/Timer/Theme/多窗口/View DSL）
│   ├── general.rs   # 通用 widgets
│   ├── layout.rs    # 布局 / 容器
│   ├── nav.rs       # 导航 + Navigation/NavGroup
│   ├── input.rs     # 输入控件
│   ├── data.rs      # 数据展示
│   ├── feedback.rs  # 反馈 + Overlay 浮层
│   ├── charts.rs    # 图表
│   ├── other.rs     # Transfer/Upload/DesignTokens
│   ├── framework.rs # LocaleProvider/ConfigProvider/构造覆盖/typed token/统一空态
│   ├── gallery.rs   # 覆盖清单（参考）
│   └── context.rs   # DemoCtx（共享 State）
└── common/
    ├── page.rs      # PageBuilder、PAGE_TITLES、侧边栏分组
    ├── showcase.rs  # labeled_row、info_note 辅助
    └── widgets.rs   # Counter / PulseRing / BounceBall
```

## 运行模式

| 模式 | 命令 | 说明 |
|------|------|------|
| **GUI（默认）** | `cargo run --bin uix-demo` | 12 页多页应用：80+ Widget + App / Provider 能力全景 |
| **GUI + 系统主题** | `--bin uix-demo -- --follow-system-theme` | 显式 opt-in 系统主题初始值与 live 变化 |
| **GUI + 图形恢复验收** | `--features test-harness --bin uix-demo -- --graphics-recovery-acceptance` | 下一次真实绘制帧 typed 失败，恢复后继续交互 |
| **GUI + Agent Bridge** | `--features agent-control --bin uix-demo -- --agent-control` | 显式启用本机 `uix.agent.v1` 端点 |
| **CLI** | `--cli` | `core` / `native` / `draw` / `ui` / `app` / `data` 无 GUI API |

## 侧边栏分组

| 分组 | 页面 |
|------|------|
| **入门** | 首页、应用能力、框架能力 |
| **组件** | 通用、布局、导航、输入、数据展示、反馈、图表、其他 |
| **参考** | 覆盖清单 |

## 页面索引

| # | 页 | 文件 | 覆盖 widget / 特性 |
|---|-----|------|-------------------|
| 0 | 首页 | `home.rs` | 入门 State、Timer tick、快捷导航 |
| 1 | 应用能力 | `runtime.rs` | State、dynamic_label、run_interval、ThemeToggle、系统主题跟随、多窗口主题联动、component!、动画 demo |
| 2 | 通用 | `general.rs` | Button、Icon、Typography、Label、Divider、Space、FloatButton、FloatButtonBackTop、Tag |
| 3 | 布局 | `layout.rs` | Container、Grid、Layout/Header/Sider/Content/Footer、Splitter、ScrollView、Affix、BackTop |
| 4 | 导航 | `nav.rs` | Navigation、NavGroup、Menu、Tabs、Dropdown、Breadcrumb、Anchor、Steps、Pagination |
| 5 | 输入 | `input.rs` | Input、InputNumber、Select、TreeSelect、Cascader、AutoComplete、Mentions、Checkbox、Radio、Switch、Slider、Rate、DatePicker、TimePicker、ColorPicker、Segmented、Form/FormItem |
| 6 | 数据展示 | `data.rs` | Card、List、Tree、Collapse、Descriptions、Timeline、Calendar、Carousel、Avatar、Badge、Tag、Image、Empty、Result、Skeleton、Table、SelectableList、RichText |
| 7 | 反馈 | `feedback.rs` | Alert、Message、Notification、ProgressBar、Spin、Modal、Drawer、Tooltip、Popover、Popconfirm |
| 8 | 图表 | `charts.rs` | BarChart、LineChart、PieChart |
| 9 | 其他 | `other.rs` | Transfer、Upload、QRCode、Watermark、DesignTokens |
| 10 | 框架能力 | `framework.rs` | LocaleProvider、ConfigProvider、ControlSize、ComponentOverrides、typed component token、统一空态 View |
| 11 | 覆盖清单 | `gallery.rs` | 交互式覆盖矩阵；点击 ✓ 行跳转对应页 |

机器可读矩阵 → [`demos/gallery.rs`](src/demos/gallery.rs) 中 `COVERAGE` 常量。

## 覆盖矩阵摘要

| 分类 | 已展示 | 说明 |
|------|--------|------|
| 通用 | 9/9 | 含 FloatButtonBackTop |
| 布局 | 9/9 | 含 ScrollView、Layout 区段 |
| 导航 | 11/11 | 含 Navigation/NavGroup builder |
| 输入 | 16/16 | Form + FormItem 分开展示 |
| 数据展示 | 18/18 | 含 Table、SelectableList、RichText |
| 反馈 | 10/10 | Overlay 浮层集中在反馈页 |
| 图表 | 3/3 | Bar / Line / Pie |
| 其他 | 7/7 | Transfer、Upload、自定义 component! |
| 应用能力 | 8/8 + 验收路径 | Timer 在 `on_start` 注册；多窗口主题联动、系统主题与图形恢复提供 live 用户路径 |
| 框架能力 | 4/4 | LocaleProvider、ConfigProvider/ControlSize、ComponentOverrides、typed token/render_empty 均有独立可观察样例 |
| CLI | 10 项 | `--cli` 模式 |

## 可执行验收场景

| 场景 | 用户路径与可观察结果 | 自动化 |
|------|----------------------|--------|
| Windows 多窗口主题联动 | 主窗进入“应用能力” → 打开 `UIX Theme Window` → 副窗切到暗色 → 两窗均显示“当前主题：暗色” → 独立关闭副窗 | 稳定 `automation_id` 定位；等待两窗 `presented_revision`；断言窗口标题、语义状态和窗口数量 |
| Windows 系统主题 live | 以 `--follow-system-theme` 启动 → UI 显示“跟随系统”且无手动开关 → OS 应用主题反转后窗口完成对应换色 → 恢复原 OS 设置后窗口恢复原配色 | 稳定 `automation_id` 断言模式；等待 `revision/presented_revision`；对真实前景窗口做桌面合成像素比较；测试始终恢复 `AppsUseLightTheme` |
| Windows 图形故障恢复 | 以 `--graphics-recovery-acceptance` 启动 → 进入“应用能力” → 注入下一帧 `DeviceLost` → 状态在恢复后的真实 present 才可见 → 再执行一次用户操作并呈现“恢复后交互成功” | `test-harness` 只在恢复包装器边界注入 typed fault；稳定 `automation_id` 与 `presented_revision` 证明失败后恢复和继续交互；进程日志确认命中 fault，且 D3D11 同 recipe 重建成功 |
| Provider 子树切换 | 主窗进入“框架能力” → 点击 `English` → 状态变为“当前语言：English”且空态文案变为 `No data`；同页可观察 Large/Small 优先级、构造覆盖和 typed token/统一空态 | 稳定 `automation_id`；`test-harness` 构建中文/英文子树并断言 `render_empty` 结果；真实窗口交互可由应用控制能力执行 |

以下 Windows 集成场景运行真实 `uix-demo --agent-control` 进程，需要可交互的 Windows 桌面：

```bash
cargo test --features agent-control --test agent_gui_windows real_demo_opens_theme_window_and_syncs_observable_state -- --ignored --nocapture
cargo test --features agent-control --test agent_gui_windows real_demo_follows_live_windows_system_theme_and_restores_preference -- --ignored --nocapture
cargo test --features agent-control,test-harness --test agent_gui_windows real_demo_recovers_from_injected_device_loss_and_accepts_followup_interaction -- --ignored --nocapture
cargo test --features agent-control,test-harness --test agent_gui_windows real_demo_switches_provider_locale_and_presents_framework_capabilities -- --ignored --nocapture
```

Provider 子树场景通过 Agent Bridge 执行页面导航、语言切换与滚动，等待每次真实 present 后断言语义树中的文案、Provider 尺寸优先级、typed token 与自定义空态；设置 `UIX_GUI_EVIDENCE_DIR` 可同时输出首页、中文、English 和滚动后四张 PNG。它使用 Vulkan pixel-upload 路径，以便现有桌面像素 oracle 对真实窗口进行严格取证；D3D11 swapchain 另由 WGC smoke 证据覆盖。

另有快速的无窗口构建契约测试：

```bash
cargo test --features test-harness --bin uix-demo framework_
```

## 验收 gap

本 demo 仍缺 macOS / Wayland 的 `follow_system_theme` 实机切换。当前图形恢复场景验证 Windows D3D11 的 typed fault 跨模块集成与同 recipe 重建；真实驱动触发的 `DeviceLost`、AMD / Intel 和跨平台硬件证据不由注入场景替代，仍按框架矩阵记录 → [`进度`](../docs/进度.md)。

## CLI 演示项

`cli/mod.rs` 顺序：`demo_core_types` · `demo_errors` · `demo_middleware` · `demo_state` · `demo_flex` · `demo_settings` · `demo_file_service` · `demo_theme` · `demo_graphics_engine` · `demo_di_container`
