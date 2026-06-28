//! 空间坐标系统——通用 3D 空间核心。
//!
//! 提供 4×4 矩阵、3D 向量、物理单位、空间上下文等基础类型，
//! 支撑 2D UI（零成本路径）和 3D 场景（通用路径）的统一坐标体系。
//!
//! 本模块是纯数学 + 空间管理层，不耦合任何渲染方式。

pub mod aabb3d;
pub mod context;
pub mod mat4;
pub mod physical_box;
pub mod platform_adapter;
pub mod quad2d;
pub mod ray3d;
pub mod unit;
pub mod vec3;

pub use aabb3d::AABB3D;
pub use context::SpatialContext;
pub use mat4::Mat4;
pub use physical_box::{IntoAABB3D, PhysicalBox};
pub use platform_adapter::Orientation;
pub use quad2d::{Quad2D, Vec2};
pub use ray3d::Ray3D;
pub use unit::{AngleExt, PhysicalUnit, PhysicalUnitExt};
pub use vec3::{Vec3, Vec4};
