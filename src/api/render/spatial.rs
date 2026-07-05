//! 空间坐标协议 — 物理单位、变换矩阵与脏区域。

pub use crate::render::spatial::unit::AngleExt;
pub use crate::render::spatial::{
    DirtyRegion3D, IntoAABB3D, Mat4, Orientation, PhysicalBox, PhysicalUnit, PhysicalUnitExt,
    Quad2D, Ray3D, SpatialContext, Vec2, Vec3, Vec4, AABB3D,
};
