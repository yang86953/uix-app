//! uix-graphics crate 集成测试。

use uix_graphics::blur::gaussian_blur;
use uix_graphics::color::{colors, Color};
use uix_graphics::flattener::flatten;
use uix_graphics::path::{FillRule, LineCap, LineJoin, PathBuilder, PathSegment};
use uix_graphics::rasterizer::core::{
    apply_opacity, blend_srcover, clip_to_int, color_to_premul, fill_rect_raw, fill_span,
    intersect_rect, line_segment_sdf, premul, put_pixel, put_pixel_aa, rect_to_pixels,
    rounded_rect_sdf, sdf_to_coverage, sdf_to_coverage_aa, shadow_coverage,
    shadow_coverage_ambient,
};
use uix_graphics::stroker::{stroke_path, StrokeOptions};
use uix_graphics::types::{
    BlendMode, DirtyRegion, FontHandle, HAlign, ImageHandle, Radius, TextLayoutOptions, Transform,
    VAlign,
};
use uix_platform::geometry::{Point, Rect};

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

// ════════════════════════════════════════════════════════════════════════════
// color 扩展测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn color_red_green_blue() {
    assert_eq!(Color::red(), Color::from_rgb(255, 0, 0));
    assert_eq!(Color::green(), Color::from_rgb(0, 255, 0));
    assert_eq!(Color::blue(), Color::from_rgb(0, 0, 255));
}

#[test]
fn color_lighten_factor_zero() {
    let c = Color::from_rgb(100, 100, 100);
    assert_eq!(c.lighten(0.0), c);
}

#[test]
fn color_lighten_factor_one() {
    let c = Color::from_rgb(100, 100, 100);
    assert_eq!(c.lighten(1.0), Color::white());
}

#[test]
fn color_lighten_factor_half() {
    let c = Color::from_rgb(100, 100, 100);
    let l = c.lighten(0.5);
    // each channel: 100 + (255-100)*0.5 = 100 + 77.5 = 177.5 -> 177
    assert_eq!(l, Color::from_rgb(177, 177, 177));
}

#[test]
fn color_darken_factor_zero() {
    let c = Color::from_rgb(100, 100, 100);
    assert_eq!(c.darken(0.0), c);
}

#[test]
fn color_darken_factor_one() {
    let c = Color::from_rgb(100, 100, 100);
    assert_eq!(c.darken(1.0), Color::black());
}

#[test]
fn color_mix_t_zero() {
    let a = Color::from_rgb(255, 0, 0);
    let b = Color::from_rgb(0, 255, 0);
    assert_eq!(a.mix(&b, 0.0), a);
}

#[test]
fn color_mix_t_one() {
    let a = Color::from_rgb(255, 0, 0);
    let b = Color::from_rgb(0, 255, 0);
    assert_eq!(a.mix(&b, 1.0), b);
}

#[test]
fn color_mix_t_half() {
    let a = Color::from_rgb(255, 0, 0);
    let b = Color::from_rgb(0, 255, 0);
    let m = a.mix(&b, 0.5);
    assert_eq!(m, Color::from_rgb(127, 127, 0));
}

#[test]
fn color_mix_with_alpha() {
    let a = Color::from_rgba(255, 0, 0, 200);
    let b = Color::from_rgba(0, 255, 0, 100);
    let m = a.mix(&b, 0.5);
    assert_eq!(m, Color::from_rgba(127, 127, 0, 150));
}

#[test]
fn color_with_alpha() {
    let c = Color::from_rgb(255, 128, 64);
    let c = c.with_alpha(128);
    assert_eq!(c, Color::from_rgba(255, 128, 64, 128));
}

#[test]
fn color_luminance() {
    // sRGB weights: R*0.2126 + G*0.7152 + B*0.0722
    let c = Color::from_rgb(255, 255, 255);
    assert_eq!(c.luminance(), 255);
    let c = Color::from_rgb(0, 0, 0);
    assert_eq!(c.luminance(), 0);
    // pure green dominates luminance
    let c = Color::from_rgb(0, 255, 0);
    assert_eq!(c.luminance(), (255.0 * 0.7152) as u8);
}

