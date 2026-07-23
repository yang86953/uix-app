# UIX

UIX（/ˈjuːɪks/）是一个 Rust 原生 UI 框架。

> UIX 目前是闭源软件。访问 [ui-x.dev](https://ui-x.dev) 了解更多。

## 功能域

| 域 | 路径 | 用途 |
|----|------|------|
| 入口 | `uix::prelude::*` | App、View、State、组件 |
| core | `uix::core::*` | typed 错误与基础几何 |
| platform | `uix::platform::*` | 线程亲和的平台、硬件、GPU 描述与独立系统服务 |
| diagnostics | `uix::diagnostics::*` | 运行保障：恢复登记、最终错误报告与快照 |
| graphics | `uix::draw::*` | 统一 wgpu GPU backend、Software、颜色、字体 |
| ui | `uix::ui::*` | 组件、布局、主题、动画 |
| app | `uix::app::*` | App、Window、CLI、多窗、Agent Bridge |
| data | `uix::data::*` | SettingsService（opt-in KV） |
