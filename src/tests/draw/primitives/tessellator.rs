use super::*;
use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::primitives::color::Color;
use crate::draw::primitives::path::PathBuilder;
use crate::draw::traits::Canvas2D;

fn add_rect(
    builder: &mut PathBuilder,
    x0: f32,
    y0: f32,
    x1: f32,
    y1: f32,
    positive_area: bool,
) {
    if positive_area {
        builder
            .move_to(x0, y0)
            .line_to(x1, y0)
            .line_to(x1, y1)
            .line_to(x0, y1)
            .close();
    } else {
        builder
            .move_to(x0, y0)
            .line_to(x0, y1)
            .line_to(x1, y1)
            .line_to(x1, y0)
            .close();
    }
}

fn add_outer(builder: &mut PathBuilder) {
    add_rect(builder, 0.0, 0.0, 20.0, 20.0, true);
}

fn add_inner(builder: &mut PathBuilder, clockwise: bool) {
    add_rect(builder, 5.0, 5.0, 15.0, 15.0, !clockwise);
}

fn nested_path(clockwise_inner: bool, inner_first: bool) -> Path {
    let mut builder = PathBuilder::new();
    if inner_first {
        add_inner(&mut builder, clockwise_inner);
        add_outer(&mut builder);
    } else {
        add_outer(&mut builder);
        add_inner(&mut builder, clockwise_inner);
    }
    builder.build()
}

fn triangle_mesh_area(vertices: &[f32]) -> f32 {
    vertices
        .chunks_exact(6)
        .map(|triangle| {
            ((triangle[2] - triangle[0]) * (triangle[5] - triangle[1])
                - (triangle[3] - triangle[1]) * (triangle[4] - triangle[0]))
                .abs()
                * 0.5
        })
        .sum()
}

fn triangle_mesh_contains(vertices: &[f32], point: Point) -> bool {
    vertices.chunks_exact(6).any(|triangle| {
        point_in_triangle(
            point,
            Point::new(triangle[0], triangle[1]),
            Point::new(triangle[2], triangle[3]),
            Point::new(triangle[4], triangle[5]),
        )
    })
}

fn triangle_mesh_strict_hits(vertices: &[f32], point: Point) -> usize {
    vertices
        .chunks_exact(6)
        .filter(|triangle| {
            let a = Point::new(triangle[0], triangle[1]);
            let b = Point::new(triangle[2], triangle[3]);
            let c = Point::new(triangle[4], triangle[5]);
            let signs = [cross(a, b, point), cross(b, c, point), cross(c, a, point)];
            signs.iter().all(|value| *value > 1e-5) || signs.iter().all(|value| *value < -1e-5)
        })
        .count()
}

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

fn assert_single_hole(vertices: &[f32]) {
    assert!((triangle_mesh_area(vertices) - 300.0).abs() < 1e-3);
    assert!(triangle_mesh_contains(vertices, Point::new(2.0, 2.0)));
    assert!(!triangle_mesh_contains(vertices, Point::new(10.0, 10.0)));
}

fn assert_mesh_matches_cpu(
    path: &Path,
    fill_rule: FillRule,
    width: i32,
    height: i32,
    expected_area: f32,
) {
    let vertices = tessellate_fill(path, fill_rule).expect("native contour forest mesh");
    assert!(!vertices.is_empty());
    assert!(vertices.chunks_exact(6).remainder().is_empty());
    assert!(vertices.iter().all(|coordinate| coordinate.is_finite()));
    assert!((triangle_mesh_area(&vertices) - expected_area).abs() < 1e-3);

    let mut cpu = SharedRasterizer::new(PixelSurface::new(width, height));
    cpu.fill_path(path, Color::white(), fill_rule);
    let pixels = cpu.surface().pixels();
    let polys = flattener::flatten(path.segments(), 0.25);
    let mut cpu_covered = 0usize;
    for y in 0..height {
        for x in 0..width {
            let index = (y * width + x) as usize;
            let cpu_contains = pixels[index] >> 24 != 0;
            let reference = reference_contains(
                &polys,
                Point::new(x as f32 + 0.5, y as f32 + 0.5),
                fill_rule,
            );
            let mesh_contains =
                triangle_mesh_contains(&vertices, Point::new(x as f32 + 0.5, y as f32 + 0.5));
            assert_eq!(cpu_contains, reference, "CPU mismatch at ({x}, {y})");
            assert_eq!(
                mesh_contains, reference,
                "coverage mismatch at ({x}, {y}) for {fill_rule:?}"
            );
            cpu_covered += usize::from(cpu_contains);
        }
    }
    assert_eq!(cpu_covered, expected_area as usize);
}

