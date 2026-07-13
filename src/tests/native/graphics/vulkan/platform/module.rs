use crate::tests::common::*;
use std::ffi::c_void;
use crate::core::{ Result };
#[cfg(target_os = "macos")]
#[test]
fn macos_vulkan_null_layer_is_typed() {
    let err = crate::native::graphics::vulkan::platform::create(std::ptr::null_mut(), 1, 1).expect_err("null CAMetalLayer");
    assert!(
        err.message().contains("CAMetalLayer")
            || err.message().contains("load Vulkan")
            || err.message().contains("vkCreate"),
        "unexpected macOS Vulkan null-layer error: {}",
        err.message()
    );
}
