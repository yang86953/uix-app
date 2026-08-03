//! GPU graphics contexts.
//!
//! 生产渲染使用原生 D3D11（Windows）；其余 per-API 实现保留为
//! test-only，直到其低层回归 fixture 退役。

#[cfg(feature = "d3d11")]
pub mod d3d11;
#[cfg(all(test, feature = "d3d12"))]
pub mod d3d12;
#[cfg(all(test, feature = "metal"))]
pub mod metal;
#[cfg(all(test, feature = "opengles"))]
pub mod opengl;
pub mod platform;
#[cfg(all(feature = "vulkan", any(test, windows)))]
pub mod vulkan;
