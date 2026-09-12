//! 基础设施 — 错误、几何。
//!
//! # SMC 定级（SMC-01）
//!
//! `core` 审计为**模式外基础依赖**，不提升为 System：它只提供跨系统稳定、
//! 无运行时策略的值类型与失败模型，没有独立领域状态、策略和生命周期，
//! 也不拥有资源。目标模块边界按仓库 `docs/架构/系统列表.md` 的 core 目录组织：
//!
//! | 模块 | 职责 | 依赖 |
//! |------|------|------|
//! | [`identity`] | 稳定身份与代际失效（`WidgetId` / `WindowId`） | 无 |
//! | [`geometry`] | logical 坐标、尺寸、矩形和约束 | 无 |
//! | [`damage`] | 增量失效与提交前后真相 | `geometry`（窄契约） |
//! | [`error`] | typed error 和统一结果语义 | 无 |
//!
//! # 旧路径 compile-fail 测试
//!
//! 下列契约锁定 SMC-01 后的模块边界：旧平铺路径对外不可达，
//! 任何恢复旧路径的改动都会编译失败。
//!
//!
//!

pub mod damage;
pub mod error;
/// 核心二维几何值类型与约束原语。
pub mod geometry;
pub mod identity;

pub use damage::*;
pub use error::*;
pub use geometry::*;
pub use identity::*;
