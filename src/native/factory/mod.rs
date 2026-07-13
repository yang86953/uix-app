//! 平台工厂函数 — #[cfg] 只在此处与 backends/ 内。

pub(crate) mod registry;
#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) mod registry_linux;
#[cfg(target_os = "macos")]
pub(crate) mod registry_macos;
#[cfg(windows)]
pub(crate) mod registry_windows;
pub(crate) mod thread_bound;

use crate::core::error::{Errc, Error};
use crate::native::traits::platform::Platform;
use crate::native::traits::present::{GraphicsBackend, IGraphicsContext};
#[cfg(any(windows, all(unix, not(target_os = "macos"))))]
use crate::native::traits::system::ISystemInfo;
use std::ffi::c_void;

pub use registry::{
    describe_backend_availability, graphics_runtime_platform, BackendStatus, GraphicsBackendEntry,
    GraphicsRecipe, active_entries, entry_for, entry_for_recipe, gpu_probe_candidates,
    gpu_recipe_candidates, try_create_gpu_recipe,
};

#[cfg(feature = "d3d11")]
pub(crate) fn create_d3d11_warp_test_context(
    surface: *mut std::ffi::c_void,
    width: i32,
    height: i32,
) -> crate::core::Result<Box<dyn crate::native::traits::present::IGraphicsContext>> {
    crate::native::graphics::d3d11::create_warp_test_context(surface, width, height)
        .map(thread_bound::bind_to_current_thread)
}

#[cfg(feature = "d3d11")]
pub(crate) fn d3d11_warp_test_context_available() -> bool {
    crate::native::graphics::d3d11::warp_test_context_available()
}

#[cfg(feature = "d3d12")]
pub(crate) fn create_d3d12_warp_test_context(
    surface: *mut std::ffi::c_void,
    width: i32,
    height: i32,
) -> crate::core::Result<Box<dyn crate::native::traits::present::IGraphicsContext>> {
    crate::native::graphics::d3d12::create_warp_test_context(surface, width, height)
        .map(thread_bound::bind_to_current_thread)
}

#[cfg(feature = "d3d12")]
pub(crate) fn d3d12_warp_test_context_available() -> bool {
    crate::native::graphics::d3d12::warp_test_context_available()
}

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

#[cfg(target_os = "macos")]
pub fn create_platform() -> Result<Box<dyn Platform>, Error> {
    Ok(Box::new(
        crate::native::backends::macos::platform::MacosPlatform::new(),
    ))
}

#[cfg(not(any(windows, unix)))]
pub fn create_platform() -> Result<Box<dyn Platform>, Error> {
    Err(Error::new(
        Errc::PlatformError,
        unsupported_platform_message(),
    ))
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn unsupported_platform_message() -> String {
    "Unsupported platform: only Windows, Linux, and macOS are supported".to_string()
}

/// 创建 GPU 图形上下文，指定单个 API；无 probe 循环。
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn create_gpu_context_with_backend(
    native_surface: *mut c_void,
    width: i32,
    height: i32,
    requested: GraphicsBackend,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    if requested == GraphicsBackend::Auto {
        return Err(Error::new(
            Errc::PlatformError,
            "create_gpu_context: Auto requires draw::bootstrap_graphics_engine (sole probe loop)",
        ));
    }
    registry::try_create_gpu_context(requested, native_surface, width, height)
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

#[cfg(target_os = "macos")]
pub fn available_memory_bytes() -> u64 {
    512 * 1024 * 1024
}

#[cfg(not(any(windows, all(unix, not(target_os = "macos")), target_os = "macos")))]
pub fn available_memory_bytes() -> u64 {
    512 * 1024 * 1024
}

