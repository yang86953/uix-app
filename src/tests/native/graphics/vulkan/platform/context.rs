#[cfg(windows)]
use crate::native::graphics::platform::windows as win_surface;
use crate::native::graphics::vulkan::platform::adapter::VulkanAdapterInfo;
use crate::native::graphics::vulkan::platform::context::*;
use crate::native::graphics::vulkan::platform::surface::{
    choose_composite_alpha, choose_surface_format,
};
use crate::tests::common::*;
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
fn staging_size_is_full_rgba_frame() {
    assert_eq!(staging_size(4, 3), 48);
    assert_eq!(staging_size(0, 0), 4);
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
    assert_eq!(context.graphics_backend(), GraphicsBackend::Vulkan);
    assert_eq!(context.caps().present, PresentMode::PixelUpload);
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

    window
        .properties_mut()
        .set_size(192, 128)
        .expect("resize HWND");
    let _ = platform.event_loop().poll_event(&|_| true);
    let drawable = win_surface::drawable_size(surface, 192, 128);
    context
        .resize(drawable.width, drawable.height)
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
            .resize(drawable.width, drawable.height)
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
