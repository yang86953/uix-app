//! CPU 渲染引擎——SoftwareEngine 的新实现。
//!
//! 组合 PixelSurface + CpuCanvas2D，实现 GraphicsEngine trait。

pub mod canvas_2d;
pub mod noop_canvas_2d;
pub mod pixel_surface;
pub mod raster_renderer;
pub mod software;
