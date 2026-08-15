#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "macos")]
mod macos;
// Unix 通知 Provider 的可执行文件探测由 Linux 与 macOS Adapter 复用。
#[cfg(any(target_os = "linux", target_os = "macos"))]
mod unix_provider;
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
mod unsupported;
#[cfg(windows)]
mod windows;

#[cfg(target_os = "linux")]
pub(crate) use linux::*;
#[cfg(target_os = "macos")]
pub(crate) use macos::*;
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub(crate) use unsupported::*;
#[cfg(windows)]
pub(crate) use windows::*;
