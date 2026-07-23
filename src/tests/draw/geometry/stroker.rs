use crate::draw::geometry::flattener;
use crate::draw::geometry::path::PathBuilder;
use crate::draw::geometry::stroker::*;
use crate::tests::common::*;

fn horizontal_line(cap: LineCap) -> Vec<Point> {
    let mut builder = PathBuilder::new();
    builder.move_to(24.0, 48.0).line_to(88.0, 48.0);
    stroke_outline_rings(
        &builder.build(),
        &StrokeOptions {
            width: 16.0,
            cap,
            ..Default::default()
        },
    )
    .expect("stroke outline")
    .remove(0)
}

#[test]
fn straight_caps_match_analytic_area_and_bounds() {
    let cases = [
        (LineCap::Butt, 1_024.0, (24.0, 40.0, 88.0, 56.0)),
        (LineCap::Square, 1_280.0, (16.0, 40.0, 96.0, 56.0)),
    ];
    for (cap, area, bounds) in cases {
        let ring = horizontal_line(cap);
        assert!((polygon_area(&ring).abs() - area).abs() < 0.01);
        let actual = ring.iter().fold(
            (
                f32::INFINITY,
                f32::INFINITY,
                f32::NEG_INFINITY,
                f32::NEG_INFINITY,
            ),
            |(min_x, min_y, max_x, max_y), point| {
                (
                    min_x.min(point.x),
                    min_y.min(point.y),
                    max_x.max(point.x),
                    max_y.max(point.y),
                )
            },
        );
        assert_eq!(actual, bounds);
    }

    let round = horizontal_line(LineCap::Round);
    let expected = 64.0 * 16.0 + std::f32::consts::PI * 8.0 * 8.0;
    assert!((polygon_area(&round).abs() - expected).abs() < 8.0);
}

#[test]
fn right_angle_joins_and_miter_limit_match_analytic_area() {
    let mut builder = PathBuilder::new();
    builder
        .move_to(24.0, 24.0)
        .line_to(72.0, 24.0)
        .line_to(72.0, 72.0);
    let path = builder.build();
    for (join, limit, expected) in [
        (LineJoin::Miter, 2.0, 1_536.0),
        (LineJoin::Miter, std::f32::consts::SQRT_2, 1_536.0),
        (LineJoin::Miter, 1.0, 1_504.0),
        (LineJoin::Bevel, 4.0, 1_504.0),
    ] {
        let rings = stroke_outline_rings(
            &path,
            &StrokeOptions {
                width: 16.0,
                cap: LineCap::Butt,
                join,
                miter_limit: limit,
            },
        )
        .expect("stroke outline");
        assert_eq!(rings.len(), 1);
        assert!((polygon_area(&rings[0]).abs() - expected).abs() < 0.01);
    }

    let round = stroke_outline_rings(
        &path,
        &StrokeOptions {
            width: 16.0,
            cap: LineCap::Butt,
            join: LineJoin::Round,
            miter_limit: 4.0,
        },
    )
    .expect("round join outline");
    let round_area = polygon_area(&round[0]).abs();
    assert!(round_area > 1_504.0 && round_area < 1_536.0);
}

#[test]
fn closed_subpath_uses_join_instead_of_caps() {
    let mut builder = PathBuilder::new();
    builder
        .move_to(24.0, 24.0)
        .line_to(88.0, 24.0)
        .line_to(88.0, 72.0)
        .line_to(24.0, 72.0)
        .close();
    for cap in [LineCap::Butt, LineCap::Round, LineCap::Square] {
        let rings = stroke_outline_rings(
            &builder.build(),
            &StrokeOptions {
                width: 16.0,
                cap,
                ..Default::default()
            },
        )
        .expect("closed stroke outline");
        assert_eq!(rings.len(), 2);
        let area: f32 = rings.iter().map(|ring| polygon_area(ring)).sum();
        assert!((area - 3_584.0).abs() < 0.01);
    }
}

#[test]
fn explicit_return_to_start_remains_open_and_subpaths_stay_independent() {
    let mut builder = PathBuilder::new();
    builder
        .move_to(16.0, 16.0)
        .line_to(16.0, 16.0)
        .line_to(48.0, 16.0)
        .line_to(16.0, 16.0);
    builder.move_to(16.0, 48.0).line_to(48.0, 48.0);
    let rings = stroke_outline_rings(
        &builder.build(),
        &StrokeOptions {
            width: 8.0,
            cap: LineCap::Square,
            join: LineJoin::Bevel,
            miter_limit: 4.0,
        },
    )
    .expect("multiple stroke outlines");
    assert_eq!(rings.len(), 2);
    assert!(rings.iter().all(|ring| polygon_area(ring) > 0.0));

    let mut cubic_loop = PathBuilder::new();
    cubic_loop
        .move_to(40.0, 40.0)
        .cubic_to(80.0, 0.0, 80.0, 80.0, 40.0, 40.0);
    let flattened = flattener::flatten_subpaths(cubic_loop.build().segments(), 0.25);
    assert_eq!(flattened.len(), 1);
    assert!(flattened[0].points.len() > 4);
    assert!(flattened[0]
        .points
        .iter()
        .all(|point| point.x.is_finite() && point.y.is_finite()));

    let mut invalid = PathBuilder::new();
    invalid
        .move_to(0.0, 0.0)
        .cubic_to(f32::NAN, 10.0, 20.0, 30.0, 40.0, 50.0);
    assert!(flattener::flatten_subpaths(invalid.build().segments(), 0.25).is_empty());
}
