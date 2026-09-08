//! Regression contract: authored pixels are em units, including fractional DPI.
use ab_glyph::{Font, FontRef};
use uix::draw::resources::font::text_backend::{TextLayoutOptions, normal_line_height};
use uix::draw::resources::font::text_backends::ab_glyph::AbGlyphBackend;
use uix::draw::{FontService, HAlign, TextBackend, VAlign};

const FONT: &[u8] = include_bytes!("../assets/fonts/NotoSansCJKsc-Regular.otf");

fn options(font_size: f32, line_height: f32) -> TextLayoutOptions {
    TextLayoutOptions {
        max_width: f32::MAX,
        max_height: 0.0,
        line_height,
        word_wrap: false,
        h_align: HAlign::Left,
        v_align: VAlign::Top,
        font_size,
    }
}

fn near(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 0.01, "{actual} != {expected}");
}

// Change only the em-unit declaration; ascent/descent and outlines stay intact.
// This valid SFNT exercises fonts whose em and metric height are not identical,
// without depending on any font installed on the test machine.
fn font_with_em(units: u16) -> Vec<u8> {
    let mut data = FONT.to_vec();
    let tables = u16::from_be_bytes([data[4], data[5]]) as usize;
    let record = (0..tables)
        .map(|i| 12 + 16 * i)
        .find(|&offset| &data[offset..offset + 4] == b"head")
        .unwrap();
    let head = u32::from_be_bytes(data[record + 8..record + 12].try_into().unwrap()) as usize;
    data[head + 18..head + 20].copy_from_slice(&units.to_be_bytes());
    data
}

#[test]
fn metrics_and_shaping_use_em_not_ascent_descent_height() {
    let data = font_with_em(2000);
    let face = FontRef::try_from_slice(&data).unwrap();
    let mut backend = AbGlyphBackend::new();
    let font = backend.load_font(&data).unwrap();
    for size in [12.0, 14.0, 17.5, 21.0, 32.0, 48.0] {
        let metrics = backend.horizontal_line_metrics(&font, size).unwrap();
        near(metrics.ascent, face.ascent_unscaled() * size / 2000.0);
        near(metrics.descent, -face.descent_unscaled() * size / 2000.0);
        for height in [size, normal_line_height(size), size * 2.0] {
            let layout = backend.layout_text(&font, "H", &options(size, height));
            near(
                layout.width,
                face.h_advance_unscaled(face.glyph_id('H')) * size / 2000.0,
            );
            near(layout.glyphs[0].y, metrics.baseline_in_line_box(height));
            near(layout.height, height);
        }
    }
}

#[test]
fn font_service_and_backend_share_baselines_and_fractional_raster_sizes() {
    let data = font_with_em(2000);
    let mut service = FontService::new();
    let font = service.load_font(&data).unwrap();
    for size in [14.0, 17.5, 21.0, 28.0] {
        let opts = options(size, size * 1.65);
        let metrics = service.horizontal_line_metrics(&font, size).unwrap();
        let layout = service.layout_text_shared(&font, "Hg中文", &opts);
        for glyph in &layout.glyphs {
            near(glyph.y, metrics.baseline_in_line_box(opts.line_height));
        }
    }
    let glyph = service
        .layout_text_shared(&font, "H", &options(20.0, 30.0))
        .glyphs[0]
        .glyph_id;
    let a = service.rasterize_glyph(&font, glyph, 20.125);
    let again = service.rasterize_glyph(&font, glyph, 20.125);
    let b = service.rasterize_glyph(&font, glyph, 20.375);
    assert!(a.width > 0 && b.height > 0);
    assert!(std::sync::Arc::ptr_eq(&a.coverage, &again.coverage));
    assert!(
        !std::sync::Arc::ptr_eq(&a.coverage, &b.coverage),
        "fractional sizes collided in cache"
    );
    assert!(a.coverage != b.coverage || a.outline_mesh != b.outline_mesh);
}

#[test]
fn invalid_sizes_and_extreme_em_bounds_do_not_allocate_unbounded_rasters() {
    let mut backend = AbGlyphBackend::new();
    let data = font_with_em(16);
    let font = backend.load_font(&data).unwrap();
    let glyph = FontRef::try_from_slice(&data).unwrap().glyph_id('H').0 as u32;
    for size in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(backend.horizontal_line_metrics(&font, size).is_none());
        let raster = backend.rasterize_glyph(&font, glyph, size);
        assert_eq!(raster.width * raster.height, 0);
    }
    let raster = backend.rasterize_glyph(&font, glyph, 512.0);
    assert_eq!(
        raster.width * raster.height,
        0,
        "extreme em outline exceeded raster budget"
    );
}
