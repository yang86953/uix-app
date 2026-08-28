//! 光栅化纯函数模块——不持任何状态。
//!
//! 所有函数纯输入→纯输出：给像素缓冲 + 参数 → 修改像素缓冲。
//! 仅供绘图引擎内部执行器复用，不属于组件或应用绘制入口。
//!
//! 子模块：
//! - `fill` — 矢量填充（矩形/圆/椭圆/扇形/路径）
//! - `stroke` — 矢量描边
//! - `shadow` — 阴影绘制
//! - `glyph` — 字形混合
//! - `image` — 图像混合
//! - `polygon` — 多边形扫描线填充（原 rasterizer.rs）
//! - `core` — 共享纯函数工具

#![allow(clippy::too_many_arguments)]

pub(crate) mod blur;
pub(crate) mod core;
#[cfg(test)]
#[doc(hidden)]
pub mod fill;
pub(crate) mod glyph;
pub(crate) mod polygon;

pub(crate) use core::*;
