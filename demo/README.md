# UIX 演示程序

> **AI 何时打开**：跑 demo / 对照页面覆盖。设计权威 → [`docs/架构.md`](../docs/架构.md)；落地硬约束 → [`AGENTS.md`](../AGENTS.md)。

## 快速运行

```bash
cargo run --bin uix-demo              # GUI（默认）
cargo run --bin uix-demo -- --cli     # CLI
RUST_LOG=debug cargo run --bin uix-demo
```

Linux GUI 需 Wayland。页面与 CLI 细节见下；上手 → [`架构 · 应用作者入口`](../docs/架构.md#应用作者入口)。

## 目录结构

```text
demo/src/
├── main.rs          # 薄入口：gui | cli
├── gui/             # 多页 GUI 壳层：分组侧边栏、Timer、动画 time
├── cli/             # core/native/draw/ui/app/data CLI
├── demos/           # 11 个内容页（含 dashboard/ 子页）
│   ├── home.rs      # 首页：入门 + 快捷导航
│   ├── runtime.rs   # 应用能力（State/Timer/Theme/View DSL）
│   ├── general.rs   # 通用 widgets
│   ├── layout.rs    # 布局 / 容器
│   ├── nav.rs       # 导航 + Navigation/NavGroup
│   ├── input.rs     # 输入控件
│   ├── data.rs      # 数据展示
│   ├── feedback.rs  # 反馈 + Overlay 浮层
│   ├── charts.rs    # 图表
│   ├── other.rs     # Transfer/Upload/DesignTokens
│   ├── gallery.rs   # 覆盖清单（参考）
│   └── context.rs   # DemoCtx（共享 State）
└── common/
    ├── page.rs      # PageBuilder、PAGE_TITLES、侧边栏分组
    ├── showcase.rs  # labeled_row、info_note 辅助
    └── widgets.rs   # Counter / PulseRing / BounceBall
```

## 两种模式

| 模式 | 命令 | 说明 |
|------|------|------|
| **GUI（默认）** | `cargo run --bin uix-demo` | 11 页多页应用：80+ Widget + App 能力全景 |
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
| 1 | 应用能力 | `runtime.rs` | State、dynamic_label、run_interval、ThemeToggle、component!、动画 demo |
| 2 | 通用 | `general.rs` | Button、Icon、Typography、Label、Divider、Space、FloatButton、FloatButtonBackTop、Tag |
| 3 | 布局 | `layout.rs` | Container、Grid、Layout/Header/Sider/Content/Footer、Splitter、ScrollView、Affix、BackTop |
| 4 | 导航 | `nav.rs` | Navigation、NavGroup、Menu、Tabs、Dropdown、Breadcrumb、Anchor、Steps、Pagination |
| 5 | 输入 | `input.rs` | Input、InputNumber、Select、TreeSelect、Cascader、AutoComplete、Mentions、Checkbox、Radio、Switch、Slider、Rate、DatePicker、TimePicker、ColorPicker、Segmented、Form/FormItem |
| 6 | 数据展示 | `data.rs` | Card、List、Tree、Collapse、Descriptions、Timeline、Calendar、Carousel、Avatar、Badge、Tag、Image、Empty、Result、Skeleton、Table、SelectableList、RichText |
| 7 | 反馈 | `feedback.rs` | Alert、Message、Notification、ProgressBar、Spin、Modal、Drawer、Tooltip、Popover、Popconfirm |
| 8 | 图表 | `charts.rs` | BarChart、LineChart、PieChart |
| 9 | 其他 | `other.rs` | Transfer、Upload、QRCode、Watermark、DesignTokens |
| 10 | 覆盖清单 | `gallery.rs` | 交互式覆盖矩阵；点击 ✓ 行跳转对应页 |

权威 widget 清单 → `src/ui/widgets/` · demo Gallery；导航 → [`架构 · 源码入口`](../docs/架构.md#源码入口)。
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
| 应用能力 | 8/8 | Timer 在 `on_start` 注册；多窗口见覆盖清单说明 |
| CLI | 10 项 | `--cli` 模式 |

## 演示与验证 gap

> “未单独展示”与“待真机验证”不等同于实现 backlog；在本演示 gap 清单中，只有屏幕阅读器平台桥属于框架实现 backlog。

| 项 | 实现状态 | 剩余 gap |
|----|----------|----------|
| 屏幕阅读器平台桥 (#99) | 静态 ARIA、role/name/state、键盘导航与焦点链已落地 | 平台屏幕阅读器桥待后续 |
| macOS 原生验证 | AppKit backend 与 Metal `Cpu × PixelUpload` 已编码 | 需 macOS 真机运行与硬件证据 |
| Handler fingerprint (#159) | `semantic_handler!` 显式 capture list 与稳定 fingerprint 已落地 | 无独立视觉演示；由自动化测试覆盖 |
| Computed 独立 slot | `Computed::new` 分配独立 `StateSlotId`，clone 共享 | 内部身份契约，无独立视觉演示 |
| 错误 Toast 自动 overlay (#89) | App 默认挂载 Notification overlay；`notify_error` 已接 | 无专用故障注入演示页 |
| 多窗口 live demo | `open_window` / 多窗路由 API 已落地 | 单进程多窗 live demo 待补 |
| `follow_system_theme` 自动跟 OS | `.follow_system_theme(true)` 已落地且默认关闭 | 跨 OS live 切换需真实桌面环境 |

## 从目标到代码（~15 分钟）

应用作者快速路径（与 [架构 · 应用作者入口](../docs/架构.md#应用作者入口) 一致）：

1. **为什么** — [#105 零闲置](../docs/决策.md#d105)：`App::run()` 空闲时不空转；Timer/动画由框架 register。
2. **声明 UI** — `State::new` + `label` / `map_text` + `button(…).on_click(&state, …)`；见 [README Counter 示例](../README.md#示例)。
3. **布局** — `column([...]).gap(12).padding(16)`；更多 → `src/ui/layout/` · [#165](../docs/决策.md#d165)。
4. **运行** — `App::new().title(...).root(|| ...).run()`；Timer/Theme → demo **应用能力** 页（`runtime.rs`）。
5. **深入** — [架构 · 源码入口](../docs/架构.md#源码入口) · [#178](../docs/决策.md#d178)–[#180](../docs/决策.md#d180)。

## CLI 演示项

`cli/mod.rs` 顺序：`demo_core_types` · `demo_errors` · `demo_middleware` · `demo_state` · `demo_flex` · `demo_settings` · `demo_file_service` · `demo_theme` · `demo_graphics_engine` · `demo_di_container`

## 测试

```bash
cargo test --bin uix-demo
```

- 全部 11 页 `build_page` smoke test
- GUI 壳层 layout
- Gallery 覆盖条目数量
- 首页 build + 页面切换 reconcile
- 首页 / 应用能力页局部 State 在根 reconcile 后保持

## 对应文档

| 想学 | 读 |
|------|-----|
| 上手 / 公开面 | [`架构 · 应用作者入口`](../docs/架构.md#应用作者入口) · [#178](../docs/决策.md#d178)–[#180](../docs/决策.md#d180) |
| View / State | `src/ui/view/` · `src/ui/foundation/state.rs` |
| App / Timer / 多窗 | `src/app/` · [#132](../docs/决策.md#d132) |
| 组件目录 | `src/ui/widgets/` · Gallery |
| Overlay | `src/ui/overlay.rs` · [#96](../docs/决策.md#d96)–[#100](../docs/决策.md#d100) |
