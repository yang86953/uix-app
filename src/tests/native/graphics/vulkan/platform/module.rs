#[cfg(target_os = "macos")]
#[test]
fn macos_vulkan_null_layer_is_typed() {
    let Err(err) = crate::native::graphics::vulkan::platform::create(std::ptr::null_mut(), 1, 1)
    else {
        panic!("null CAMetalLayer must fail");
    };
    assert!(
        err.message().contains("CAMetalLayer")
            || err.message().contains("load Vulkan")
            || err.message().contains("vkCreate"),
        "unexpected macOS Vulkan null-layer error: {}",
        err.message()
    );
}
