# UIX 演示程序

> 可运行示例：验证 API、对照设计文档、回归 GUI 行为。设计权威 → [`docs/Main.md`](../docs/Main.md)。

## 快速运行

```bash
cargo run --bin uix-demo                    # 入门演示（App + View DSL，默认）
cargo run --bin uix-demo -- --dashboard     # GUI 组件库全景
cargo run --bin uix-demo -- --dashboard --gpu   # 同上（App 默认优先 GPU，失败回退 CPU）
cargo run --bin uix-demo -- --cli           # 无窗口 CLI 功能域演示
RUST_LOG=debug cargo run --bin uix-demo     # 任意模式 + 调试日志
```

Linux GUI 需 Wayland 会话。

## 目录结构

```text
demo/src/
├── main.rs              # 薄入口：解析 --cli / --dashboard / --gpu
├── modes/               # 三种运行模式
│   ├── default.rs       # 入门：State + View DSL + component!
│   ├── dashboard/       # 组件库：App 壳层 + State 分页
│   └── cli.rs           # 各功能域 CLI 打印演示
├── demos/               # 组件库分类页（对照 component.md）
│   ├── general.rs       # 通用
│   ├── layout.rs        # 布局
│   ├── nav.rs           # 导航
│   ├── input.rs         # 输入
│   ├── data.rs          # 数据展示
│   ├── feedback.rs      # 反馈
│   ├── charts.rs        # 图表
│   └── other.rs         # 其他 + component! 动画示例
└── common/              # 共享辅助
    ├── page.rs          # PageBuilder、常量
    └── widgets.rs       # Counter / PulseRing / BounceBall
```

## 推荐学习路径

1. **入门**（默认模式）→ [`modes/default.rs`](src/modes/default.rs)：`prelude`、`App::new().root()`、`State`、`column`/`row`/`button`、`dynamic_label`、`component!`
2. **组件库**（`--dashboard`）→ [`modes/dashboard/mod.rs`](src/modes/dashboard/mod.rs) + [`demos/`](src/demos/)：内置 Widget 全景；复杂片段用 `embed(tree!(...))` 桥接
3. **CLI**（`--cli`）→ [`modes/cli.rs`](src/modes/cli.rs)：无 GUI 域 API

## 模式一览

| 模式 | 命令 | 源码 | 教什么 |
|------|------|------|--------|
| **入门**（默认） | `cargo run --bin uix-demo` | [`modes/default.rs`](src/modes/default.rs) | `App` + View DSL + `State` + `on_click` + `component!` |
| **组件库** | `--dashboard` | [`modes/dashboard/`](src/modes/dashboard/) · [`demos/`](src/demos/) | 分类多页、`embed` 桥接、`PageBuilder` |
| **CLI** | `--cli` | [`modes/cli.rs`](src/modes/cli.rs) | `core` / `native` / `draw` / `ui` / `app` / `data` 无 GUI API |

入口：[`main.rs`](src/main.rs) — 解析 `--cli` / `--dashboard` / `--gpu` 并分派。

## Dashboard 页面

`--dashboard` 侧边栏对应 [`common/page.rs`](src/common/page.rs) 中 `PAGE_TITLES`：

| 页 | 文件 | 内容 |
|----|------|------|
| 通用 | [`demos/general.rs`](src/demos/general.rs) | Button、Typography、Icon 等 |
| 布局 | [`demos/layout.rs`](src/demos/layout.rs) | Layout、Grid、Space、Divider |
| 导航 | [`demos/nav.rs`](src/demos/nav.rs) | Menu、Breadcrumb、Tabs、Steps |
| 输入 | [`demos/input.rs`](src/demos/input.rs) | Input、Select、Form 等 |
| 数据展示 | [`demos/data.rs`](src/demos/data.rs) | Table、Tree、List、Tag |
| 反馈 | [`demos/feedback.rs`](src/demos/feedback.rs) | Modal、Message、Progress |
| 图表 | [`demos/charts.rs`](src/demos/charts.rs) | Bar / Line / Pie |
| 其他 | [`demos/other.rs`](src/demos/other.rs) | Transfer、Upload、自定义 widget |

`--gpu`：`App` 启动时默认优先 GPU 引擎（与不带 `--gpu` 行为一致）；失败自动回退 CPU。

## CLI 演示项

`run()` 顺序（[`modes/cli.rs`](src/modes/cli.rs)）：

| 函数 | 功能域 | 要点 |
|------|--------|------|
| `demo_core_types` | `core` | Point、Rect、Size、EdgeInsets、colors |
| `demo_errors` | `core` | Error、Errc、链式 cause |
| `demo_middleware` | `core` | 诊断中间件管道 |
| `demo_state` | `ui` | State get/set、clone 共享 |
| `demo_flex` | `ui/layout` | FlexLayout measure |
| `demo_settings` | `data` | SettingsService 读写 |
| `demo_file_service` | `native` | 文件服务 trait |
| `demo_theme` | `ui/theme` | Theme、DesignTokens |
| `demo_graphics_engine` | `draw` | NullEngine / SoftwareEngine |
| `demo_di_container` | `app` | DI 注册与 resolve |

## 对应文档

| 想学 | 读 |
|------|-----|
| 公开 API 清单 | [`public-api.md`](../docs/systems/public-api.md) |
| View / State | [`view-reactive.md`](../docs/systems/view-reactive.md) |
| App / 主循环 | [`application.md`](../docs/systems/application.md) |
| 组件 trait | [`component.md`](../docs/systems/component.md) |
| 测试 FakePlatform | [`testing.md`](../docs/systems/testing.md#fakeplatform) |

Automated 测试：`demo/src/tests/demos/dashboard.rs`。
