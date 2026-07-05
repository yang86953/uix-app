//! 空间坐标系统——通用 3D 空间核心。
//!
//! 提供 4×4 矩阵、3D 向量、物理单位、空间上下文等基础类型，
//! 支撑 2D UI（零成本路径）和 3D 场景（通用路径）的统一坐标体系。
//!
//! 本模块是纯数学 + 空间管理层，不耦合任何渲染方式。
//!
//! # 坐标系约定
//!
//! | 约定项 | 取值 | 说明 |
//! |--------|------|------|
//! | 原点 | 左上角 | 屏幕坐标原点 |
//! | y 轴方向 | 向下 | 默认 [`Orientation::YDown`]；y-up 平台由适配层翻转 |
//! | 2D 单位 | dip（设备无关像素） | [`PhysicalUnit`] 经 `to_dip(dpi)` 归一化 |
//! | 物理像素 | dip × device_pixel_ratio | [`SpatialContext`] 在写入 Canvas2D 时统一乘 dpr |
//! | 3D 世界单位 | 米（m） | [`PhysicalUnit::M`]；经投影落到 dip/物理像素 |
//! | z 轴 | 指向观察者（出屏） | 正交投影默认 z ∈ [-1, 1] |
//!
//! # 坐标空间流转
//!
//! ```text
//!   模型空间(model)  ──current_matrix──▶  世界空间(world)
//!        │                                   │
//!        │              view_matrix          │
//!        ▼                                   ▼
//!   相机空间(view)  ──projection_matrix──▶  裁剪空间(clip)
//!        │                                   │
//!        │         齐次除法 + 视口映射        │
//!        ▼                                   ▼
//!   屏幕空间(screen, dip)  ──× dpr──▶  设备像素空间(device)
//! ```
//!
//! - **2D UI**：模型=世界（identity view），正交投影使 dip≈screen，零成本路径。
//! - **3D 场景**：完整 MVP 链路，透视投影下 dip 需经投影计算。
//! - 单位换算见 [`unit`] 模块；坐标方向见 [`platform_adapter`]。

pub mod aabb3d;
pub mod context;
pub mod dirty_region;
pub mod mat4;
pub mod physical_box;
pub mod platform_adapter;
pub mod quad2d;
pub mod ray3d;
pub mod unit;
pub mod vec3;

pub use aabb3d::AABB3D;
pub use context::SpatialContext;
pub use dirty_region::DirtyRegion3D;
pub use mat4::Mat4;
pub use physical_box::{IntoAABB3D, PhysicalBox};
pub use platform_adapter::Orientation;
pub use quad2d::{Quad2D, Vec2};
pub use ray3d::Ray3D;
pub use unit::{AngleExt, PhysicalUnit, PhysicalUnitExt};
pub use vec3::{Vec3, Vec4};