#[test]
fn color_is_light() {
    assert!(!Color::black().is_light());
    assert!(Color::white().is_light());
}

#[test]
fn color_display() {
    let c = Color::from_rgba(10, 20, 30, 40);
    assert_eq!(format!("{}", c), "#0A141E28");
}

#[test]
fn color_default_is_black() {
    assert_eq!(Color::default(), Color::black());
}

#[test]
fn colors_module_constants() {
    assert_eq!(colors::PRIMARY, Color::from_rgb(24, 144, 255));
    assert_eq!(colors::SUCCESS, Color::from_rgb(82, 196, 26));
    assert_eq!(colors::WARNING, Color::from_rgb(250, 173, 20));
    assert_eq!(colors::DANGER, Color::from_rgb(255, 77, 79));
    assert_eq!(colors::INFO, Color::from_rgb(22, 119, 255));
    assert_eq!(colors::BG_DARK, Color::from_rgb(30, 30, 30));
    assert_eq!(colors::BG_LIGHT, Color::from_rgb(245, 245, 245));
    assert_eq!(colors::SURFACE_DARK, Color::from_rgb(45, 45, 45));
    assert_eq!(colors::SURFACE_LIGHT, Color::from_rgb(255, 255, 255));
    assert_eq!(colors::TEXT_DARK, Color::from_rgb(200, 200, 200));
    assert_eq!(colors::TEXT_LIGHT, Color::from_rgb(51, 51, 51));
    assert_eq!(colors::DISABLED, Color::from_rgb(191, 191, 191));
    assert_eq!(colors::BORDER, Color::from_rgb(217, 217, 217));
}

// ════════════════════════════════════════════════════════════════════════════
// rasterizer/core 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn premul_opaque() {
    // 完全不透明颜色不变
    assert_eq!(premul(0xFFFF8000), 0xFFFF8000);
}

#[test]
fn premul_transparent() {
    // 完全透明返回 0
    assert_eq!(premul(0x00FF8000), 0x00000000);
}

#[test]
fn premul_semi_transparent() {
    // 半透明正确预乘: a=128, r=255*128/255=128, g=128*128/255=64, b=0
    assert_eq!(premul(0x80FF8000), 0x80804000);
}

#[test]
fn blend_srcover_opaque_src() {
    // src 完全不透明 -> 覆盖 dst
    assert_eq!(blend_srcover(255, 0, 255, 0, 0, 0, 0, 0), 0xFFFF0000);
    assert_eq!(
        blend_srcover(255, 255, 255, 255, 255, 128, 128, 128),
        0xFFFFFFFF
    );
}

#[test]
fn blend_srcover_transparent_src() {
    // src 完全透明 -> 保留 dst
    assert_eq!(blend_srcover(0, 255, 0, 0, 0, 255, 128, 64), 0xFFFF8040);
}

#[test]
fn blend_srcover_semi_transparent() {
    // src 半透明混合
    let result = blend_srcover(128, 128, 200, 100, 50, 100, 200, 50);
    let out_a = 128 + 128 - (128 * 128 / 255);
    let out_r = 200 + (100 * (255 - 128) / 255);
    let out_g = 100 + (200 * (255 - 128) / 255);
    let out_b = 50 + (50 * (255 - 128) / 255);
    let expected =
        (out_a.min(255) << 24) | (out_r.min(255) << 16) | (out_g.min(255) << 8) | out_b.min(255);
    assert_eq!(result, expected);
}

#[test]
fn apply_opacity_full() {
    let c = 0xFFFF8000;
    assert_eq!(apply_opacity(c, 1.0), c);
}

#[test]
fn apply_opacity_zero() {
    assert_eq!(apply_opacity(0xFFFF8000, 0.0), 0x00000000);
}

#[test]
fn apply_opacity_partial() {
    // channels = 255,255,128,0 scaled by 0.5
    let result = apply_opacity(0xFFFF8000, 0.5);
    assert_eq!(result, 0x7F7F4000);
}

#[test]
fn color_to_premul_opaque() {
    assert_eq!(color_to_premul(255, 128, 64, 255, 1.0), 0xFFFF8040);
}