fn assert_complex_mesh_matches_reference(
    path: &Path,
    fill_rule: FillRule,
    width: i32,
    height: i32,
    expected_area: f32,
    expected_pixels: usize,
) -> Vec<f32> {
    let vertices = tessellate_fill(path, fill_rule).expect("complex native mesh");
    assert!(vertices.chunks_exact(6).remainder().is_empty());
    assert!(vertices.iter().all(|coordinate| coordinate.is_finite()));
    assert!((triangle_mesh_area(&vertices) - expected_area).abs() < 1e-3);

    let polys = flattener::flatten(path.segments(), 0.25);
    let mut cpu = SharedRasterizer::new(PixelSurface::new(width, height));
    cpu.fill_path(path, Color::white(), fill_rule);
    let cpu_pixels = cpu.surface().pixels();
    let mut covered = 0usize;
    for y in 0..height {
        for x in 0..width {
            let point = Point::new(x as f32 + 0.5, y as f32 + 0.5);
            let reference = reference_contains(&polys, point, fill_rule);
            let mesh = triangle_mesh_contains(&vertices, point);
            let cpu = cpu_pixels[(y * width + x) as usize] >> 24 != 0;
            assert_eq!(mesh, reference, "mesh mismatch at ({x}, {y})");
            assert_eq!(cpu, reference, "CPU mismatch at ({x}, {y})");
            covered += usize::from(reference);

            let interior_probe = Point::new(x as f32 + 0.37, y as f32 + 0.61);
            assert!(
                triangle_mesh_strict_hits(&vertices, interior_probe) <= 1,
                "triangle interiors overlap near ({x}, {y})"
            );
        }
    }
    assert_eq!(covered, expected_pixels);
    vertices
}

fn assert_sampled_mesh_matches_reference(
    path: &Path,
    fill_rule: FillRule,
    width: i32,
    height: i32,
) {
    let vertices = tessellate_fill(path, fill_rule).expect("bounded sampled mesh");
    let polys = flattener::flatten(path.segments(), 0.25);
    for y in 0..height {
        for x in 0..width {
            let point = Point::new(x as f32 + 0.37, y as f32 + 0.61);
            assert_eq!(
                triangle_mesh_contains(&vertices, point),
                reference_contains(&polys, point, fill_rule),
                "sample mismatch at ({x}, {y}) for {fill_rule:?}"
            );
            assert!(
                triangle_mesh_strict_hits(&vertices, point) <= 1,
                "sampled triangle interiors overlap at ({x}, {y})"
            );
        }
    }
}

#[test]
fn tessellates_triangle() {
    let mut pb = PathBuilder::new();
    pb.move_to(0.0, 0.0)
        .line_to(10.0, 0.0)
        .line_to(5.0, 8.0)
        .close();
    let v = tessellate_fill(&pb.build(), FillRule::NonZero).expect("tri");
    assert_eq!(v.len(), 6);
}

#[test]
fn tessellates_quad_to_two_tris() {
    let mut pb = PathBuilder::new();
    pb.move_to(0.0, 0.0)
        .line_to(20.0, 0.0)
        .line_to(20.0, 10.0)
        .line_to(0.0, 10.0)
        .close();
    let v = tessellate_fill(&pb.build(), FillRule::NonZero).expect("quad");
    assert_eq!(v.len(), 12);
}

#[test]
fn tessellates_two_disjoint_tris() {
    let mut pb = PathBuilder::new();
    pb.move_to(0.0, 0.0)
        .line_to(10.0, 0.0)
        .line_to(5.0, 8.0)
        .close();
    pb.move_to(20.0, 0.0)
        .line_to(30.0, 0.0)
        .line_to(25.0, 8.0)
        .close();
    let v = tessellate_fill(&pb.build(), FillRule::NonZero).expect("two");
    assert_eq!(v.len(), 12);
}

