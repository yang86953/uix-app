# 变更记录

这里记录 UIX 闭源版本的用户可观察变化。`0.0.1` 的当前事实见[交付与许可](docs/产品/交付与许可.md)。

## 0.0.1（未发布）

### 首发能力

- Windows 与 Linux 均纳入 `0.0.1` 首发平台范围；Windows 当前基线为 x64/D3D11，Linux 当前原生路径为 Wayland/EGL OpenGL ES。
- 提供 Rust 声明式 View、State / Computed、应用壳、本地 Settings 与桌面组件体系。
- `demo/` 调整为独立多项目工作区：`uix-lang-demo`（仓库主演示，uix-lang 声明式多页应用）与 `cli-demo`（CLI 功能域演示）各自独立构建，bin 名称与运行模式保持不变。
- 编译产物配置入仓：根包与 demo 工作区显式声明 release/dev/test profile（体积优先 `opt-level="s"`、fat LTO、符号表全剥离、`codegen-units=1`），不再依赖机器级 cargo 配置；release 的过程宏与构建脚本单独提速编译。
- Windows 默认选择原生 D3D11 图形后端（默认 feature 集启用 `d3d11`），并在有界恢复耗尽后整体回退 Software。
- 支持多窗口、IME、主题、定时与动画、浮层、结构化语义快照及显式启用的本机 Agent 开发预览。
- 提供 `uix-lang-demo`、公开 API 使用示例和 8 个独立真窗验收程序（`badge-visual` / `selectable-list-visual` / `collapse-visual` / `popconfirm-visual` / `upload-visual` / `backdrop-visual` / `rich-text-visual` / `rich-text-image-visual`）。
- uix-lang 能力扩展：组件私有 state 支持类型注解（`u32` / `usize` / `f32` / `i32`）；受控组件与双向绑定属性（`open` / `value` / `checked` / `current` 等句柄位）可直接绑定组件私有 state；数据类属性按值 clone 消费，同一绑定可被多个标签复用；`uix-lang-demo` 对齐 API GUI Demo 全部已注册标签，并按页拆分为 12 个声明式页面组件。
- uix-lang 图表声明补齐类型化气泡数据、共同高级配置、热力图自定义色阶与四类高级坐标图参考线；颜色和参考线集合在生成代码中保持精确公开类型并复用既有运行时绘制契约。
- 所有产品与 Demo 图标统一走 `Icon` 组件管线；内置紧凑槽位复用 `Icon::paint_in_frame`，不再以 Unicode、ASCII、本地化字符或独立手绘形状充当图标。
- 捆绑字体固定为 `lucide-static` 1.17.0，并随内部包保留 ISC 与 Feather 派生图标 MIT 声明。

### 08-14 功能扩展

- Windows 系统通知接通：完成通知身份（AUMID）与发送契约（`NotificationService`），并提供真发送运行时验收；Linux（notify-send）与 macOS（osascript）经平台命令发送。
- 富文本内联图片完整生命周期：解析边界、图片资源后台解码轮询、绘制与无障碍快照事实，随 RichText 一起交付。
- 浮层背景模糊：Modal / Drawer 等浮层接入背景模糊策略与区域失效、快照重建，含真窗验收。
- Popconfirm 组合触发器：多触发器组合支持（父级测量、键盘释放捕获与回归覆盖）。

### 08-15 功能扩展

- 新增 `rich-text-visual`（RichText 主题分隔线）与 `rich-text-image-visual`（RichText 内联图片）两个独立真窗验收程序，真窗验收程序合计 8 个。

### 08-15 测试精简

- `cargo test` 只编译运行公开 API 契约测试（`app/core/data/diagnostics/draw/native/ui` 的 `tests/*_public_api.rs`）：移除 38 个非 API 集成测试（`flex_overflow_*`、`uix_lang_*_windows`、`usage_*`、`semantic_actions`、`layout_invariants`、`tessellator_contract` 等）与未被引用的 `tests/unit` 死代码，同步清理 `platform_notification_runtime` 的 `[test]` 表声明；设备丢失/表面丢失真窗注入验收由 demo 的 `--test-graphics-recovery` 页面承担，真窗视觉与通知运行时验收随精简移除。

### 修复与平台完善

- OpenGL ES 后端按渲染目标修正纵向行序：`opengles` feature 下 Wayland EGL 界面不再上下颠倒、Windows WGL 回读行序规范化，并完成 Linux EGL GUI 路径首次构建验证。
- Wayland 接通自定义标题栏装饰模式：GUI Demo 跨平台启用自定义标题栏，Wayland 通过 xdg-decoration 在客户端与服务端装饰间切换。
- macOS 修复 platform 拆分遗留属性：恢复 `MacosAppEvent` 的 `Debug` / `Clone` 派生，并补充源码契约防止回归。

### 发布状态

- 版权归 `yangyanhui` 所有；版本按专有闭源、仅限内部授权使用的方式交付，不对外分发源码或二进制制品。
- 已定义的 Windows 内部交付载体为 Windows 11 24H2 x64 ZIP：release Demo、内部 `.crate`、UIX 专有许可、Lucide / Feather 第三方声明、说明文件、运行时 Demo 图片与 SHA-256 清单；release 使用默认 feature 集（原生 D3D11），并保留 Software 整体回退。Linux 同属首发范围，其平台专用交付物细节由 Vikunja #770 管理。
- 本版本仍未发布；完成状态只以仓库自动测试为准。
