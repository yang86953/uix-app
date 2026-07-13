use crate::draw::rasterizer::polygon::*;
use crate::tests::common::*;

fn reference_contains(polys: &[Vec<Point>], point: Point, fill_rule: FillRule) -> bool {
    let mut parity = false;
    let mut winding = 0i32;
    for poly in polys {
        for index in 0..poly.len() {
            let a = poly[index];
            let b = poly[(index + 1) % poly.len()];
            let upward = a.y <= point.y && point.y < b.y;
            let downward = b.y <= point.y && point.y < a.y;
            if !upward && !downward {
                continue;
            }
            let x = a.x + (point.y - a.y) * (b.x - a.x) / (b.y - a.y);
            if x > point.x {
                parity = !parity;
                winding += if upward { 1 } else { -1 };
            }
        }
    }
    match fill_rule {
        FillRule::EvenOdd => parity,
        FillRule::NonZero => winding != 0,
    }
}

fn assert_matches_reference(
    polys: &[Vec<Point>],
    width: i32,
    height: i32,
    clip: Rect,
    fill_rule: FillRule,
    expected_pixels: usize,
) {
    let mut pixels = vec![0u32; (width * height) as usize];
    let mut global_edges = Vec::new();
    let mut active_edges = Vec::new();
    fill_polygons(
        polys,
        &mut pixels,
        width,
        height,
        clip,
        0xFFFF_FFFF,
        fill_rule,
        &mut global_edges,
        &mut active_edges,
    );

    let mut reference_pixels = 0usize;
    for y in 0..height {
        for x in 0..width {
            let in_clip = clip.contains(Point::new(x as f32 + 0.5, y as f32 + 0.5));
            let expected = in_clip
                && reference_contains(polys, Point::new(x as f32 + 0.5, y as f32 + 0.5), fill_rule);
            let actual = pixels[(y * width + x) as usize] >> 24 != 0;
            assert_eq!(actual, expected, "coverage mismatch at ({x}, {y})");
            reference_pixels += usize::from(expected);
        }
    }
    assert_eq!(reference_pixels, expected_pixels);
}

#[test]
fn clipped_sloped_edge_activates_at_the_current_pixel_center() {
    let triangle = vec![vec![
        Point::new(8.0, 8.0),
        Point::new(56.0, 8.0),
        Point::new(56.0, 55.0),
    ]];
    assert_matches_reference(
        &triangle,
        64,
        64,
        Rect::new(0.0, 24.0, 64.0, 24.0),
        FillRule::NonZero,
        468,
    );
}

#[test]
fn open_subpaths_close_implicitly_and_preserve_fill_rules() {
    let same_winding = vec![
        vec![
            Point::new(8.0, 8.0),
            Point::new(48.0, 8.0),
            Point::new(48.0, 40.0),
            Point::new(8.0, 40.0),
        ],
        vec![
            Point::new(32.0, 24.0),
            Point::new(72.0, 24.0),
            Point::new(72.0, 56.0),
            Point::new(32.0, 56.0),
        ],
    ];
    let clip = Rect::new(0.0, 0.0, 80.0, 64.0);
    assert_matches_reference(&same_winding, 80, 64, clip, FillRule::EvenOdd, 2_048);
    assert_matches_reference(&same_winding, 80, 64, clip, FillRule::NonZero, 2_304);
}
