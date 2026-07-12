//! Peer GPU API implementations (`IGraphicsContext`).
//!
//! OS-specific surface binding lives under each API's `platform/` submodule or
//! shared helpers in `platform/`. Factory/registry code is the only upstream
//! consumer of these modules (#164).

#[cfg(feature = "d3d11")]
pub mod d3d11;
#[cfg(feature = "d3d12")]
pub mod d3d12;
#[cfg(feature = "metal")]
pub mod metal;
#[cfg(feature = "opengles")]
pub mod opengl;
pub mod platform;
#[cfg(feature = "vulkan")]
pub mod vulkan;
