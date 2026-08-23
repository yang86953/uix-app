# UIX

UIX（/ˈjuːɪks/）是一个 Rust 原生 UI 框架。

> UIX 是专有闭源软件，仅供内部授权使用。

## 当前状态

- 当前版本为 `0.0.1`，处于开发中，尚未发布。
- Windows 与 Linux 均纳入 `0.0.1` 首发交付范围；两平台统一以 Vulkan GPU-native swapchain 为首选，Windows 保留 D3D11、Linux/Wayland 保留 EGL OpenGL ES 兼容回退；平台与硬件完成度以 [Gitea 0.0.1 产品完成父 Issue #1](http://100.79.245.29:3000/admin/uix-app/issues/1) 为准。
- 版本与交付物口径见[交付与许可](docs/产品/交付与许可.md)，近期变化见[变更记录](CHANGELOG.md)。
- 仓库文档描述稳定的产品、使用与架构契约；交付任务、负责人、阻塞和带时点的测试结果只在 Gitea 中维护。

## 从这里开始

- **推荐方式**：从 [UIX Lang 快速开始](docs/uix-lang/指南/快速开始.md)创建第一个 `.uix` 界面。
- **Rust API**：需要直接使用 Rust 声明式 API 时，阅读[使用 · 快速开始](docs/使用/入门/快速开始.md)。
- **了解能力**：先看[产品能力与边界](docs/产品/能力.md)，再按[文档中心](docs/README.md)选择专题。

## 仓库验证

以下命令用于验证 UIX 仓库，不是创建第一个应用的前置步骤。在已安装 Python、Rust 与 Cargo 的环境中，从仓库根目录执行：

```powershell
python -X utf8 -m unittest discover -s tests -p "test_*.py"
cargo test --lib --quiet
cargo test --tests --quiet
cargo run --release --manifest-path demo/Cargo.toml --bin uix-lang-demo
```

这些命令只验证当前工作树的对应契约，不等同于正式发布、平台矩阵或交付任务全部完成；带时点的完成状态仍以 Gitea 为准。

默认构建在 Windows 与 Linux 统一优先选择 Vulkan GPU-native swapchain，以同一 Drawing 语义保证外观和行为一致；Windows 的 D3D11 与 Linux/Wayland 的 OpenGL ES 只作为兼容回退。配置方法、失败语义与回退边界见[图形后端配置](docs/使用/框架设施/配置.md#图形后端)；真实环境覆盖以 Gitea 中的记录为准。

## Linux 内部候选包

在 Linux x86_64 的干净工作树中生成并校验内部候选包：

```bash
bash scripts/build_internal_release.sh
```

产物写入 `target/internal-release/uix-0.0.1-internal-linux-x64.tar.gz`。构建器会调用独立校验器核对精确载荷、规范路径、归档元数据与 SHA-256；候选包可生成不表示版本已正式发布，完整契约见[交付与许可](docs/产品/交付与许可.md)。

## Windows 内部候选包

在真实 Windows x64 桌面环境的干净 `main` 工作树中执行完整验收：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/accept_windows_release.ps1
```

该入口验证 Vulkan 主路径、D3D11 兼容回退和主演示最终 surface 像素，连续构建并校验两份字节一致的 `uix-0.0.1-internal-win-x64.zip`，同时生成不含凭据的环境证据 JSON。只需要生成单份开发候选包时可直接运行 `scripts/build_internal_release.ps1`。

## 文档导航

| 入口 | 用途 |
|---|---|
| [文档中心](docs/README.md) | 按问题选择对应的权威文档 |
| [产品](docs/产品.md) | 查看项目定位、目标用户、产品范围与明确边界 |
| [使用](docs/使用.md) | 查找公开 API、任务用法和示例 |
| [Agent 控制](docs/使用/能力与边界/Agent控制.md) | 显式启用的本机 AI/自动化控制端点（`agent-control`） |
| [UIX Lang](docs/uix-lang/README.md) | 使用推荐界面描述方式（`.uix` 文档，编译时转换为 Rust） |
| [架构](docs/架构.md) | 查看系统边界、依赖、不变量与设计映射 |
| [交付与许可](docs/产品/交付与许可.md) | 查看版本、平台、许可与交付物范围 |
| [变更记录](CHANGELOG.md) | 查看用户可观察的版本变化 |

产品、UIX Lang、使用与架构文档均随本项目维护；各类事实的归属、状态含义和交叉引用规则见[文档中心](docs/README.md#文档边界与事实来源)。

## 功能域

| 域 | 路径 | 用途 |
|---|---|---|
| 入口 | `uix::prelude::*` | App、View、State、组件 |
| core | `uix::core::*` | typed 错误与基础几何 |
| platform | `uix::platform::*` | 线程亲和的平台、硬件、GPU 描述与独立系统服务 |
| diagnostics | `uix::diagnostics::*` | 恢复登记、最终错误报告与快照 |
| graphics | `uix::draw::*` | 统一 GPU backend（D3D11/OpenGL ES）、Software、颜色、字体 |
| ui | `uix::ui::*` | 组件、布局、主题、动画 |
| app | `uix::app::*` | App、Window、CLI、多窗、Agent Bridge |
| data | `uix::data::*` | SettingsService（opt-in KV） |

## 仓库结构

| 路径 | 内容 |
|---|---|
| `src/` | 框架实现与公开 API |
| `demo/` | 只包含 `uix-lang-demo`：以 `.uix` 声明全部已登记组件、展示数据、界面状态与局部交互的全组件主演示 |
| `uix-derive/` | UIX 派生宏 |
| `tests/` | 契约、集成与真窗测试入口 |
| `scripts/` | 内部打包工具 |
| `docs/` | 产品、架构、使用文档 |
| `assets/` | 编译期与 Demo 运行时资源 |

文档改动运行 `python tests/test_check_docs_links.py -q`；涉及 `uix-compile` 围栏时同时运行 `python tests/test_docs_compile_coverage.py -q`。
