#[cfg(windows)]
use crate::native::graphics::platform::windows as win_surface;
use crate::native::graphics::vulkan::platform::adapter::VulkanAdapterInfo;
use crate::native::graphics::vulkan::platform::context::*;
use crate::native::graphics::vulkan::platform::surface::{
    choose_composite_alpha, choose_surface_format,
};
use crate::tests::common::*;
#[cfg(windows)]
use crate::tests::native::gfx_r5::expected_gfx_r5_vendor;
use ash::vk;
#[cfg(windows)]
use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessHandleCount};

#[cfg(windows)]
fn current_process_handle_count() -> u32 {
    let mut count = 0;
    unsafe {
        // SAFETY: pseudo handle 属于当前进程，输出指针指向有效的局部 u32。
        GetProcessHandleCount(GetCurrentProcess(), &mut count).expect("GetProcessHandleCount");
    }
    count
}

#[cfg(windows)]
fn requested_vulkan_soak_duration() -> std::time::Duration {
    let seconds = std::env::var("UIX_VULKAN_SOAK_SECONDS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(30)
        .clamp(1, 3_600);
    std::time::Duration::from_secs(seconds)
}

#[test]
fn vulkan_adapter_diagnostic_summary_is_stable() {
    let info = VulkanAdapterInfo {
        description: "Example GPU".to_string(),
        device_type: "discrete_gpu",
        vendor_id: 0x10DE,
        device_id: 0x2705,
        api_version: vk::make_api_version(0, 1, 3, 281),
        driver_version: 0x0102_0304,
        queue_family_index: 7,
    };

    let summary = info.diagnostic_summary();
    assert!(summary.contains("adapter=\"Example GPU\""));
    assert!(summary.contains("type=discrete_gpu"));
    assert!(summary.contains("vendor=0x10DE"));
    assert!(summary.contains("device=0x2705"));
    assert!(summary.contains("api=1.3.281"));
    assert!(summary.contains("driver=0x01020304"));
    assert!(summary.contains("queue_family=7"));
}

#[test]
fn vulkan_split_sources_retain_every_desktop_wsi_contract() {
    let (surface, adapter) = platform_contract_sources();
    assert!(surface.contains("create_win32_surface"));
    assert!(surface.contains("create_wayland_surface"));
    assert!(surface.contains("create_metal_surface"));
    assert!(surface.contains("portability_enumeration"));
    assert!(adapter.contains("portability_subset"));
}

#[test]
fn crop_cpu_shadow_extracts_rect() {
    let shadow = vec![1, 2, 3, 4, 5, 6];
    let cropped = crop_cpu_shadow(&shadow, 3, 2, 1, 0, 2, 2).expect("crop");
    assert_eq!(cropped, vec![2, 3, 5, 6]);
}

#[test]
fn crop_cpu_shadow_rejects_out_of_bounds() {
    let shadow = vec![0; 4];
    let err = crop_cpu_shadow(&shadow, 2, 2, 1, 1, 2, 1).expect_err("oob");
    assert_eq!(err.code(), Errc::InvalidArgument);
}

#[test]
fn choose_surface_format_prefers_bgra_srgb() {
    let formats = [
        vk::SurfaceFormatKHR {
            format: vk::Format::R8G8B8A8_UNORM,
            color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
        },
        vk::SurfaceFormatKHR {
            format: vk::Format::B8G8R8A8_UNORM,
            color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
        },
    ];
    assert!(choose_surface_format(&formats).format == vk::Format::B8G8R8A8_UNORM);
}

#[test]
fn swapchain_inventory_rejects_empty_driver_results() {
    let format = vk::SurfaceFormatKHR {
        format: vk::Format::B8G8R8A8_UNORM,
        color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
    };
    let mode = vk::PresentModeKHR::FIFO;

    let error = validate_swapchain_support(&[], &[mode]).expect_err("empty formats");
    assert_eq!(error.code(), Errc::PlatformError);
    assert!(error.message().contains("no swapchain formats"));

    let error = validate_swapchain_support(&[format], &[]).expect_err("empty present modes");
    assert_eq!(error.code(), Errc::PlatformError);
    assert!(error.message().contains("no present modes"));

    let error = validate_swapchain_images(&[]).expect_err("empty images");
    assert_eq!(error.code(), Errc::PlatformError);
    assert!(error.message().contains("no images"));

    assert!(validate_swapchain_support(&[format], &[mode]).is_ok());
    assert!(validate_swapchain_images(&[vk::Image::null()]).is_ok());
}

#[test]
fn staging_size_is_full_rgba_frame() {
    assert_eq!(staging_size(4, 3), 48);
    assert_eq!(staging_size(0, 0), 4);
}

#[test]
fn cpu_shadow_allocation_is_typed_and_initialized() {
    assert_eq!(allocate_cpu_shadow(3).expect("small shadow"), vec![0; 3]);

    let error = allocate_cpu_shadow(usize::MAX).expect_err("capacity overflow");
    assert_eq!(error.code(), Errc::GraphicsOutOfMemory);
    assert!(error.message().contains("CPU readback shadow allocation"));
}

#[test]
fn vulkan_present_statuses_are_typed_graphics_failures() {
    assert_eq!(
        vk_err("vkQueuePresentKHR", vk::Result::ERROR_OUT_OF_DATE_KHR).code(),
        Errc::GraphicsSurfaceLost
    );
    assert_eq!(
        vk_err("vkQueuePresentKHR", vk::Result::SUBOPTIMAL_KHR).code(),
        Errc::GraphicsSurfaceLost
    );
    assert_eq!(
        vk_err("vkQueuePresentKHR", vk::Result::ERROR_DEVICE_LOST).code(),
        Errc::GraphicsDeviceLost
    );
    assert_eq!(
        vk_err("vkQueuePresentKHR", vk::Result::ERROR_OUT_OF_DEVICE_MEMORY).code(),
        Errc::GraphicsOutOfMemory
    );
}

#[test]
fn failed_submit_keeps_the_submit_code_and_appends_fence_recovery_failure() {
    let error = failed_submit_error(
        vk::Result::ERROR_DEVICE_LOST,
        Err(Error::new(
            Errc::GraphicsOutOfMemory,
            "replacement fence allocation failed",
        )),
    );

    assert_eq!(error.code(), Errc::GraphicsDeviceLost);
    assert!(error.message().contains("vkQueueSubmit"));
    let recovery = error
        .source_error()
        .expect("fence recovery failure must remain in the cause chain");
    assert_eq!(recovery.code(), Errc::GraphicsOutOfMemory);
    assert_eq!(recovery.message(), "replacement fence allocation failed");

    let recovered = failed_submit_error(vk::Result::ERROR_DEVICE_LOST, Ok(()));
    assert_eq!(recovered.code(), Errc::GraphicsDeviceLost);
    assert!(recovered.source_error().is_none());
}

#[test]
fn fatal_swapchain_recreate_failure_is_not_hidden_by_surface_status() {
    let surface = vk_err("vkQueuePresentKHR", vk::Result::ERROR_OUT_OF_DATE_KHR);
    let device_lost = vk_err(
        "vkDeviceWaitIdle before swapchain recreate",
        vk::Result::ERROR_DEVICE_LOST,
    );
    let error = merge_surface_recreate_failure(surface.clone(), device_lost);

    assert_eq!(error.code(), Errc::GraphicsDeviceLost);
    assert!(error.message().contains("vkDeviceWaitIdle"));
    assert_eq!(error.source_error(), Some(&surface));

    let out_of_memory = vk_err(
        "vkCreateSwapchainKHR",
        vk::Result::ERROR_OUT_OF_DEVICE_MEMORY,
    );
    let error = merge_surface_recreate_failure(surface.clone(), out_of_memory);
    assert_eq!(error.code(), Errc::GraphicsOutOfMemory);
    assert_eq!(error.source_error(), Some(&surface));

    let ordinary = Error::new(Errc::PlatformError, "swapchain format query failed");
    let error = merge_surface_recreate_failure(surface.clone(), ordinary.clone());
    assert_eq!(error.code(), Errc::GraphicsSurfaceLost);
    assert_eq!(error.source_error(), Some(&ordinary));
}

#[test]
fn vulkan_drawable_adapter_reuses_the_shared_windows_contract() {
    assert!(drawable_contract_source()
        .contains("crate::native::graphics::platform::windows::drawable_size"));
}

#[test]
fn vulkan_shutdown_releases_lost_device_but_keeps_other_wait_failures_typed() {
    assert!(accept_device_wait_for_shutdown(Ok(())).is_ok());
    assert!(accept_device_wait_for_shutdown(Err(vk::Result::ERROR_DEVICE_LOST)).is_ok());

    let error = accept_device_wait_for_shutdown(Err(vk::Result::ERROR_OUT_OF_DEVICE_MEMORY))
        .expect_err("out-of-memory wait must remain observable");
    assert_eq!(error.code(), Errc::GraphicsOutOfMemory);
    assert!(error.message().contains("vkDeviceWaitIdle during shutdown"));
}

#[test]
fn composite_alpha_prefers_opaque_but_uses_a_supported_fallback() {
    assert!(choose_composite_alpha(
        vk::CompositeAlphaFlagsKHR::OPAQUE | vk::CompositeAlphaFlagsKHR::INHERIT
    )
    .is_some_and(|mode| mode == vk::CompositeAlphaFlagsKHR::OPAQUE));
    assert!(
        choose_composite_alpha(vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED)
            .is_some_and(|mode| mode == vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED)
    );
    assert!(choose_composite_alpha(vk::CompositeAlphaFlagsKHR::empty()).is_none());
}

#[cfg(windows)]
#[test]
#[ignore = "requires a Vulkan-capable Windows driver"]
fn windows_vulkan_hardware_resize_readback_and_present() {
    let mut platform = crate::native::create_platform().expect("platform");
    let mut window = platform
        .window_manager()
        .create_window("Vulkan hardware matrix", 128, 96)
        .expect("window");
    let surface = window.native_surface_ptr();
    assert!(!surface.is_null(), "Windows HWND must be available");
    window.show().expect("show window");
    let _ = platform.event_loop().poll_event(&|_| true);

    let mut context = VulkanContext::new(surface, 128, 96).expect("VulkanContext");
    let initial_drawable = win_surface::drawable_size(surface, 128, 96);
    assert_eq!(context.graphics_backend(), GraphicsBackend::Vulkan);
    assert_eq!(context.caps().present, PresentMode::PixelUpload);
    assert_eq!(
        (context.width(), context.height()),
        (initial_drawable.width, initial_drawable.height)
    );
    assert!(
        (context.device_pixel_ratio()
            - initial_drawable.width as f32 / initial_drawable.logical_width as f32)
            .abs()
            < f32::EPSILON
    );
    assert!(!context.adapter_info.description.trim().is_empty());
    assert_ne!(context.adapter_info.vendor_id, 0);
    assert_ne!(context.adapter_info.device_id, 0);
    assert!(matches!(
        context.adapter_info.device_type,
        "discrete_gpu" | "integrated_gpu" | "virtual_gpu"
    ));
    println!(
        "Vulkan hardware adapter: {}",
        context.adapter_info.diagnostic_summary()
    );

    let first_size = (context.width(), context.height());
    let first_pixels = vec![0xFF12_3456; (first_size.0 * first_size.1) as usize];
    context
        .present_pixels(
            &first_pixels,
            first_size.0,
            first_size.1,
            PresentDamage::Full,
        )
        .expect("first Vulkan present");
    assert_eq!(
        context.read_pixels(3, 4, 2, 2).expect("first readback"),
        vec![0xFF12_3456; 4]
    );
    let mismatch = context
        .present_pixels(
            &first_pixels,
            first_size.0 - 1,
            first_size.1,
            PresentDamage::Full,
        )
        .expect_err("physical upload mismatch must not be reinterpreted as logical resize");
    assert_eq!(mismatch.code(), Errc::GraphicsSurfaceLost);
    assert_eq!((context.width(), context.height()), first_size);

    window
        .properties_mut()
        .set_size(192, 128)
        .expect("resize HWND");
    let _ = platform.event_loop().poll_event(&|_| true);
    let drawable = win_surface::drawable_size(surface, 192, 128);
    context
        .resize(drawable.logical_width, drawable.logical_height)
        .expect("recreate Vulkan swapchain");
    let resized = (context.width(), context.height());
    assert_ne!(resized, first_size);
    assert_eq!(resized, (drawable.width, drawable.height));

    let resized_pixels = vec![0xFF7A_4BC2; (resized.0 * resized.1) as usize];
    context
        .present_pixels(&resized_pixels, resized.0, resized.1, PresentDamage::Full)
        .expect("Vulkan present after resize");
    assert_eq!(
        context
            .read_pixels(resized.0 - 1, resized.1 - 1, 1, 1)
            .expect("far-corner readback"),
        vec![0xFF7A_4BC2]
    );

    context.try_shutdown().expect("shutdown");
    window.close().expect("close window");
}

#[cfg(windows)]
#[test]
#[ignore = "requires a Vulkan-capable Windows driver and UIX_GFX_R5_EXPECT_VENDOR=nvidia|amd|intel"]
fn windows_vulkan_gfx_r5_expected_vendor_resize_present_readback() {
    let expected = expected_gfx_r5_vendor().unwrap_or_else(|error| panic!("{error}"));
    let mut platform = crate::native::create_platform().expect("platform");
    let mut window = platform
        .window_manager()
        .create_window("Vulkan GFX-R5 vendor matrix", 137, 103)
        .expect("window");
    let surface = window.native_surface_ptr();
    assert!(!surface.is_null(), "Windows HWND must be available");
    window.show().expect("show window");
    let _ = platform.event_loop().poll_event(&|_| true);

    let mut context = VulkanContext::new(surface, 137, 103).expect("VulkanContext");
    expected.assert_adapter(&context.adapter_info);

    let first_color = 0xFF34_78BC;
    let first_pixels = vec![first_color; (context.width() * context.height()) as usize];
    context
        .present_pixels(
            &first_pixels,
            context.width(),
            context.height(),
            PresentDamage::Full,
        )
        .expect("initial matrix present");
    assert_eq!(
        context
            .read_pixels(context.width() - 1, context.height() - 1, 1, 1)
            .expect("initial matrix readback"),
        vec![first_color]
    );

    window
        .properties_mut()
        .set_size(211, 149)
        .expect("resize matrix HWND");
    let _ = platform.event_loop().poll_event(&|_| true);
    let drawable = win_surface::drawable_size(surface, 211, 149);
    context
        .resize(drawable.logical_width, drawable.logical_height)
        .expect("resize matrix Vulkan surface");
    let second_color = 0xFF9A_5C21;
    let second_pixels = vec![second_color; (context.width() * context.height()) as usize];
    context
        .present_pixels(
            &second_pixels,
            context.width(),
            context.height(),
            PresentDamage::Full,
        )
        .expect("resized matrix present");
    assert_eq!(
        context
            .read_pixels(context.width() - 1, context.height() - 1, 1, 1)
            .expect("resized matrix readback"),
        vec![second_color]
    );

    println!(
        "GFX-R5 Vulkan vendor evidence: expected={}; {}",
        expected.label(),
        context.adapter_info.diagnostic_summary()
    );
    context.try_shutdown().expect("shutdown matrix context");
    window.close().expect("close matrix window");
}

#[cfg(windows)]
#[test]
#[ignore = "requires a Vulkan-capable Windows driver and UIX_GFX_R5_EXPECT_VENDOR=nvidia|amd|intel"]
fn windows_vulkan_gfx_r5_native_out_of_date_is_typed_and_recovers() {
    let expected = expected_gfx_r5_vendor().unwrap_or_else(|error| panic!("{error}"));
    let mut platform = crate::native::create_platform().expect("platform");
    let mut window = platform
        .window_manager()
        .create_window("Vulkan GFX-R5 native surface fault", 139, 107)
        .expect("window");
    let surface = window.native_surface_ptr();
    assert!(!surface.is_null(), "Windows HWND must be available");
    window.show().expect("show window");
    let _ = platform.event_loop().poll_event(&|_| true);

    let mut context = VulkanContext::new(surface, 139, 107).expect("VulkanContext");
    expected.assert_adapter(&context.adapter_info);
    let initial_extent = (context.width(), context.height());
    let initial_pixels = vec![0xFF31_5A9C; (initial_extent.0 * initial_extent.1) as usize];
    context
        .present_pixels(
            &initial_pixels,
            initial_extent.0,
            initial_extent.1,
            PresentDamage::Full,
        )
        .expect("initial native-fault present");

    window
        .properties_mut()
        .set_size(223, 157)
        .expect("resize HWND without resizing Vulkan context");
    let resized_drawable = win_surface::drawable_size(surface, 223, 157);
    assert_ne!(
        (resized_drawable.width, resized_drawable.height),
        initial_extent
    );
    let fault = context
        .present_pixels(
            &initial_pixels,
            initial_extent.0,
            initial_extent.1,
            PresentDamage::Full,
        )
        .expect_err("stale native swapchain must report a typed surface fault");
    assert_eq!(fault.code(), Errc::GraphicsSurfaceLost);
    assert!(
        fault.message().contains("vkAcquireNextImageKHR")
            || fault.message().contains("vkQueuePresentKHR"),
        "fault must come from the native WSI path: {}",
        fault.short_what()
    );

    context
        .resize(
            resized_drawable.logical_width,
            resized_drawable.logical_height,
        )
        .expect("recover resized Vulkan swapchain");
    let recovered_extent = (context.width(), context.height());
    assert_eq!(
        recovered_extent,
        (resized_drawable.width, resized_drawable.height)
    );
    let recovered_color = 0xFFB7_642D;
    let recovered_pixels =
        vec![recovered_color; (recovered_extent.0 * recovered_extent.1) as usize];
    context
        .present_pixels(
            &recovered_pixels,
            recovered_extent.0,
            recovered_extent.1,
            PresentDamage::Full,
        )
        .expect("present after native surface recovery");
    assert_eq!(
        context
            .read_pixels(recovered_extent.0 - 1, recovered_extent.1 - 1, 1, 1)
            .expect("far-corner readback after native recovery"),
        vec![recovered_color]
    );

    println!(
        "GFX-R5 Vulkan native surface fault evidence: expected={}; fault={}; {}",
        expected.label(),
        fault.message(),
        context.adapter_info.diagnostic_summary()
    );
    context.try_shutdown().expect("shutdown fault context");
    window.close().expect("close fault window");
}

#[cfg(windows)]
#[test]
#[ignore = "requires a Vulkan-capable Windows driver and UIX_GFX_R5_EXPECT_VENDOR=nvidia|amd|intel"]
fn windows_vulkan_gfx_r5_destroyed_hwnd_returns_native_surface_lost() {
    let expected = expected_gfx_r5_vendor().unwrap_or_else(|error| panic!("{error}"));
    let mut platform = crate::native::create_platform().expect("platform");
    let mut window = platform
        .window_manager()
        .create_window("Vulkan GFX-R5 fatal native surface", 157, 119)
        .expect("window");
    let surface = window.native_surface_ptr();
    assert!(!surface.is_null(), "Windows HWND must be available");
    window.show().expect("show window");
    let _ = platform.event_loop().poll_event(&|_| true);

    let mut context = VulkanContext::new(surface, 157, 119).expect("VulkanContext");
    expected.assert_adapter(&context.adapter_info);
    let extent = (context.width(), context.height());
    let pixels = vec![0xFF5C_82B4; (extent.0 * extent.1) as usize];
    context
        .present_pixels(&pixels, extent.0, extent.1, PresentDamage::Full)
        .expect("initial fatal-surface present");

    // 故意绕开正常的“先释放图形、后销毁 HWND”顺序，以取得驱动返回的真实 fatal surface 错误。
    window.close().expect("destroy HWND before Vulkan surface");
    let fault = context
        .present_pixels(&pixels, extent.0, extent.1, PresentDamage::Full)
        .expect_err("destroyed HWND must produce a native Vulkan surface fault");
    assert_eq!(fault.code(), Errc::GraphicsSurfaceLost);
    assert!(
        fault.message().contains("vkAcquireNextImageKHR")
            || fault.message().contains("vkQueuePresentKHR"),
        "fault must originate in native WSI present: {}",
        fault.what()
    );
    let root_cause = fault.root_cause();
    assert_eq!(root_cause.code(), Errc::GraphicsSurfaceLost);
    assert!(
        root_cause
            .message()
            .contains("vkGetPhysicalDeviceSurfaceCapabilitiesKHR"),
        "fatal root cause must come from native surface capabilities: {}",
        fault.what()
    );
    assert!(
        root_cause.message().contains("ERROR_SURFACE_LOST_KHR"),
        "destroyed HWND must retain the native fatal surface root cause: {}",
        fault.what()
    );

    println!(
        "GFX-R5 Vulkan fatal surface evidence: expected={}; fault={}; {}",
        expected.label(),
        fault.what(),
        context.adapter_info.diagnostic_summary()
    );
    context
        .try_shutdown()
        .expect("shutdown fatal-surface context");
}

#[cfg(windows)]
#[test]
#[ignore = "requires a Vulkan-capable Windows driver"]
fn windows_vulkan_two_surfaces_share_device_and_keep_independent_frames() {
    let mut platform = crate::native::create_platform().expect("platform");
    let mut first_window = platform
        .window_manager()
        .create_window("Vulkan shared device A", 96, 64)
        .expect("first window");
    let mut second_window = platform
        .window_manager()
        .create_window("Vulkan shared device B", 128, 72)
        .expect("second window");
    first_window.show().expect("show first window");
    second_window.show().expect("show second window");
    let _ = platform.event_loop().poll_event(&|_| true);

    let mut first =
        VulkanContext::new(first_window.native_surface_ptr(), 96, 64).expect("first VulkanContext");
    let mut second = VulkanContext::new(second_window.native_surface_ptr(), 128, 72)
        .expect("second VulkanContext");
    assert_ne!(first.shared_device_identity(), 0);
    assert_eq!(
        first.shared_device_identity(),
        second.shared_device_identity(),
        "compatible surfaces on one UI thread must reuse the logical device"
    );

    let first_color = 0xFFB0_2030;
    let second_color = 0xFF20_A050;
    let first_pixels = vec![first_color; (first.width() * first.height()) as usize];
    let second_pixels = vec![second_color; (second.width() * second.height()) as usize];
    first
        .present_pixels(
            &first_pixels,
            first.width(),
            first.height(),
            PresentDamage::Full,
        )
        .expect("present first surface");
    second
        .present_pixels(
            &second_pixels,
            second.width(),
            second.height(),
            PresentDamage::Full,
        )
        .expect("present second surface");
    assert_eq!(
        first.read_pixels(3, 3, 1, 1).expect("first readback"),
        vec![first_color]
    );
    assert_eq!(
        second.read_pixels(3, 3, 1, 1).expect("second readback"),
        vec![second_color]
    );

    first.try_shutdown().expect("shutdown first surface");
    drop(first);
    first_window.close().expect("close first window");

    let surviving_color = 0xFF30_6090;
    let surviving_pixels = vec![surviving_color; (second.width() * second.height()) as usize];
    second
        .present_pixels(
            &surviving_pixels,
            second.width(),
            second.height(),
            PresentDamage::Full,
        )
        .expect("shared device must survive first surface shutdown");
    assert_eq!(
        second
            .read_pixels(second.width() - 1, second.height() - 1, 1, 1)
            .expect("surviving surface readback"),
        vec![surviving_color]
    );

    second.try_shutdown().expect("shutdown second surface");
    drop(second);
    second_window.close().expect("close second window");
}

#[cfg(windows)]
#[test]
#[ignore = "requires a Vulkan-capable Windows driver"]
fn windows_vulkan_shared_device_loss_rejects_peers_and_new_context_replaces_it() {
    let mut platform = crate::native::create_platform().expect("platform");
    let mut first_window = platform
        .window_manager()
        .create_window("Vulkan shared loss A", 64, 48)
        .expect("first window");
    let mut second_window = platform
        .window_manager()
        .create_window("Vulkan shared loss B", 64, 48)
        .expect("second window");
    let mut replacement_window = platform
        .window_manager()
        .create_window("Vulkan shared loss replacement", 64, 48)
        .expect("replacement window");

    let mut first =
        VulkanContext::new(first_window.native_surface_ptr(), 64, 48).expect("first VulkanContext");
    let mut second = VulkanContext::new(second_window.native_surface_ptr(), 64, 48)
        .expect("second VulkanContext");
    let lost_identity = first.shared_device_identity();
    assert_eq!(lost_identity, second.shared_device_identity());

    first.mark_shared_device_lost_for_test();
    let pixels = vec![0xFF11_2233; (second.width() * second.height()) as usize];
    let error = second
        .present_pixels(
            &pixels,
            second.width(),
            second.height(),
            PresentDamage::Full,
        )
        .expect_err("a peer must observe shared device loss before submitting");
    assert_eq!(error.code(), Errc::GraphicsDeviceLost);
    assert!(
        error
            .source_error()
            .is_some_and(|source| source.message().contains("marked lost by test")),
        "peer failure must retain the first shared-device loss: {}",
        error.what()
    );

    let mut replacement = VulkanContext::new(replacement_window.native_surface_ptr(), 64, 48)
        .expect("replacement VulkanContext");
    assert_ne!(replacement.shared_device_identity(), lost_identity);
    let replacement_pixels =
        vec![0xFF44_5566; (replacement.width() * replacement.height()) as usize];
    replacement
        .present_pixels(
            &replacement_pixels,
            replacement.width(),
            replacement.height(),
            PresentDamage::Full,
        )
        .expect("replacement device present");

    first.try_shutdown().expect("shutdown first lost peer");
    second.try_shutdown().expect("shutdown second lost peer");
    replacement
        .try_shutdown()
        .expect("shutdown replacement context");
    drop((first, second, replacement));
    first_window.close().expect("close first window");
    second_window.close().expect("close second window");
    replacement_window
        .close()
        .expect("close replacement window");
}

#[cfg(windows)]
#[test]
#[ignore = "requires a Vulkan-capable Windows driver; set UIX_VULKAN_SOAK_SECONDS=900 for the gate"]
fn windows_vulkan_shared_device_multiwindow_soak_is_bounded() {
    let mut platform = crate::native::create_platform().expect("platform");
    let mut first_window = platform
        .window_manager()
        .create_window("Vulkan shared soak A", 160, 120)
        .expect("first window");
    let mut second_window = platform
        .window_manager()
        .create_window("Vulkan shared soak B", 176, 132)
        .expect("second window");
    first_window.show().expect("show first window");
    second_window.show().expect("show second window");
    let _ = platform.event_loop().poll_event(&|_| true);

    let mut first = VulkanContext::new(first_window.native_surface_ptr(), 160, 120)
        .expect("first VulkanContext");
    let mut second = VulkanContext::new(second_window.native_surface_ptr(), 176, 132)
        .expect("second VulkanContext");
    assert_eq!(
        first.shared_device_identity(),
        second.shared_device_identity()
    );

    let handles_before = current_process_handle_count();
    let duration = requested_vulkan_soak_duration();
    let deadline = std::time::Instant::now() + duration;
    let first_sizes = [(128, 96), (224, 144), (176, 132), (256, 160)];
    let second_sizes = [(192, 128), (144, 112), (240, 152), (168, 124)];
    let mut rounds = 0_u64;
    let mut peak_handles = handles_before;

    while std::time::Instant::now() < deadline || rounds < first_sizes.len() as u64 {
        if rounds % 2 == 0 {
            let requested = first_sizes[rounds as usize % first_sizes.len()];
            first_window
                .properties_mut()
                .set_size(requested.0, requested.1)
                .expect("resize first HWND during shared soak");
            let _ = platform.event_loop().poll_event(&|_| true);
            first
                .resize(requested.0, requested.1)
                .expect("resize first shared surface");
        } else {
            let requested = second_sizes[rounds as usize % second_sizes.len()];
            second_window
                .properties_mut()
                .set_size(requested.0, requested.1)
                .expect("resize second HWND during shared soak");
            let _ = platform.event_loop().poll_event(&|_| true);
            second
                .resize(requested.0, requested.1)
                .expect("resize second shared surface");
        }

        let first_color = 0xFF00_0000 | ((rounds as u32).wrapping_mul(0x0001_0203) & 0x00FF_FFFF);
        let second_color = 0xFF00_0000 | ((rounds as u32).wrapping_mul(0x0003_0201) & 0x00FF_FFFF);
        let first_pixels = vec![first_color; (first.width() * first.height()) as usize];
        let second_pixels = vec![second_color; (second.width() * second.height()) as usize];
        first
            .present_pixels(
                &first_pixels,
                first.width(),
                first.height(),
                PresentDamage::Full,
            )
            .expect("present first shared surface during soak");
        second
            .present_pixels(
                &second_pixels,
                second.width(),
                second.height(),
                PresentDamage::Full,
            )
            .expect("present second shared surface during soak");
        if rounds % 32 == 0 {
            assert_eq!(
                first
                    .read_pixels(first.width() - 1, first.height() - 1, 1, 1)
                    .expect("first shared soak readback"),
                vec![first_color]
            );
            assert_eq!(
                second
                    .read_pixels(second.width() - 1, second.height() - 1, 1, 1)
                    .expect("second shared soak readback"),
                vec![second_color]
            );
        }
        peak_handles = peak_handles.max(current_process_handle_count());
        rounds += 1;
    }

    let handles_after = current_process_handle_count();
    assert!(
        peak_handles <= handles_before.saturating_add(32),
        "shared multiwindow handles grew beyond the bounded envelope: before={handles_before}, peak={peak_handles}, after={handles_after}"
    );
    println!(
        "Vulkan shared-device soak: duration={:.1}s rounds={rounds} handles={handles_before}->{handles_after} peak={peak_handles}; {}",
        duration.as_secs_f64(),
        first.adapter_info.diagnostic_summary()
    );

    first.try_shutdown().expect("shutdown first shared surface");
    drop(first);
    first_window.close().expect("close first shared window");
    let final_pixels = vec![0xFF52_7193; (second.width() * second.height()) as usize];
    second
        .present_pixels(
            &final_pixels,
            second.width(),
            second.height(),
            PresentDamage::Full,
        )
        .expect("surviving shared surface present after soak");
    second
        .try_shutdown()
        .expect("shutdown second shared surface");
    drop(second);
    second_window.close().expect("close second shared window");
}

#[cfg(windows)]
#[test]
#[ignore = "requires a Vulkan-capable Windows driver; set UIX_VULKAN_SOAK_SECONDS=900 for the gate"]
fn windows_vulkan_hardware_resize_present_soak_is_bounded() {
    let mut platform = crate::native::create_platform().expect("platform");
    let mut window = platform
        .window_manager()
        .create_window("Vulkan resize/present soak", 160, 120)
        .expect("window");
    let surface = window.native_surface_ptr();
    assert!(!surface.is_null(), "Windows HWND must be available");
    window.show().expect("show window");
    let _ = platform.event_loop().poll_event(&|_| true);

    let mut context = VulkanContext::new(surface, 160, 120).expect("VulkanContext");
    let warmup = vec![0xFF24_68AC; (context.width() * context.height()) as usize];
    context
        .present_pixels(
            &warmup,
            context.width(),
            context.height(),
            PresentDamage::Full,
        )
        .expect("warmup present");
    let handles_before = current_process_handle_count();
    let duration = requested_vulkan_soak_duration();
    let deadline = std::time::Instant::now() + duration;
    let sizes = [(128, 96), (224, 144), (176, 132), (256, 160)];
    let mut rounds = 0_u64;
    let mut peak_handles = handles_before;

    while std::time::Instant::now() < deadline || rounds < sizes.len() as u64 {
        let requested = sizes[rounds as usize % sizes.len()];
        window
            .properties_mut()
            .set_size(requested.0, requested.1)
            .expect("resize HWND during soak");
        let _ = platform.event_loop().poll_event(&|_| true);
        let drawable = win_surface::drawable_size(surface, requested.0, requested.1);
        context
            .resize(drawable.logical_width, drawable.logical_height)
            .expect("recreate swapchain during soak");

        let color = 0xFF00_0000 | ((rounds as u32).wrapping_mul(0x0001_0203) & 0x00FF_FFFF);
        let pixels = vec![color; (context.width() * context.height()) as usize];
        context
            .present_pixels(
                &pixels,
                context.width(),
                context.height(),
                PresentDamage::Full,
            )
            .expect("present during soak");
        if rounds % 32 == 0 {
            assert_eq!(
                context
                    .read_pixels(context.width() - 1, context.height() - 1, 1, 1)
                    .expect("soak far-corner readback"),
                vec![color]
            );
        }
        peak_handles = peak_handles.max(current_process_handle_count());
        rounds += 1;
    }

    let handles_after = current_process_handle_count();
    assert!(
        peak_handles <= handles_before.saturating_add(32),
        "process handles grew beyond the bounded envelope: before={handles_before}, peak={peak_handles}, after={handles_after}"
    );
    println!(
        "Vulkan soak: duration={:.1}s rounds={rounds} handles={handles_before}->{handles_after} peak={peak_handles}; {}",
        duration.as_secs_f64(),
        context.adapter_info.diagnostic_summary()
    );

    context.try_shutdown().expect("shutdown");
    window.close().expect("close window");
}
