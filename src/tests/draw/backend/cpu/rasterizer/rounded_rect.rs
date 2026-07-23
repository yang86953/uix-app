use crate::core::Rect;
use crate::draw::backend::cpu::rasterizer::{
    clip_to_int, color_to_premul, fill, intersect_rect, put_pixel_aa, rounded_rect_sdf,
    sdf_to_coverage, stroke,
};
use crate::draw::backend::cpu::software_rasterizer::SoftwareRasterizer;
use crate::draw::{Color, Radius};

const WIDTH: i32 = 64;
const HEIGHT: i32 = 48;

fn reference_fill(pixels: &mut [u32], clip: Rect, rect: Rect, color: Color, radius: Radius) {
    let color = color_to_premul(color.r, color.g, color.b, color.a, 1.0);
    let (cx0, cy0, cx1, cy1) = clip_to_int(&clip);
    let expanded = Rect::new(rect.x - 1.0, rect.y - 1.0, rect.w + 2.0, rect.h + 2.0);
    let Some(clipped) = intersect_rect(&expanded, &clip) else {
        return;
    };
    for y in clipped.y as i32..(clipped.y + clipped.h) as i32 {
        for x in clipped.x as i32..(clipped.x + clipped.w) as i32 {
            let coverage = sdf_to_coverage(rounded_rect_sdf(
                x as f32 + 0.5,
                y as f32 + 0.5,
                &rect,
                &radius,
            ));
            if coverage > 0.0 {
                put_pixel_aa(pixels, WIDTH, x, y, cx0, cy0, cx1, cy1, color, coverage);
            }
        }
    }
}

fn reference_stroke(
    pixels: &mut [u32],
    clip: Rect,
    rect: Rect,
    color: Color,
    line_width: f32,
    radius: Radius,
) {
    let half_width = line_width.max(0.0) * 0.5;
    let color = color_to_premul(color.r, color.g, color.b, color.a, 1.0);
    let expand = half_width + 1.0;
    let expanded = Rect::new(
        rect.x - expand,
        rect.y - expand,
        rect.w + expand * 2.0,
        rect.h + expand * 2.0,
    );
    let Some(clipped) = intersect_rect(&expanded, &clip) else {
        return;
    };
    let (cx0, cy0, cx1, cy1) = clip_to_int(&clip);
    for y in clipped.y as i32..(clipped.y + clipped.h) as i32 {
        for x in clipped.x as i32..(clipped.x + clipped.w) as i32 {
            let distance = rounded_rect_sdf(x as f32 + 0.5, y as f32 + 0.5, &rect, &radius);
            let coverage = sdf_to_coverage(distance.abs() - half_width);
            if coverage > 0.0 {
                put_pixel_aa(pixels, WIDTH, x, y, cx0, cy0, cx1, cy1, color, coverage);
            }
        }
    }
}

fn cases() -> [(Rect, Radius, Rect); 4] {
    [
        (
            Rect::new(5.0, 4.0, 40.0, 30.0),
            Radius::uniform(8.0),
            Rect::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32),
        ),
        (
            Rect::new(4.25, 3.75, 42.5, 31.25),
            Radius {
                tl: 3.0,
                tr: 9.0,
                br: 12.0,
                bl: 5.0,
            },
            Rect::new(8.3, 6.2, 34.7, 25.6),
        ),
        (
            Rect::new(10.5, 8.25, 18.0, 15.0),
            Radius {
                tl: 12.0,
                tr: 2.0,
                br: 10.0,
                bl: 4.0,
            },
            Rect::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32),
        ),
        (
            Rect::new(-3.75, 12.5, 55.25, 22.75),
            Radius {
                tl: 0.5,
                tr: 14.0,
                br: 6.5,
                bl: 11.0,
            },
            Rect::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32),
        ),
    ]
}

#[test]
fn scanline_rounded_fill_matches_full_sdf_reference() {
    let color = Color::from_rgba(220, 80, 40, 160);
    for (rect, radius, clip) in cases() {
        let mut expected = vec![0xFF20_3040; (WIDTH * HEIGHT) as usize];
        let mut actual = expected.clone();
        reference_fill(&mut expected, clip, rect, color, radius);
        fill::fill_rect(
            &mut actual,
            WIDTH,
            HEIGHT,
            clip,
            1.0,
            rect,
            color,
            Some(radius),
        );
        assert_eq!(
            actual, expected,
            "rounded fill mismatch for {rect:?} {radius:?}"
        );
    }
}

#[test]
fn scanline_rounded_stroke_matches_full_sdf_reference() {
    let color = Color::from_rgba(30, 180, 220, 190);
    for (rect, radius, clip) in cases() {
        for line_width in [0.0, 1.0, 2.5, 6.0] {
            let mut expected = vec![0xFF30_2010; (WIDTH * HEIGHT) as usize];
            let mut actual = expected.clone();
            reference_stroke(&mut expected, clip, rect, color, line_width, radius);
            stroke::stroke_rect(
                &mut actual,
                WIDTH,
                HEIGHT,
                clip,
                1.0,
                rect,
                color,
                line_width,
                Some(radius),
            );
            assert_eq!(
                actual, expected,
                "rounded stroke mismatch for {rect:?} {radius:?} width={line_width}"
            );
        }
    }
}

#[test]
fn shared_cpu_scanline_rounded_fill_matches_full_sdf_reference() {
    let color = Color::from_rgba(220, 80, 40, 160);
    for (rect, radius, clip) in cases() {
        let mut expected = vec![0xFF20_3040; (WIDTH * HEIGHT) as usize];
        let mut actual = expected.clone();
        reference_fill(&mut expected, clip, rect, color, radius);
        let mut renderer = SoftwareRasterizer::new(WIDTH, HEIGHT);
        renderer.push_clip_surface(clip);
        renderer.fill_rect(&mut actual, WIDTH, HEIGHT, rect, color, Some(radius));
        assert_eq!(
            actual, expected,
            "shared CPU rounded fill mismatch for {rect:?} {radius:?}"
        );
    }
}

#[test]
fn shared_cpu_scanline_rounded_stroke_matches_full_sdf_reference() {
    let color = Color::from_rgba(30, 180, 220, 190);
    for (rect, radius, clip) in cases() {
        for line_width in [0.0, 1.0, 2.5, 6.0] {
            let mut expected = vec![0xFF30_2010; (WIDTH * HEIGHT) as usize];
            let mut actual = expected.clone();
            reference_stroke(&mut expected, clip, rect, color, line_width, radius);
            let mut renderer = SoftwareRasterizer::new(WIDTH, HEIGHT);
            renderer.push_clip_surface(clip);
            renderer.stroke_rect(
                &mut actual,
                WIDTH,
                HEIGHT,
                rect,
                color,
                line_width,
                Some(radius),
            );
            assert_eq!(
                actual, expected,
                "shared CPU rounded stroke mismatch for {rect:?} {radius:?} width={line_width}"
            );
        }
    }
}
