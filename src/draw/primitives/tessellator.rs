//! Conservative tessellation for GPU-native path fills/strokes (#169).
//!
//! - **Fill**: flatten → validate topology → ear-clip simple rings
//!   (disjoint rings OK), with one strictly nested ring handled as either a
//!   fill-rule-neutral contour or a hole through `earcut`.
//! - **Stroke**: flatten → thick-line segment quads (butt ends; joins overlap).
//!
//! Complex fill cases outside that narrow slice (self-intersections,
//! intersecting/touching contours, multiple holes or deeper nesting, ear-clip
//! failure) return `None` so callers can soft-fallback.

use super::flattener;
use super::path::{FillRule, Path};
use super::stroker::StrokeOptions;
use crate::core::Point;

/// Max vertices in a ring we attempt to ear-clip (keeps GPU upload bounded).
const MAX_RING_VERTS: usize = 512;
/// Max flattened vertices across all rings before the topology guard falls back.
const MAX_PATH_VERTS: usize = 2048;

/// Tessellate a path into a triangle-list of xy pairs (`[x0,y0, x1,y1, …]`).
///
/// Native slice constraints:
/// - identity caller (transform applied upstream)
/// - simple polygon rings after flatten (self-intersections rejected)
/// - disjoint rings, or exactly one strictly nested inner ring
/// - `EvenOdd` always treats that inner ring as a hole
/// - `NonZero` treats opposite winding as a hole and same winding as redundant
pub fn tessellate_fill(path: &Path, fill_rule: FillRule) -> Option<Vec<f32>> {
    let polys = flattener::flatten(path.segments(), 0.25);
    if polys.is_empty() {
        return None;
    }
    let mut rings = Vec::with_capacity(polys.len());
    let mut path_vertices = 0usize;
    for poly in &polys {
        let Some(ring) = clean_ring(poly) else {
            continue;
        };
        path_vertices = path_vertices.checked_add(ring.len())?;
        if ring.len() > MAX_RING_VERTS || path_vertices > MAX_PATH_VERTS || !ring_is_simple(&ring) {
            return None;
        }
        rings.push(ring);
    }
    if rings.is_empty() {
        return None;
    }

    let nested = classify_single_nested_pair(&rings)?;
    if let Some((outer, inner)) = nested {
        return tessellate_single_nested(&rings[outer], &rings[inner], fill_rule);
    }

    let mut tris: Vec<f32> = Vec::new();
    for ring in &rings {
        let ear = ear_clip(ring)?;
        tris.extend(ear);
    }
    if tris.len() < 6 {
        return None;
    }
    Some(tris)
}

/// Tessellate a stroke as thick-line segment quads (triangle list).
///
/// Uses butt-style segment ends; adjacent segments overlap at joins (fine for
/// opaque Alpha/SrcOver). Round/Square caps are not modeled exactly.
pub fn tessellate_stroke(path: &Path, opts: &StrokeOptions) -> Option<Vec<f32>> {
    let half = opts.width * 0.5;
    if half < 1e-4 {
        return None;
    }
    let polys = flattener::flatten(path.segments(), 0.25);
    if polys.is_empty() {
        return None;
    }
    let mut tris: Vec<f32> = Vec::new();
    for poly in &polys {
        if poly.len() < 2 {
            continue;
        }
        for i in 0..poly.len() - 1 {
            let p0 = poly[i];
            let p1 = poly[i + 1];
            let dx = p1.x - p0.x;
            let dy = p1.y - p0.y;
            let len = (dx * dx + dy * dy).sqrt();
            if len < 1e-4 {
                continue;
            }
            let nx = -dy / len * half;
            let ny = dx / len * half;
            let a = Point::new(p0.x + nx, p0.y + ny);
            let b = Point::new(p1.x + nx, p1.y + ny);
            let c = Point::new(p1.x - nx, p1.y - ny);
            let d = Point::new(p0.x - nx, p0.y - ny);
            // Two triangles: a-b-c, a-c-d
            tris.extend_from_slice(&[a.x, a.y, b.x, b.y, c.x, c.y]);
            tris.extend_from_slice(&[a.x, a.y, c.x, c.y, d.x, d.y]);
        }
    }
    if tris.len() < 6 {
        return None;
    }
    Some(tris)
}

fn clean_ring(pts: &[Point]) -> Option<Vec<Point>> {
    if pts.len() < 3 {
        return None;
    }
    let mut out: Vec<Point> = Vec::with_capacity(pts.len());
    for p in pts {
        if !p.x.is_finite() || !p.y.is_finite() {
            return None;
        }
        if let Some(last) = out.last() {
            if (last.x - p.x).abs() < 1e-4 && (last.y - p.y).abs() < 1e-4 {
                continue;
            }
        }
        out.push(*p);
    }
    if out.len() >= 2 {
        let first = out[0];
        let &last = out.last()?;
        if (first.x - last.x).abs() < 1e-4 && (first.y - last.y).abs() < 1e-4 {
            out.pop();
        }
    }
    if out.len() < 3 {
        return None;
    }
    Some(out)
}

