//! 平台工厂函数 — #[cfg] 只在此处与 backends/ 内。

mod registry;
#[cfg(all(unix, not(target_os = "macos")))]
mod registry_linux;
#[cfg(target_os = "macos")]
mod registry_macos;
#[cfg(windows)]
mod registry_windows;

use crate::core::error::{Errc, Error};
use crate::native::traits::platform::Platform;
use crate::native::traits::present::{GraphicsBackend, IGraphicsContext};
#[cfg(any(windows, all(unix, not(target_os = "macos"))))]
use crate::native::traits::system::ISystemInfo;
use std::ffi::c_void;

pub use registry::{
    active_entries, entry_for, gpu_probe_candidates, try_create_context, try_create_gpu_context,
    BackendStatus, GraphicsBackendEntry,
};

#[cfg(all(test, feature = "d3d12"))]
pub(crate) fn create_d3d12_warp_test_context(
    surface: *mut std::ffi::c_void,
    width: i32,
    height: i32,
) -> crate::core::Result<Box<dyn crate::native::traits::present::IGraphicsContext>> {
    crate::native::graphics::d3d12::create_warp_test_context(surface, width, height)
}

#[cfg(all(test, feature = "d3d12"))]
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
fn unsupported_platform_message() -> String {
    "Unsupported platform: only Windows, Linux, and macOS are supported".to_string()
}

/// 创建 GPU 图形上下文（单条目；`Auto` 须用 `draw::bootstrap_graphics_engine`）。
pub fn create_gpu_context(
    native_surface: *mut c_void,
    width: i32,
    height: i32,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    create_gpu_context_with_backend(native_surface, width, height, GraphicsBackend::Auto)
}

/// 创建 GPU 图形上下文，指定单个 API；无 probe 循环。
pub fn create_gpu_context_with_backend(
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
    try_create_gpu_context(requested, native_surface, width, height)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native::traits::present::GraphicsBackend;

    #[test]
    fn unsupported_platform_message_tracks_platform_boundary() {
        let message = unsupported_platform_message();

        assert!(message.contains("only Windows, Linux, and macOS"));
    }

    #[test]
    fn create_gpu_context_rejects_auto_without_probe_loop() {
        let err = match create_gpu_context(std::ptr::null_mut(), 1, 1) {
            Ok(_) => panic!("Auto must not probe inside factory"),
            Err(err) => err,
        };
        assert!(err.message().contains("bootstrap_graphics_engine"));
    }

    #[test]
    fn create_gpu_context_with_backend_creates_single_entry() {
        let context =
            create_gpu_context_with_backend(std::ptr::null_mut(), 1, 1, GraphicsBackend::OpenGlEs);
        // Null surface fails context creation, but factory must not iterate candidates.
        assert!(context.is_err());
    }

    #[test]
    fn d3d12_explicit_request_reports_build_state_or_surface_error() {
        let err = match try_create_gpu_context(GraphicsBackend::D3d12, std::ptr::null_mut(), 1, 1) {
            Ok(_) => panic!("null surface must not create D3D12"),
            Err(err) => err,
        };

        #[cfg(all(windows, feature = "d3d12"))]
        assert!(err.message().contains("native window handle is null"));

        #[cfg(all(windows, not(feature = "d3d12")))]
        assert!(err.message().contains("disabled"));

        #[cfg(not(windows))]
        assert!(err.message().contains("no registry entry"));
    }

    #[cfg(all(windows, feature = "d3d12"))]
    #[test]
    fn d3d12_explicit_factory_creates_active_real_window_context() {
        use crate::native::traits::present::{NativeRasterCaps, PresentMode, RasterMode};

        let mut platform = crate::native::create_platform().expect("platform");
        let mut window = platform
            .window_manager()
            .create_window("D3D12 factory test", 120, 80)
            .expect("window");
        let surface = window.native_surface_ptr();
        let mut context = try_create_gpu_context(GraphicsBackend::D3d12, surface, 120, 80)
            .expect("active D3D12 factory row");
        assert_eq!(context.graphics_backend(), GraphicsBackend::D3d12);
        assert_eq!(context.caps().raster, RasterMode::GpuNative);
        assert_eq!(context.caps().present, PresentMode::Swapchain);
        assert_eq!(
            context.native_raster_caps(),
            NativeRasterCaps {
                clear_target: true,
                soft_blit: true,
                solid_rects: true,
                ..NativeRasterCaps::default()
            }
        );
        context.shutdown();
        drop(context);
        window.close().expect("close factory test window");
    }

    #[test]
    fn vulkan_candidate_is_real_linux_backend_or_platform_specific_error() {
        let err = match try_create_gpu_context(GraphicsBackend::Vulkan, std::ptr::null_mut(), 1, 1)
        {
            Ok(_) => panic!("null surface should not create a Vulkan context"),
            Err(err) => err,
        };

        #[cfg(all(unix, not(target_os = "macos")))]
        assert!(err.message().contains("WaylandSurfaceHandle"));

        #[cfg(not(all(unix, not(target_os = "macos"))))]
        assert!(
            err.message().contains("no registry entry")
                || err.message().contains("only supported on Linux Wayland")
        );
    }
}
