//! 物理单位盒子（PhysicalBox）和 AABB3D 统一转换接口（IntoAABB3D）。
//!
//! PhysicalBox 是带物理单位的 3D 盒子，通过 IntoAABB3D 统一转换为 AABB3D（dip 空间）。

use super::aabb3d::AABB3D;
use super::unit::PhysicalUnit;

/// 带物理单位的 3D 盒子。
///
/// 允许用不同单位描述各轴尺寸：
/// ```rust
/// use uix_app::draw::geometry::spatial::{PhysicalBox, PhysicalUnitExt};
/// let pb = PhysicalBox::new(10.0.mm(), 5.0.mm(), 0.0.mm(), 5.0.cm(), 3.0.cm(), 1.0.mm());
/// ```
#[derive(Debug, Clone, Copy)]
pub struct PhysicalBox {
    /// 盒子最小角的 X 轴坐标。
    pub x: PhysicalUnit,
    /// 盒子最小角的 Y 轴坐标。
    pub y: PhysicalUnit,
    /// 盒子最小角的 Z 轴坐标。
    pub z: PhysicalUnit,
    /// 盒子沿 X 轴的宽度。
    pub w: PhysicalUnit,
    /// 盒子沿 Y 轴的高度。
    pub h: PhysicalUnit,
    /// 盒子沿 Z 轴的深度。
    pub d: PhysicalUnit,
}

impl PhysicalBox {
    /// 完整 3D 盒子构造。
    #[inline(always)]
    pub fn new(
        x: PhysicalUnit,
        y: PhysicalUnit,
        z: PhysicalUnit,
        w: PhysicalUnit,
        h: PhysicalUnit,
        d: PhysicalUnit,
    ) -> Self {
        Self { x, y, z, w, h, d }
    }

    /// 2D 矩形快捷构造（z=0, d=0）。
    #[inline(always)]
    pub fn new_2d(x: PhysicalUnit, y: PhysicalUnit, w: PhysicalUnit, h: PhysicalUnit) -> Self {
        Self {
            x,
            y,
            z: PhysicalUnit::Mm(0.0),
            w,
            h,
            d: PhysicalUnit::Mm(0.0),
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════

/// 统一 3D AABB 转换接口。
///
/// 允许 `fill_rect` 等方法统一接受 `Rect`、`AABB3D`、`PhysicalBox`。
pub trait IntoAABB3D {
    /// 转换为 AABB3D（以 dip 为单位）。
    #[allow(clippy::wrong_self_convention)]
    fn into_aabb(&self, dpi: f32, dpr: f32) -> AABB3D;
}

/// 从 `crate::core::Rect` 转换（纯 2D 逻辑像素）。
impl IntoAABB3D for crate::core::Rect {
    fn into_aabb(&self, _dpi: f32, _dpr: f32) -> AABB3D {
        AABB3D::new(
            super::vec3::Vec3::new(self.x, self.y, 0.0),
            super::vec3::Vec3::new(self.x + self.w, self.y + self.h, 0.0),
        )
    }
}

/// 从 AABB3D 直通（无转换）。
impl IntoAABB3D for AABB3D {
    fn into_aabb(&self, _dpi: f32, _dpr: f32) -> AABB3D {
        *self
    }
}

/// 从 PhysicalBox 转换（物理单位 → dip → AABB3D）。
impl IntoAABB3D for PhysicalBox {
    fn into_aabb(&self, dpi: f32, dpr: f32) -> AABB3D {
        AABB3D::new(
            super::vec3::Vec3::new(
                self.x.to_px(dpi, dpr),
                self.y.to_px(dpi, dpr),
                self.z.to_px(dpi, dpr),
            ),
            super::vec3::Vec3::new(
                (self.x + self.w).to_px(dpi, dpr),
                (self.y + self.h).to_px(dpi, dpr),
                (self.z + self.d).to_px(dpi, dpr),
            ),
        )
    }
}

// ════════════════════════════════════════════════════════════════════════════
