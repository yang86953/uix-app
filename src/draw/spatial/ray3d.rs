//! 3D 射线——用于命中测试（屏幕点击 → 3D 场景）。
//!
//! 提供射线与 AABB、z=0 平面的相交检测。

use super::aabb3d::AABB3D;
use super::vec3::Vec3;

/// 3D 射线，由起点和方向定义。
///
/// 通常由屏幕点击位置经 [`crate::draw::spatial::SpatialContext::unproject`] 反投影得到，
/// 处于**模型空间**（model space），用于命中测试。
#[derive(Debug, Clone, Copy)]
pub struct Ray3D {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray3D {
    /// 创建射线。
    ///
    /// `direction` 应为归一化向量。
    #[inline(always)]
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self { origin, direction }
    }

    /// 射线与 AABB 的相交测试（Slabs Method）。
    ///
    /// 返回 `true` 当射线与 AABB 有交点且交点在射线正方向（t ≥ 0）。
    pub fn intersects_aabb(&self, aabb: &AABB3D) -> bool {
        let inv_dir = Vec3::new(
            1.0 / self.direction.x,
            1.0 / self.direction.y,
            1.0 / self.direction.z,
        );

        let t1 = (aabb.min - self.origin).component_mul(&inv_dir);
        let t2 = (aabb.max - self.origin).component_mul(&inv_dir);

        let tmin = t1.x.min(t2.x).max(t1.y.min(t2.y)).max(t1.z.min(t2.z));
        let tmax = t1.x.max(t2.x).min(t1.y.max(t2.y)).min(t1.z.max(t2.z));

        tmax >= 0.0 && tmax >= tmin
    }

    /// 射线与 z=0 平面的交点（2D UI 命中测试用）。
    ///
    /// 返回交点的 3D 坐标。当射线平行于 z=0 平面时返回 None。
    pub fn intersect_z0(&self) -> Option<Vec3> {
        if self.direction.z.abs() < 1e-10 {
            return None;
        }
        let t = -self.origin.z / self.direction.z;
        if t < 0.0 {
            return None;
        }
        Some(self.origin + self.direction * t)
    }

    /// 给定参数 t，返回射线上的点。
    #[inline(always)]
    pub fn at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }
}

// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
#[path = "../../tests/draw/spatial/ray3d.rs"]
mod tests;
