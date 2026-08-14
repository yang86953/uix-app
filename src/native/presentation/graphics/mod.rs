//! GPU graphics contexts.
//!
//! 默认生产渲染使用原生 D3D11（Windows）；启用 `opengles` feature 时，
//! WGL/EGL OpenGL ES adapter 也进入生产 registry，并复用同一套薄 RHI。
//! D3D12、Vulkan 与 Metal 仍受各自 feature/target 门控，尚未完成同等测试。

#[cfg(feature = "d3d11")]
pub(crate) mod d3d11;
#[cfg(all(test, feature = "d3d12"))]
pub(crate) mod d3d12;
#[cfg(all(test, feature = "metal"))]
pub(crate) mod metal;
#[cfg(feature = "opengles")]
pub(crate) mod opengl;
pub(crate) mod platform;
#[cfg(all(feature = "vulkan", any(test, windows)))]
pub(crate) mod vulkan;
