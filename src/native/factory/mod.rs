//! 平台工厂函数 — #[cfg] 只在此处与 backends/ 内。

pub(crate) mod registry;
#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) mod registry_linux;
#[cfg(target_os = "macos")]
pub(crate) mod registry_macos;
#[cfg(windows)]
pub(crate) mod registry_windows;
pub(crate) mod thread_bound;

use crate::core::error::Error;
// 仅不受支持的平台构建需要统一的平台错误码。
#[cfg(not(any(windows, unix)))]
use crate::core::error::Errc;
use crate::diagnostics::PendingFailureQueue;
#[cfg(any(windows, all(unix, not(target_os = "macos"))))]
use crate::native::capabilities::system::ISystemInfo;
use crate::native::platform::Platform;

pub(crate) use registry::try_create_gpu_recipe_with_queue;
pub use registry::{
    describe_backend_availability, gpu_recipe_candidates, graphics_runtime_platform, GraphicsRecipe,
};

// 测试目标保留 D3D11 WARP 工厂入口，供显式后端矩阵按需调用。
#[cfg_attr(test, allow(dead_code))]
#[cfg(all(test, feature = "d3d11"))]
pub(crate) fn create_d3d11_warp_test_context(
    surface: *mut std::ffi::c_void,
    width: i32,
    height: i32,
) -> crate::core::Result<Box<dyn crate::native::present::IGraphicsContext>> {
    crate::native::presentation::graphics::d3d11::create_warp_test_context(surface, width, height)
        .map(thread_bound::bind_to_current_thread)
}

// 测试目标保留 D3D11 WARP 可用性探测，供显式后端矩阵按需调用。
#[cfg_attr(test, allow(dead_code))]
#[cfg(all(test, feature = "d3d11"))]
pub(crate) fn d3d11_warp_test_context_available() -> bool {
    crate::native::presentation::graphics::d3d11::warp_test_context_available()
}

#[cfg(all(test, feature = "d3d12"))]
pub(crate) fn create_d3d12_warp_test_context(
    surface: *mut std::ffi::c_void,
    width: i32,
    height: i32,
) -> crate::core::Result<Box<dyn crate::native::present::IGraphicsContext>> {
    crate::native::presentation::graphics::d3d12::create_warp_test_context(surface, width, height)
        .map(thread_bound::bind_to_current_thread)
}

#[cfg(all(test, feature = "d3d12"))]
pub(crate) fn d3d12_warp_test_context_available() -> bool {
    crate::native::presentation::graphics::d3d12::warp_test_context_available()
}

#[cfg(windows)]
pub(crate) fn create_platform_with_pending(
    pending_failures: PendingFailureQueue,
) -> Result<Box<dyn Platform>, Error> {
    Ok(Box::new(
        crate::native::backends::windows::platform::WindowsPlatform::new_with_pending(
            pending_failures,
        ),
    ))
}

#[cfg(all(unix, not(target_os = "macos")))]
pub(crate) fn create_platform_with_pending(
    pending_failures: PendingFailureQueue,
) -> Result<Box<dyn Platform>, Error> {
    let platform = crate::native::backends::linux::platform::LinuxPlatform::new(pending_failures)?;
    Ok(Box::new(platform))
}

#[cfg(target_os = "macos")]
pub(crate) fn create_platform_with_pending(
    pending_failures: PendingFailureQueue,
) -> Result<Box<dyn Platform>, Error> {
    Ok(Box::new(
        crate::native::backends::macos::platform::MacosPlatform::new(pending_failures),
    ))
}

#[cfg(not(any(windows, unix)))]
pub(crate) fn create_platform_with_pending(
    _pending_failures: PendingFailureQueue,
) -> Result<Box<dyn Platform>, Error> {
    Err(Error::new(
        Errc::PlatformError,
        unsupported_platform_message(),
    ))
}

// 跨平台不支持分支保留统一错误文案，供兼容工厂和目标矩阵按需调用。
#[allow(dead_code)]
pub(crate) fn unsupported_platform_message() -> String {
    "Unsupported platform: only Windows, Linux, and macOS are supported".to_string()
}

/// 探测系统可用空闲内存（字节）。
// 保留平台诊断查询入口，默认应用路径不直接依赖它。
#[allow(dead_code)]
#[cfg(all(unix, not(target_os = "macos")))]
pub fn available_memory_bytes() -> u64 {
    crate::native::backends::linux::system_info::LinuxSystemInfo::new()
        .memory_info()
        .map(|info| info.available_bytes)
        .unwrap_or(0)
}

// 保留平台诊断查询入口，默认应用路径不直接依赖它。
#[allow(dead_code)]
#[cfg(windows)]
pub fn available_memory_bytes() -> u64 {
    crate::native::backends::windows::system_info::WindowsSystemInfo::new()
        .memory_info()
        .map(|info| info.available_bytes)
        .unwrap_or(0)
}

// 保留平台诊断查询入口，默认应用路径不直接依赖它。
#[allow(dead_code)]
#[cfg(target_os = "macos")]
pub fn available_memory_bytes() -> u64 {
    512 * 1024 * 1024
}

// 保留平台诊断查询入口，默认应用路径不直接依赖它。
#[allow(dead_code)]
#[cfg(not(any(windows, all(unix, not(target_os = "macos")), target_os = "macos")))]
pub fn available_memory_bytes() -> u64 {
    512 * 1024 * 1024
}
