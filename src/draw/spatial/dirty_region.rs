//! 3D 脏区域——AABB 在屏幕空间的投影。
//!
//! 用于增量更新：经过 3D 变换的 widget 的脏区域不是简单矩形，
//! 需要投影到屏幕空间后再合并到渲染的脏区域中。
//!
//! > **状态警告**：当前 pipeline 仍以 2D [`crate::DirtyRegion`] 为准，
//! > 本类型尚未接入帧渲染管线。保留它是为 3D 增量渲染预留的公开契约，
//! > 待 SpatialContext 全面接入后再启用，勿在业务代码中依赖其行为。

use super::aabb3d::AABB3D;
use super::context::SpatialContext;
use crate::core::Rect;

/// 3D 脏区域：模型空间 AABB + 投影到屏幕。
///
/// 投影后得到屏幕外接矩形（透视下为外接近似）。
/// 详见模块级「状态警告」。
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

    /// 投影到屏幕空间，返回屏幕矩形（物理像素，已乘 device_pixel_ratio）。
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

// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
#[path = "../../tests/draw/spatial/dirty_region.rs"]
mod tests;

