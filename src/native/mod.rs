//! 平台能力 — OS 抽象，差异封装在此域。
//!
//! # SMC 边界（SMC-02）
//!
//! 本模块是 platform System 的**私有实现边界**（`pub(crate)`），公开面由
//! `crate::platform`（capabilities 门面）与 `crate::diagnostics` 提供。
//! System 私有边界拥有跨 Module 共享契约与组装机制：
//!
//! | 归属 | 内容 |
//! |------|------|
//! | [`factory`] | 原生图形 recipe 登记与 `available_memory_bytes` 诊断查询；平台聚合改由 `platform::composition_root` 唯一组装 |
//! | [`backends`] | OS 适配层：唯一允许 `#[cfg(target_os = ...)]` 的目录，实现各 Module 的窄能力契约 |
//! | [`presentation`] | concrete 图形 API、surface 与 presenter Adapter；中立合同由 `platform::presentation` 拥有 |
//! | [`test_harness`] | 测试替身（`test-harness` feature） |
//!
//! 四个私有 Module（目标边界见仓库 `docs/架构/系统列表.md`）：
//!
//! | Module | 职责 | 窄契约依赖 |
//! |--------|------|------------|
//! | [`capabilities`] | 系统、硬件、显示器查询与轻量系统服务 | 无（只依赖 core） |
//! | [`windowing`] | 原生窗口、事件源、输入、剪贴板与 IME | 无（只依赖 core） |
//! | [`presentation`] | surface、图形 recipe 与 presenter | `windowing::window`（取 surface 的窄契约） |
//! | `diagnostics` | runtime 级观察、报告与恢复协调 | 无（见 `crate::diagnostics`） |
//!
//! 业务流向由 Platform System 组合根与公开门面编排，Module 之间除上表
//! 窄契约外零依赖；禁止 Module 直接引用、持有、发现或回调兄弟 Module。
pub(crate) mod backends;
pub(crate) mod capabilities;
pub(crate) mod factory;
pub(crate) mod presentation;
// native 测试替身（FakePlatform/FakeWindow 等）在 0.0.6 平台 facade 重构后
// 引用的系统服务 trait（IConsole/IFileDialog/INotification/ITimer）已被移除，
// 且无任何测试消费方；恢复 native parity 夹具时需同步重写该模块。
// UI 层 TestApp（tests/support/ui）不依赖本模块，由 test-harness feature 独立提供。
pub(crate) mod windowing;

pub(crate) use crate::core::error::*;
