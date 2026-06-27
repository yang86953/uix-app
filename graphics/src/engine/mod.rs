//! 帧渲染结果 + CPU 子模块。
//!
//! 新版 GraphicsEngine trait 在 `crate::traits::GraphicsEngine`。

pub mod cpu;

/// 帧渲染结果。
#[derive(Debug, Clone, PartialEq)]
pub enum RenderOutcome {
    /// 零帧开销（无需呈现）
    Idle,
    /// 已渲染，需呈现。`None` = 全屏，`Some((x,y,w,h))` = 局部损伤矩形
    Present(Option<(i32, i32, i32, i32)>),
}
