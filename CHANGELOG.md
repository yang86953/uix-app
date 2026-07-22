# 变更记录

这里记录 UIX 闭源版本的用户可观察变化。`0.0.1` 冻结前仍以[首发计划](docs/进度.md#001-首发计划)的 gate 与证据为准。

## 0.0.1（未发布）

### 首发能力

- 提供 Rust 声明式 View、State / Computed、应用壳、本地 Settings 与桌面组件体系。
- Windows 默认以 `Auto` 探测统一 wgpu 渲染器支持的 GPU backend，并在有界恢复耗尽后整体回退 Software。
- 支持多窗口、IME、主题、定时与动画、浮层、结构化语义快照及显式启用的本机 Agent 开发预览。
- 提供 `uix-demo`、公开 API 使用示例和 Windows 组件验收入口。
- 所有产品与 Demo 图标统一走 `Icon` 组件管线；内置紧凑槽位复用 `Icon::paint_in_frame`，不再以 Unicode、ASCII、本地化字符或独立手绘形状充当图标。
- 捆绑字体固定为经 npm 完整性核验的 `lucide-static` 1.17.0，并随内部包保留 ISC 与 Feather 派生图标 MIT 声明。

### 发布状态

- 版权归 `yangyanhui` 所有；版本按专有闭源、仅限内部授权使用的方式交付，不对外分发源码或二进制制品。
- 冻结交付载体为 Windows 11 24H2 x64 内部 ZIP：release Demo、内部 `.crate`、UIX 专有许可、Lucide / Feather 第三方声明、说明文件与 SHA-256 清单；release 只启用默认 Vulkan feature，并保留 Software 整体回退。
- 功能、真窗、硬件、长稳、性能、打包与缺陷门禁尚未全部闭合，因此本版本仍未发布。