#[test]
fn tessellates_strict_contour_forests_for_fill_rules_winding_and_path_order() {
    for path in [
        nested_path(true, false),
        nested_path(false, false),
        nested_path(true, true),
    ] {
        let vertices = tessellate_fill(&path, FillRule::EvenOdd).expect("EvenOdd hole");
        assert_single_hole(&vertices);
    }

    for path in [nested_path(true, false), nested_path(true, true)] {
        let opposite =
            tessellate_fill(&path, FillRule::NonZero).expect("opposite-winding NonZero hole");
        assert_single_hole(&opposite);
    }

    for path in [nested_path(false, false), nested_path(false, true)] {
        let same =
            tessellate_fill(&path, FillRule::NonZero).expect("same-winding NonZero fill");
        assert!((triangle_mesh_area(&same) - 400.0).abs() < 1e-3);
        assert!(triangle_mesh_contains(&same, Point::new(10.0, 10.0)));
    }

    let mut two_holes = PathBuilder::new();
    add_rect(&mut two_holes, 4.0, 4.0, 92.0, 92.0, true);
    add_rect(&mut two_holes, 12.0, 12.0, 36.0, 36.0, true);
    add_rect(&mut two_holes, 60.0, 12.0, 84.0, 36.0, true);
    assert_mesh_matches_cpu(&two_holes.build(), FillRule::EvenOdd, 96, 96, 6592.0);

    let mut two_holes_scrambled = PathBuilder::new();
    add_rect(&mut two_holes_scrambled, 60.0, 12.0, 84.0, 36.0, false);
    add_rect(&mut two_holes_scrambled, 4.0, 4.0, 92.0, 92.0, true);
    add_rect(&mut two_holes_scrambled, 12.0, 12.0, 36.0, 36.0, false);
    let path = two_holes_scrambled.build();
    assert_mesh_matches_cpu(&path, FillRule::EvenOdd, 96, 96, 6592.0);
    assert_mesh_matches_cpu(&path, FillRule::NonZero, 96, 96, 6592.0);

    let mut two_holes_reversed = PathBuilder::new();
    add_rect(&mut two_holes_reversed, 12.0, 12.0, 36.0, 36.0, true);
    add_rect(&mut two_holes_reversed, 60.0, 12.0, 84.0, 36.0, true);
    add_rect(&mut two_holes_reversed, 4.0, 4.0, 92.0, 92.0, false);
    assert_mesh_matches_cpu(
        &two_holes_reversed.build(),
        FillRule::NonZero,
        96,
        96,
        6592.0,
    );

    let mut deep_even_odd = PathBuilder::new();
    for (x0, y0, x1, y1) in [
        (4.0, 4.0, 92.0, 92.0),
        (12.0, 12.0, 84.0, 84.0),
        (28.0, 28.0, 68.0, 68.0),
        (36.0, 36.0, 60.0, 60.0),
    ] {
        add_rect(&mut deep_even_odd, x0, y0, x1, y1, true);
    }
    assert_mesh_matches_cpu(&deep_even_odd.build(), FillRule::EvenOdd, 96, 96, 3584.0);

    let mut deep_nonzero = PathBuilder::new();
    for (rect, positive) in [
        ((4.0, 4.0, 92.0, 92.0), true),
        ((12.0, 12.0, 84.0, 84.0), false),
        ((28.0, 28.0, 68.0, 68.0), true),
        ((36.0, 36.0, 60.0, 60.0), false),
    ] {
        add_rect(&mut deep_nonzero, rect.0, rect.1, rect.2, rect.3, positive);
    }
    assert_mesh_matches_cpu(&deep_nonzero.build(), FillRule::NonZero, 96, 96, 3584.0);

    let mut deep_neutral = PathBuilder::new();
    for (rect, positive) in [
        ((36.0, 36.0, 60.0, 60.0), false),
        ((4.0, 4.0, 92.0, 92.0), true),
        ((28.0, 28.0, 68.0, 68.0), false),
        ((12.0, 12.0, 84.0, 84.0), true),
    ] {
        add_rect(&mut deep_neutral, rect.0, rect.1, rect.2, rect.3, positive);
    }
    assert_mesh_matches_cpu(&deep_neutral.build(), FillRule::NonZero, 96, 96, 7168.0);

    let mut islands = PathBuilder::new();
    add_rect(&mut islands, 80.0, 16.0, 112.0, 48.0, true);
    add_rect(&mut islands, 4.0, 4.0, 60.0, 60.0, true);
    add_rect(&mut islands, 68.0, 4.0, 124.0, 60.0, false);
    add_rect(&mut islands, 16.0, 16.0, 48.0, 48.0, false);
    assert_mesh_matches_cpu(&islands.build(), FillRule::NonZero, 128, 64, 4224.0);

    assert_invalid_or_oversized_paths_fall_back();
}

