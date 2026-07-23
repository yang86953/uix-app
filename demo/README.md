# UIX 演示程序

> **角色**：能力全景 + 可执行测试场景（多页组件 / Timer / 主题 / 多窗口 / View DSL），**不是**入门 starter。
> 最小应用请复制 [README · 示例](../README.md#示例)；门面速查 → [`使用`](../docs/使用.md)。

## 快速运行

```bash
cargo run --bin uix-demo              # GUI（默认）
cargo run --bin uix-demo -- --follow-system-theme
cargo run --features test-harness --bin uix-demo -- --graphics-recovery-acceptance
cargo run --features "test-harness agent-control" --bin uix-demo -- --component-qa
cargo run --bin uix-demo -- --cli     # CLI
RUST_LOG=debug cargo run --bin uix-demo
cargo run --features agent-control --bin uix-demo -- --agent-control
```

Linux GUI 需 Wayland。

`--follow-system-theme` 通过公开 `App::follow_system_theme(true)` 让 GUI 初始主题和后续变化由 OS 托管；该模式显示“跟随系统”状态，不再提供会覆盖托管状态的手动主题开关。`--graphics-recovery-acceptance` 仅在 `test-harness` feature 下显示下一帧 typed `DeviceLost` 注入与恢复后交互路径，Demo 仍通过公开 `AppHandle::inject_graphics_device_lost_for_test()` 使用框架。`--component-qa` 以隔离页面打开完整公开组件库存，供真实窗口逐组件视觉验收。`--agent-control` 仅用于本机开发预览，必须与 `agent-control` feature 同时启用；它会按 [`使用 · Agent Bridge`](../docs/使用/Agent控制.md#agent-bridge-开发预览) 启动当前用户私有端点。这些 GUI 参数都不能与 `--cli` 同时使用。

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
| **GUI + 逐组件视觉验收** | `--features "test-harness agent-control" --bin uix-demo -- --component-qa` | 隔离展示当前公开组件库存及其适用状态，供真实窗口自动取证 |
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

## 测试覆盖现状

- Windows 集成测试覆盖多窗口主题联动、系统主题 live 更新、注入式图形故障恢复、Provider 子树切换与逐组件视觉矩阵。
- 逐组件页面由 `--component-qa` 提供，包含公开组件库存、适用状态、浅色桌面、深色紧凑及交互状态；原始数据默认位于 `target/debug-captures/uix-component-visual/`。
- 2026-07-17 的组件 / 状态映射、联系表和结论见 [`逐组件视觉质量报告`](../test-reports/20260717-uix-component-visual/report.md)，只属于旧渲染基线。
- macOS / Wayland 的 `follow_system_theme` 实机数据、真实驱动触发的 `DeviceLost`、AMD 与 Intel 数据当前缺失，详见[`进度`](../docs/进度.md)。

## CLI 演示项

`cli/mod.rs` 顺序：`demo_core_types` · `demo_errors` · `demo_middleware` · `demo_state` · `demo_flex` · `demo_settings` · `demo_file_service` · `demo_theme` · `demo_renderer` · `demo_di_container`