#[test]
fn color_to_premul_zero_alpha() {
    assert_eq!(color_to_premul(100, 100, 100, 0, 0.5), 0x00000000);
}

#[test]
fn color_to_premul_partial_opacity() {
    // a=128*0.5=64, r=255*64/255=64, g=128*64/255=32, b=64*64/255=16
    assert_eq!(color_to_premul(255, 128, 64, 128, 0.5), 0x40402010);
}

#[test]
fn put_pixel_basic_write() {
    let mut pixels = vec![0u32; 16];
    put_pixel(&mut pixels, 4, 1, 1, 0, 0, 4, 4, 0xFFFFFFFF);
    assert_eq!(pixels[5], 0xFFFFFFFF);
}

#[test]
fn put_pixel_clip_left() {
    let mut pixels = vec![0u32; 16];
    put_pixel(&mut pixels, 4, -1, 1, 0, 0, 4, 4, 0xFFFFFFFF);
    assert_eq!(pixels[5], 0);
}

#[test]
fn put_pixel_clip_right() {
    let mut pixels = vec![0u32; 16];
    put_pixel(&mut pixels, 4, 4, 1, 0, 0, 4, 4, 0xFFFFFFFF);
    assert_eq!(pixels[5], 0);
}

#[test]
fn put_pixel_clip_top() {
    let mut pixels = vec![0u32; 16];
    put_pixel(&mut pixels, 4, 1, -1, 0, 0, 4, 4, 0xFFFFFFFF);
    assert_eq!(pixels[5], 0);
}

#[test]
fn put_pixel_clip_bottom() {
    let mut pixels = vec![0u32; 16];
    put_pixel(&mut pixels, 4, 1, 4, 0, 0, 4, 4, 0xFFFFFFFF);
    assert_eq!(pixels[5], 0);
}

#[test]
fn put_pixel_out_of_buffer_bounds() {
    let mut pixels = vec![0u32; 16];
    // y * stride + x = 5*4 + 0 = 20 >= 16, no crash
    put_pixel(&mut pixels, 4, 0, 5, 0, 0, 10, 10, 0xFFFFFFFF);
    assert_eq!(pixels[0], 0);
}

#[test]
fn put_pixel_zero_alpha_does_nothing() {
    let mut pixels = vec![0x12345678u32; 16];
    put_pixel(&mut pixels, 4, 1, 1, 0, 0, 4, 4, 0x00FF8000);
    assert_eq!(pixels[5], 0x12345678);
}

#[test]
fn put_pixel_blend() {
    let mut pixels = vec![0u32; 16];
    // dst is black, src is semi-transparent red
    put_pixel(&mut pixels, 4, 1, 1, 0, 0, 4, 4, 0x80FF0000);
    // out = blend_srcover(128, 0, 255, 0, 0, 0, 0, 0)
    // out_a = 128, out_r = 255, out_g = 0, out_b = 0
    assert_eq!(pixels[5], 0x80FF0000);
}

#[test]
fn put_pixel_aa_full_coverage() {
    let mut pixels = vec![0u32; 16];
    put_pixel_aa(&mut pixels, 4, 1, 1, 0, 0, 4, 4, 0xFFFF8000, 1.0);
    // coverage >= 1.0 -> equals put_pixel
    assert_eq!(pixels[5], 0xFFFF8000);
}

#[test]
fn put_pixel_aa_zero_coverage() {
    let mut pixels = vec![0u32; 16];
    put_pixel_aa(&mut pixels, 4, 1, 1, 0, 0, 4, 4, 0xFFFF8000, 0.0);
    assert_eq!(pixels[5], 0);
}

#[test]
fn put_pixel_aa_partial_coverage() {
    let mut pixels = vec![0u32; 16];
    put_pixel_aa(&mut pixels, 4, 1, 1, 0, 0, 4, 4, 0x80FF0000, 0.5);
    // dst=0, src_a_premul=128*0.5=64, src_r_premul=255*0.5=127.5
    // inv = 1.0 - 64/255 ≈ 0.749, out_a = 64, out_r_p = 127.5
    let src_a_s = 128.0f32 * 0.5;
    let src_r_p = 255.0f32 * 0.5;
    let _inv = 1.0 - src_a_s / 255.0;
    let out_a = src_a_s;
    let out_r_p = src_r_p;
    let expected: u32 =
        ((out_a.round() as u32).min(255) << 24) | ((out_r_p.round() as u32).min(255) << 16);
    assert_eq!(pixels[5], expected);
}

