//! Shared platform surface descriptors for graphics API modules.

#[cfg(windows)]
pub(crate) mod windows;

#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) mod linux;
