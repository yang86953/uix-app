//! 3D 射线——用于命中测试（屏幕点击 → 3D 场景）。
//!
//! 提供射线与 AABB、z=0 平面的相交检测。

use super::aabb3d::AABB3D;
use super::vec3::Vec3;

/// 3D 射线，由起点和方向定义。
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
mod tests {
    use super::*;

    #[test]
    fn ray_intersects_aabb() {
        let ray = Ray3D::new(Vec3::new(0.0, 0.0, 5.0), Vec3::new(0.0, 0.0, -1.0));
        let aabb = AABB3D::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
        assert!(ray.intersects_aabb(&aabb));
    }

    #[test]
    fn ray_misses_aabb() {
        let ray = Ray3D::new(Vec3::new(10.0, 10.0, 5.0), Vec3::new(0.0, 0.0, -1.0));
        let aabb = AABB3D::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
        assert!(!ray.intersects_aabb(&aabb));
    }

    #[test]
    fn ray_intersects_z0_plane() {
        let ray = Ray3D::new(Vec3::new(10.0, 20.0, 5.0), Vec3::new(0.0, 0.0, -1.0));
        let hit = ray.intersect_z0().unwrap();
        assert!((hit.x - 10.0).abs() < 1e-6);
        assert!((hit.y - 20.0).abs() < 1e-6);
        assert!((hit.z).abs() < 1e-6);
    }

    #[test]
    fn ray_parallel_to_z0() {
        let ray = Ray3D::new(Vec3::new(0.0, 0.0, 5.0), Vec3::new(1.0, 0.0, 0.0));
        assert!(ray.intersect_z0().is_none());
    }

    #[test]
    fn ray_behind_z0() {
        let ray = Ray3D::new(Vec3::new(0.0, 0.0, -5.0), Vec3::new(0.0, 0.0, -1.0));
        // 射线从 z=-5 向 -z 方向，不会碰到 z=0
        assert!(ray.intersect_z0().is_none());
    }

    #[test]
    fn ray_at_parameter() {
        let ray = Ray3D::new(Vec3::new(1.0, 2.0, 3.0), Vec3::new(1.0, 0.0, 0.0));
        let p = ray.at(5.0);
        assert!((p.x - 6.0).abs() < 1e-6);
        assert!((p.y - 2.0).abs() < 1e-6);
        assert!((p.z - 3.0).abs() < 1e-6);
    }

    #[test]
    fn ray_aabb_edge_case_origin_inside() {
        let ray = Ray3D::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        let aabb = AABB3D::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
        assert!(ray.intersects_aabb(&aabb));
    }
}
