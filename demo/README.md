# UIX 演示程序

> **角色**：能力全景（多页组件 / Timer / 主题 / View DSL），**不是**入门 starter。  
> 最小应用请复制 [README · 示例](../README.md#示例)；门面速查 → [`使用`](../docs/使用.md)。

## 快速运行

```bash
cargo run --bin uix-demo              # GUI（默认）
cargo run --bin uix-demo -- --cli     # CLI
RUST_LOG=debug cargo run --bin uix-demo
cargo run --features agent-control --bin uix-demo -- --agent-control
```

Linux GUI 需 Wayland。

`--agent-control` 仅用于本机开发验收，必须与 `agent-control` feature 同时启用；它会按 [`使用 · Agent Bridge`](../docs/使用.md#agent-bridge-开发预览) 启动当前用户私有端点。该参数不能与 `--cli` 同时使用。

## 目录结构

```text
demo/src/
├── main.rs          # 薄入口：gui | cli
├── gui/             # 多页 GUI 壳层：分组侧边栏、Timer、动画 time
├── cli/             # core/native/draw/ui/app/data CLI
├── demos/           # 11 个内容页
│   ├── home.rs      # 首页：入门 + 快捷导航
│   ├── runtime.rs   # 应用能力（State/Computed/Effect/Animation/Timer/Theme）
│   ├── general.rs   # 通用 widgets + ConfigProvider
│   ├── layout.rs    # 布局 / 容器 / Grid responsive / VirtualScroll
│   ├── nav.rs       # 导航 + Menu/NavGroup/Dropdown/Pagination
│   ├── input.rs     # 输入控件 + Form validation
│   ├── data.rs      # 数据展示 + Table（sortable/selection/fixed columns）
│   ├── feedback.rs  # 反馈 + Overlay 浮层（Modal/Drawer/Popover/Tooltip）
│   ├── charts.rs    # 图表（Bar/Line/Pie）
│   ├── other.rs     # Transfer/Upload/QRCode/Watermark/canvas/Accessibility
│   ├── gallery.rs   # 覆盖清单（参考）
│   └── context.rs   # DemoCtx（共享 State）
└── common/
    ├── page.rs      # PageBuilder、PAGE_TITLES、侧边栏分组
    ├── showcase.rs  # 展示辅助（panel、info_note、demo_card）
    └── widgets.rs   # Counter / PulseRing / BounceBall（component!）
```

## 两种模式

| 模式 | 命令 | 说明 |
|------|------|------|
| **GUI（默认）** | `cargo run --bin uix-demo` | 11 页多页应用：80+ Widget + App 能力全景 |
| **GUI + Agent Bridge** | `--features agent-control --bin uix-demo -- --agent-control` | 显式启用本机 `uix.agent.v1` 端点 |
| **CLI** | `--cli` | `core` / `native` / `draw` / `ui` / `app` / `data` 无 GUI API |

## 侧边栏分组

| 分组 | 页面 |
|------|------|
| **入门** | 首页、应用能力 |
| **组件** | 通用、布局、导航、输入、数据展示、反馈、图表、其他 |
| **参考** | 覆盖清单 |

## 页面索引

| # | 页 | 文件 | 覆盖 widget / 特性 |
|---|-----|------|-------------------|
| 0 | 首页 | `home.rs` | 入门 State、Timer tick、快捷导航 |
| 1 | 应用能力 | `runtime.rs` | State、Computed、Effect、Animation、Timer、ThemeToggle、component!、多窗 |
| 2 | 通用 | `general.rs` | Button、Icon、Typography、Label、Divider、Space、ConfigProvider、Size 系统、FloatButton、Tag |
| 3 | 布局 | `layout.rs` | Container、Grid、Grid responsive、Layout/Header/Sider/Content/Footer、Splitter、ScrollView、VirtualScroll、Affix、BackTop |
| 4 | 导航 | `nav.rs` | Navigation、NavGroup、Menu、Tabs、Dropdown、Breadcrumb、Anchor、Steps、Pagination |
| 5 | 输入 | `input.rs` | Input、InputNumber、Select、TreeSelect、Cascader、AutoComplete、Mentions、Checkbox、Radio、Switch、Slider、Rate、DatePicker、TimePicker、ColorPicker、Segmented、Form、Upload |
| 6 | 数据展示 | `data.rs` | Card、List、Tree、Collapse、Descriptions、Timeline、Calendar、Carousel、Avatar、Badge、Tag、Image、Empty、Result、Skeleton、Table（sortable/selection/bordered/fixed columns）、SelectableList、RichText |
| 7 | 反馈 | `feedback.rs` | Alert、Message、Notification、ProgressBar、Spin、Modal、Drawer、Tooltip、Popover、Popconfirm |
| 8 | 图表 | `charts.rs` | BarChart、LineChart（多系列）、PieChart |
| 9 | 其他 | `other.rs` | Transfer、Upload、QRCode、Watermark、canvas、DesignTokens、Accessibility |
| 10 | 覆盖清单 | `gallery.rs` | 交互式覆盖矩阵；点击 ✓ 行跳转对应页 |

机器可读矩阵 → [`demos/gallery.rs`](src/demos/gallery.rs) 中 `COVERAGE` 常量。

## 覆盖矩阵摘要

| 分类 | 项目 | 说明 |
|------|------|------|
| 通用 | 10/10 | 含 ConfigProvider、Size 系统 |
| 布局 | 9/9 | 含 Grid responsive、VirtualScroll |
| 导航 | 11/11 | 含 Navigation/NavGroup builder |
| 输入 | 16/16 | Form + FormItem 分开展示、含 Form validation |
| 数据展示 | 18/18 | 含 Table sortable/selection/bordered/fixed columns |
| 反馈 | 10/10 | Overlay 浮层集中在反馈页 |
| 图表 | 3/3 | Bar / Line（多系列）/ Pie |
| 其他 | 9/9 | 含 canvas、Accessibility |
| 应用能力 | 11/11 | 含 Computed、Effect、Animation |
| CLI | 10 项 | `--cli` 模式 |

## 演示 gap

本 demo 缺：多窗口 live 演示页、故障注入页、跨 OS `follow_system_theme` 实机切换。框架 backlog → [`进度`](../docs/进度.md)。

## CLI 演示项

`cli/mod.rs` 顺序：`demo_core_types` · `demo_errors` · `demo_middleware` · `demo_state` · `demo_flex` · `demo_settings` · `demo_file_service` · `demo_theme` · `demo_graphics_engine` · `demo_di_container`

## 测试

```bash
cargo test --bin uix-demo
cargo test --features agent-control --test agent_gui_windows -- --ignored --nocapture
```

- 全部 11 页 `build_page` smoke test
- GUI 壳层 layout
- Gallery 覆盖条目数量
- 首页 build + 页面切换 reconcile
- 首页 / 应用能力页局部 State 在根 reconcile 后保持
- Windows 真窗 Agent Bridge：显式运行 ignored 测试，覆盖默认 Vulkan 与显式 D3D11 recipe 及其遮挡能力声明、认证、快照、语义动作、原生 HWND Tab、呈现/最小化恢复与正常退出清理
