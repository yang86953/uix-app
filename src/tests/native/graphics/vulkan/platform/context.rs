use super::*;

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
    assert!(
        choose_composite_alpha(
            vk::CompositeAlphaFlagsKHR::OPAQUE | vk::CompositeAlphaFlagsKHR::INHERIT
        )
        .is_some_and(|mode| mode == vk::CompositeAlphaFlagsKHR::OPAQUE)
    );
    assert!(
        choose_composite_alpha(vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED)
            .is_some_and(|mode| mode == vk::CompositeAlphaFlagsKHR::PRE_MULTIPLIED)
    );
    assert!(choose_composite_alpha(vk::CompositeAlphaFlagsKHR::empty()).is_none());
}
