//! draw 域 — 字体服务集成测试。
//! 覆盖字体服务的基本构造、文本布局、测量、命中测试。

use uix::draw::font_service::FontService;
use uix::draw::text_backend::TextLayoutOptions;
use uix::draw::types::{FontHandle, HAlign, VAlign};
use uix::core::geometry::Point;

fn default_opts() -> TextLayoutOptions {
    TextLayoutOptions {
        max_width: 0.0,
        max_height: 0.0,
        line_height: 14.0 * 1.5,
        word_wrap: false,
        h_align: HAlign::Left,
        v_align: VAlign::Top,
        font_size: 14.0,
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 基础构造
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn font_service_new() {
    let fs = FontService::new();
    assert!(!fs.is_valid(&FontHandle::new(0)));
}

#[test]
fn font_service_default_family_not_available() {
    let fs = FontService::new();
    let fam = fs.font_family(&FontHandle::new(0));
    assert!(fam.is_none());
}

// ════════════════════════════════════════════════════════════════════════════
// 文本布局（无已加载字体时返回空布局）
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn layout_text_empty_returns_empty_layout() {
    let fs = FontService::new();
    let fh = FontHandle::default();
    let layout = fs.layout_text(&fh, "", &default_opts());
    assert_eq!(layout.width, 0.0);
    assert_eq!(layout.height, 0.0);
    assert!(layout.glyphs.is_empty());
}

#[test]
fn layout_text_invalid_font_returns_empty() {
    let fs = FontService::new();
    let layout = fs.layout_text(&FontHandle::new(999), "Hello", &default_opts());
    assert!(layout.glyphs.is_empty());
}

#[test]
fn layout_text_default_font_without_system_font_returns_empty() {
    // 未调用 load_default_system_font 时，默认 FontHandle 无效
    let fs = FontService::new();
    let layout = fs.layout_text(&FontHandle::default(), "Hello", &default_opts());
    assert!(layout.glyphs.is_empty());
}

#[test]
fn load_font_from_path_nonexistent() {
    let mut fs = FontService::new();
    let result = fs.load_font_from_path("/nonexistent/font.ttf", 14.0);
    assert!(result.is_none());
}

// ════════════════════════════════════════════════════════════════════════════
// 文本测量（无已加载字体时返回零尺寸）
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn measure_text_no_font_returns_basic_metrics() {
    let fs = FontService::new();
    let fh = FontHandle::default();
    let sz = fs.measure_text(&fh, "Hello", &default_opts());
    // ab_glyph 后端即使无字体也有基本 fallback 度量
    assert!(sz.w >= 0.0);
    assert!(sz.h >= 0.0);
}

#[test]
fn measure_text_empty_returns_minimal_height() {
    let fs = FontService::new();
    let fh = FontHandle::default();
    let sz = fs.measure_text(&fh, "", &default_opts());
    assert_eq!(sz.w, 0.0);
    assert!(sz.h >= 0.0);
}

// ════════════════════════════════════════════════════════════════════════════
// 命中测试（无已加载字体时后端仍有 fallback 行为）
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn hit_test_text_no_font_returns_some() {
    let fs = FontService::new();
    let fh = FontHandle::default();
    let result = fs.hit_test_text(&fh, "Hello", &default_opts(), Point::new(0.0, 0.0));
    // ab_glyph 后端 fallback 返回有效索引
    assert!(result.is_some());
}

#[test]
fn hit_test_text_empty() {
    let fs = FontService::new();
    let fh = FontHandle::default();
    let result = fs.hit_test_text(&fh, "", &default_opts(), Point::new(0.0, 0.0));
    assert!(result.is_none());
}

// ════════════════════════════════════════════════════════════════════════════
// 字形光栅化
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn rasterize_glyph_no_font_returns_empty() {
    let fs = FontService::new();
    let fh = FontHandle::default();
    let raster = fs.rasterize_glyph(&fh, 0, 14.0);
    assert_eq!(raster.width, 0);
}

#[test]
fn clear_glyph_cache() {
    let mut fs = FontService::new();
    fs.clear_glyph_cache();
}

// ════════════════════════════════════════════════════════════════════════════
// Fallback 管理
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn fallback_chain_empty_by_default() {
    let fs = FontService::new();
    assert!(fs.fallback_chain().is_empty());
}

#[test]
fn set_fallback_chain() {
    let mut fs = FontService::new();
    let handles = vec![FontHandle::new(1), FontHandle::new(2)];
    fs.set_fallback_chain(&handles);
    assert_eq!(fs.fallback_chain().len(), 2);
}

#[test]
fn set_font_family_noop_without_backend_change() {
    let mut fs = FontService::new();
    fs.set_font_family("Arial");
    // 只是确认不 panic
}

#[test]
fn add_fallback() {
    let mut fs = FontService::new();
    fs.add_fallback(FontHandle::new(42));
    assert_eq!(fs.fallback_chain().len(), 1);
}
