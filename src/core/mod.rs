//! 基础设施 — 错误、几何。
//!
//! # SMC 定级（SMC-01）
//!
//! `core` 审计为**模式外基础依赖**，不提升为 System：它只提供跨系统稳定、
//! 无运行时策略的值类型与失败模型，没有独立领域状态、策略和生命周期，
//! 也不拥有资源。目标模块边界按 [系统列表] 的 core 目录组织：
//!
//! | 模块 | 职责 | 依赖 |
//! |------|------|------|
//! | [`identity`] | 稳定身份与代际失效（`ComponentId` / `WindowId`） | 无 |
//! | [`geometry`] | logical 坐标、尺寸、矩形和约束 | 无 |
//! | [`damage`] | 增量失效与提交前后真相 | `geometry`（窄契约） |
//! | [`error`] | typed error 和统一结果语义 | 无 |
//!
//! `perf_probe` / `glyph_outline` 是跨 System 共享的模式外基础探针与纯算法
//! （无独立契约、替换价值或变化原因），暂留 core 根，不作为角色登记。
//!
//! [系统列表]: <file:///C:/data/note/我的项目/软件/UIX App/架构/系统列表.md>
//!
//! # 旧路径 deny 证据（compile-fail）
//!
//! 下列契约锁定 SMC-01 后的模块边界：旧平铺路径与私有基础算法对外不可达，
//! 任何恢复旧路径或把 `glyph_outline` 提升为公开模块的改动都会编译失败。
//!
//! ```compile_fail
//! // 旧平铺路径不得复活：ComponentId 归 identity 模块。
//! use uix::core::component_id::ComponentId;
//! ```
//!
//! ```compile_fail
//! // 旧平铺路径不得复活：WindowId 归 identity 模块。
//! use uix::core::window_id::WindowId;
//! ```
//!
//! ```compile_fail
//! // glyph_outline 是模式外私有基础算法，不得作为公开面。
//! use uix::core::glyph_outline::colorize_edges;
//! ```

pub mod damage;
pub mod error;
pub mod geometry;
pub(crate) mod glyph_outline;
pub mod identity;
pub mod perf_probe;

pub use damage::*;
pub use error::*;
pub use geometry::*;
pub use identity::*;