fn assert_invalid_or_oversized_paths_fall_back() {
    assert_eq!(
        tessellate_fill(&PathBuilder::new().build(), FillRule::NonZero),
        Some(Vec::new())
    );
    let mut finite_line = PathBuilder::new();
    finite_line.move_to(0.0, 0.0).line_to(10.0, 10.0);
    assert_eq!(
        tessellate_fill(&finite_line.build(), FillRule::NonZero),
        Some(Vec::new())
    );

    let mut nonfinite = PathBuilder::new();
    nonfinite
        .move_to(0.0, 0.0)
        .line_to(f32::NAN, 0.0)
        .line_to(0.0, 10.0)
        .close();
    assert!(tessellate_fill(&nonfinite.build(), FillRule::NonZero).is_none());

    let mut oversized_ring = PathBuilder::new();
    for i in 0..513 {
        let angle = std::f32::consts::TAU * i as f32 / 513.0;
        let (x, y) = (100.0 + 80.0 * angle.cos(), 100.0 + 80.0 * angle.sin());
        if i == 0 {
            oversized_ring.move_to(x, y);
        } else {
            oversized_ring.line_to(x, y);
        }
    }
    oversized_ring.close();
    assert!(tessellate_fill(&oversized_ring.build(), FillRule::NonZero).is_none());

    let mut oversized_path = PathBuilder::new();
    for i in 0..683 {
        let x = i as f32 * 4.0;
        oversized_path
            .move_to(x, 0.0)
            .line_to(x + 1.0, 0.0)
            .line_to(x, 1.0)
            .close();
    }
    assert!(tessellate_fill(&oversized_path.build(), FillRule::NonZero).is_none());

    let work_limited_ring = (0..1792)
        .map(|index| Point::new((index % 2) as f32, index as f32))
        .collect::<Vec<_>>();
    assert!(
        tessellate_complex_fill(&[work_limited_ring], FillRule::NonZero).is_none(),
        "complex scan/sort work must stay bounded"
    );

    let mut at_triangle_budget = vec![0.0; MAX_COMPLEX_TRIANGLES * 6];
    assert!(append_complex_triangle(
        &mut at_triangle_budget,
        F64Point { x: 0.0, y: 0.0 },
        F64Point { x: 1.0, y: 0.0 },
        F64Point { x: 0.0, y: 1.0 },
    )
    .is_none());
}

#[test]
fn self_intersections_match_fill_rules_without_triangle_overlap() {
    let mut orthogonal = PathBuilder::new();
    orthogonal
        .move_to(8.0, 8.0)
        .line_to(56.0, 8.0)
        .line_to(56.0, 40.0)
        .line_to(24.0, 40.0)
        .line_to(24.0, 24.0)
        .line_to(72.0, 24.0)
        .line_to(72.0, 56.0)
        .line_to(8.0, 56.0)
        .close();
    let path = orthogonal.build();
    assert_complex_mesh_matches_reference(&path, FillRule::EvenOdd, 80, 64, 2_304.0, 2_304);
    assert_complex_mesh_matches_reference(&path, FillRule::NonZero, 80, 64, 2_816.0, 2_816);

    let mut bow_tie = PathBuilder::new();
    bow_tie
        .move_to(8.0, 8.0)
        .line_to(56.0, 55.0)
        .line_to(8.0, 55.0)
        .line_to(56.0, 8.0)
        .close();
    let bow_tie = bow_tie.build();
    for fill_rule in [FillRule::EvenOdd, FillRule::NonZero] {
        let vertices =
            assert_complex_mesh_matches_reference(&bow_tie, fill_rule, 64, 64, 1_128.0, 1_104);
        assert!(triangle_mesh_contains(&vertices, Point::new(20.0, 16.0)));
        assert!(triangle_mesh_contains(&vertices, Point::new(20.0, 48.0)));
    }

    // Deterministic small-grid property sweep: arbitrary directed rings,
    // including repeated vertices and self-crosses, must match an
    // independent ray/winding evaluator without triangle-interior overlap.
    let mut seed = 0x6D2B_79F5u32;
    for case in 0..48 {
        let mut random_path = PathBuilder::new();
        let point_count = 6 + case % 5;
        let mut previous = None::<(f32, f32)>;
        for index in 0..point_count {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let mut x = 2.0 + (seed % 28) as f32;
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            let y = 2.0 + (seed % 28) as f32;
            if previous == Some((x, y)) {
                x = 2.0 + ((x as u32 + 1) % 28) as f32;
            }
            if index == 0 {
                random_path.move_to(x, y);
            } else {
                random_path.line_to(x, y);
            }
            previous = Some((x, y));
        }
        random_path.close();
        let path = random_path.build();
        for fill_rule in [FillRule::EvenOdd, FillRule::NonZero] {
            assert_sampled_mesh_matches_reference(&path, fill_rule, 32, 32);
        }
    }
}

