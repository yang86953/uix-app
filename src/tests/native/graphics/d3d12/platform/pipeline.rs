use crate::native::graphics::d3d12::platform::pipeline::{
    glyph_segment_staging_bytes, glyph_upload_fits_retained_budget, validated_glyph_layout,
    validated_soft_layout, GlyphAtlasState,
};

#[test]
fn soft_upload_layout_aligns_rows_and_rejects_short_input() {
    assert_eq!(validated_soft_layout(65, 37, 65 * 37), Ok((512, 512 * 37)));
    let error = validated_soft_layout(65, 37, 65 * 37 - 1).expect_err("short input");
    assert_eq!(error.code(), crate::core::Errc::InvalidArgument);
    assert!(validated_soft_layout(i32::MAX, 1, usize::MAX).is_err());
}

#[test]
fn glyph_atlas_state_is_bounded_and_resets_deterministically() {
    assert_eq!(validated_glyph_layout(65, 3, 65 * 3), Ok((256, 768)));
    assert!(validated_glyph_layout(65, 3, 65 * 3 - 1).is_err());
    assert!(validated_glyph_layout(2049, 1, 2049).is_err());

    let mut state = GlyphAtlasState::default();
    assert!(state.can_fit(2048, 2048));
    assert!(state.pack(2048, 2048).is_some());
    assert!(!state.can_fit(1, 1));
    assert!(state.pack(1, 1).is_none());
    state.reset();
    assert!(state.can_fit(1, 1));
    assert!(state.pack(1, 1).is_some());

    assert_eq!(glyph_segment_staging_bytes(2048), Ok(4 * 1024 * 1024));
    assert!(glyph_segment_staging_bytes(2049).is_err());
    assert!(glyph_upload_fits_retained_budget(
        Some(4 * 1024 * 1024),
        0,
        4 * 1024 * 1024,
    ));
    assert!(!glyph_upload_fits_retained_budget(
        Some(8 * 1024 * 1024),
        0,
        1,
    ));
    assert!(
        glyph_upload_fits_retained_budget(Some(12 * 1024 * 1024), 4 * 1024 * 1024, 1),
        "reusing an existing slot must not allocate more retained memory"
    );
}
