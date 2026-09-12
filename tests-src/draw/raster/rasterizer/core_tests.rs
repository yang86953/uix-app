//! `src/draw/raster/rasterizer/core.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的自由 cfg(test) 项 ——

// 仅剩 cfg(test) 的参考填充实现（rasterizer::fill）消费；生产路径统一走 SoftwareRasterizer。
