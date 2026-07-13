//! 平台工厂函数 — #[cfg] 只在此处与 backends/ 内。

mod registry;
#[cfg(all(unix, not(target_os = "macos")))]
mod registry_linux;
#[cfg(target_os = "macos")]
mod registry_macos;
#[cfg(windows)]
mod registry_windows;
mod thread_bound;

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

#[cfg(all(test, feature = "d3d11"))]
pub(crate) fn create_d3d11_warp_test_context(
    surface: *mut std::ffi::c_void,
    width: i32,
    height: i32,
) -> crate::core::Result<Box<dyn crate::native::traits::present::IGraphicsContext>> {
    crate::native::graphics::d3d11::create_warp_test_context(surface, width, height)
        .map(thread_bound::bind_to_current_thread)
}

#[cfg(all(test, feature = "d3d11"))]
pub(crate) fn d3d11_warp_test_context_available() -> bool {
    crate::native::graphics::d3d11::warp_test_context_available()
}

#[cfg(all(test, feature = "d3d12"))]
pub(crate) fn create_d3d12_warp_test_context(
    surface: *mut std::ffi::c_void,
    width: i32,
    height: i32,
) -> crate::core::Result<Box<dyn crate::native::traits::present::IGraphicsContext>> {
    crate::native::graphics::d3d12::create_warp_test_context(surface, width, height)
        .map(thread_bound::bind_to_current_thread)
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
    fn create_gpu_context_with_backend_creates_single_entry() {
        let context =
            create_gpu_context_with_backend(std::ptr::null_mut(), 1, 1, GraphicsBackend::OpenGlEs);
        // Null surface fails context creation, but factory must not iterate candidates.
        assert!(context.is_err());
    }

    #[test]
    fn d3d12_explicit_request_reports_build_state_or_surface_error() {
        let err = match registry::try_create_gpu_context(
            GraphicsBackend::D3d12,
            std::ptr::null_mut(),
            1,
            1,
        ) {
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

    #[cfg(all(windows, feature = "d3d11"))]
    #[test]
    fn d3d11_warp_test_factory_returns_a_gpu_native_context() {
        use crate::native::traits::present::{NativeRasterCaps, PresentMode, RasterMode};

        assert!(d3d11_warp_test_context_available());
        let mut platform = crate::native::create_platform().expect("platform");
        let mut window = platform
            .window_manager()
            .create_window("D3D11 WARP test factory", 120, 80)
            .expect("window");
        let mut context = create_d3d11_warp_test_context(window.native_surface_ptr(), 120, 80)
            .expect("D3D11 WARP context");

        assert_eq!(context.graphics_backend(), GraphicsBackend::D3d11);
        assert_eq!(context.caps().raster, RasterMode::GpuNative);
        assert_eq!(context.caps().present, PresentMode::Swapchain);
        assert_eq!(context.native_raster_caps(), NativeRasterCaps::d3d11_full());

        context
            .try_shutdown()
            .expect("thread-bound D3D11 WARP shutdown");
        window.close().expect("close WARP test window");
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
        let mut context =
            registry::try_create_gpu_context(GraphicsBackend::D3d12, surface, 120, 80)
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
        let _ = context.try_shutdown();
        drop(context);
        window.close().expect("close factory test window");
    }

    #[test]
    fn vulkan_candidate_is_real_linux_backend_or_platform_specific_error() {
        let err = match registry::try_create_gpu_context(
            GraphicsBackend::Vulkan,
            std::ptr::null_mut(),
            1,
            1,
        ) {
            Ok(_) => panic!("null surface should not create a Vulkan context"),
            Err(err) => err,
        };

        #[cfg(all(unix, not(target_os = "macos")))]
        assert!(err.message().contains("WaylandSurfaceHandle"));

        #[cfg(not(all(unix, not(target_os = "macos"))))]
        assert!(
            err.message().contains("planned but not implemented")
                || err.message().contains("WSI adapter is not implemented on Windows")
                || err.message().contains("portability adapter is not implemented on macOS")
                || err.message().contains("no registry entry")
                || err.message().contains("only supported on Linux Wayland")
        );
    }
}
