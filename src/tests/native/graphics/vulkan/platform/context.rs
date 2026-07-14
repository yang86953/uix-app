#[cfg(windows)]
use crate::native::graphics::platform::windows as win_surface;
use crate::native::graphics::vulkan::platform::adapter::VulkanAdapterInfo;
use crate::native::graphics::vulkan::platform::context::*;
use crate::native::graphics::vulkan::platform::surface::{
    choose_composite_alpha, choose_surface_format,
};
use crate::tests::common::*;
use ash::vk;

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
