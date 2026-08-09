# UIX

UIX（/ˈjuːɪks/）是一个 Rust 原生 UI 框架。

> UIX 是专有闭源软件，仅供内部授权使用。访问 [ui-x.dev](https://ui-x.dev) 了解更多。

## 当前状态

- 当前版本为 `0.0.1`，处于开发中，尚未发布。
- Windows x64 是当前优先交付平台；其他平台与硬件的完成度以 [Vikunja 项目 4](https://yang-server.tail9d5559.ts.net:3456/projects/4) 为准。
- 版本与交付物口径见[交付与许可](docs/产品/交付与许可.md)，近期变化见[变更记录](CHANGELOG.md)。
- 交付任务、负责人、阻塞与测试结果以 [Vikunja 项目 4](https://yang-server.tail9d5559.ts.net:3456/projects/4) 为准。

## 快速开始

在已安装 Rust 与 Cargo 的环境中，从仓库根目录执行：

```powershell
python -X utf8 -m unittest discover -s tests -p "test_*.py"
cargo test --lib --quiet
cargo test --tests --quiet
cargo test --manifest-path demo/Cargo.toml -p uix-gui-demo --bin uix-demo --quiet
cargo run --release --manifest-path demo/Cargo.toml --bin uix-demo
```

项目完成状态只以这些测试目标通过为准，不维护独立验证命令。

默认 feature 启用原生 D3D11；Windows 默认选择 D3D11，启用 `opengles` 时可构造 WGL/EGL OpenGL ES。运行环境不满足时，以 [Vikunja 项目 4](https://yang-server.tail9d5559.ts.net:3456/projects/4) 记录的支持边界为准。

完整依赖配置、第一个应用和示例说明见[使用 · 快速开始](docs/使用/快速开始.md)。

## 文档导航

| 入口 | 用途 |
|---|---|
| [文档中心](docs/README.md) | 按问题选择对应的事实角色和权威文档 |
| [产品](docs/产品.md) | 查看项目定位、目标用户与交付策略 |
| [功能](docs/功能.md) | 查看功能能力清单与边界 |
| [使用](docs/使用.md) | 查找公开 API、任务用法和示例 |
| [架构](docs/架构.md) | 查看系统边界、依赖、不变量与设计映射 |
| [交付与许可](docs/产品/交付与许可.md) | 查看版本、平台、许可与交付物范围 |
| [变更记录](CHANGELOG.md) | 查看用户可观察的版本变化 |

产品、功能、架构与使用文档均随本项目维护；事实归属和交叉引用规则见[文档中心](docs/README.md#文档边界)。

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
| `demo/` | 多项目演示工作区：`gui-demo`（`uix-demo` 多页应用与组件测试入口）、`cli-demo`（CLI 功能域演示） |
| `uix-derive/` | UIX 派生宏 |
| `tests/` | 契约、集成与真窗测试入口 |
| `scripts/` | 内部打包工具 |
| `docs/` | 产品、功能、架构、使用文档 |
| `assets/` | 编译期与 Demo 运行时资源 |

文档改动运行 `python tests/test_check_docs_links.py -q`。