fn ring_is_simple(ring: &[Point]) -> bool {
    let len = ring.len();
    for i in 0..len {
        let a0 = ring[i];
        let a1 = ring[(i + 1) % len];
        for j in i + 1..len {
            if edges_are_adjacent(i, j, len) {
                continue;
            }
            let b0 = ring[j];
            let b1 = ring[(j + 1) % len];
            if segments_intersect_or_touch(a0, a1, b0, b1) {
                return false;
            }
        }
    }
    true
}

/// Validate pairwise contour topology and identify the only nested shape this
/// native slice accepts: exactly one outer ring plus one strictly inner ring.
/// `Some(None)` means all rings are disjoint; `None` means fallback.
fn classify_single_nested_pair(rings: &[Vec<Point>]) -> Option<Option<(usize, usize)>> {
    let bounds: Vec<_> = rings.iter().map(|ring| ring_bounds(ring)).collect();
    let mut nested = None;
    for i in 0..rings.len() {
        for j in i + 1..rings.len() {
            if !bounds_overlap(bounds[i], bounds[j]) {
                continue;
            }
            if rings_intersect_or_touch(&rings[i], &rings[j]) {
                return None;
            }
            let i_in_j = point_in_ring(rings[i][0], &rings[j]);
            let j_in_i = point_in_ring(rings[j][0], &rings[i]);
            if i_in_j || j_in_i {
                if nested.is_some() {
                    return None;
                }
                nested = Some(if i_in_j { (j, i) } else { (i, j) });
            }
        }
    }

    if nested.is_some() && rings.len() != 2 {
        return None;
    }
    Some(nested)
}

fn tessellate_single_nested(
    outer: &[Point],
    inner: &[Point],
    fill_rule: FillRule,
) -> Option<Vec<f32>> {
    let outer_area = polygon_area(outer);
    let inner_area = polygon_area(inner);
    if outer_area.abs() < 1e-8 || inner_area.abs() < 1e-8 {
        return None;
    }

    if fill_rule == FillRule::NonZero && outer_area.signum() == inner_area.signum() {
        return ear_clip(outer);
    }

    tessellate_single_hole(outer, inner)
}

fn tessellate_single_hole(outer: &[Point], inner: &[Point]) -> Option<Vec<f32>> {
    let vertices: Vec<[f32; 2]> = outer
        .iter()
        .chain(inner.iter())
        .map(|point| [point.x, point.y])
        .collect();
    let hole_indices = [outer.len()];
    let mut indices = Vec::<usize>::new();
    earcut::Earcut::<f32>::new().earcut(vertices.iter().copied(), &hole_indices, &mut indices);
    if indices.len() < 3 || !indices.len().is_multiple_of(3) {
        return None;
    }

    let mut tris = Vec::with_capacity(indices.len() * 2);
    let mut triangulated_area = 0.0f32;
    for triangle in indices.chunks_exact(3) {
        let a = *vertices.get(triangle[0])?;
        let b = *vertices.get(triangle[1])?;
        let c = *vertices.get(triangle[2])?;
        triangulated_area +=
            ((b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])).abs() * 0.5;
        tris.extend_from_slice(&[a[0], a[1], b[0], b[1], c[0], c[1]]);
    }

    let expected_area = polygon_area(outer).abs() - polygon_area(inner).abs();
    let tolerance = expected_area.max(1.0) * 1e-4;
    if expected_area <= 1e-8 || (triangulated_area - expected_area).abs() > tolerance {
        return None;
    }
    Some(tris)
}

fn ring_bounds(ring: &[Point]) -> (f32, f32, f32, f32) {
    ring.iter().fold(
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
    )
}

fn bounds_overlap(a: (f32, f32, f32, f32), b: (f32, f32, f32, f32)) -> bool {
    const EPSILON: f32 = 1e-5;
    a.0 <= b.2 + EPSILON && a.2 + EPSILON >= b.0 && a.1 <= b.3 + EPSILON && a.3 + EPSILON >= b.1
}

fn edges_are_adjacent(a: usize, b: usize, len: usize) -> bool {
    a == b || (a + 1) % len == b || (b + 1) % len == a
}

fn rings_intersect_or_touch(a: &[Point], b: &[Point]) -> bool {
    for i in 0..a.len() {
        let a0 = a[i];
        let a1 = a[(i + 1) % a.len()];
        for j in 0..b.len() {
            let b0 = b[j];
            let b1 = b[(j + 1) % b.len()];
            if segments_intersect_or_touch(a0, a1, b0, b1) {
                return true;
            }
        }
    }
    false
}

