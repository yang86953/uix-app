# 变更记录

这里记录 UIX 闭源版本的用户可观察变化。`0.0.1` 的当前事实见[交付与许可](docs/产品/交付与许可.md)。

## 0.0.1（未发布）

### 首发能力

- 提供 Rust 声明式 View、State / Computed、应用壳、本地 Settings 与桌面组件体系。
- `demo/` 调整为独立多项目工作区：`gui-demo`（`uix-demo` 多页应用与组件测试入口）与 `cli-demo`（CLI 功能域演示）各自独立构建，bin 名称与运行模式保持不变。
- 编译产物配置入仓：根包与 demo 工作区显式声明 release/dev/test profile（体积优先 `opt-level="s"`、fat LTO、符号表全剥离、`codegen-units=1`），不再依赖机器级 cargo 配置；release 的过程宏与构建脚本单独提速编译。
- Windows 默认以 `Auto` 探测统一 wgpu 渲染器支持的 GPU backend，并在有界恢复耗尽后整体回退 Software。
- 支持多窗口、IME、主题、定时与动画、浮层、结构化语义快照及显式启用的本机 Agent 开发预览。
- 提供 `uix-demo`、公开 API 使用示例和 Windows 组件测试入口。
- 所有产品与 Demo 图标统一走 `Icon` 组件管线；内置紧凑槽位复用 `Icon::paint_in_frame`，不再以 Unicode、ASCII、本地化字符或独立手绘形状充当图标。
- 捆绑字体固定为 `lucide-static` 1.17.0，并随内部包保留 ISC 与 Feather 派生图标 MIT 声明。

### 发布状态

- 版权归 `yangyanhui` 所有；版本按专有闭源、仅限内部授权使用的方式交付，不对外分发源码或二进制制品。
- 内部交付载体为 Windows 11 24H2 x64 ZIP：release Demo、内部 `.crate`、UIX 专有许可、Lucide / Feather 第三方声明、说明文件、运行时 Demo 图片与 SHA-256 清单；release 只启用默认 Vulkan feature，并保留 Software 整体回退。
- 本版本仍未发布；完成状态只以仓库自动测试为准。
