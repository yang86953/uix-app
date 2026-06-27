//! uix-graphics crate 集成测试。

use uix::graphics::blur::gaussian_blur;
use uix::graphics::color::Color;
use uix::graphics::flattener::flatten;
use uix::graphics::path::PathBuilder;
use uix::graphics::types::{
    BlendMode, DirtyRegion, FontHandle, HAlign, ImageHandle, Radius, TextLayoutOptions,
    Transform, VAlign,
};
use uix::platform::geometry::{Point, Rect, Size};

// ════════════════════════════════════════════════════════════════════════════
// color 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn color_premultiplied_opaque() {
    let c = Color::from_rgb(255, 128, 64);
    let p = c.premultiplied();
    assert_eq!(p, 0xFF4080FF);
}

#[test]
fn color_premultiplied_transparent() {
    let c = Color::from_rgba(255, 255, 255, 0);
    assert_eq!(c.premultiplied(), 0x00000000);
}

#[test]
fn color_premultiplied_semi_transparent() {
    let c = Color::from_rgba(100, 0, 0, 128);
    let p = c.premultiplied();
    let expected_r = (100u32 * 128 / 255) as u32;
    let expected = (128u32 << 24) | (expected_r & 0xFF);
    assert_eq!(p, expected);
}

#[test]
fn color_to_rgba() {
    let c = Color::from_rgba(10, 20, 30, 40);
    assert_eq!(c.to_rgba(), 0x28_1E_14_0A);
}

#[test]
fn color_black_and_white() {
    assert_eq!(Color::black(), Color::from_rgb(0, 0, 0));
    assert_eq!(Color::white(), Color::from_rgb(255, 255, 255));
}

#[test]
fn color_transparent() {
    assert_eq!(Color::transparent(), Color::from_rgba(0, 0, 0, 0));
}

// ════════════════════════════════════════════════════════════════════════════
// blur 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_blur_noop() {
    let mut pixels = vec![0xFFFFFFFFu32; 100];
    gaussian_blur(&mut pixels, 10, 10, Rect::new(0.0, 0.0, 10.0, 10.0), 0.0);
    assert!(pixels.iter().all(|&p| p == 0xFFFFFFFF));
}

#[test]
fn test_blur_small() {
    let mut pixels = vec![0u32; 400];
    pixels[11 * 20 + 10] = 0xFFFFFFFF;
    gaussian_blur(&mut pixels, 20, 20, Rect::new(0.0, 0.0, 20.0, 20.0), 3.0);
    let non_zero = pixels.iter().filter(|&&p| p != 0).count();
    assert!(non_zero > 1);
}

// ════════════════════════════════════════════════════════════════════════════
// flattener 测试
// ════════════════════════════════════════════════════════════════════════════

use uix::graphics::path::PathSegment;

#[test]
fn test_flatten_rect() {
    let segs = [
        PathSegment::MoveTo(Point::new(0.0, 0.0)),
        PathSegment::LineTo(Point::new(100.0, 0.0)),
        PathSegment::LineTo(Point::new(100.0, 100.0)),
        PathSegment::LineTo(Point::new(0.0, 100.0)),
        PathSegment::Close,
    ];
    let polys = flatten(&segs, 0.5);
    assert!(!polys.is_empty());
    assert!(!polys[0].is_empty());
}

#[test]
fn test_flatten_quad() {
    let segs = [
        PathSegment::MoveTo(Point::new(0.0, 0.0)),
        PathSegment::QuadTo(Point::new(50.0, 100.0), Point::new(100.0, 0.0)),
    ];
    let polys = flatten(&segs, 0.25);
    assert!(!polys.is_empty());
    assert!(polys[0].len() > 2);
}

// ════════════════════════════════════════════════════════════════════════════
// path 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_simple_rect_path() {
    let mut pb = PathBuilder::new();
    pb.move_to(0.0, 0.0)
        .line_to(100.0, 0.0)
        .line_to(100.0, 100.0)
        .line_to(0.0, 100.0)
        .close();
    let path = pb.build();
    assert_eq!(path.segments().len(), 5);
    assert!(!path.is_empty());
}

#[test]
fn test_bounds() {
    let mut pb = PathBuilder::new();
    pb.move_to(10.0, 20.0)
        .line_to(110.0, 20.0)
        .line_to(110.0, 80.0)
        .close();
    let path = pb.build();
    let bounds = path.bounds().unwrap();
    assert!((bounds.x - 10.0).abs() < 0.001);
    assert!((bounds.y - 20.0).abs() < 0.001);
    assert!((bounds.w - 100.0).abs() < 0.001);
    assert!((bounds.h - 60.0).abs() < 0.001);
}

// ════════════════════════════════════════════════════════════════════════════
// types 测试
// ════════════════════════════════════════════════════════════════════════════

// ── Radius ──

#[test]
fn radius_uniform() {
    let r = Radius::uniform(8.0);
    assert_eq!(r.tl, 8.0);
    assert_eq!(r.tr, 8.0);
    assert_eq!(r.br, 8.0);
    assert_eq!(r.bl, 8.0);
}

