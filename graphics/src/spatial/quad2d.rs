//! 2D 四边形——3D AABB 投影到屏幕后的形状。
//!
//! 在透视投影下，3D 矩形的投影可能是梯形或任意四边形。
//! Quad2D 提供外接矩形计算、凸四边形包含测试、多边形路径导出。
//!
//! 本模块同时定义唯一的 2D 向量/点类型 [`Vec2`]，供整个空间系统复用。

use std::ops::{Add, Mul, Sub};

use uix_platform::Rect;

/// 2D 向量/点（屏幕空间）。
///
/// 这是空间系统中唯一的 2D 向量类型；3D 场景请使用 [`super::vec3::Vec3`]。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    #[inline(always)]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    #[inline(always)]
    pub const fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }

    /// 点积
    #[inline(always)]
    pub fn dot(&self, rhs: &Self) -> f32 {
        self.x * rhs.x + self.y * rhs.y
    }

    /// 长度
    #[inline(always)]
    pub fn length(&self) -> f32 {
        self.dot(self).sqrt()
    }

    /// 归一化。零向量返回自身（避免除零）。
    #[inline(always)]
    pub fn normalized(&self) -> Self {
        let l = self.length();
        if l > 1e-10 {
            Self::new(self.x / l, self.y / l)
        } else {
            *self
        }
    }
}

impl From<(f32, f32)> for Vec2 {
    fn from((x, y): (f32, f32)) -> Self {
        Self { x, y }
    }
}

impl Add for Vec2 {
    type Output = Self;
    #[inline(always)]
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Vec2 {
    type Output = Self;
    #[inline(always)]
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Self;
    #[inline(always)]
    fn mul(self, s: f32) -> Self {
        Self::new(self.x * s, self.y * s)
    }
}

impl Mul<Vec2> for f32 {
    type Output = Vec2;
    #[inline(always)]
    fn mul(self, v: Vec2) -> Vec2 {
        Vec2::new(self * v.x, self * v.y)
    }
}

/// 屏幕空间的四边形（由四个 2D 顶点定义）。
///
/// 顶点顺序：p0→p1→p2→p3 构成顺时针或逆时针环。
/// 通常由 [`crate::spatial::AABB3D`] 经投影得到，处于**屏幕空间**（screen space）。
#[derive(Debug, Clone, Copy)]
pub struct Quad2D {
    pub p0: Vec2,
    pub p1: Vec2,
    pub p2: Vec2,
    pub p3: Vec2,
}

impl Quad2D {
    /// 从四个顶点构造。
    #[inline(always)]
    pub const fn new(p0: Vec2, p1: Vec2, p2: Vec2, p3: Vec2) -> Self {
        Self { p0, p1, p2, p3 }
    }

    /// 四边形的外接矩形。
    pub fn bounds(&self) -> Rect {
        let xs = [self.p0.x, self.p1.x, self.p2.x, self.p3.x];
        let ys = [self.p0.y, self.p1.y, self.p2.y, self.p3.y];
        let min_x = xs.iter().cloned().fold(f32::MAX, f32::min);
        let max_x = xs.iter().cloned().fold(f32::MIN, f32::max);
        let min_y = ys.iter().cloned().fold(f32::MAX, f32::min);
        let max_y = ys.iter().cloned().fold(f32::MIN, f32::max);
        Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
    }

    /// 转为多边形路径顶点列表（用于多边形填充）。
    pub fn to_path(&self) -> Vec<Vec2> {
        vec![self.p0, self.p1, self.p2, self.p3]
    }

    /// 2D 点是否在四边形内（凸四边形包含测试）。
    ///
    /// 使用符号法：点在凸四边形内当且仅当点在四条边的同侧。
    pub fn contains(&self, p: Vec2) -> bool {
        let d1 = Self::cross_sign(p, self.p0, self.p1);
        let d2 = Self::cross_sign(p, self.p1, self.p2);
        let d3 = Self::cross_sign(p, self.p2, self.p3);
        let d4 = Self::cross_sign(p, self.p3, self.p0);

        let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0) || (d4 < 0.0);
        let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0) || (d4 > 0.0);

        !(has_neg && has_pos)
    }

    /// 从一组 2D 点构建轴对齐的外接四边形（简化）。
    ///
    /// 这不是严格的凸包，而是取所有点的 x/y 极值构造矩形。
    /// 在透视投影下可以接受，因为最终会用多边形填充。
    pub fn from_points(points: &[Vec2]) -> Self {
        let min_x = points.iter().map(|p| p.x).fold(f32::MAX, f32::min);
        let max_x = points.iter().map(|p| p.x).fold(f32::MIN, f32::max);
        let min_y = points.iter().map(|p| p.y).fold(f32::MAX, f32::min);
        let max_y = points.iter().map(|p| p.y).fold(f32::MIN, f32::max);
        Self {
            p0: Vec2::new(min_x, min_y),
            p1: Vec2::new(max_x, min_y),
            p2: Vec2::new(max_x, max_y),
            p3: Vec2::new(min_x, max_y),
        }
    }

    /// 跨积符号：p × (a→b) 的 z 分量
    #[inline(always)]
    fn cross_sign(p: Vec2, a: Vec2, b: Vec2) -> f32 {
        (p.x - b.x) * (a.y - b.y) - (a.x - b.x) * (p.y - b.y)
    }
}

// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_axis_aligned() {
        let q = Quad2D::new(
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(10.0, 5.0),
            Vec2::new(0.0, 5.0),
        );
        let b = q.bounds();
        assert!((b.x - 0.0).abs() < 1e-10);
        assert!((b.y - 0.0).abs() < 1e-10);
        assert!((b.w - 10.0).abs() < 1e-10);
        assert!((b.h - 5.0).abs() < 1e-10);
    }

    #[test]
    fn bounds_rotated() {
        // 旋转 45 度的正方形投影
        let q = Quad2D::new(
            Vec2::new(5.0, 0.0),
            Vec2::new(10.0, 5.0),
            Vec2::new(5.0, 10.0),
            Vec2::new(0.0, 5.0),
        );
        let b = q.bounds();
        assert!((b.x - 0.0).abs() < 1e-10);
        assert!((b.y - 0.0).abs() < 1e-10);
        assert!((b.w - 10.0).abs() < 1e-10);
        assert!((b.h - 10.0).abs() < 1e-10);
    }

    #[test]
    fn contains_inside() {
        let q = Quad2D::new(
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(0.0, 10.0),
        );
        assert!(q.contains(Vec2::new(5.0, 5.0)));
    }

    #[test]
    fn contains_outside() {
        let q = Quad2D::new(
            Vec2::new(0.0, 0.0),
            Vec2::new(10.0, 0.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(0.0, 10.0),
        );
        assert!(!q.contains(Vec2::new(15.0, 5.0)));
    }

    #[test]
    fn contains_trapezoid() {
        // 梯形（透视投影的常见形状）
        let q = Quad2D::new(
            Vec2::new(2.0, 0.0),
            Vec2::new(8.0, 0.0),
            Vec2::new(10.0, 10.0),
            Vec2::new(0.0, 10.0),
        );
        assert!(q.contains(Vec2::new(5.0, 5.0)));
        assert!(!q.contains(Vec2::new(0.0, 0.0))); // 在梯形左上方之外
    }

    #[test]
    fn to_path_has_four_points() {
        let q = Quad2D::new(
            Vec2::new(0.0, 0.0),
            Vec2::new(1.0, 0.0),
            Vec2::new(1.0, 1.0),
            Vec2::new(0.0, 1.0),
        );
        let path = q.to_path();
        assert_eq!(path.len(), 4);
    }

    #[test]
    fn from_points() {
        let pts = vec![
            Vec2::new(5.0, 10.0),
            Vec2::new(20.0, 3.0),
            Vec2::new(15.0, 25.0),
            Vec2::new(1.0, 8.0),
        ];
        let q = Quad2D::from_points(&pts);
        let b = q.bounds();
        assert!((b.x - 1.0).abs() < 1e-10);
        assert!((b.y - 3.0).abs() < 1e-10);
        assert!((b.w - 19.0).abs() < 1e-10);
        assert!((b.h - 22.0).abs() < 1e-10);
    }

    // ── Vec2 运算符与向量方法 ──

    #[test]
    fn vec2_add_sub_mul() {
        let a = Vec2::new(1.0, 2.0);
        let b = Vec2::new(10.0, 20.0);
        assert_eq!(a + b, Vec2::new(11.0, 22.0));
        assert_eq!(b - a, Vec2::new(9.0, 18.0));
        assert_eq!(a * 2.0, Vec2::new(2.0, 4.0));
        assert_eq!(2.0 * a, Vec2::new(2.0, 4.0));
    }

    #[test]
    fn vec2_dot_length_normalized() {
        let a = Vec2::new(3.0, 4.0);
        assert!((a.length() - 5.0).abs() < 1e-6);
        assert!((a.dot(&Vec2::new(1.0, 0.0)) - 3.0).abs() < 1e-10);
        let n = a.normalized();
        assert!((n.length() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn vec2_normalized_zero_is_safe() {
        let n = Vec2::zero().normalized();
        assert_eq!(n, Vec2::zero());
    }

    #[test]
    fn vec2_from_tuple() {
        let v: Vec2 = (1.5, 2.5).into();
        assert!((v.x - 1.5).abs() < 1e-10);
        assert!((v.y - 2.5).abs() < 1e-10);
    }
}
