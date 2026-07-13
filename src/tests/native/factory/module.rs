use crate::tests::common::*;
use crate::native::factory::*;

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

    #[cfg(windows)]
    assert!(
        err.message().contains("HWND")
            || err.message().contains("native_surface")
            || err.message().contains("load Vulkan")
            || err.message().contains("vkCreate")
            || err.message().contains("no registry entry")
            || err.message().contains("disabled in this build"),
        "unexpected Windows Vulkan null-surface error: {}",
        err.message()
    );

    #[cfg(target_os = "macos")]
    assert!(
        err.message().contains("CAMetalLayer")
            || err.message().contains("load Vulkan")
            || err.message().contains("vkCreate")
            || err.message().contains("no registry entry")
            || err.message().contains("disabled in this build")
            || err.message().contains("portability"),
        "unexpected macOS Vulkan null-surface error: {}",
        err.message()
    );

    #[cfg(not(any(
        all(unix, not(target_os = "macos")),
        windows,
        target_os = "macos"
    )))]
    assert!(
        err.message().contains("planned but not implemented")
            || err.message().contains("no registry entry")
            || err.message().contains("not supported on this platform")
    );
}
