use crate::native::graphics::vulkan::platform::fault::{
    select_feature_query_mode, DeviceFaultFeatureQueryMode,
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