fn segments_intersect_or_touch(a0: Point, a1: Point, b0: Point, b1: Point) -> bool {
    const EPSILON: f32 = 1e-5;

    let o1 = cross(a0, a1, b0);
    let o2 = cross(a0, a1, b1);
    let o3 = cross(b0, b1, a0);
    let o4 = cross(b0, b1, a1);
    let proper = ((o1 > EPSILON && o2 < -EPSILON) || (o1 < -EPSILON && o2 > EPSILON))
        && ((o3 > EPSILON && o4 < -EPSILON) || (o3 < -EPSILON && o4 > EPSILON));
    proper
        || (o1.abs() <= EPSILON && point_on_segment(b0, a0, a1, EPSILON))
        || (o2.abs() <= EPSILON && point_on_segment(b1, a0, a1, EPSILON))
        || (o3.abs() <= EPSILON && point_on_segment(a0, b0, b1, EPSILON))
        || (o4.abs() <= EPSILON && point_on_segment(a1, b0, b1, EPSILON))
}

fn point_on_segment(p: Point, a: Point, b: Point, epsilon: f32) -> bool {
    p.x >= a.x.min(b.x) - epsilon
        && p.x <= a.x.max(b.x) + epsilon
        && p.y >= a.y.min(b.y) - epsilon
        && p.y <= a.y.max(b.y) + epsilon
}

