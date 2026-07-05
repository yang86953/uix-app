//! 帧渲染结果 + CPU 子模块。
//!
//! 内部渲染引擎 trait 在 `crate::render::traits::GraphicsEngine`。

use crate::render::backend::DamageRegion;

pub mod cpu;

/// 帧渲染结果。
#[derive(Debug, Clone, PartialEq)]
pub enum RenderOutcome {
    /// 零帧开销（无需呈现）
    Idle,
    /// 已渲染，需呈现（多矩形损伤）。
    Present(DamageRegion),
}
