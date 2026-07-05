//! 平台工厂函数 — #[cfg] 只在此处与 backends/ 内。

use crate::core::error::{Errc, Error};
use crate::native::traits::platform::Platform;
use crate::native::traits::present::IGraphicsContext;
use crate::native::traits::system::ISystemInfo;

/// 创建当前平台对应的 Platform 实例。
#[cfg(windows)]
pub fn create_platform() -> Result<Box<dyn Platform>, Error> {
    Ok(Box::new(
        crate::native::backends::windows::platform::WindowsPlatform::new(),
    ))
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn create_platform() -> Result<Box<dyn Platform>, Error> {
    let platform = crate::native::backends::linux::platform::LinuxPlatform::new()?;
    Ok(Box::new(platform))
}

#[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
pub fn create_platform() -> Result<Box<dyn Platform>, Error> {
    compile_error!("Unsupported platform: only Windows and Linux are supported");
}

/// 创建 GPU 图形上下文。
#[cfg(all(unix, not(target_os = "macos")))]
pub fn create_gpu_context(
    native_surface: *mut std::ffi::c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    let egl = crate::native::backends::linux::gpu::egl::EglContext::new(
        native_surface,
        width,
        height,
    )?;
    Ok(Box::new(egl))
}

#[cfg(not(all(unix, not(target_os = "macos"))))]
pub fn create_gpu_context(
    _native_surface: *mut std::ffi::c_void,
    _width: i32,
    _height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    Err(Error::new(
        Errc::PlatformError,
        "GPU rendering is not supported on this platform".to_string(),
    ))
}

/// 探测系统可用空闲内存（字节）。
#[cfg(all(unix, not(target_os = "macos")))]
pub fn available_memory_bytes() -> u64 {
    crate::native::backends::linux::system_info::LinuxSystemInfo::new()
        .memory_info()
        .available_bytes
}

#[cfg(windows)]
pub fn available_memory_bytes() -> u64 {
    crate::native::backends::windows::system_info::WindowsSystemInfo::new()
        .memory_info()
        .available_bytes
}

#[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
pub fn available_memory_bytes() -> u64 {
    512 * 1024 * 1024
}
