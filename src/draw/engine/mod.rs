//! 帧渲染结果 + CPU 子模块。
//!
//! 内部渲染引擎 trait 在 `crate::draw::traits::GraphicsEngine`。

use crate::draw::backend::DamageRegion;

pub mod bootstrap;
pub mod cpu;
pub mod factory;
pub mod present_upload;

/// 帧渲染结果。
#[derive(Debug, Clone, PartialEq)]
pub enum RenderOutcome {
    /// 零帧开销（无需呈现）
    Idle,
    /// 已渲染，需呈现（多矩形损伤）。
    Present(DamageRegion),
}
