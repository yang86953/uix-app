//! 4×4 列主序矩阵（Mat4）。
//!
//! 所有 2D UI 变换（translate/rotate/scale）都是 3D 变换在 z=0 平面的特例。
//! 提供透视投影和正交投影构造器，支撑 2D（零成本路径）和 3D（通用路径）的统一坐标体系。
//!
//! 矩阵布局（列主序）：
//! ```text
//! m[0]  m[4]  m[8]  m[12]     第 0 列 = [m[0], m[1], m[2], m[3]]
//! m[1]  m[5]  m[9]  m[13]     第 1 列 = [m[4], m[5], m[6], m[7]]
//! m[2]  m[6]  m[10] m[14]     第 2 列 = [m[8], m[9], m[10], m[11]]
//! m[3]  m[7]  m[11] m[15]     第 3 列 = [m[12], m[13], m[14], m[15]]
//! ```
//!
//! 齐次坐标约定：
//! - 点变换：w = 1（平移、旋转、缩放、透视全生效）
//! - 方向向量变换：w = 0（平移不生效，旋转/缩放生效）

use super::vec3::{Vec3, Vec4};
use std::ops::Mul;

/// 4×4 列主序矩阵。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Mat4(pub [f32; 16]);

impl Mat4 {
    // ════════════════════════════════════════════════════════════════════
    // 构造器
    // ════════════════════════════════════════════════════════════════════

    /// 单位矩阵
    #[inline(always)]
    pub const fn identity() -> Self {
        Self([
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ])
    }

    /// 零矩阵
    #[inline(always)]
    pub const fn zero() -> Self {
        Self([0.0; 16])
    }

    /// 从 16 个元素的数组构造
    #[inline(always)]
    pub const fn from_array(m: [f32; 16]) -> Self {
        Self(m)
    }

    /// 是否为 identity（精确比较）
    #[inline(always)]
    pub fn is_identity(&self) -> bool {
        self.0
            == [
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ]
    }

    // ════════════════════════════════════════════════════════════════════
    // 3D 变换构造器
    // ════════════════════════════════════════════════════════════════════

    /// 3D 平移矩阵
    #[inline(always)]
    pub fn translate(x: f32, y: f32, z: f32) -> Self {
        Self([
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, x, y, z, 1.0,
        ])
    }

    /// 绕 X 轴旋转（俯仰），angle 为弧度
    #[inline(always)]
    pub fn rotate_x(angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self([
            1.0, 0.0, 0.0, 0.0, 0.0, c, s, 0.0, 0.0, -s, c, 0.0, 0.0, 0.0, 0.0, 1.0,
        ])
    }

    /// 绕 Y 轴旋转（偏航），angle 为弧度
    #[inline(always)]
    pub fn rotate_y(angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self([
            c, 0.0, -s, 0.0, 0.0, 1.0, 0.0, 0.0, s, 0.0, c, 0.0, 0.0, 0.0, 0.0, 1.0,
        ])
    }

    /// 绕 Z 轴旋转（2D 平面旋转），angle 为弧度
    #[inline(always)]
    pub fn rotate_z(angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        Self([
            c, s, 0.0, 0.0, -s, c, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
        ])
    }

    /// 绕任意轴旋转，axis 必须为单位向量，angle 为弧度
    pub fn rotate_axis(axis: Vec3, angle: f32) -> Self {
        let (s, c) = angle.sin_cos();
        let t = 1.0 - c;
        let (x, y, z) = (axis.x, axis.y, axis.z);
        Self([
            t * x * x + c,
            t * x * y + z * s,
            t * x * z - y * s,
            0.0,
            t * x * y - z * s,
            t * y * y + c,
            t * y * z + x * s,
            0.0,
            t * x * z + y * s,
            t * y * z - x * s,
            t * z * z + c,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
        ])
    }

    /// 3D 缩放矩阵
    #[inline(always)]
    pub fn scale(x: f32, y: f32, z: f32) -> Self {
        Self([
            x, 0.0, 0.0, 0.0, 0.0, y, 0.0, 0.0, 0.0, 0.0, z, 0.0, 0.0, 0.0, 0.0, 1.0,
        ])
    }

    // ════════════════════════════════════════════════════════════════════
    // 投影构造器
    // ════════════════════════════════════════════════════════════════════

