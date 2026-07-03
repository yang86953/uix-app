//! CPU 2D 绘制上下文——`SharedRasterizer` 的类型别名。

pub use super::shared_rasterizer::SharedRasterizer;

/// CPU 后端 Canvas2D 实现。
pub type CpuCanvas2D = SharedRasterizer;
