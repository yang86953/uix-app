//! 平台系统 — OS 抽象（Win32 / Wayland）。

pub mod api;
pub mod services;
pub mod log;
pub mod presenter;
pub mod shared;
pub mod diagnostic;

#[cfg(windows)]
pub mod windows;

#[cfg(all(unix, not(target_os = "macos")))]
pub mod linux;

pub use api::*;
pub use services::file_service;
pub use services::notification;
pub use services::settings;

/// 创建当前平台对应的 Platform 实例。
#[cfg(windows)]
pub fn create_platform() -> Result<Box<dyn Platform>, Error> {
    Ok(Box::new(windows::platform::WindowsPlatform::new()))
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn create_platform() -> Result<Box<dyn Platform>, Error> {
    let platform = linux::platform::LinuxPlatform::new()?;
    Ok(Box::new(platform))
}

#[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
pub fn create_platform() -> Result<Box<dyn Platform>, Error> {
    compile_error!("Unsupported platform: only Windows and Linux are supported");
}

/// 创建 GPU 图形上下文（IGraphicsContext）。
#[cfg(all(unix, not(target_os = "macos")))]
pub fn create_gpu_context(
    native_surface: *mut std::ffi::c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    let egl = linux::gpu::egl::EglContext::new(native_surface, width, height)?;
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
    linux::system_info::LinuxSystemInfo::new()
        .memory_info()
        .available_bytes
}

#[cfg(windows)]
pub fn available_memory_bytes() -> u64 {
    windows::system_info::WindowsSystemInfo::new()
        .memory_info()
        .available_bytes
}

#[cfg(not(any(windows, all(unix, not(target_os = "macos")))))]
pub fn available_memory_bytes() -> u64 {
    512 * 1024 * 1024
}

pub mod test_harness;