#[test]
fn fill_span_opaque_memset() {
    let mut pixels = vec![0u32; 16];
    fill_span(&mut pixels, 4, 0, 4, 1, 0, 0, 4, 4, 0xFFFF0000);
    for x in 0..4 {
        assert_eq!(pixels[4 + x], 0xFFFF0000);
    }
}

#[test]
fn fill_span_alpha_per_pixel() {
    let mut pixels = vec![0u32; 16];
    fill_span(&mut pixels, 4, 0, 4, 1, 0, 0, 4, 4, 0x80FF0000);
    // each pixel gets blend_srcover with dst=0
    for x in 0..4 {
        assert_eq!(pixels[4 + x], 0x80FF0000);
    }
}

#[test]
fn fill_span_clip_boundaries() {
    let mut pixels = vec![0u32; 16];
    fill_span(&mut pixels, 4, -2, 6, 1, 0, 0, 4, 4, 0xFFFF0000);
    // only x in [0, 4) should be filled
    for x in 0..4 {
        assert_eq!(pixels[4 + x], 0xFFFF0000);
    }
}

#[test]
fn fill_span_clip_outside_y() {
    let mut pixels = vec![0u32; 16];
    fill_span(&mut pixels, 4, 0, 4, -1, 0, 0, 4, 4, 0xFFFF0000);
    assert!(pixels.iter().all(|&p| p == 0));
}

#[test]
fn fill_span_clip_outside_x() {
    let mut pixels = vec![0u32; 16];
    fill_span(&mut pixels, 4, -4, -2, 1, 0, 0, 4, 4, 0xFFFF0000);
    assert!(pixels.iter().all(|&p| p == 0));
}

#[test]
fn fill_rect_raw_fills_region() {
    let mut pixels = vec![0u32; 36];
    fill_rect_raw(&mut pixels, 6, 1, 1, 3, 3, 0, 0, 6, 6, 0xFFFF0000);
    for y in 1..4 {
        for x in 1..4 {
            assert_eq!(pixels[(y * 6 + x) as usize], 0xFFFF0000);
        }
    }
    // outside rect remains 0
    assert_eq!(pixels[0], 0);
    assert_eq!(pixels[5], 0);
}

#[test]
fn rect_to_pixels_exact() {
    let r = Rect::new(10.0, 20.0, 100.0, 50.0);
    let (x, y, w, h) = rect_to_pixels(&r);
    assert_eq!(x, 10);
    assert_eq!(y, 20);
    assert_eq!(w, 100);
    assert_eq!(h, 50);
}

#[test]
fn rect_to_pixels_fractional() {
    let r = Rect::new(10.3, 20.7, 100.5, 50.2);
    let (x, y, w, h) = rect_to_pixels(&r);
    assert_eq!(x, 10);
    assert_eq!(y, 21);
    assert_eq!(w, 101);
    assert_eq!(h, 50);
}

#[test]
fn clip_to_int_conversion() {
    let r = Rect::new(10.0, 20.0, 100.0, 50.0);
    let (x0, y0, x1, y1) = clip_to_int(&r);
    assert_eq!(x0, 10);
    assert_eq!(y0, 20);
    assert_eq!(x1, 110);
    assert_eq!(y1, 70);
}

#[test]
fn intersect_rect_overlapping() {
    let a = Rect::new(0.0, 0.0, 100.0, 100.0);
    let b = Rect::new(50.0, 50.0, 100.0, 100.0);
    let result = intersect_rect(&a, &b);
    assert!(result.is_some());
    let r = result.unwrap();
    assert!((r.x - 50.0).abs() < 0.001);
    assert!((r.y - 50.0).abs() < 0.001);
    assert!((r.w - 50.0).abs() < 0.001);
    assert!((r.h - 50.0).abs() < 0.001);
}

