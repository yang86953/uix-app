//! 3D 轴对齐包围盒（AABB3D）。
//!
//! 提供 AABB 的构造、顶点提取、包含测试等基础操作。

use super::vec3::Vec3;

/// 3D 轴对齐包围盒（Axis-Aligned Bounding Box）。
///
/// 默认处于**模型空间**（model space），经 [`crate::render::spatial::SpatialContext`] 的
/// MVP 矩阵投影后落到屏幕空间。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AABB3D {
    pub min: Vec3,
    pub max: Vec3,
}

impl AABB3D {
    /// 从最小和最大角点构造。
    #[inline(always)]
    pub const fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    /// 从中心点和尺寸构造。
    #[inline(always)]
    pub fn from_center(center: Vec3, size: Vec3) -> Self {
        let half = Vec3::new(size.x * 0.5, size.y * 0.5, size.z * 0.5);
        Self {
            min: Vec3::new(center.x - half.x, center.y - half.y, center.z - half.z),
            max: Vec3::new(center.x + half.x, center.y + half.y, center.z + half.z),
        }
    }

    /// 从 2D Rect + z 深度构造。
    ///
    /// 用于将 2D UI 的 frame 转为 3D 空间中的 AABB。
    #[inline(always)]
    pub fn from_rect_z(x: f32, y: f32, w: f32, h: f32, z: f32, d: f32) -> Self {
        Self {
            min: Vec3::new(x, y, z),
            max: Vec3::new(x + w, y + h, z + d),
        }
    }

    /// 8 个顶点（用于投影到屏幕）。
    ///
    /// 顺序：先 min→max 遍历 x/y，再 z，共 8 个。
    pub fn corners(&self) -> [Vec3; 8] {
        let (min, max) = (self.min, self.max);
        [
            Vec3::new(min.x, min.y, min.z),
            Vec3::new(max.x, min.y, min.z),
            Vec3::new(min.x, max.y, min.z),
            Vec3::new(max.x, max.y, min.z),
            Vec3::new(min.x, min.y, max.z),
            Vec3::new(max.x, min.y, max.z),
            Vec3::new(min.x, max.y, max.z),
            Vec3::new(max.x, max.y, max.z),
        ]
    }

    /// 盒子中心点。
    #[inline(always)]
    pub fn center(&self) -> Vec3 {
        Vec3::new(
            (self.min.x + self.max.x) * 0.5,
            (self.min.y + self.max.y) * 0.5,
            (self.min.z + self.max.z) * 0.5,
        )
    }

    /// 盒子尺寸。
    #[inline(always)]
    pub fn size(&self) -> Vec3 {
        Vec3::new(
            self.max.x - self.min.x,
            self.max.y - self.min.y,
            self.max.z - self.min.z,
        )
    }

    /// 3D 点是否在盒内（包含边界）。
    #[inline(always)]
    pub fn contains(&self, p: Vec3) -> bool {
        p.x >= self.min.x
            && p.x <= self.max.x
            && p.y >= self.min.y
            && p.y <= self.max.y
            && p.z >= self.min.z
            && p.z <= self.max.z
    }

    /// 是否与另一个 AABB 相交。
    #[inline(always)]
    pub fn intersects(&self, other: &Self) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
            && self.min.z <= other.max.z
            && self.max.z >= other.min.z
    }

    /// 合并两个 AABB（取并集）。
    #[inline(always)]
    pub fn union(&self, other: &Self) -> Self {
        Self {
            min: Vec3::new(
                self.min.x.min(other.min.x),
                self.min.y.min(other.min.y),
                self.min.z.min(other.min.z),
            ),
            max: Vec3::new(
                self.max.x.max(other.max.x),
                self.max.y.max(other.max.y),
                self.max.z.max(other.max.z),
            ),
        }
    }

    /// 是否为空（零体积）。
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.max.x <= self.min.x || self.max.y <= self.min.y || self.max.z <= self.min.z
    }
}

// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_center_size() {
        let aabb = AABB3D::from_center(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 2.0, 2.0));
        assert!((aabb.min.x + 1.0).abs() < 1e-10);
        assert!((aabb.max.x - 1.0).abs() < 1e-10);
    }

    #[test]
    fn corners_count() {
        let aabb = AABB3D::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));
        let corners = aabb.corners();
        assert_eq!(corners.len(), 8);
        // 验证所有顶点都在范围内
        for c in &corners {
            assert!(aabb.contains(*c));
        }
    }

    #[test]
    fn contains_point() {
        let aabb = AABB3D::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(10.0, 10.0, 10.0));
        assert!(aabb.contains(Vec3::new(5.0, 5.0, 5.0)));
        assert!(!aabb.contains(Vec3::new(15.0, 5.0, 5.0)));
        assert!(!aabb.contains(Vec3::new(5.0, -1.0, 5.0)));
    }

    #[test]
    fn intersects_overlap() {
        let a = AABB3D::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(5.0, 5.0, 5.0));
        let b = AABB3D::new(Vec3::new(3.0, 3.0, 3.0), Vec3::new(8.0, 8.0, 8.0));
        assert!(a.intersects(&b));
    }

    #[test]
    fn intersects_no_overlap() {
        let a = AABB3D::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 2.0, 2.0));
        let b = AABB3D::new(Vec3::new(5.0, 5.0, 5.0), Vec3::new(8.0, 8.0, 8.0));
        assert!(!a.intersects(&b));
    }

    #[test]
    fn union_expands() {
        let a = AABB3D::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(3.0, 3.0, 3.0));
        let b = AABB3D::new(Vec3::new(5.0, 5.0, 5.0), Vec3::new(8.0, 8.0, 8.0));
        let u = a.union(&b);
        assert!((u.min.x - 0.0).abs() < 1e-10);
        assert!((u.max.x - 8.0).abs() < 1e-10);
    }

    #[test]
    fn is_empty_positive() {
        let aabb = AABB3D::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.0));
        assert!(aabb.is_empty());
    }

    #[test]
    fn from_rect_z() {
        let aabb = AABB3D::from_rect_z(10.0, 20.0, 100.0, 50.0, -5.0, 10.0);
        assert!((aabb.min.x - 10.0).abs() < 1e-10);
        assert!((aabb.min.y - 20.0).abs() < 1e-10);
        assert!((aabb.min.z - (-5.0)).abs() < 1e-10);
        assert!((aabb.max.x - 110.0).abs() < 1e-10);
        assert!((aabb.max.y - 70.0).abs() < 1e-10);
        assert!((aabb.max.z - 5.0).abs() < 1e-10);
    }
}
