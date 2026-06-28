//! 3D 脏区域——AABB 在屏幕空间的投影。
//!
//! 用于增量更新：经过 3D 变换的 widget 的脏区域不是简单矩形，
//! 需要投影到屏幕空间后再合并到渲染的脏区域中。

use super::aabb3d::AABB3D;
use super::context::SpatialContext;
use uix_platform::Rect;

/// 3D 脏区域：AABB + 投影到屏幕。
#[derive(Debug, Clone, Copy)]
pub struct DirtyRegion3D {
    /// 3D 空间中的脏区域（模型空间 AABB）。
    pub aabb: AABB3D,
}

impl DirtyRegion3D {
    /// 从 3D AABB 创建脏区域。
    #[inline(always)]
    pub fn new(aabb: AABB3D) -> Self {
        Self { aabb }
    }

    /// 从 2D Rect + z 深度创建。
    #[inline(always)]
    pub fn from_rect_z(rect: Rect, z: f32, d: f32) -> Self {
        Self {
            aabb: AABB3D::from_rect_z(rect.x, rect.y, rect.w, rect.h, z, d),
        }
    }

    /// 投影到屏幕空间，返回屏幕矩形。
    ///
    /// 在透视投影下，返回的是外接矩形（可能大于实际脏区域）。
    pub fn project(&self, spatial: &SpatialContext) -> Rect {
        let quad = spatial.project_aabb(&self.aabb);
        let bounds = quad.bounds();
        Rect::new(
            bounds.x * spatial.device_pixel_ratio(),
            bounds.y * spatial.device_pixel_ratio(),
            bounds.w * spatial.device_pixel_ratio(),
            bounds.h * spatial.device_pixel_ratio(),
        )
    }
}
