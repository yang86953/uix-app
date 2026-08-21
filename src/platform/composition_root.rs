//! Platform System 唯一原生组合根。
//!
//! 本叶只负责消费中立启动输入，选择当前目标的具体实现并转移对象所有权。

use crate::core::Result;
#[cfg(not(any(windows, unix)))]
use crate::core::{Errc, Error};
use crate::platform::platform::Platform;

use super::composition::PendingNativeOptions;

/// 为当前编译目标创建并拥有唯一的平台聚合。
#[cfg(windows)]
pub(crate) fn create_platform_with_pending(
    options: PendingNativeOptions,
) -> Result<Box<dyn Platform>> {
    let pending_failures = options.into_pending_failures();
    Ok(Box::new(
        crate::native::backends::windows::platform::WindowsPlatform::new_with_pending(
            pending_failures,
        ),
    ))
}

/// 为当前编译目标创建并拥有唯一的平台聚合。
#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) fn create_platform_with_pending(
    options: PendingNativeOptions,
) -> Result<Box<dyn Platform>> {
    let pending_failures = options.into_pending_failures();
    let platform = crate::native::backends::linux::platform::LinuxPlatform::new(pending_failures)?;
    Ok(Box::new(platform))
}

/// 为当前编译目标创建并拥有唯一的平台聚合。
#[cfg(target_os = "macos")]
pub(crate) fn create_platform_with_pending(
    options: PendingNativeOptions,
) -> Result<Box<dyn Platform>> {
    let pending_failures = options.into_pending_failures();
    Ok(Box::new(
        crate::native::backends::macos::platform::MacosPlatform::new(pending_failures),
    ))
}

/// 对尚未适配的编译目标返回稳定的平台错误。
#[cfg(not(any(windows, unix)))]
pub(crate) fn create_platform_with_pending(
    options: PendingNativeOptions,
) -> Result<Box<dyn Platform>> {
    let _pending_failures = options.into_pending_failures();
    Err(Error::new(
        Errc::PlatformError,
        unsupported_platform_message(),
    ))
}

// 跨平台不支持分支保留统一错误文案，供目标矩阵静态核对。
#[cfg_attr(any(windows, unix), allow(dead_code))]
fn unsupported_platform_message() -> String {
    "Unsupported platform: only Windows, Linux, and macOS are supported".to_string()
}
