use crate::native::graphics::d3d12::platform::pipeline::validated_soft_layout;

#[test]
fn soft_upload_layout_aligns_rows_and_rejects_short_input() {
    assert_eq!(validated_soft_layout(65, 37, 65 * 37), Ok((512, 512 * 37)));
    let error = validated_soft_layout(65, 37, 65 * 37 - 1).expect_err("short input");
    assert_eq!(error.code(), crate::core::Errc::InvalidArgument);
    assert!(validated_soft_layout(i32::MAX, 1, usize::MAX).is_err());
}
