// Auto-organized test modules. Tests live only under src/tests.

#[cfg(feature = "d3d11")]
mod d3d11;
#[cfg(feature = "d3d12")]
mod d3d12;
#[cfg(feature = "opengles")]
mod opengl;
mod platform;
#[cfg(feature = "vulkan")]
mod vulkan;