#[test]
fn radius_zero() {
    let r = Radius::zero();
    assert_eq!(r.tl, 0.0);
    assert_eq!(r.tr, 0.0);
    assert_eq!(r.br, 0.0);
    assert_eq!(r.bl, 0.0);
}

#[test]
fn radius_default_is_zero() {
    let r = Radius::default();
    assert_eq!(r, Radius::zero());
}

// ── Transform ──

#[test]
fn transform_identity() {
    let t = Transform::identity();
    assert_eq!(t.m, [1.0, 0.0, 0.0, 0.0, 1.0, 0.0]);
}

#[test]
fn transform_translate() {
    let t = Transform::translate(10.0, 20.0);
    assert_eq!(t.m, [1.0, 0.0, 10.0, 0.0, 1.0, 20.0]);
}

#[test]
fn transform_scale() {
    let t = Transform::scale(2.0, 3.0);
    assert_eq!(t.m, [2.0, 0.0, 0.0, 0.0, 3.0, 0.0]);
}

#[test]
fn transform_default_is_identity() {
    assert_eq!(Transform::default(), Transform::identity());
}

// ── DirtyRegion ──

#[test]
fn dirty_region_empty() {
    let d = DirtyRegion::empty();
    assert!(!d.full_frame);
    assert!(!d.clear_required);
    assert!(d.rects.is_empty());
    assert!(d.is_empty());
}

#[test]
fn dirty_region_full() {
    let d = DirtyRegion::full();
    assert!(d.full_frame);
    assert!(d.clear_required);
}

#[test]
fn dirty_region_area() {
    let d = DirtyRegion::area(Rect::new(10.0, 20.0, 100.0, 50.0));
    assert!(!d.full_frame);
    assert!(d.clear_required);
    assert_eq!(d.rects.len(), 1);
    assert_eq!(d.rects[0], Rect::new(10.0, 20.0, 100.0, 50.0));
}

#[test]
fn dirty_region_zero_area_rect_is_empty() {
    let d = DirtyRegion::area(Rect::new(0.0, 0.0, 0.0, 0.0));
    assert!(d.rects.is_empty());
}

#[test]
fn dirty_region_add_rect() {
    let mut d = DirtyRegion::empty();
    d.add_rect(Rect::new(0.0, 0.0, 10.0, 10.0));
    d.add_rect(Rect::new(20.0, 20.0, 5.0, 5.0));
    assert_eq!(d.rects.len(), 2);
    assert!(d.clear_required);
}

#[test]
fn dirty_region_bounds() {
    let mut d = DirtyRegion::empty();
    d.add_rect(Rect::new(0.0, 0.0, 10.0, 10.0));
    d.add_rect(Rect::new(20.0, 20.0, 10.0, 10.0));
    let b = d.bounds();
    assert_eq!(b.x, 0.0);
    assert_eq!(b.y, 0.0);
    assert_eq!(b.w, 30.0);
    assert_eq!(b.h, 30.0);
}

#[test]
fn dirty_region_intersects() {
    let mut d = DirtyRegion::empty();
    d.add_rect(Rect::new(10.0, 10.0, 50.0, 50.0));
    assert!(d.intersects(Rect::new(20.0, 20.0, 5.0, 5.0)));
    assert!(!d.intersects(Rect::new(100.0, 100.0, 5.0, 5.0)));
}

#[test]
fn dirty_region_full_intersects_all() {
    let d = DirtyRegion::full();
    assert!(d.intersects(Rect::new(0.0, 0.0, 1.0, 1.0)));
    assert!(d.intersects(Rect::new(9999.0, 9999.0, 1.0, 1.0)));
}

#[test]
fn dirty_region_reset() {
    let mut d = DirtyRegion::full();
    d.reset();
    assert!(d.is_empty());
    assert!(!d.full_frame);
}

#[test]
fn dirty_region_merge_threshold() {
    let mut d = DirtyRegion::empty();
    for i in 0..16 {
        d.add_rect(Rect::new(i as f32, 0.0, 1.0, 1.0));
    }
    assert_eq!(d.rects.len(), 16);
    d.add_rect(Rect::new(99.0, 0.0, 1.0, 1.0));
    assert_eq!(d.rects.len(), 1);
    d.add_rect(Rect::new(100.0, 0.0, 1.0, 1.0));
    assert_eq!(d.rects.len(), 2);
}

// ── Misc types ──

#[test]
fn image_handle_default() {
    let h = ImageHandle::default();
    assert_eq!(h.0, 0);
}

#[test]
fn font_handle_new() {
    let h = FontHandle::new(42);
    assert_eq!(h.0, 42);
}

#[test]
fn font_handle_default() {
    assert_eq!(FontHandle::default(), FontHandle::new(0));
}

#[test]
fn text_layout_defaults() {
    let opts = TextLayoutOptions::default();
    assert_eq!(opts.max_width, f32::MAX);
    assert!(opts.word_wrap);
    assert_eq!(opts.h_align, HAlign::Left);
    assert_eq!(opts.v_align, VAlign::Top);
    assert_eq!(opts.font_size, 14.0);
}

#[test]
fn blend_mode_default_is_alpha() {
    assert_eq!(BlendMode::default(), BlendMode::Alpha);
}
