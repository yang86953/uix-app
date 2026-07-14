use crate::native::graphics::vulkan::platform::adapter::{
    select_graphics_present_queue, AdapterSelectionRejections,
};
use crate::tests::common::*;
use ash::vk;

fn queue(flags: vk::QueueFlags) -> vk::QueueFamilyProperties {
    vk::QueueFamilyProperties {
        queue_flags: flags,
        queue_count: 1,
        ..Default::default()
    }
}

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

#[test]
fn queue_selection_continues_after_a_present_probe_failure() {
    let queues = [
        queue(vk::QueueFlags::TRANSFER),
        queue(vk::QueueFlags::GRAPHICS),
        queue(vk::QueueFlags::GRAPHICS),
    ];
    let mut queried = Vec::new();
    let mut rejections = AdapterSelectionRejections::default();

    let selected =
        select_graphics_present_queue(&queues, "adapter=hybrid", &mut rejections, |index| {
            queried.push(index);
            if index == 1 {
                Err(Error::new(
                    Errc::GraphicsSurfaceLost,
                    "first graphics queue probe failed",
                ))
            } else {
                Ok(true)
            }
        });

    assert_eq!(selected, Some(2));
    assert_eq!(queried, [1, 2]);
    let diagnostic = rejections.into_error();
    assert_eq!(diagnostic.code(), Errc::GraphicsSurfaceLost);
    assert!(diagnostic.message().contains("queue_family=1"));
}

#[test]
fn queue_selection_reports_each_graphics_queue_rejection() {
    let queues = [
        queue(vk::QueueFlags::GRAPHICS),
        queue(vk::QueueFlags::COMPUTE),
        queue(vk::QueueFlags::GRAPHICS),
    ];
    let mut rejections = AdapterSelectionRejections::default();

    let selected =
        select_graphics_present_queue(&queues, "adapter=no-present", &mut rejections, |_| {
            Ok(false)
        });

    assert_eq!(selected, None);
    let error = rejections.into_error();
    assert!(error.message().contains("queue_family=0"));
    assert!(error.message().contains("queue_family=2"));
    assert!(!error.message().contains("queue_family=1"));
}

#[test]
fn queue_selection_reports_missing_graphics_capability_without_querying_present() {
    let queues = [
        queue(vk::QueueFlags::TRANSFER),
        queue(vk::QueueFlags::COMPUTE),
    ];
    let mut queried = false;
    let mut rejections = AdapterSelectionRejections::default();

    let selected =
        select_graphics_present_queue(&queues, "adapter=compute-only", &mut rejections, |_| {
            queried = true;
            Ok(true)
        });

    assert_eq!(selected, None);
    assert!(!queried);
    assert!(rejections
        .into_error()
        .message()
        .contains("no graphics queue"));
}
