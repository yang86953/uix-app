//! GPU graphics contexts.
//!
//! Production uses one shared wgpu renderer. The former per-API implementations
//! remain test-only until their low-level regression fixtures are retired.

#[cfg(all(test, feature = "d3d11"))]
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
pub(crate) mod wgpu_backend;