fn point_in_ring(point: Point, ring: &[Point]) -> bool {
    let mut inside = false;
    let mut j = ring.len() - 1;
    for i in 0..ring.len() {
        let a = ring[i];
        let b = ring[j];
        let crosses = (a.y > point.y) != (b.y > point.y);
        if crosses {
            let x = (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x;
            if point.x < x {
                inside = !inside;
            }
        }
        j = i;
    }
    inside
}

fn polygon_area(pts: &[Point]) -> f32 {
    let n = pts.len();
    let mut a = 0.0;
    for i in 0..n {
        let p = pts[i];
        let q = pts[(i + 1) % n];
        a += p.x * q.y - q.x * p.y;
    }
    a * 0.5
}

fn cross(o: Point, a: Point, b: Point) -> f32 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

fn point_in_triangle(p: Point, a: Point, b: Point, c: Point) -> bool {
    let c1 = cross(a, b, p);
    let c2 = cross(b, c, p);
    let c3 = cross(c, a, p);
    let has_neg = (c1 < 0.0) || (c2 < 0.0) || (c3 < 0.0);
    let has_pos = (c1 > 0.0) || (c2 > 0.0) || (c3 > 0.0);
    !(has_neg && has_pos)
}

fn is_convex(prev: Point, curr: Point, next: Point, ccw: bool) -> bool {
    let c = cross(prev, curr, next);
    if ccw {
        c > 1e-6
    } else {
        c < -1e-6
    }
}

fn is_ear(pts: &[Point], indices: &[usize], ear_i: usize, ccw: bool) -> bool {
    let n = indices.len();
    let i_prev = indices[(ear_i + n - 1) % n];
    let i_curr = indices[ear_i];
    let i_next = indices[(ear_i + 1) % n];
    let a = pts[i_prev];
    let b = pts[i_curr];
    let c = pts[i_next];
    if !is_convex(a, b, c, ccw) {
        return false;
    }
    for (j, &idx) in indices.iter().enumerate() {
        if j == (ear_i + n - 1) % n || j == ear_i || j == (ear_i + 1) % n {
            continue;
        }
        if point_in_triangle(pts[idx], a, b, c) {
            return false;
        }
    }
    true
}

fn ear_clip(ring: &[Point]) -> Option<Vec<f32>> {
    let area = polygon_area(ring);
    if area.abs() < 1e-8 {
        return None;
    }
    let ccw = area > 0.0;
    let mut indices: Vec<usize> = (0..ring.len()).collect();
    let mut tris: Vec<f32> = Vec::with_capacity((ring.len().saturating_sub(2)) * 6);
    let mut guard = ring.len() * ring.len() + 8;
    while indices.len() > 3 {
        if guard == 0 {
            return None;
        }
        guard -= 1;
        let n = indices.len();
        let mut found = None;
        for i in 0..n {
            if is_ear(ring, &indices, i, ccw) {
                found = Some(i);
                break;
            }
        }
        let i = found?;
        let i_prev = indices[(i + n - 1) % n];
        let i_curr = indices[i];
        let i_next = indices[(i + 1) % n];
        let a = ring[i_prev];
        let b = ring[i_curr];
        let c = ring[i_next];
        tris.extend_from_slice(&[a.x, a.y, b.x, b.y, c.x, c.y]);
        indices.remove(i);
    }
    let a = ring[indices[0]];
    let b = ring[indices[1]];
    let c = ring[indices[2]];
    tris.extend_from_slice(&[a.x, a.y, b.x, b.y, c.x, c.y]);
    Some(tris)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::primitives::path::PathBuilder;

    fn add_outer(builder: &mut PathBuilder) {
        builder
            .move_to(0.0, 0.0)
            .line_to(20.0, 0.0)
            .line_to(20.0, 20.0)
            .line_to(0.0, 20.0)
            .close();
    }

    fn add_inner(builder: &mut PathBuilder, clockwise: bool) {
        if clockwise {
            builder
                .move_to(5.0, 5.0)
                .line_to(5.0, 15.0)
                .line_to(15.0, 15.0)
                .line_to(15.0, 5.0)
                .close();
        } else {
            builder
                .move_to(5.0, 5.0)
                .line_to(15.0, 5.0)
                .line_to(15.0, 15.0)
                .line_to(5.0, 15.0)
                .close();
        }
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

    fn assert_single_hole(vertices: &[f32]) {
        assert!((triangle_mesh_area(vertices) - 300.0).abs() < 1e-3);
        assert!(triangle_mesh_contains(vertices, Point::new(2.0, 2.0)));
        assert!(!triangle_mesh_contains(vertices, Point::new(10.0, 10.0)));
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
    fn tessellates_single_nested_hole_for_fill_rules_winding_and_path_order() {
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

        assert_unsupported_nested_topologies_fall_back();
    }

    fn assert_unsupported_nested_topologies_fall_back() {
        let mut multiple_holes = PathBuilder::new();
        add_outer(&mut multiple_holes);
        add_inner(&mut multiple_holes, true);
        multiple_holes
            .move_to(2.0, 2.0)
            .line_to(2.0, 4.0)
            .line_to(4.0, 4.0)
            .line_to(4.0, 2.0)
            .close();
        assert!(tessellate_fill(&multiple_holes.build(), FillRule::EvenOdd).is_none());

        let mut hole_and_island = PathBuilder::new();
        add_outer(&mut hole_and_island);
        add_inner(&mut hole_and_island, true);
        hole_and_island
            .move_to(30.0, 0.0)
            .line_to(35.0, 0.0)
            .line_to(32.5, 5.0)
            .close();
        assert!(tessellate_fill(&hole_and_island.build(), FillRule::EvenOdd).is_none());

        let mut deep = PathBuilder::new();
        add_outer(&mut deep);
        add_inner(&mut deep, true);
        deep.move_to(7.0, 7.0)
            .line_to(13.0, 7.0)
            .line_to(13.0, 13.0)
            .line_to(7.0, 13.0)
            .close();
        assert!(tessellate_fill(&deep.build(), FillRule::EvenOdd).is_none());

        let mut touching = PathBuilder::new();
        add_outer(&mut touching);
        touching
            .move_to(0.0, 5.0)
            .line_to(0.0, 15.0)
            .line_to(10.0, 15.0)
            .line_to(10.0, 5.0)
            .close();
        assert!(tessellate_fill(&touching.build(), FillRule::EvenOdd).is_none());
    }

    #[test]
    fn self_intersecting_ring_requires_soft_fallback() {
        let ring = vec![
            Point::new(0.0, 0.0),
            Point::new(4.0, 0.0),
            Point::new(0.0, 4.0),
            Point::new(4.0, 4.0),
            Point::new(2.0, 1.0),
        ];
        assert!(polygon_area(&ring).abs() > 1e-6);
        assert!(!ring_is_simple(&ring));

        let mut pb = PathBuilder::new();
        pb.move_to(0.0, 0.0)
            .line_to(4.0, 0.0)
            .line_to(0.0, 4.0)
            .line_to(4.0, 4.0)
            .line_to(2.0, 1.0)
            .close();

        assert!(tessellate_fill(&pb.build(), FillRule::EvenOdd).is_none());
    }

    #[test]
    fn intersecting_contours_require_soft_fallback() {
        let mut pb = PathBuilder::new();
        pb.move_to(0.0, 0.0)
            .line_to(12.0, 0.0)
            .line_to(12.0, 12.0)
            .line_to(0.0, 12.0)
            .close();
        pb.move_to(8.0, 8.0)
            .line_to(20.0, 8.0)
            .line_to(20.0, 20.0)
            .line_to(8.0, 20.0)
            .close();

        assert!(tessellate_fill(&pb.build(), FillRule::NonZero).is_none());
    }

    #[test]
    fn stroke_line_tessellates() {
        let mut pb = PathBuilder::new();
        pb.move_to(0.0, 0.0).line_to(40.0, 0.0);
        let opts = StrokeOptions {
            width: 4.0,
            ..Default::default()
        };
        let v = tessellate_stroke(&pb.build(), &opts).expect("stroke");
        assert_eq!(v.len(), 12); // one segment → 2 tris
    }
}
