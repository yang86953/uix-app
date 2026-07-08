# Changelog

本文件记录 UIX 的**公开 API 与迁移锚点**。crate 当前 `publish = false`（`Cargo.toml`），但仍作为应用作者与贡献者的 breaking / migration 参考。

格式基于 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/)。版本号遵循 [Semantic Versioning](https://semver.org/lang/zh-CN/)。

设计决策详情 → [`docs/decisions.md`](docs/decisions.md) · 术语对照 → [`docs/glossary.md#术语对照`](docs/glossary.md#术语对照)。

---

## [Unreleased]

### Added

- 公开 API 单页 [`docs/systems/public-api.md`](docs/systems/public-api.md)
- 演示文档 [`demo/README.md`](demo/README.md)
- 平台贡献指南 [`docs/systems/platform.md#平台贡献指南`](docs/systems/platform.md#平台贡献指南)
- `VirtualScroll` 设计/实现状态文档 → [`docs/systems/layout.md#virtual-scroll`](docs/systems/layout.md#virtual-scroll)
- `VirtualScroll` / `VirtualListScroll` 已与 Table、Tree、SelectableList、Select、TreeSelect 集成 → [layout · VirtualScroll](docs/systems/layout.md#virtual-scroll)

### Changed

- 无待发布 breaking 变更

---

## [0.1.0] - 2026-07-08

首个内部 baseline。Windows + Linux Wayland GUI；`use uix::prelude::*` 为推荐入口。

### Added

- **App 运行时**：`App::new().root(...).run()` GUI 路径；`AppMode::CLI` CLI 路径（[#59](docs/decisions.md#d59)）
- **View DSL**：`column` / `row` / `grid` / `scroll` / `button` / `input` / `label` / `dynamic_label`（[#21](docs/decisions.md#d21) [#49](docs/decisions.md#d49)）
- **响应式**：`State<T>` · `Computed<T>` · `Effect`；Reconciler 自动 invalidation（[#24](docs/decisions.md#d24)）
- **按需零闲置主循环**：DeepIdle / RegisteredActive / Active 三态（[#105–#111](docs/decisions.md#d105)）
- **运行中 API**：`AppHandle::run_after` / `run_interval` · `post_to_ui` · `on_start` · `open_window` · `update_view`（[#132–#144](docs/decisions.md#d132)）
- **AppState + ComponentHandle**：跨窗 snapshot registry、lookup invalidate/emit（[#145–#147](docs/decisions.md#d145)）
- **80+ 内置 Widget**；Ant Design 5 风格主题（[#58](docs/decisions.md#d58)）
- **平台**：Win32 + Wayland backend；`FakePlatform` 测试 harness（[#40](docs/decisions.md#d40)）
- **绘制**：SoftwareEngine CPU 路径；Linux EGL GPU 可选（[#59](docs/decisions.md#d59) [#70](docs/decisions.md#d70)）

### Changed（命名对齐，迁移参考）

以下变更已在 v0.1.0 源码落地；自旧 fork / 早期原型迁移时对照：

| 旧 / 文档名 | 现行 | 决策 |
|-------------|------|------|
| `NodeId` / `WidgetId`（公开边界） | `ComponentId` | [#101](docs/decisions.md#d101) |
| `ScrollContainer` | `ScrollView` | [#104](docs/decisions.md#d104) |
| `preferred_size(engine)` | `measure(Constraints)` | [#103](docs/decisions.md#d103) |
| `StyleManager` preset（prelude） | 移除；用 `Style` / manager 显式路径 | component 实现注记 |
| 进程级 loop 设计名 `run_app_loop` | 源码 `run_widget_loop` | [#116](docs/decisions.md#d116) |

### Migration — AppState / Handle（[#145](docs/decisions.md#d145)）

| 场景 | 指引 |
|------|------|
| 现有 `State<T>` 闭包捕获 | **无需改动**；仍为响应式主路径 |
| 跨组件只读配置 + 窄 invalidate | 新代码优先 `ComponentHandle`（mount 自动注册） |
| 跨窗 / 异步 lookup | `AppState::get_handle(ComponentId)` → `invalidate()` / `emit()` |
| `State` vs `AppState` | **并存**；AppState 不替代 State |

详情 → [application · AppState](docs/systems/application.md#appstate--多窗--settings) · [application · 迁移表](docs/systems/application.md#appstate--componenthandle-规格3236172101145)。

### Known limitations（v0.1.0）

- macOS backend 未实现 → [roadmap · 后续工作](docs/roadmap.md#后续工作)
- crate 未发布 crates.io；API 仍可能按文档重构（冲突时 docs 为准，见 [AGENTS.md](AGENTS.md)）

---

[Unreleased]: https://github.com/your-org/uix-app/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/your-org/uix-app/releases/tag/v0.1.0
