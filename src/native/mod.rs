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
//! | [`present`] | 迁移中的原生 recipe 生命周期与私有 `GraphicsApi`；中立 `IPresenter` 由 `platform::presentation` 拥有，`PlatformWindow` 只借用 presenter |
//! | [`factory`] | 组合根：`create_platform_with_pending` / `available_memory_bytes` 与 `GraphicsRecipe` 登记（只负责选择、创建、注入，不拥有领域规则） |
//! | [`backends`] | OS 适配层：唯一允许 `#[cfg(target_os = ...)]` 的目录，实现各 Module 的窄能力契约 |
//! | [`test_harness`] | 测试替身（`test-harness` feature） |
//!
//! 五个私有 Module（目标边界见仓库 `docs/架构/系统列表.md`）：
//!
//! | Module | 职责 | 窄契约依赖 |
//! |--------|------|------------|
//! | [`capabilities`] | 系统、硬件、显示器查询与轻量系统服务 | 无（只依赖 core） |
//! | [`windowing`] | 原生窗口、事件源、输入、剪贴板与 IME | 无（只依赖 core） |
//! | [`presentation`] | surface、图形 recipe 与 presenter | `windowing::window`（取 surface 的窄契约） |
//! | `diagnostics` | runtime 级观察、报告与恢复协调 | 无（见 `crate::diagnostics`） |
//! | `agent_transport` | 可选同用户本机 IPC 与 discovery | `windowing` / `diagnostics`（feature `agent-control`） |
//!
//! 业务流向由 System 编排（`create_platform_with_pending` 与公开门面），Module 之间除上表
//! 窄契约外零依赖；禁止 Module 直接引用、持有、发现或回调兄弟 Module。
#[cfg(feature = "agent-control")]
pub(crate) mod agent_transport;
pub(crate) mod backends;
pub(crate) mod capabilities;
pub(crate) mod factory;
// 保留 `native::present` 逻辑路径，物理实现归入 presentation 契约组。
#[path = "presentation/contracts/mod.rs"]
pub(crate) mod present;
pub(crate) mod presentation;
#[cfg(feature = "test-harness")]
// 将测试支撑实现统一存放在根 tests 目录。
#[path = "../../tests/support/native/test_harness/mod.rs"]
pub(crate) mod test_harness;
pub(crate) mod windowing;

pub(crate) use crate::core::error::*;
