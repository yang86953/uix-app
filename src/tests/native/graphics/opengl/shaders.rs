use crate::native::graphics::opengl::shaders::*;

#[test]
fn native_shader_sources_cover_all_current_gl_pipeline_stages() {
    for source in [
        RECT_VERT,
        RECT_FRAG,
        GLYPH_VERT,
        GLYPH_FRAG,
        BLUR_FRAG,
        BLIT_FRAG,
        BLIT_RGBA_FRAG,
        FULLSCREEN_VERT,
    ] {
        assert!(source.starts_with("#version 300 es"));
        assert!(source.contains("void main()"));
    }
}

#[test]
fn glyph_shader_preserves_cpu_integer_premultiplied_coverage_contract() {
    assert!(GLYPH_VERT.contains("layout(location = 2) in vec4 a_color"));
    assert!(GLYPH_FRAG.contains("texture(u_atlas, v_uv).r"));
    assert!(GLYPH_FRAG.contains("floor(color.a * coverage / 255.0)"));
    assert!(GLYPH_FRAG.contains("floor(color.rgb * color.a / 255.0)"));
    assert!(GLYPH_FRAG.contains("floor(premul * coverage / 255.0)"));
}

#[test]
fn rounded_rect_fragment_shader_matches_cpu_sdf_and_premultiplied_coverage_blend() {
    assert!(RECT_FRAG.contains("rounded_rect_sdf"));
    assert!(RECT_FRAG.contains("0.5 - rounded_rect_sdf"));
    assert!(RECT_FRAG.contains("greaterThan(u_radius"));
    assert!(RECT_FRAG.contains("floor(color.rgb * color.a / 255.0)"));
    assert!(RECT_FRAG.contains("vec4(premul * mask, color.a * mask) / 255.0"));
    assert!(!RECT_FRAG.contains("smoothstep"));
}
