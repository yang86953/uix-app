//! 物理单位盒子（PhysicalBox）和 AABB3D 统一转换接口（IntoAABB3D）。
//!
//! PhysicalBox 是带物理单位的 3D 盒子，通过 IntoAABB3D 统一转换为 AABB3D（dip 空间）。

use super::aabb3d::AABB3D;
use super::unit::PhysicalUnit;

/// 带物理单位的 3D 盒子。
///
/// 允许用不同单位描述各轴尺寸：
/// ```rust
/// use uix::render::spatial::{PhysicalBox, PhysicalUnitExt};
/// let pb = PhysicalBox::new(10.0.mm(), 5.0.mm(), 0.0.mm(), 5.0.cm(), 3.0.cm(), 1.0.mm());
/// ```
#[derive(Debug, Clone, Copy)]
pub struct PhysicalBox {
    pub x: PhysicalUnit,
    pub y: PhysicalUnit,
    pub z: PhysicalUnit,
    pub w: PhysicalUnit,
    pub h: PhysicalUnit,
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
/// 允许 `fill_rect` 等方法同时接受 `Rect`（向后兼容）、`AABB3D`、`PhysicalBox`。
pub trait IntoAABB3D {
    /// 转换为 AABB3D（以 dip 为单位）。
    #[allow(clippy::wrong_self_convention)]
    fn into_aabb(&self, dpi: f32, dpr: f32) -> AABB3D;
}

/// 从 `crate::platform::Rect` 转换（向后兼容，纯 2D）。
impl IntoAABB3D for crate::platform::Rect {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::spatial::PhysicalUnitExt;

    #[test]
    fn physical_box_new() {
        let pb = PhysicalBox::new(10.mm(), 5.mm(), 0.mm(), 5.cm(), 3.cm(), 1.mm());
        assert!(matches!(pb.x, PhysicalUnit::Mm(v) if (v - 10.0).abs() < 1e-10));
    }

    #[test]
    fn physical_box_new_2d() {
        let pb = PhysicalBox::new_2d(10.mm(), 5.mm(), 5.cm(), 3.cm());
        assert!(matches!(pb.z, PhysicalUnit::Mm(v) if v.abs() < 1e-10));
        assert!(matches!(pb.d, PhysicalUnit::Mm(v) if v.abs() < 1e-10));
    }

    #[test]
    fn rect_into_aabb() {
        let rect = crate::platform::Rect::new(10.0, 20.0, 100.0, 50.0);
        let aabb = rect.into_aabb(96.0, 1.0);
        assert!((aabb.min.x - 10.0).abs() < 1e-10);
        assert!((aabb.min.y - 20.0).abs() < 1e-10);
        assert!((aabb.min.z).abs() < 1e-10);
        assert!((aabb.max.x - 110.0).abs() < 1e-10);
        assert!((aabb.max.y - 70.0).abs() < 1e-10);
    }

    #[test]
    fn aabb3d_into_aabb_identity() {
        let aabb = AABB3D::new(
            super::super::vec3::Vec3::new(1.0, 2.0, 3.0),
            super::super::vec3::Vec3::new(4.0, 5.0, 6.0),
        );
        let result = aabb.into_aabb(96.0, 1.0);
        assert!((result.min.x - 1.0).abs() < 1e-10);
        assert!((result.max.x - 4.0).abs() < 1e-10);
    }

    #[test]
    fn physical_box_into_aabb() {
        let pb = PhysicalBox::new_2d(10.mm(), 5.mm(), 5.cm(), 3.cm());
        // 10mm @ 96dpi = 37.8px, 5mm = 18.9px
        // 5cm = 50mm = 189px, 3cm = 30mm = 113.4px
        let aabb = pb.into_aabb(96.0, 1.0);
        assert!((aabb.min.x - 37.795_276).abs() < 1.0);
        assert!((aabb.min.y - 18.897_638).abs() < 0.5);
        assert!((aabb.max.x - (37.795_276 + 188.976_38)).abs() < 2.0);
    }
}
