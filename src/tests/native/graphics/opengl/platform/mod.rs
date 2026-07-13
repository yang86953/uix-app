#[test]
fn egl_damage_extension_does_not_imply_partial_present() {
    assert!(
        !super::EGL_PARTIAL_PRESENT,
        "partial present requires a proven preservation/buffer-age contract"
    );
}
