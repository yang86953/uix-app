//! 路径描边器 — 将路径描边转换为无分段重叠的填充轮廓。
//!
//! 开放子路径生成单个外轮廓；闭合子路径生成方向相反的外/内轮廓。
//! `LineCap`、`LineJoin` 与 `miter_limit` 均在共享轮廓阶段处理，CPU
//! rasterizer 与 GPU tessellator 因而使用同一套几何语义。

use super::flattener;
use super::path::{LineCap, LineJoin, Path, PathBuilder};
use crate::core::Point;

const GEOMETRY_EPSILON: f32 = 1e-4;
const ARC_TOLERANCE: f32 = 0.25;
const MAX_ARC_STEPS: usize = 512;

/// 描边参数。
#[derive(Debug, Clone, Copy)]
pub struct StrokeOptions {
    pub width: f32,
    pub cap: LineCap,
    pub join: LineJoin,
    pub miter_limit: f32,
}

impl Default for StrokeOptions {
    fn default() -> Self {
        Self {
            width: 1.0,
            cap: LineCap::Butt,
            join: LineJoin::Miter,
            miter_limit: 4.0,
        }
    }
}

/// 将路径描边转换为填充路径（供 rasterizer 与 GPU tessellator 复用）。
pub fn stroke_path(path: &Path, options: &StrokeOptions) -> Path {
    let Some(rings) = stroke_outline_rings(path, options) else {
        return Path {
            segments: Vec::new(),
        };
    };
    path_from_outline_rings(rings)
}

pub(crate) fn path_from_outline_rings(rings: Vec<Vec<Point>>) -> Path {
    let mut builder = PathBuilder::new();
    for ring in rings {
        let Some(first) = ring.first() else {
            continue;
        };
        builder.move_to(first.x, first.y);
        for point in ring.iter().skip(1) {
            builder.line_to(point.x, point.y);
        }
        builder.close();
    }
    builder.build()
}

/// 生成描边区域的闭合轮廓。
///
/// `None` 表示输入含非有限参数；
/// `Some(Vec::new())` 表示路径为空、线宽为零或没有有效线段。
pub(crate) fn stroke_outline_rings(
    path: &Path,
    options: &StrokeOptions,
) -> Option<Vec<Vec<Point>>> {
    if !options.width.is_finite() || !options.miter_limit.is_finite() {
        return None;
    }
    let half_w = options.width * 0.5;
    if half_w < GEOMETRY_EPSILON {
        return Some(Vec::new());
    }

    let mut rings = Vec::new();
    for subpath in flattener::flatten_subpaths(path.segments(), ARC_TOLERANCE) {
        let mut points = clean_points(&subpath.points)?;
        if subpath.closed && points.len() >= 2 && points_near(points[0], points[points.len() - 1]) {
            points.pop();
        }
        let minimum = if subpath.closed { 3 } else { 2 };
        if points.len() < minimum {
            continue;
        }

        let directions = segment_directions(&points, subpath.closed)?;
        if subpath.closed {
            let left = build_side(&points, &directions, 1.0, half_w, options, true)?;
            let right = build_side(&points, &directions, -1.0, half_w, options, true)?;
            let (mut outer, mut inner) = if polygon_area(&left).abs() >= polygon_area(&right).abs()
            {
                (left, right)
            } else {
                (right, left)
            };
            orient_ring(&mut outer, true);
            orient_ring(&mut inner, false);
            if outer.len() >= 3 {
                rings.push(outer);
            }
            if inner.len() >= 3 && polygon_area(&inner).abs() > GEOMETRY_EPSILON {
                rings.push(inner);
            }
        } else {
            let left = build_side(&points, &directions, 1.0, half_w, options, false)?;
            let mut right = build_side(&points, &directions, -1.0, half_w, options, false)?;
            right.reverse();
            let mut outline = left;
            append_end_cap(
                &mut outline,
                points[points.len() - 1],
                directions[directions.len() - 1],
                half_w,
                options.cap,
            )?;
            append_chain_without_first(&mut outline, &right);
            append_start_cap(&mut outline, points[0], directions[0], half_w, options.cap)?;
            remove_closing_duplicate(&mut outline);
            orient_ring(&mut outline, true);
            if outline.len() >= 3 && polygon_area(&outline).abs() > GEOMETRY_EPSILON {
                rings.push(outline);
            }
        }
    }
    Some(rings)
}

