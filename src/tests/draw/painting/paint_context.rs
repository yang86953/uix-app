use super::*;
use crate::draw::spatial::PhysicalUnit;

#[test]
fn resolve_font_size_no_unit_returns_base() {
    assert_eq!(resolve_font_size(14.0, None, 96.0), 14.0);
}

#[test]
fn resolve_font_size_zero_base() {
    assert_eq!(resolve_font_size(0.0, None, 96.0), 0.0);
}

#[test]
fn resolve_font_size_with_dip_unit_returns_base() {
    let unit = PhysicalUnit::Px(16.0);
    let result = resolve_font_size(14.0, Some(unit), 96.0);
    // Dip 直接返回其值
    assert_eq!(result, 16.0);
}

#[test]
fn resolve_font_size_with_mm_unit() {
    let unit = PhysicalUnit::Mm(10.0);
    // 10mm @ 96 DPI ≈ 37.795
    let result = resolve_font_size(14.0, Some(unit), 96.0);
    assert!((result - 37.795).abs() < 0.01);
}

#[test]
fn resolve_font_size_with_pt_unit() {
    let unit = PhysicalUnit::Pt(12.0);
    // 12pt @ 96 DPI = 12 * 96/72 = 16
    let result = resolve_font_size(14.0, Some(unit), 96.0);
    assert!((result - 16.0).abs() < 0.01);
}

#[test]
fn resolve_font_size_with_pt_unit_high_dpi() {
    let unit = PhysicalUnit::Pt(12.0);
    let result = resolve_font_size(14.0, Some(unit), 192.0);
    // 12pt @ 192 DPI = 12 * 192/72 = 32
    assert!((result - 32.0).abs() < 0.01);
}

#[test]
fn resolve_font_size_with_px_unit() {
    let unit = PhysicalUnit::Px(20.0);
    let result = resolve_font_size(14.0, Some(unit), 96.0);
    // px 直接返回其值
    assert_eq!(result, 20.0);
}

/// PaintContext 默认 debug=false；未 set_debug_mode(true) 时边框绘制为 no-op。
/// LayerTree::draw_debug_for_widget 必须先打开，否则 debug overlay 不可见。
#[test]
fn paint_context_debug_mode_defaults_off_until_enabled() {
    use crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
    use crate::draw::font::font_service::FontService;
    use crate::draw::image::ImageService;
    use crate::draw::spatial::Orientation;
    use crate::draw::FontHandle;
    use crate::ui::theme::DesignTokens;

    let mut canvas = NoopCanvas2D;
    let fs = FontService::new();
    let img = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let mut ctx = PaintContext::new(
        &mut canvas,
        FontHandle::default(),
        &fs,
        &img,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        100,
        100,
    );
    assert!(!ctx.debug_mode());
    ctx.set_debug_mode(true);
    assert!(ctx.debug_mode());
    // 打开后边框路径可走通（NoopCanvas 不记录，仅防 panic）
    ctx.draw_debug_border(Rect::new(0.0, 0.0, 40.0, 20.0), 0, false);
}

#[test]
fn display_list_recording_rejects_untracked_canvas_access() {
    use crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
    use crate::draw::font::font_service::FontService;
    use crate::draw::image::ImageService;
    use crate::draw::painting::DisplayList;
    use crate::draw::spatial::Orientation;
    use crate::draw::{Color, FontHandle};
    use crate::ui::theme::DesignTokens;

    let mut canvas = NoopCanvas2D;
    let fs = FontService::new();
    let img = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let mut ctx = PaintContext::new(
        &mut canvas,
        FontHandle::default(),
        &fs,
        &img,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        100,
        100,
    );
    let mut list = DisplayList::new();
    ctx.set_recorder(Some(&mut list));
    ctx.fill_rect(Rect::new(0.0, 0.0, 10.0, 10.0), Color::red(), None);
    assert!(ctx.recording_complete());

    let _ = ctx.canvas_2d();
    assert!(!ctx.recording_complete());
}
