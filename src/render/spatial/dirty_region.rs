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
use crate::platform::Rect;

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
mod tests {
    use super::*;
    use crate::render::engine::cpu::noop_canvas_2d::NoopCanvas2D;
    use crate::render::spatial::vec3::Vec3;
    use crate::render::spatial::Orientation;

    #[test]
    fn new_stores_aabb() {
        let aabb = AABB3D::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 20.0, 0.0));
        let region = DirtyRegion3D::new(aabb);
        assert_eq!(region.aabb, aabb);
    }

    #[test]
    fn from_rect_z_builds_aabb() {
        let region = DirtyRegion3D::from_rect_z(Rect::new(10.0, 20.0, 100.0, 50.0), -1.0, 2.0);
        // z 范围 [-1, -1+2] = [-1, 1]
        assert!((region.aabb.min.z - (-1.0)).abs() < 1e-6);
        assert!((region.aabb.max.z - 1.0).abs() < 1e-6);
        assert!((region.aabb.min.x - 10.0).abs() < 1e-6);
        assert!((region.aabb.max.x - 110.0).abs() < 1e-6);
    }

    #[test]
    fn project_2d_maps_directly_to_screen() {
        let mut canvas = NoopCanvas2D;
        let ctx = SpatialContext::new(&mut canvas, 96.0, 1.0, Orientation::YDown, 800, 600);
        // 2D 正交投影下，模型 (100,100)-(300,200) 直接映射到屏幕同坐标
        let region = DirtyRegion3D::from_rect_z(Rect::new(100.0, 100.0, 200.0, 100.0), 0.0, 0.0);
        let r = region.project(&ctx);
        assert!((r.x - 100.0).abs() < 1.0, "x: {r:?}");
        assert!((r.y - 100.0).abs() < 1.0, "y: {r:?}");
        assert!((r.w - 200.0).abs() < 1.0, "w: {r:?}");
        assert!((r.h - 100.0).abs() < 1.0, "h: {r:?}");
    }

    #[test]
    fn project_scales_by_device_pixel_ratio() {
        let mut canvas = NoopCanvas2D;
        // 2x DPR：屏幕坐标应翻倍
        let ctx = SpatialContext::new(&mut canvas, 96.0, 2.0, Orientation::YDown, 800, 600);
        let region = DirtyRegion3D::from_rect_z(Rect::new(50.0, 50.0, 100.0, 100.0), 0.0, 0.0);
        let r = region.project(&ctx);
        assert!((r.x - 100.0).abs() < 2.0, "x: {r:?}");
        assert!((r.w - 200.0).abs() < 2.0, "w: {r:?}");
    }
}
