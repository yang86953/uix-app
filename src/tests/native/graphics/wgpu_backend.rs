use crate::native::graphics::wgpu_backend::renderer::grow_zeroed;

#[test]
fn glyph_staging_never_shrinks_when_shorter_glyphs_share_a_row() {
    let mut staging = Vec::new();
    grow_zeroed(&mut staging, 56 * 1024);
    grow_zeroed(&mut staging, 49 * 1024);

    assert_eq!(staging.len(), 56 * 1024);
}