#[test]
fn intersecting_touching_and_coincident_contours_match_fill_rule_oracle() {
    let mut overlapping = PathBuilder::new();
    add_rect(&mut overlapping, 8.0, 8.0, 48.0, 40.0, true);
    add_rect(&mut overlapping, 32.0, 24.0, 72.0, 56.0, true);
    let path = overlapping.build();
    assert_complex_mesh_matches_reference(&path, FillRule::EvenOdd, 80, 64, 2_048.0, 2_048);
    assert_complex_mesh_matches_reference(&path, FillRule::NonZero, 80, 64, 2_304.0, 2_304);

    let mut opposite = PathBuilder::new();
    add_rect(&mut opposite, 8.0, 8.0, 48.0, 40.0, true);
    add_rect(&mut opposite, 32.0, 24.0, 72.0, 56.0, false);
    assert_complex_mesh_matches_reference(
        &opposite.build(),
        FillRule::NonZero,
        80,
        64,
        2_048.0,
        2_048,
    );

    let mut collinear_overlap = PathBuilder::new();
    add_rect(&mut collinear_overlap, 8.0, 8.0, 48.0, 40.0, true);
    add_rect(&mut collinear_overlap, 32.0, 8.0, 72.0, 40.0, true);
    let path = collinear_overlap.build();
    assert_complex_mesh_matches_reference(&path, FillRule::EvenOdd, 80, 48, 1_536.0, 1_536);
    assert_complex_mesh_matches_reference(&path, FillRule::NonZero, 80, 48, 2_048.0, 2_048);

    for (first, second, expected) in [
        ((8.0, 8.0, 32.0, 32.0), (32.0, 8.0, 56.0, 32.0), 1_152.0),
        ((8.0, 8.0, 32.0, 32.0), (32.0, 32.0, 56.0, 56.0), 1_152.0),
        ((8.0, 8.0, 48.0, 40.0), (20.0, 40.0, 36.0, 56.0), 1_536.0),
    ] {
        let mut touching = PathBuilder::new();
        add_rect(&mut touching, first.0, first.1, first.2, first.3, true);
        add_rect(&mut touching, second.0, second.1, second.2, second.3, true);
        let path = touching.build();
        for fill_rule in [FillRule::EvenOdd, FillRule::NonZero] {
            assert_complex_mesh_matches_reference(
                &path,
                fill_rule,
                64,
                64,
                expected,
                expected as usize,
            );
        }
    }

    let mut duplicate = PathBuilder::new();
    add_rect(&mut duplicate, 8.0, 8.0, 32.0, 32.0, true);
    add_rect(&mut duplicate, 8.0, 8.0, 32.0, 32.0, true);
    let duplicate = duplicate.build();
    assert_eq!(
        tessellate_fill(&duplicate, FillRule::EvenOdd),
        Some(Vec::new())
    );
    assert_complex_mesh_matches_reference(&duplicate, FillRule::NonZero, 40, 40, 576.0, 576);

    let mut cancel = PathBuilder::new();
    add_rect(&mut cancel, 8.0, 8.0, 32.0, 32.0, true);
    add_rect(&mut cancel, 8.0, 8.0, 32.0, 32.0, false);
    assert_eq!(
        tessellate_fill(&cancel.build(), FillRule::NonZero),
        Some(Vec::new())
    );

    let mut near_but_distinct = PathBuilder::new();
    add_rect(&mut near_but_distinct, -2.0, 0.0, 0.0, 8.0, true);
    add_rect(&mut near_but_distinct, 5.0e-11, 0.0, 2.0, 8.0, true);
    for fill_rule in [FillRule::EvenOdd, FillRule::NonZero] {
        assert!(
            tessellate_fill(&near_but_distinct.build(), fill_rule).is_none(),
            "near but non-collinear boundaries must soft-fallback"
        );
    }

    let mut near_direction = PathBuilder::new();
    near_direction
        .move_to(0.0, 0.0)
        .line_to(0.0, 8.0)
        .line_to(-2.0, 8.0)
        .close()
        .move_to(0.0, 0.0)
        .line_to(5.0e-11, 8.0)
        .line_to(2.0, 8.0)
        .close();
    for fill_rule in [FillRule::EvenOdd, FillRule::NonZero] {
        assert!(
            tessellate_fill(&near_direction.build(), fill_rule).is_none(),
            "near but non-parallel boundaries must soft-fallback"
        );
    }
}

