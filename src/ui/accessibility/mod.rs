//! 无障碍 — 语义树快照与语义覆盖。
//!
//! # SMC 边界（SMC-04）
//!
//! accessibility Module 拥有无障碍语义模型（`semantic_snapshot`）与
//! 覆盖（`accessibility_override`）；语义动作执行器（`semantic_action`）
//! 因跨 automation / agent 协调归 System 私有边界。

pub(crate) mod accessibility_override;
pub(crate) mod semantic_snapshot;
