use crate::native::graphics::vulkan::platform::fault::{
    select_feature_query_mode, DeviceFaultAddress, DeviceFaultFeatureQueryMode, DeviceFaultReport,
    DeviceFaultVendor,
};
use ash::vk;

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
