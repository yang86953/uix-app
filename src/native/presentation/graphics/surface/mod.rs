//! Shared platform surface descriptors for graphics API modules.

#[cfg(windows)]
pub(crate) mod windows;

#[cfg(target_os = "linux")]
pub(crate) mod linux;
