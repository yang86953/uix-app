//! 平台实现 — 唯一允许 #[cfg(windows/linux)] 的目录。

#[cfg(windows)]
pub(crate) mod windows;

#[cfg(target_os = "linux")]
pub(crate) mod linux;

#[cfg(target_os = "macos")]
pub(crate) mod macos;