#[test]
fn stroke_caps_and_joins_tessellate_without_gaps_or_area_overlap() {
    let mut pb = PathBuilder::new();
    pb.move_to(24.0, 24.0)
        .line_to(72.0, 24.0)
        .line_to(72.0, 72.0);
    let path = pb.build();
    for (join, limit, expected_area) in [
        (super::super::path::LineJoin::Miter, 2.0, 1_536.0),
        (super::super::path::LineJoin::Miter, 1.0, 1_504.0),
        (super::super::path::LineJoin::Bevel, 4.0, 1_504.0),
    ] {
        let vertices = tessellate_stroke(
            &path,
            &StrokeOptions {
                width: 16.0,
                cap: super::super::path::LineCap::Butt,
                join,
                miter_limit: limit,
            },
        )
        .expect("stroke mesh");
        assert!((triangle_mesh_area(&vertices) - expected_area).abs() < 0.01);
        if expected_area == 1_536.0 {
            let covered = (0..96)
                .flat_map(|y| (0..96).map(move |x| (x, y)))
                .filter(|&(x, y)| {
                    triangle_mesh_contains(
                        &vertices,
                        Point::new(x as f32 + 0.5, y as f32 + 0.5),
                    )
                })
                .count();
            assert_eq!(covered, expected_area as usize);
        }
    }

    let mut line = PathBuilder::new();
    line.move_to(24.0, 48.0).line_to(88.0, 48.0);
    for cap in [
        super::super::path::LineCap::Butt,
        super::super::path::LineCap::Round,
        super::super::path::LineCap::Square,
    ] {
        assert!(tessellate_stroke(
            &line.build(),
            &StrokeOptions {
                width: 16.0,
                cap,
                join: super::super::path::LineJoin::Round,
                miter_limit: 4.0,
            },
        )
        .is_some());
    }

    let mut closed = PathBuilder::new();
    closed
        .move_to(24.0, 24.0)
        .line_to(88.0, 24.0)
        .line_to(88.0, 72.0)
        .line_to(24.0, 72.0)
        .close();
    for join in [
        super::super::path::LineJoin::Miter,
        super::super::path::LineJoin::Bevel,
        super::super::path::LineJoin::Round,
    ] {
        assert!(tessellate_stroke(
            &closed.build(),
            &StrokeOptions {
                width: 8.0,
                cap: super::super::path::LineCap::Square,
                join,
                miter_limit: 4.0,
            },
        )
        .is_some());
    }

    let mut reversed = PathBuilder::new();
    reversed
        .move_to(72.0, 72.0)
        .line_to(72.0, 24.0)
        .line_to(24.0, 24.0);
    let reversed_mesh = tessellate_stroke(
        &reversed.build(),
        &StrokeOptions {
            width: 16.0,
            cap: super::super::path::LineCap::Butt,
            join: super::super::path::LineJoin::Miter,
            miter_limit: 2.0,
        },
    )
    .expect("reversed stroke mesh");
    assert!((triangle_mesh_area(&reversed_mesh) - 1_536.0).abs() < 0.01);
}