#[test]
fn intersect_rect_non_overlapping() {
    let a = Rect::new(0.0, 0.0, 100.0, 100.0);
    let b = Rect::new(200.0, 200.0, 100.0, 100.0);
    assert!(intersect_rect(&a, &b).is_none());
}

#[test]
fn intersect_rect_contained() {
    let a = Rect::new(0.0, 0.0, 100.0, 100.0);
    let b = Rect::new(10.0, 10.0, 20.0, 20.0);
    let result = intersect_rect(&a, &b);
    assert!(result.is_some());
    let r = result.unwrap();
    assert!((r.x - 10.0).abs() < 0.001);
    assert!((r.y - 10.0).abs() < 0.001);
    assert!((r.w - 20.0).abs() < 0.001);
    assert!((r.h - 20.0).abs() < 0.001);
}

#[test]
fn rounded_rect_sdf_zero_radius() {
    let r = Rect::new(0.0, 0.0, 100.0, 100.0);
    let rad = Radius::zero();
    // 中心点内部 -> negative
    let inside = rounded_rect_sdf(50.0, 50.0, &r, &rad);
    assert!(inside < 0.0);
    // 外部点 -> positive
    let outside = rounded_rect_sdf(200.0, 200.0, &r, &rad);
    assert!(outside > 0.0);
}

#[test]
fn rounded_rect_sdf_point_inside() {
    let r = Rect::new(0.0, 0.0, 100.0, 100.0);
    let rad = Radius::uniform(10.0);
    let d = rounded_rect_sdf(10.0, 10.0, &r, &rad);
    assert!(d < 0.0);
}

#[test]
fn rounded_rect_sdf_point_outside() {
    let r = Rect::new(0.0, 0.0, 100.0, 100.0);
    let rad = Radius::uniform(10.0);
    let d = rounded_rect_sdf(200.0, 200.0, &r, &rad);
    assert!(d > 0.0);
}

#[test]
fn rounded_rect_sdf_with_radius() {
    let r = Rect::new(0.0, 0.0, 100.0, 100.0);
    let rad = Radius::uniform(10.0);
    // 在圆角内侧一点
    let d = rounded_rect_sdf(5.0, 5.0, &r, &rad);
    // 由于圆角，距离应该大于零半径时
    let d_zero = rounded_rect_sdf(5.0, 5.0, &r, &Radius::zero());
    assert!(d > d_zero);
}

#[test]
fn line_segment_sdf_point_on_line() {
    let d = line_segment_sdf(50.0, 0.0, 0.0, 0.0, 100.0, 0.0);
    assert!(d.abs() < 0.001);
}

#[test]
fn line_segment_sdf_point_off_line() {
    let d = line_segment_sdf(50.0, 50.0, 0.0, 0.0, 100.0, 0.0);
    assert!((d - 50.0).abs() < 0.001);
}

#[test]
fn line_segment_sdf_degenerate() {
    let d = line_segment_sdf(50.0, 50.0, 100.0, 100.0, 100.0, 100.0);
    // 线段退化为点，返回点到该点的距离
    let dx = 50.0f32 - 100.0f32;
    let expected = (dx * dx + dx * dx).sqrt();
    assert!((d - expected).abs() < 0.001);
}

#[test]
fn sdf_to_coverage_exact_edge() {
    let c = sdf_to_coverage(0.0);
    assert!((c - 0.5).abs() < 0.001);
}

#[test]
fn sdf_to_coverage_inside() {
    let c = sdf_to_coverage(-0.5);
    assert!((c - 1.0).abs() < 0.001);
}

#[test]
fn sdf_to_coverage_outside() {
    let c = sdf_to_coverage(1.0);
    assert!((c - 0.0).abs() < 0.001);
}

#[test]
fn sdf_to_coverage_aa_custom_half() {
    let c = sdf_to_coverage_aa(0.0, 1.0);
    assert!((c - 0.5).abs() < 0.001);
    let c = sdf_to_coverage_aa(-1.0, 1.0);
    assert!((c - 1.0).abs() < 0.001);
    let c = sdf_to_coverage_aa(1.0, 1.0);
    assert!((c - 0.0).abs() < 0.001);
}

