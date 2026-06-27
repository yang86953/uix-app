//! 渲染后端——只管像素存储和呈现。
//!
//! 新后端只需实现 6 个方法。

use uix_platform::{Rect, Size};

use crate::Color;

/// 渲染后端接口。
///
/// 抽象像素缓冲区 + 呈现语义，CPU 和 GPU 后端各实现一份。
pub trait RenderingBackend {
    /// 表面尺寸（像素）。
    fn surface_size(&self) -> Size;

    /// 只读像素缓冲（CPU 后端有效，GPU 返回空切片）。
    fn pixels(&self) -> &[u32];

    /// 可变像素缓冲（CPU 后端有效，GPU 后端 panic）。
    fn pixels_mut(&mut self) -> &mut [u32];

    /// 清除指定矩形区域为指定颜色。
    fn clear_rect(&mut self, rect: Rect, color: Color);

    /// 呈现到屏幕（CPU 空操作，GPU swap buffers）。
    fn present(&mut self);

    /// 将 src 区域像素复制到 (dst_x, dst_y)。
    ///
    /// GPU 后端可选实现（默认空操作）。
    /// 核心节能手段：列表滚动时避免重绘 90% 像素。
    fn copy_region(&mut self, _src: Rect, _dst_x: i32, _dst_y: i32) {}
}
