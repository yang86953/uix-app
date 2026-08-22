//! GPU graphics contexts.
//!
//! Vulkan 是三平台优先生产 API；D3D11 与 OpenGL ES 作为兼容候选，继续
//! 复用 `platform::presentation` 的同一套图形契约。D3D12 仅显式 feature 编译且
//! 尚未注册为生产候选，Metal 暂缓。

#[cfg(feature = "d3d11")]
pub(crate) mod d3d11;
#[cfg(feature = "d3d12")]
pub(crate) mod d3d12;
#[cfg(all(test, feature = "metal"))]
pub(crate) mod metal;
#[cfg(feature = "opengles")]
pub(crate) mod opengl;
// 保留 `graphics::platform` 逻辑路径，原生 surface 物理归入 surface。
#[path = "surface/mod.rs"]
pub(crate) mod platform;
#[cfg(feature = "vulkan")]
pub(crate) mod vulkan;
