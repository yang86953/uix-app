//! 平台实现 — 唯一允许 #[cfg(windows/unix)] 的目录。

#[cfg(windows)]
pub(crate) mod windows;

#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) mod linux;

#[cfg(target_os = "macos")]
pub(crate) mod macos;