    /// 透视投影矩阵。
    ///
    /// 使用对称视锥体（平截头体）。
    ///
    /// - `fov_y`：垂直视野（弧度）
    /// - `aspect`：宽高比（width / height）
    /// - `near`：近平面距离（必须 > 0）
    /// - `far`：远平面距离（必须 > near）
    pub fn perspective(fov_y: f32, aspect: f32, near: f32, far: f32) -> Self {
        let f = 1.0 / (fov_y * 0.5).tan();
        let range_inv = 1.0 / (near - far);
        Self([
            f / aspect,
            0.0,
            0.0,
            0.0,
            0.0,
            f,
            0.0,
            0.0,
            0.0,
            0.0,
            (near + far) * range_inv,
            -1.0,
            0.0,
            0.0,
            near * far * 2.0 * range_inv,
            0.0,
        ])
    }

    /// 正交投影矩阵（y-down，适合 2D UI）。
    ///
    /// - NDC 空间：x ∈ [-1, 1], y ∈ [-1, 1], z ∈ [-1, 1]
    /// - y-down：top < bottom 使 y 轴方向翻转
    pub fn orthographic(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Self {
        let rml = right - left;
        let tmb = top - bottom;
        let fmn = far - near;
        Self([
            2.0 / rml,
            0.0,
            0.0,
            0.0,
            0.0,
            2.0 / tmb,
            0.0,
            0.0,
            0.0,
            0.0,
            -2.0 / fmn,
            0.0,
            -(right + left) / rml,
            -(top + bottom) / tmb,
            -(far + near) / fmn,
            1.0,
        ])
    }

    /// 默认 2D UI 正交投影。
    ///
    /// 原点在左上角，y 向下，z 范围 [-1, 1]。
    /// left=0, right=width, top=0, bottom=height（y-down）。
    #[inline(always)]
    pub fn orthographic_2d(width: f32, height: f32) -> Self {
        Self::orthographic(0.0, width, height, 0.0, -1.0, 1.0)
    }

    // ════════════════════════════════════════════════════════════════════
    // 视图构造器
    // ════════════════════════════════════════════════════════════════════

    /// 视图矩阵（look-at）：从 `eye` 位置看向 `target` 方向。
    ///
    /// 使用右手系。
    pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Self {
        let f = (target - eye).normalized();
        let s = f.cross(&up).normalized();
        let u = s.cross(&f);
        Self([
            s.x,
            u.x,
            -f.x,
            0.0,
            s.y,
            u.y,
            -f.y,
            0.0,
            s.z,
            u.z,
            -f.z,
            0.0,
            -s.dot(&eye),
            -u.dot(&eye),
            f.dot(&eye),
            1.0,
        ])
    }

    // ════════════════════════════════════════════════════════════════════
    // 矩阵运算
    // ════════════════════════════════════════════════════════════════════

    /// 矩阵乘法：self × rhs。
    ///
    /// 列主序乘法：result[col][row] = Σ self[k][row] * rhs[col][k]
    pub fn mul(&self, rhs: &Self) -> Self {
        let a = &self.0;
        let b = &rhs.0;
        let mut r = [0.0f32; 16];
        for col in 0..4 {
            for row in 0..4 {
                r[col * 4 + row] = a[row] * b[col * 4]
                    + a[4 + row] * b[col * 4 + 1]
                    + a[2 * 4 + row] * b[col * 4 + 2]
                    + a[3 * 4 + row] * b[col * 4 + 3];
            }
        }
        Self(r)
    }

    /// 变换 4D 齐次向量。
    #[inline(always)]
    pub fn transform_vec4(&self, v: &Vec4) -> Vec4 {
        let m = &self.0;
        Vec4::new(
            m[0] * v.x + m[4] * v.y + m[8] * v.z + m[12] * v.w,
            m[1] * v.x + m[5] * v.y + m[9] * v.z + m[13] * v.w,
            m[2] * v.x + m[6] * v.y + m[10] * v.z + m[14] * v.w,
            m[3] * v.x + m[7] * v.y + m[11] * v.z + m[15] * v.w,
        )
    }

    /// 变换 3D 点（w=1，齐次除法）。
    ///
    /// 等效于 transform_vec4(Vec4::from_vec3(p, 1)).to_vec3_homogeneous()。
    #[inline(always)]
    pub fn transform_point(&self, p: &Vec3) -> Vec3 {
        let v = self.transform_vec4(&Vec4::from_vec3(p, 1.0));
        v.to_vec3_homogeneous()
    }

    /// 变换 3D 方向向量（w=0，平移不生效）。
    #[inline(always)]
    pub fn transform_direction(&self, d: &Vec3) -> Vec3 {
        Vec3::from_vec4(&self.transform_vec4(&Vec4::from_vec3(d, 0.0)))
    }

    /// 逆矩阵。
    ///
    /// 使用伴随矩阵法求逆。返回 None 当行列式接近 0（奇异矩阵）。
    pub fn inverse(&self) -> Option<Self> {
        let m = &self.0;

        // 2x2 子式（辅助计算）
        let s0 = m[0] * m[5] - m[1] * m[4];
        let s1 = m[0] * m[6] - m[2] * m[4];
        let s2 = m[0] * m[7] - m[3] * m[4];
        let s3 = m[1] * m[6] - m[2] * m[5];
        let s4 = m[1] * m[7] - m[3] * m[5];
        let s5 = m[2] * m[7] - m[3] * m[6];

        let c0 = m[8] * m[13] - m[9] * m[12];
        let c1 = m[8] * m[14] - m[10] * m[12];
        let c2 = m[8] * m[15] - m[11] * m[12];
        let c3 = m[9] * m[14] - m[10] * m[13];
        let c4 = m[9] * m[15] - m[11] * m[13];
        let c5 = m[10] * m[15] - m[11] * m[14];

        let det = s0 * c5 - s1 * c4 + s2 * c3 + s3 * c2 - s4 * c1 + s5 * c0;
        if det.abs() < 1e-15 {
            return None;
        }
        let inv_det = 1.0 / det;

        Some(Self([
            (m[5] * c5 - m[6] * c4 + m[7] * c3) * inv_det,
            (-m[1] * c5 + m[2] * c4 - m[3] * c3) * inv_det,
            (m[13] * s5 - m[14] * s4 + m[15] * s3) * inv_det,
            (-m[9] * s5 + m[10] * s4 - m[11] * s3) * inv_det,
            (-m[4] * c5 + m[6] * c2 - m[7] * c1) * inv_det,
            (m[0] * c5 - m[2] * c2 + m[3] * c1) * inv_det,
            (-m[12] * s5 + m[14] * s2 - m[15] * s1) * inv_det,
            (m[8] * s5 - m[10] * s2 + m[11] * s1) * inv_det,
            (m[4] * c4 - m[5] * c2 + m[7] * c0) * inv_det,
            (-m[0] * c4 + m[1] * c2 - m[3] * c0) * inv_det,
            (m[12] * s4 - m[13] * s2 + m[15] * s0) * inv_det,
            (-m[8] * s4 + m[9] * s2 - m[11] * s0) * inv_det,
            (-m[4] * c3 + m[5] * c1 - m[6] * c0) * inv_det,
            (m[0] * c3 - m[1] * c1 + m[2] * c0) * inv_det,
            (-m[12] * s3 + m[13] * s1 - m[14] * s0) * inv_det,
            (m[8] * s3 - m[9] * s1 + m[10] * s0) * inv_det,
        ]))
    }

    /// 转置矩阵
    pub fn transpose(&self) -> Self {
        let m = &self.0;
        Self([
            m[0], m[4], m[8], m[12], m[1], m[5], m[9], m[13], m[2], m[6], m[10], m[14], m[3], m[7],
            m[11], m[15],
        ])
    }

    // ════════════════════════════════════════════════════════════════════
    // 查询方法
    // ════════════════════════════════════════════════════════════════════

    /// 是否为正交投影矩阵。
    ///
    /// 检查投影矩阵的最后一行特征：
    /// 正交投影 = (0, 0, 0, 1)，透视投影 = (0, 0, -1, 0) 或类似。
    #[inline(always)]
    pub fn is_orthographic(&self) -> bool {
        (self.0[3]).abs() < 1e-6 && (self.0[7]).abs() < 1e-6 && (self.0[15] - 1.0).abs() < 1e-6
    }

    /// 当前矩阵是否仅包含 2D 变换（不影响 x/y 渲染的 z 轴操作允许存在）。
    ///
    /// 检查 z 轴相关分量是否会将 z 值泄露到 x/y 中：
    /// - m[2]  (col2 row0): z → x 的影响（旋转/缩放 z 到 x）
    /// - m[6]  (col2 row1): z → y 的影响
    /// - m[8]  (col0 row2): x → z 的影响（不影响 x/y 渲染，但标识 3D 旋转存在）
    /// - m[9]  (col1 row2): y → z 的影响
    /// - m[14] (col3 row2): w → z 的影响（平移 z）
    ///
    /// 不检查 m[10] (z→z) 和 m[11] (w→z)：
    /// z 轴自身的缩放/平移不影响 x/y 屏幕位置。
    #[inline(always)]
    pub fn is_2d_only(&self) -> bool {
        (self.0[2]).abs() < 1e-6   // z not affecting x
            && (self.0[6]).abs() < 1e-6   // z not affecting y
            && (self.0[8]).abs() < 1e-6   // x not affecting z (no x rotation)
            && (self.0[9]).abs() < 1e-6   // y not affecting z (no y rotation)
            && (self.0[14]).abs() < 1e-6 // no z-affecting translation
    }

    /// 提取 2D 仿射分量（如果矩阵是 2D only）。
    ///
    /// 返回 `(a, b, tx, c, d, ty)`，对应 3×2 仿射矩阵：
    /// ```text
    /// [a  b  tx]
    /// [c  d  ty]
    /// ```
    pub fn to_affine_2d(&self) -> Option<(f32, f32, f32, f32, f32, f32)> {
        if self.is_2d_only() {
            Some((
                self.0[0], self.0[4], self.0[12], self.0[1], self.0[5], self.0[13],
            ))
        } else {
            None
        }
    }

    /// 转换为 2D 仿射 [`crate::draw::Transform`]。
    ///
    /// 仅当矩阵为 2D-only（无 z 轴旋转/平移，非透视）时成功，
    /// 否则返回 `None`——Fail-Fast，不静默降级丢弃 3D 信息。
    ///
    /// 内存布局与 [`Mat4::from`](`From<crate::draw::Transform>`) 及光栅器对
    /// `Transform.m = [a, b, tx, c, d, ty]` 的索引完全一致，可安全往返：
    /// `Transform → Mat4 → Transform` 等值。
    pub fn to_transform(&self) -> Option<crate::draw::Transform> {
        let (a, b, tx, c, d, ty) = self.to_affine_2d()?;
        Some(crate::draw::Transform {
            m: [a, b, tx, c, d, ty],
        })
    }

    /// 是否为透视投影
    #[inline(always)]
    pub fn is_perspective(&self) -> bool {
        !self.is_orthographic()
    }
}

// ════════════════════════════════════════════════════════════════════════════

impl Default for Mat4 {
    #[inline(always)]
    fn default() -> Self {
        Self::identity()
    }
}

impl Mul for Mat4 {
    type Output = Self;
    #[inline(always)]
    fn mul(self, rhs: Self) -> Self {
        Mat4::mul(&self, &rhs)
    }
}

// ════════════════════════════════════════════════════════════════════════════

/// 从现有 `crate::Transform`（3×2 仿射）转换为 4×4 Mat4。
///
/// 将 3×2 矩阵嵌入到 4×4 的 z=0 平面（不丢失任何信息）：
/// ```text
/// Transform.m = [a, b, tx, c, d, ty]   (第 0 行: a, b, tx; 第 1 行: c, d, ty)
/// ↓
/// 4×4 = [a, c, 0, 0,  b, d, 0, 0,  0, 0, 1, 0,  tx, ty, 0, 1]
/// ```
impl From<crate::draw::Transform> for Mat4 {
    fn from(t: crate::draw::Transform) -> Self {
        Self([
            t.m[0], t.m[3], 0.0, 0.0, t.m[1], t.m[4], 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, t.m[2], t.m[5],
            0.0, 1.0,
        ])
    }
}

// ════════════════════════════════════════════════════════════════════════════

#[cfg(test)]
#[path = "../../tests/draw/spatial/mat4.rs"]
mod tests;

