use crate::native::graphics::vulkan::platform::adapter::AdapterSelectionRejections;
use crate::tests::common::*;

#[test]
fn adapter_rejections_list_every_checked_candidate() {
    let mut rejections = AdapterSelectionRejections::default();
    rejections.reject("adapter=integrated", "missing VK_KHR_swapchain");
    rejections.reject("adapter=discrete", "no graphics+present queue");

    let error = rejections.into_error();

    assert_eq!(error.code(), Errc::PlatformError);
    assert!(error.message().contains("adapter=integrated"));
    assert!(error.message().contains("missing VK_KHR_swapchain"));
    assert!(error.message().contains("adapter=discrete"));
    assert!(error.message().contains("no graphics+present queue"));
    assert!(error.source_error().is_none());
}

#[test]
fn adapter_rejections_preserve_the_first_typed_probe_failure() {
    let mut rejections = AdapterSelectionRejections::default();
    rejections.reject_with_error(
        "adapter=first",
        "vkGetPhysicalDeviceSurfaceSupportKHR",
        Error::new(Errc::GraphicsSurfaceLost, "native surface was lost"),
    );
    rejections.reject_with_error(
        "adapter=second",
        "vkEnumerateDeviceExtensionProperties",
        Error::new(Errc::GraphicsOutOfMemory, "driver allocation failed"),
    );

    let error = rejections.into_error();

    assert_eq!(error.code(), Errc::GraphicsSurfaceLost);
    assert!(error.message().contains("adapter=first"));
    assert!(error.message().contains("adapter=second"));
    assert_eq!(error.root_cause().code(), Errc::GraphicsSurfaceLost);
    assert_eq!(error.root_cause().message(), "native surface was lost");
}

#[test]
fn empty_adapter_inventory_is_explicit() {
    let error = AdapterSelectionRejections::default().into_error();

    assert_eq!(error.code(), Errc::PlatformError);
    assert!(error.message().contains("candidates=[none enumerated]"));
}
