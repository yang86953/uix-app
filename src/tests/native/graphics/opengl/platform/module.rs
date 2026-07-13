use crate::tests::common::*;
use std::ffi::c_void;
use crate::core::{ Result };
#[cfg(all(unix, not(target_os = "macos")))]
#[test]
fn egl_damage_extension_does_not_imply_partial_present() {
    assert!(
        !crate::native::graphics::opengl::platform::EGL_PARTIAL_PRESENT,
        "partial present requires a proven preservation/buffer-age contract"
    );
}
