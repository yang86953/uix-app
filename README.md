# UIX

UIX（/ˈjuːɪks/）是一个 Rust 原生 UI 框架。

> UIX 是专有闭源软件，仅供内部授权使用。

## 当前状态

- 当前 crate 候选版本为 `0.0.5`，正式发布版仍为 `0.0.4`；只有 Gitea tag、Release 与制品全部核验后才更新发布事实。
- Windows 与 Linux 均纳入 `0.0.5` 候选产品范围；两平台统一以 Vulkan GPU-native swapchain 为首选，Windows 保留已完成真实主机验收的 D3D11 兼容回退，Linux/Wayland 保留 EGL OpenGL ES 兼容回退。平台与硬件完成度以实际代码、公开 API 测试与[图形后端状态](docs/架构/graphics/backend.md#当前实现状态)为准。
- 版本与交付物口径见[交付与许可](docs/产品/交付与许可.md)，近期变化见[变更记录](CHANGELOG.md)。
- 仓库文档描述稳定的产品、使用与架构契约；任务级进度不在仓库内复制（原 Gitea 父 Issue #1 已停止维护，相关链接按撰写时点存档）。

## 从这里开始

- **推荐方式**：从 [UIX Lang 快速开始](docs/uix-lang/指南/快速开始.md)创建第一个 `.uix` 界面。
- **Rust API**：需要直接使用 Rust 声明式 API 时，阅读[使用 · 快速开始](docs/使用/入门/快速开始.md)。
- **了解能力**：先看[产品能力与边界](docs/产品/能力.md)，再按[文档中心](docs/README.md)选择专题。

## 公开 API 测试

本项目只测试已经写入产品、UIX Lang 或使用文档的公开 API。测试必须站在外部使用方视角，只通过 `uix::prelude`、各 System 的公开门面或公开 UIX Lang 入口消费框架；统一放在 `tests/*_public_api.rs`。

在已安装 Rust 与 Cargo 的环境中，从仓库根目录执行：

```powershell
cargo test --features agent-control --test "*_public_api" --quiet
```

不得为私有 Module、Component、内部算法、缓存、状态机、源码目录、依赖方向、测试专用 feature/harness、GPU parity、真窗视觉、性能、打包或文档结构建立项目测试；不得使用 `cargo test --lib`、`cargo test --tests` 或 Python 源码扫描扩大测试面。发布构建与人工验收不是项目测试，也不计入公开 API 测试覆盖。带时点的公开 API 测试结果仍以 Gitea 为准。

默认构建在 Windows 与 Linux 统一优先选择 Vulkan GPU-native swapchain，以同一 Drawing 语义保证外观和行为一致；Windows 的 D3D11 与 Linux/Wayland 的 OpenGL ES 只作为兼容回退。配置方法、失败语义与回退边界见[图形后端配置](docs/使用/框架设施/配置.md#图形后端)；真实环境覆盖以 Gitea 中的记录为准。

## 桌面目标交叉编译

统一入口同时覆盖根库与带 Agent 能力的主演示：

```bash
bash scripts/cross_compile.sh check all dev
bash scripts/cross_compile.sh build linux-x64 release
bash scripts/cross_compile.sh build windows-x64 release
```

检查矩阵包含 Linux x64、Windows x64、Apple Silicon macOS 与 Intel macOS。Windows 可从非 Windows 宿主经冻结版本的 `cargo-xwin` 完成真实 PE/COFF 链接；macOS 的最终链接必须在持有合法 Apple SDK 的 macOS 宿主执行。macOS 仍不是 `0.0.5` 当前交付平台，交叉检查通过也不替代真实窗口与 GPU 验收。完整前置条件、输出和失败语义见[交叉编译](docs/使用/入门/交叉编译.md)。

## Linux 内部候选包

在 Linux x86_64 的干净工作树中生成并校验内部候选包：

```bash
bash scripts/build_internal_release.sh
```

产物写入 `target/internal-release/uix-0.0.5-internal-linux-x64.tar.gz`。构建器会调用独立校验器核对精确载荷、规范路径、归档元数据与 SHA-256；候选包可生成不表示版本已正式发布，完整契约见[交付与许可](docs/产品/交付与许可.md)。

## Windows 内部候选包

在真实 Windows x64 桌面环境的干净 `main` 工作树中生成内部候选包：

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build_internal_release.ps1
```

该入口生成并校验 `uix-0.0.5-internal-win-x64.zip` 的载荷、路径与摘要；这是交付物完整性操作，不属于项目测试。Vulkan 与 D3D11 的真实 Windows x64 验收结果及制品摘要由 Gitea Release 持有。

`v0.0.5` 候选尚未附带 Windows x64 制品；该入口保留用于后续在 Windows 环境从同一发布提交完成最终构建、校验与制品追加。

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
| graphics | `uix::draw::*` | 统一 GPU backend（Vulkan 首选，D3D11 / OpenGL ES 兼容回退）、Software、颜色、字体 |
| ui | `uix::ui::*` | 组件、布局、主题、动画 |
| app | `uix::app::*` | App、Window、CLI、多窗、Agent Bridge |
| data | `uix::data::*` | SettingsService（opt-in KV） |

## 仓库结构

| 路径 | 内容 |
|---|---|
| `src/` | 框架实现与公开 API |
| `demo/` | 只包含 `uix-lang-demo`：以 `.uix` 声明全部已登记组件、展示数据、界面状态与局部交互的全组件主演示 |
| `uix-derive/` | UIX 派生宏 |
| `tests/` | 仅允许外部消费者视角的 `*_public_api.rs` 公开 API 测试 |
| `scripts/` | 内部打包工具 |
| `docs/` | 产品、架构、使用文档 |
| `assets/` | 编译期与 Demo 运行时资源 |
