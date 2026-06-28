//! 空间坐标系统——通用 3D 空间核心。
//!
//! 提供 4×4 矩阵、3D 向量、空间上下文等基础类型，
//! 支撑 2D UI（零成本路径）和 3D 场景（通用路径）的统一坐标体系。
//!
//! 本模块是纯数学层，不耦合任何渲染方式。

pub mod mat4;
pub mod vec3;

pub use mat4::Mat4;
pub use vec3::{Vec2, Vec3, Vec4};
