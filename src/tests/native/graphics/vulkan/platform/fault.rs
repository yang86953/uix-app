use crate::native::graphics::vulkan::platform::fault::{
    select_feature_query_mode, DeviceFaultAddress, DeviceFaultFeatureQueryMode, DeviceFaultReport,
    DeviceFaultVendor, DeviceLossState,
};
use crate::tests::common::*;
use ash::vk;

#[cfg(windows)]
use crate::native::graphics::vulkan::platform::context::VulkanContext;

#[test]
fn device_fault_feature_query_prefers_vulkan_1_1_core() {
    assert_eq!(
        select_feature_query_mode(Some(vk::API_VERSION_1_3), true),
        (vk::API_VERSION_1_1, DeviceFaultFeatureQueryMode::Core11)
    );
}

#[test]
fn device_fault_feature_query_uses_khr_on_vulkan_1_0() {
    assert_eq!(
        select_feature_query_mode(Some(vk::API_VERSION_1_0), true),
        (vk::API_VERSION_1_0, DeviceFaultFeatureQueryMode::Khr)
    );
    assert_eq!(
        select_feature_query_mode(None, true),
        (vk::API_VERSION_1_0, DeviceFaultFeatureQueryMode::Khr)
    );
}

#[test]
fn device_fault_feature_query_stays_optional_without_a_query_path() {
    assert_eq!(
        select_feature_query_mode(Some(vk::API_VERSION_1_0), false),
        (
            vk::API_VERSION_1_0,
            DeviceFaultFeatureQueryMode::Unavailable
        )
    );
}

#[test]
fn device_fault_report_keeps_bounded_driver_diagnostics_readable() {
    let report = DeviceFaultReport {
        description: "  page fault\nwhile executing \"shader\"  ".to_owned(),
        addresses: vec![DeviceFaultAddress {
            address_type: vk::DeviceFaultAddressTypeEXT::READ_INVALID.as_raw(),
            reported_address: 0x1234,
            address_precision: 64,
        }],
        vendors: vec![DeviceFaultVendor {
            description: "warp timeout".to_owned(),
            code: 0xA,
            data: 0xB,
        }],
        vendor_binary_bytes: 4096,
        truncated: true,
    };

    let summary = report.diagnostic_summary();

    assert!(summary.contains("description=\"page fault while executing 'shader'\""));
    assert!(summary.contains("read_invalid@0x0000000000001234±64"));
    assert!(summary.contains("\"warp timeout\" code=0x000000000000000A"));
    assert!(summary.contains("vendor_binary_bytes=4096"));
    assert!(summary.contains("truncated=true"));
    assert!(!summary.contains('\n'));
}

#[test]
fn empty_device_fault_report_is_explicit() {
    let report = DeviceFaultReport {
        description: String::new(),
        addresses: Vec::new(),
        vendors: Vec::new(),
        vendor_binary_bytes: 0,
        truncated: false,
    };

    assert_eq!(
        report.diagnostic_summary(),
        "VK_EXT_device_fault: description=\"unavailable\"; addresses=[none]; vendors=[none]; vendor_binary_bytes=0; truncated=false"
    );
}

#[test]
fn device_loss_state_keeps_the_first_enriched_error_for_peers() {
    let state = DeviceLossState::default();
    let first = state.record_with(
        Error::new(Errc::GraphicsDeviceLost, "vkQueueSubmit failed"),
        |error| {
            error.with_source(Error::new(
                Errc::GraphicsDeviceLost,
                "VK_EXT_device_fault: page fault",
            ))
        },
    );

    assert_eq!(first.depth(), 1);
    let peer = state.peer_error().expect("loss must be visible to peers");
    assert_eq!(peer.code(), Errc::GraphicsDeviceLost);
    assert!(peer.message().contains("shared logical device"));
    assert_eq!(
        peer.root_cause().message(),
        "VK_EXT_device_fault: page fault"
    );
}

#[test]
fn repeated_device_loss_keeps_current_operation_and_first_diagnosis() {
    let state = DeviceLossState::default();
    let first = state.record_with(
        Error::new(Errc::GraphicsDeviceLost, "first device loss"),
        |error| error.with_source(Error::new(Errc::GraphicsDeviceLost, "first diagnosis")),
    );
    let repeated = state.record_with(
        Error::new(Errc::GraphicsDeviceLost, "second device loss"),
        |error| error.with_source(Error::new(Errc::GraphicsDeviceLost, "must not replace")),
    );

    assert_eq!(repeated.message(), "second device loss");
    assert_eq!(repeated.source_error(), Some(&first));
    assert_eq!(repeated.root_cause().message(), "first diagnosis");
}

#[test]
fn non_device_loss_does_not_poison_shared_device_state() {
    let state = DeviceLossState::default();
    let error = state.record_with(
        Error::new(Errc::GraphicsSurfaceLost, "surface only"),
        |error| error,
    );

    assert_eq!(error.code(), Errc::GraphicsSurfaceLost);
    assert!(state.peer_error().is_none());
}

#[cfg(windows)]
#[test]
#[ignore = "requires a Vulkan-capable Windows driver and UIX_VULKAN_EXPECT_DEVICE_FAULT=true|false"]
fn windows_vulkan_device_fault_reporting_matches_expected_capability() {
    let expected = match std::env::var("UIX_VULKAN_EXPECT_DEVICE_FAULT").as_deref() {
        Ok("true" | "1") => true,
        Ok("false" | "0") => false,
        Ok(value) => panic!("UIX_VULKAN_EXPECT_DEVICE_FAULT must be true|false|1|0, got {value:?}"),
        Err(error) => panic!("UIX_VULKAN_EXPECT_DEVICE_FAULT is required: {error}"),
    };
    let mut platform = crate::native::create_platform().expect("platform");
    let mut window = platform
        .window_manager()
        .create_window("Vulkan device fault capability", 96, 64)
        .expect("window");
    let mut context = VulkanContext::new(window.native_surface_ptr(), 96, 64)
        .expect("VulkanContext for device fault capability");

    assert_eq!(
        context.device_fault_reporting_enabled_for_test(),
        expected,
        "unexpected VK_EXT_device_fault capability: {}",
        context.adapter_info.diagnostic_summary()
    );
    println!(
        "Vulkan device fault capability: enabled={expected}; {}",
        context.adapter_info.diagnostic_summary()
    );

    context.try_shutdown().expect("shutdown");
    window.close().expect("close window");
}