#[test]
fn shadow_coverage_smooth_falloff() {
    // at sd=0, blur=10: t=0.5, result=0.5*0.5*(3-1)=0.5
    let c = shadow_coverage(0.0, 10.0);
    assert!((c - 0.5).abs() < 0.001);
    // at sd=blur: t=0, result=0
    let c = shadow_coverage(10.0, 10.0);
    assert!(c.abs() < 0.001);
    // at sd=-blur: t=1, result=1
    let c = shadow_coverage(-10.0, 10.0);
    assert!((c - 1.0).abs() < 0.001);
}

#[test]
fn shadow_coverage_ambient_softer_falloff() {
    // at sd=0, blur=10: half=5, t=5/15=1/3
    let c = shadow_coverage_ambient(0.0, 10.0);
    let expected = {
        let half = 10.0 * 0.5;
        let t = (half - 0.0) / (10.0 + half);
        let t2 = t * t;
        t2 * t2 * (5.0 - 4.0 * t)
    };
    assert!((c - expected).abs() < 0.001);
    // fully inside shadow
    let c = shadow_coverage_ambient(-10.0, 10.0);
    assert!((c - 1.0).abs() < 0.001);
    // fully outside shadow
    let c = shadow_coverage_ambient(10.0, 10.0);
    assert!(c.abs() < 0.001);
}

// ════════════════════════════════════════════════════════════════════════════
// path 扩展测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn path_quad_to() {
    let mut pb = PathBuilder::new();
    pb.move_to(0.0, 0.0).quad_to(50.0, 100.0, 100.0, 0.0);
    let path = pb.build();
    assert_eq!(path.segments().len(), 2);
    match path.segments()[1] {
        PathSegment::QuadTo(c, e) => {
            assert!((c.x - 50.0).abs() < 0.001);
            assert!((c.y - 100.0).abs() < 0.001);
            assert!((e.x - 100.0).abs() < 0.001);
            assert!((e.y - 0.0).abs() < 0.001);
        }
        _ => panic!("expected QuadTo"),
    }
}

#[test]
fn path_cubic_to() {
    let mut pb = PathBuilder::new();
    pb.move_to(0.0, 0.0)
        .cubic_to(25.0, 75.0, 75.0, 75.0, 100.0, 0.0);
    let path = pb.build();
    assert_eq!(path.segments().len(), 2);
    match path.segments()[1] {
        PathSegment::CubicTo(c1, c2, e) => {
            assert!((c1.x - 25.0).abs() < 0.001);
            assert!((c1.y - 75.0).abs() < 0.001);
            assert!((c2.x - 75.0).abs() < 0.001);
            assert!((c2.y - 75.0).abs() < 0.001);
            assert!((e.x - 100.0).abs() < 0.001);
            assert!((e.y - 0.0).abs() < 0.001);
        }
        _ => panic!("expected CubicTo"),
    }
}

#[test]
fn path_translated_shifts_all_points() {
    let mut pb = PathBuilder::new();
    pb.move_to(10.0, 10.0)
        .line_to(100.0, 10.0)
        .line_to(100.0, 100.0)
        .close();
    let path = pb.build();
    let t = path.translated(5.0, 10.0);
    let segs = t.segments();
    match segs[0] {
        PathSegment::MoveTo(p) => {
            assert!((p.x - 15.0).abs() < 0.001);
            assert!((p.y - 20.0).abs() < 0.001);
        }
        _ => panic!("expected MoveTo"),
    }
    match segs[1] {
        PathSegment::LineTo(p) => {
            assert!((p.x - 105.0).abs() < 0.001);
            assert!((p.y - 20.0).abs() < 0.001);
        }
        _ => panic!("expected LineTo"),
    }
    // Close 不变
    match segs[3] {
        PathSegment::Close => {}
        _ => panic!("expected Close"),
    }
}

#[test]
fn path_is_empty_new_path() {
    let pb = PathBuilder::new();
    let path = pb.build();
    assert!(path.is_empty());
}

