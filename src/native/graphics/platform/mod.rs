//! Shared platform surface descriptors for graphics API modules.

#[cfg(windows)]
pub mod windows;

#[cfg(all(unix, not(target_os = "macos")))]
pub mod linux;

#[cfg(all(unix, not(target_os = "macos")))]
pub use linux::WaylandSurfaceHandle;
