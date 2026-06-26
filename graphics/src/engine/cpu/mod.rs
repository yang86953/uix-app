//! CPU 渲染引擎——SoftwareEngine 的新实现。
//!
//! 组合 PixelSurface + CpuCanvas2D + NoopCanvas3D，
//! 实现新的 GraphicsEngine trait。

pub mod canvas_2d;
pub mod noop_canvas_2d;
pub mod noop_canvas_3d;
pub mod pixel_surface;
