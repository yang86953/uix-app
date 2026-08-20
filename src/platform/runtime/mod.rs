//! Platform System 的实例生命周期与 owner-thread 运行时边界。

// Linux 使用进程线程组规则识别主线程。
#[cfg(target_os = "linux")]
mod linux;
// macOS 使用 pthread 主线程谓词。
#[cfg(target_os = "macos")]
mod macos;
// 其他目标返回明确的未实现错误。
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
mod unsupported;
// Windows 在 owner thread 上配对管理 COM apartment。
#[cfg(windows)]
mod windows;

// 只向 Platform 根门面公开当前目标的生命周期实现。
#[cfg(target_os = "linux")]
pub(crate) use linux::*;
// 只向 Platform 根门面公开当前目标的生命周期实现。
#[cfg(target_os = "macos")]
pub(crate) use macos::*;
// 只向 Platform 根门面公开当前目标的生命周期实现。
#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub(crate) use unsupported::*;
// 只向 Platform 根门面公开当前目标的生命周期实现。
#[cfg(windows)]
pub(crate) use windows::*;
