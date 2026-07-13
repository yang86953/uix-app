use super::*;

#[test]
fn native_shader_sources_cover_all_current_gl_pipeline_stages() {
    for source in [
        RECT_VERT,
        RECT_FRAG,
        BLUR_FRAG,
        BLIT_FRAG,
        BLIT_RGBA_FRAG,
        FULLSCREEN_VERT,
    ] {
        assert!(source.starts_with("#version 300 es"));
        assert!(source.contains("void main()"));
    }
}
