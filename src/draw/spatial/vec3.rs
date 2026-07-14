//! 3D 向量类型：Vec3、Vec4。
//!
//! 2D 向量/点请使用 [`super::quad2d::Vec2`]（唯一定义，本模块不再重复）。
//! 提供基本的向量运算（加减、点积、叉积、缩放、归一化）。
//! 所有运算使用 f32，无动态分配。

use std::ops::{Add, Mul, Sub};

// ════════════════════════════════════════════════════════════════════════════
// Vec3 — 3D 点/向量
// ════════════════════════════════════════════════════════════════════════════

/// 3D 向量/点。
///
/// 默认处于**模型空间**（model space）；变换链路中的具体空间由调用方约定，
/// 由 [`crate::draw::spatial::SpatialContext`] 的 MVP 矩阵统一推进。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Vec3 {
    #[inline(always)]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    #[inline(always)]
    pub const fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    /// 点积
    #[inline(always)]
    pub fn dot(&self, rhs: &Self) -> f32 {
        self.x * rhs.x + self.y * rhs.y + self.z * rhs.z
    }

    /// 叉积
    #[inline(always)]
    pub fn cross(&self, rhs: &Self) -> Self {
        Self {
            x: self.y * rhs.z - self.z * rhs.y,
            y: self.z * rhs.x - self.x * rhs.z,
            z: self.x * rhs.y - self.y * rhs.x,
        }
    }

    /// 长度
    #[inline(always)]
    pub fn length(&self) -> f32 {
        self.dot(self).sqrt()
    }

    /// 归一化
    #[inline(always)]
    pub fn normalized(&self) -> Self {
        let l = self.length();
        if l > 1e-10 {
            Self::new(self.x / l, self.y / l, self.z / l)
        } else {
            *self
        }
    }

    /// 逐分量相乘
    #[inline(always)]
    pub fn component_mul(&self, rhs: &Self) -> Self {
        Self::new(self.x * rhs.x, self.y * rhs.y, self.z * rhs.z)
    }

    /// 从 Vec4 截取（丢弃 w）
    #[inline(always)]
    pub fn from_vec4(v: &Vec4) -> Self {
        Self::new(v.x, v.y, v.z)
    }
}

impl Add for Vec3 {
    type Output = Self;
    #[inline(always)]
    fn add(self, rhs: Self) -> Self {
        Self::new(self.x + rhs.x, self.y + rhs.y, self.z + rhs.z)
    }
}

impl Sub for Vec3 {
    type Output = Self;
    #[inline(always)]
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.x - rhs.x, self.y - rhs.y, self.z - rhs.z)
    }
}

impl Mul<f32> for Vec3 {
    type Output = Self;
    #[inline(always)]
    fn mul(self, s: f32) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }
}

impl Mul<Vec3> for f32 {
    type Output = Vec3;
    #[inline(always)]
    fn mul(self, v: Vec3) -> Vec3 {
        Vec3::new(self * v.x, self * v.y, self * v.z)
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Vec4 — 齐次坐标向量
// ════════════════════════════════════════════════════════════════════════════

/// 4D 齐次坐标向量。
///
/// 用于 4×4 矩阵乘法中的中间表示。
/// `w=1` 表示点，`w=0` 表示方向向量。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vec4 {
    #[inline(always)]
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    /// 从 Vec3 + w 构造
    #[inline(always)]
    pub fn from_vec3(v: &Vec3, w: f32) -> Self {
        Self::new(v.x, v.y, v.z, w)
    }

    /// 转为 Vec3（丢弃 w）
    #[inline(always)]
    pub fn to_vec3(&self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }

    /// 齐次除法：w 归一化后转为 Vec3
    ///
    /// 当 w 不为 0 且不为 1 时做齐次除法，否则直接返回 xyz。
    #[inline(always)]
    pub fn to_vec3_homogeneous(&self) -> Vec3 {
        if self.w != 0.0 && (self.w - 1.0).abs() > 1e-10 {
            Vec3::new(self.x / self.w, self.y / self.w, self.z / self.w)
        } else {
            self.to_vec3()
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