#[test]
fn path_is_empty_after_build() {
    let mut pb = PathBuilder::new();
    pb.move_to(0.0, 0.0);
    let path = pb.build();
    assert!(!path.is_empty());
}

#[test]
fn path_segment_points() {
    let p = Point::new(10.0, 20.0);
    assert_eq!(PathSegment::MoveTo(p).points(), &[p]);
    assert_eq!(PathSegment::LineTo(p).points(), &[p]);
    assert!(PathSegment::QuadTo(p, p).points().is_empty());
    assert!(PathSegment::CubicTo(p, p, p).points().is_empty());
    assert!(PathSegment::Close.points().is_empty());
}

#[test]
fn path_segment_all_points() {
    let p1 = Point::new(10.0, 20.0);
    let p2 = Point::new(30.0, 40.0);
    let p3 = Point::new(50.0, 60.0);
    assert_eq!(PathSegment::MoveTo(p1).all_points(), vec![p1]);
    assert_eq!(PathSegment::LineTo(p1).all_points(), vec![p1]);
    assert_eq!(PathSegment::QuadTo(p1, p2).all_points(), vec![p1, p2]);
    assert_eq!(
        PathSegment::CubicTo(p1, p2, p3).all_points(),
        vec![p1, p2, p3]
    );
    assert!(PathSegment::Close.all_points().is_empty());
}

#[test]
fn fill_rule_variants() {
    assert_eq!(FillRule::NonZero as u8, 0);
    assert_eq!(FillRule::EvenOdd as u8, 1);
}

#[test]
fn line_cap_variants() {
    assert_eq!(LineCap::Butt as u8, 0);
    assert_eq!(LineCap::Round as u8, 1);
    assert_eq!(LineCap::Square as u8, 2);
}

#[test]
fn line_join_variants() {
    assert_eq!(LineJoin::Miter as u8, 0);
    assert_eq!(LineJoin::Round as u8, 1);
    assert_eq!(LineJoin::Bevel as u8, 2);
}

#[test]
fn path_builder_has_segments() {
    let mut pb = PathBuilder::new();
    assert!(!pb.has_segments());
    pb.move_to(0.0, 0.0);
    assert!(pb.has_segments());
}

#[test]
fn path_empty_bounds_none() {
    let path = PathBuilder::new().build();
    assert!(path.bounds().is_none());
}

#[test]
fn path_cubic_bounds() {
    let mut pb = PathBuilder::new();
    pb.move_to(0.0, 0.0)
        .cubic_to(50.0, 100.0, 100.0, 100.0, 150.0, 0.0);
    let path = pb.build();
    let b = path.bounds().unwrap();
    assert!((b.x - 0.0).abs() < 0.001);
    assert!((b.y - 0.0).abs() < 0.001);
    assert!(b.w >= 150.0);
    assert!(b.h >= 0.0);
}

// ════════════════════════════════════════════════════════════════════════════
// stroker 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn stroke_path_simple() {
    let mut pb = PathBuilder::new();
    pb.move_to(10.0, 10.0)
        .line_to(100.0, 10.0)
        .line_to(100.0, 100.0)
        .close();
    let path = pb.build();
    let result = stroke_path(&path, &StrokeOptions::default());
    assert!(!result.is_empty());
    // stroke_path 应生成至少 4 段（两侧偏移 + cap + 闭合）
    assert!(result.segments().len() >= 4);
}

#[test]
fn stroke_path_zero_width() {
    let mut pb = PathBuilder::new();
    pb.move_to(10.0, 10.0).line_to(100.0, 10.0);
    let path = pb.build();
    let options = StrokeOptions {
        width: 0.0,
        ..StrokeOptions::default()
    };
    let result = stroke_path(&path, &options);
    assert!(result.is_empty());
}

#[test]
fn stroke_path_cap_butt() {
    let mut pb = PathBuilder::new();
    pb.move_to(10.0, 10.0).line_to(100.0, 10.0);
    let path = pb.build();
    let options = StrokeOptions {
        cap: LineCap::Butt,
        ..StrokeOptions::default()
    };
    let result = stroke_path(&path, &options);
    assert!(!result.is_empty());
}

