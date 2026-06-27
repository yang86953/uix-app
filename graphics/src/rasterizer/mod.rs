//! 光栅化纯函数模块——不持任何状态。
//!
//! 所有函数纯输入→纯输出：给像素缓冲 + 参数 → 修改像素缓冲。
//! Canvas2D 的默认实现委托这些函数，CpuCanvas2D 也可能直接调用。
//!
//! 子模块：
//! - `fill` — 矢量填充（矩形/圆/椭圆/扇形/路径）
//! - `stroke` — 矢量描边
//! - `gradient` — 渐变填充
//! - `shadow` — 阴影绘制
//! - `glyph` — 字形混合
//! - `image` — 图像混合
//! - `polygon` — 多边形扫描线填充（原 rasterizer.rs）
//! - `core` — 共享纯函数工具

pub mod core;
pub mod fill;
pub mod glyph;
pub mod gradient;
pub mod image;
pub mod polygon;
pub mod shadow;
pub mod stroke;

pub use core::*;

pub use polygon::fill_polygons;