fn clean_points(points: &[Point]) -> Option<Vec<Point>> {
    let mut cleaned = Vec::with_capacity(points.len());
    for &point in points {
        if !point.x.is_finite() || !point.y.is_finite() {
            return None;
        }
        if cleaned
            .last()
            .is_some_and(|&previous| points_near(previous, point))
        {
            continue;
        }
        cleaned.push(point);
    }
    Some(cleaned)
}

fn segment_directions(points: &[Point], closed: bool) -> Option<Vec<Point>> {
    let count = if closed {
        points.len()
    } else {
        points.len().checked_sub(1)?
    };
    let mut directions = Vec::with_capacity(count);
    for index in 0..count {
        let start = points[index];
        let end = points[(index + 1) % points.len()];
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let length = dx.hypot(dy);
        if !length.is_finite() || length < GEOMETRY_EPSILON {
            return None;
        }
        directions.push(Point::new(dx / length, dy / length));
    }
    Some(directions)
}

fn build_side(
    points: &[Point],
    directions: &[Point],
    side: f32,
    half_w: f32,
    options: &StrokeOptions,
    closed: bool,
) -> Option<Vec<Point>> {
    let mut side_points = Vec::new();
    if closed {
        for index in 0..points.len() {
            let previous = directions[(index + directions.len() - 1) % directions.len()];
            let next = directions[index];
            append_join(
                &mut side_points,
                points[index],
                previous,
                next,
                side,
                half_w,
                options,
            )?;
        }
    } else {
        push_distinct(
            &mut side_points,
            offset_point(points[0], directions[0], side, half_w),
        );
        for index in 1..points.len() - 1 {
            append_join(
                &mut side_points,
                points[index],
                directions[index - 1],
                directions[index],
                side,
                half_w,
                options,
            )?;
        }
        push_distinct(
            &mut side_points,
            offset_point(
                points[points.len() - 1],
                directions[directions.len() - 1],
                side,
                half_w,
            ),
        );
    }
    Some(side_points)
}

fn append_join(
    output: &mut Vec<Point>,
    vertex: Point,
    previous: Point,
    next: Point,
    side: f32,
    half_w: f32,
    options: &StrokeOptions,
) -> Option<()> {
    let turn = cross(previous, next);
    let alignment = dot(previous, next);
    let previous_offset = offset_point(vertex, previous, side, half_w);
    let next_offset = offset_point(vertex, next, side, half_w);

    if turn.abs() < GEOMETRY_EPSILON {
        if alignment > 0.0 {
            push_distinct(output, next_offset);
        } else {
            // 180° cusp has no finite miter. A bevel connection remains bounded;
            // topology validation decides whether the resulting stroke can be native.
            push_distinct(output, previous_offset);
            push_distinct(output, next_offset);
        }
        return Some(());
    }

    let intersection = offset_line_intersection(previous_offset, previous, next_offset, next);
    let outer_side = turn * side < 0.0;
    if !outer_side {
        push_distinct(output, intersection?);
        return Some(());
    }

    match options.join {
        LineJoin::Bevel => {
            push_distinct(output, previous_offset);
            push_distinct(output, next_offset);
        }
        LineJoin::Miter => {
            let miter = intersection?;
            let ratio = distance(vertex, miter) / half_w;
            if ratio.is_finite() && ratio <= options.miter_limit.max(1.0) {
                push_distinct(output, miter);
            } else {
                push_distinct(output, previous_offset);
                push_distinct(output, next_offset);
            }
        }
        LineJoin::Round => {
            push_distinct(output, previous_offset);
            let start = vector(vertex, previous_offset);
            let end = vector(vertex, next_offset);
            let sweep = cross(start, end).atan2(dot(start, end));
            append_arc(output, vertex, start, sweep, half_w)?;
        }
    }
    Some(())
}

fn append_end_cap(
    output: &mut Vec<Point>,
    endpoint: Point,
    direction: Point,
    half_w: f32,
    cap: LineCap,
) -> Option<()> {
    let left = offset_point(endpoint, direction, 1.0, half_w);
    let right = offset_point(endpoint, direction, -1.0, half_w);
    match cap {
        LineCap::Butt => push_distinct(output, right),
        LineCap::Square => {
            push_distinct(output, translate(left, direction, half_w));
            push_distinct(output, translate(right, direction, half_w));
            push_distinct(output, right);
        }
        LineCap::Round => {
            append_arc(
                output,
                endpoint,
                vector(endpoint, left),
                -std::f32::consts::PI,
                half_w,
            )?;
        }
    }
    Some(())
}