#[test]
fn stroke_path_cap_round() {
    let mut pb = PathBuilder::new();
    pb.move_to(10.0, 10.0).line_to(100.0, 10.0);
    let path = pb.build();
    let options = StrokeOptions {
        cap: LineCap::Round,
        ..StrokeOptions::default()
    };
    let result = stroke_path(&path, &options);
    assert!(!result.is_empty());
}

#[test]
fn stroke_path_cap_square() {
    let mut pb = PathBuilder::new();
    pb.move_to(10.0, 10.0).line_to(100.0, 10.0);
    let path = pb.build();
    let options = StrokeOptions {
        cap: LineCap::Square,
        ..StrokeOptions::default()
    };
    let result = stroke_path(&path, &options);
    assert!(!result.is_empty());
}

#[test]
fn stroke_path_join_miter() {
    let mut pb = PathBuilder::new();
    pb.move_to(10.0, 10.0)
        .line_to(100.0, 10.0)
        .line_to(100.0, 100.0);
    let path = pb.build();
    let options = StrokeOptions {
        join: LineJoin::Miter,
        ..StrokeOptions::default()
    };
    let result = stroke_path(&path, &options);
    assert!(!result.is_empty());
}

#[test]
fn stroke_path_join_bevel() {
    let mut pb = PathBuilder::new();
    pb.move_to(10.0, 10.0)
        .line_to(100.0, 10.0)
        .line_to(100.0, 100.0);
    let path = pb.build();
    let options = StrokeOptions {
        join: LineJoin::Bevel,
        ..StrokeOptions::default()
    };
    let result = stroke_path(&path, &options);
    assert!(!result.is_empty());
}

#[test]
fn stroke_path_join_round() {
    let mut pb = PathBuilder::new();
    pb.move_to(10.0, 10.0)
        .line_to(100.0, 10.0)
        .line_to(100.0, 100.0);
    let path = pb.build();
    let options = StrokeOptions {
        join: LineJoin::Round,
        ..StrokeOptions::default()
    };
    let result = stroke_path(&path, &options);
    assert!(!result.is_empty());
}

#[test]
fn stroke_path_multiple_sub_paths() {
    let mut pb = PathBuilder::new();
    pb.move_to(10.0, 10.0)
        .line_to(50.0, 10.0)
        .close()
        .move_to(60.0, 10.0)
        .line_to(100.0, 10.0)
        .close();
    let path = pb.build();
    let result = stroke_path(&path, &StrokeOptions::default());
    assert!(!result.is_empty());
}

#[test]
fn stroke_path_single_point() {
    let mut pb = PathBuilder::new();
    pb.move_to(50.0, 50.0);
    let path = pb.build();
    let result = stroke_path(&path, &StrokeOptions::default());
    assert!(result.is_empty());
}

// ════════════════════════════════════════════════════════════════════════════
// flattener 扩展测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn test_flatten_cubic() {
    let segs = [
        PathSegment::MoveTo(Point::new(0.0, 0.0)),
        PathSegment::CubicTo(
            Point::new(33.0, 100.0),
            Point::new(66.0, 100.0),
            Point::new(100.0, 0.0),
        ),
    ];
    let polys = flatten(&segs, 0.25);
    assert!(!polys.is_empty());
    // 三次贝塞尔应展平为多于 2 个点（即多于一条直线）
    assert!(polys[0].len() > 2);
}

#[test]
fn test_flatten_multiple_sub_paths() {
    let segs = [
        PathSegment::MoveTo(Point::new(0.0, 0.0)),
        PathSegment::LineTo(Point::new(100.0, 0.0)),
        PathSegment::Close,
        PathSegment::MoveTo(Point::new(0.0, 50.0)),
        PathSegment::LineTo(Point::new(100.0, 50.0)),
        PathSegment::Close,
    ];
    let polys = flatten(&segs, 0.5);
    assert_eq!(polys.len(), 2);
    assert!(!polys[0].is_empty());
    assert!(!polys[1].is_empty());
}

#[test]
fn test_flatten_empty_segments() {
    let segs: [PathSegment; 0] = [];
    let polys = flatten(&segs, 0.5);
    assert!(polys.is_empty());
}
