use crate::draw::font::text_backends::ab_glyph::*;
use crate::draw::TextBackend;
use crate::tests::common::*;

#[test]
fn new_creates_empty_backend() {
    let backend = AbGlyphBackend::new();
    assert!(backend.fonts.is_empty());
}

#[test]
fn load_font_empty_data_returns_format_error() {
    let mut backend = AbGlyphBackend::new();
    let result = backend.load_font(&[]);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.code(), crate::core::Errc::FormatError);
}

#[test]
fn unload_font_invalid_handle_does_not_panic() {
    let mut backend = AbGlyphBackend::new();
    backend.unload_font(&FontHandle::new(999));
    // 没有 panic 即通过
}

#[test]
fn is_valid_unloaded_handle_returns_false() {
    let backend = AbGlyphBackend::new();
    assert!(!backend.is_valid(&FontHandle::new(0)));
}

#[test]
fn has_glyph_unloaded_handle_returns_false() {
    let backend = AbGlyphBackend::new();
    assert!(!backend.has_glyph(&FontHandle::new(0), 'a'));
}

#[test]
fn layout_text_unloaded_handle_returns_empty_layout() {
    let backend = AbGlyphBackend::new();
    let opts = crate::draw::font::text_backends::ab_glyph::TextLayoutOptions {
        max_width: f32::MAX,
        max_height: 0.0,
        line_height: 0.0,
        word_wrap: true,
        h_align: crate::draw::HAlign::Left,
        v_align: crate::draw::VAlign::Top,
        font_size: 14.0,
    };
    let layout = backend.layout_text(&FontHandle::new(0), "hello", &opts);
    assert_eq!(layout.width, 0.0);
    assert_eq!(layout.height, 0.0);
    assert!(layout.glyphs.is_empty());
}

#[test]
fn layout_text_empty_string_unloaded_handle_returns_empty_layout() {
    let backend = AbGlyphBackend::new();
    let opts = crate::draw::font::text_backends::ab_glyph::TextLayoutOptions {
        max_width: f32::MAX,
        max_height: 0.0,
        line_height: 0.0,
        word_wrap: true,
        h_align: crate::draw::HAlign::Left,
        v_align: crate::draw::VAlign::Top,
        font_size: 14.0,
    };
    let layout = backend.layout_text(&FontHandle::new(0), "", &opts);
    assert_eq!(layout.width, 0.0);
    assert_eq!(layout.height, 0.0);
    assert!(layout.glyphs.is_empty());
}

#[test]
fn rasterize_glyph_invalid_handle_returns_empty_raster() {
    let backend = AbGlyphBackend::new();
    let raster = backend.rasterize_glyph(&FontHandle::new(0), 0, 14.0);
    assert_eq!(raster.width, 0);
    assert_eq!(raster.height, 0);
    assert!(raster.coverage.is_empty());
}

#[test]
fn horizontal_line_metrics_invalid_handle_returns_none() {
    let backend = AbGlyphBackend::new();
    let result = backend.horizontal_line_metrics(&FontHandle::new(0), 14.0);
    assert!(result.is_none());
}

#[test]
fn memory_usage_empty_backend_returns_zero() {
    let backend = AbGlyphBackend::new();
    assert_eq!(backend.memory_usage(), 0);
}

#[test]
fn clear_cache_does_not_panic() {
    let mut backend = AbGlyphBackend::new();
    backend.clear_cache();
    // 没有 panic 即通过
}

#[test]
fn font_data_default_returns_none() {
    // font_data 的默认实现返回 None
    let backend = AbGlyphBackend::new();
    let result = TextBackend::font_data(&backend, &FontHandle::new(0));
    assert!(result.is_none());
}

#[test]
fn set_fallback_fonts_does_not_panic() {
    let mut backend = AbGlyphBackend::new();
    TextBackend::set_fallback_fonts(&mut backend, &[]);
    // 没有 panic 即通过
}

#[test]
fn debug_format_does_not_panic() {
    let backend = AbGlyphBackend::new();
    let _ = format!("{:?}", backend);
}