fn append_start_cap(
    output: &mut Vec<Point>,
    endpoint: Point,
    direction: Point,
    half_w: f32,
    cap: LineCap,
) -> Option<()> {
    let right = offset_point(endpoint, direction, -1.0, half_w);
    let left = offset_point(endpoint, direction, 1.0, half_w);
    match cap {
        LineCap::Butt => push_distinct(output, left),
        LineCap::Square => {
            push_distinct(output, translate(right, direction, -half_w));
            push_distinct(output, translate(left, direction, -half_w));
            push_distinct(output, left);
        }
        LineCap::Round => {
            append_arc(
                output,
                endpoint,
                vector(endpoint, right),
                -std::f32::consts::PI,
                half_w,
            )?;
        }
    }
    Some(())
}

fn append_arc(
    output: &mut Vec<Point>,
    center: Point,
    start: Point,
    sweep: f32,
    radius: f32,
) -> Option<()> {
    let steps = arc_steps(radius, sweep)?;
    let start_angle = start.y.atan2(start.x);
    for step in 1..=steps {
        let angle = start_angle + sweep * step as f32 / steps as f32;
        push_distinct(
            output,
            Point::new(
                center.x + angle.cos() * radius,
                center.y + angle.sin() * radius,
            ),
        );
    }
    Some(())
}

fn arc_steps(radius: f32, sweep: f32) -> Option<usize> {
    if !radius.is_finite() || !sweep.is_finite() || radius < GEOMETRY_EPSILON {
        return None;
    }
    let cosine = (1.0 - ARC_TOLERANCE / radius).clamp(-1.0, 1.0);
    let max_step = 2.0 * cosine.acos();
    let required = if max_step > GEOMETRY_EPSILON {
        (sweep.abs() / max_step).ceil().max(1.0) as usize
    } else {
        MAX_ARC_STEPS
    };
    Some(required.min(MAX_ARC_STEPS))
}

fn offset_line_intersection(
    a: Point,
    a_direction: Point,
    b: Point,
    b_direction: Point,
) -> Option<Point> {
    let denominator = cross(a_direction, b_direction);
    if denominator.abs() < GEOMETRY_EPSILON {
        return None;
    }
    let delta = vector(a, b);
    let t = cross(delta, b_direction) / denominator;
    let point = translate(a, a_direction, t);
    (point.x.is_finite() && point.y.is_finite()).then_some(point)
}

fn offset_point(point: Point, direction: Point, side: f32, distance: f32) -> Point {
    Point::new(
        point.x - direction.y * side * distance,
        point.y + direction.x * side * distance,
    )
}

fn translate(point: Point, direction: Point, distance: f32) -> Point {
    Point::new(
        point.x + direction.x * distance,
        point.y + direction.y * distance,
    )
}

fn vector(from: Point, to: Point) -> Point {
    Point::new(to.x - from.x, to.y - from.y)
}

fn cross(a: Point, b: Point) -> f32 {
    a.x * b.y - a.y * b.x
}

fn dot(a: Point, b: Point) -> f32 {
    a.x * b.x + a.y * b.y
}

fn distance(a: Point, b: Point) -> f32 {
    (b.x - a.x).hypot(b.y - a.y)
}

fn points_near(a: Point, b: Point) -> bool {
    distance(a, b) < GEOMETRY_EPSILON
}

fn push_distinct(points: &mut Vec<Point>, point: Point) {
    if !points.last().is_some_and(|&last| points_near(last, point)) {
        points.push(point);
    }
}

fn append_chain_without_first(output: &mut Vec<Point>, chain: &[Point]) {
    for &point in chain.iter().skip(1) {
        push_distinct(output, point);
    }
}

fn remove_closing_duplicate(ring: &mut Vec<Point>) {
    if ring.len() >= 2 && points_near(ring[0], ring[ring.len() - 1]) {
        ring.pop();
    }
}

fn polygon_area(points: &[Point]) -> f32 {
    let mut area = 0.0;
    for index in 0..points.len() {
        let next = (index + 1) % points.len();
        area += points[index].x * points[next].y - points[next].x * points[index].y;
    }
    area * 0.5
}

fn orient_ring(ring: &mut [Point], positive: bool) {
    if (polygon_area(ring) > 0.0) != positive {
        ring.reverse();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
