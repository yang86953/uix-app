//! Conservative tessellation for GPU-native path fills/strokes (#169).
//!
//! - **Fill**: flatten → validate a strict contour forest → classify
//!   `EvenOdd` parity / `NonZero` winding transitions → tessellate each filled
//!   component and all of its holes through ear clipping / `earcut`.
//! - **Stroke**: build a shared cap/join-aware stroke outline → validate the
//!   resulting strict contour forest → tessellate it through the fill path.
//!
//! Self-intersections, intersecting/touching contours, invalid topology or
//! tessellation failure return `None` so callers can soft-fallback.

use super::flattener;
use super::path::{FillRule, Path};
use super::stroker::{self, StrokeOptions};
use crate::core::Point;

/// Max vertices in a ring we attempt to tessellate (keeps GPU upload bounded).
const MAX_RING_VERTS: usize = 512;
/// Max flattened vertices across all rings before the topology guard falls back.
const MAX_PATH_VERTS: usize = 2048;

/// Tessellate a path into a triangle-list of xy pairs (`[x0,y0, x1,y1, …]`).
///
/// Native slice constraints:
/// - identity caller (transform applied upstream)
/// - simple polygon rings after flatten (self-intersections rejected)
/// - every pair of contours is strictly disjoint or strictly nested
/// - arbitrary contour order, hole count, nesting depth and disjoint islands
/// - exact `EvenOdd` parity and `NonZero` accumulated-winding transitions
pub fn tessellate_fill(path: &Path, fill_rule: FillRule) -> Option<Vec<f32>> {
    let polys = flattener::flatten(path.segments(), 0.25);
    if polys.is_empty() {
        return None;
    }
    let mut rings = Vec::with_capacity(polys.len());
    let mut path_vertices = 0usize;
    for poly in &polys {
        let Some(ring) = clean_ring(poly).ok()? else {
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

    let groups = classify_fill_groups(&rings, fill_rule)?;
    let mut tris = Vec::new();
    for group in &groups {
        tris.extend(tessellate_fill_group(&rings, group)?);
    }
    if tris.len() < 6 {
        return None;
    }
    Some(tris)
}

/// Tessellate a stroke outline into a non-overlapping triangle list.
///
/// Cap/join geometry is shared with the CPU rasterizer. Unsupported outline
/// topology returns `None`, so callers retain the existing soft fallback.
pub fn tessellate_stroke(path: &Path, opts: &StrokeOptions) -> Option<Vec<f32>> {
    let rings = stroker::stroke_outline_rings(path, opts)?;
    if rings.is_empty() {
        return Some(Vec::new());
    }
    tessellate_fill(&stroker::path_from_outline_rings(rings), FillRule::NonZero)
}

fn clean_ring(pts: &[Point]) -> Result<Option<Vec<Point>>, ()> {
    if pts.len() < 3 {
        return Ok(None);
    }
    let mut out: Vec<Point> = Vec::with_capacity(pts.len());
    for p in pts {
        if !p.x.is_finite() || !p.y.is_finite() {
            return Err(());
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
        let &last = out.last().ok_or(())?;
        if (first.x - last.x).abs() < 1e-4 && (first.y - last.y).abs() < 1e-4 {
            out.pop();
        }
    }
    if out.len() < 3 {
        return Ok(None);
    }
    Ok(Some(out))
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

#[derive(Debug)]
struct FillGroup {
    outer: usize,
    holes: Vec<usize>,
}

/// Build a strict containment forest, then classify every ring by the fill
/// state immediately outside and inside that boundary.
fn classify_fill_groups(rings: &[Vec<Point>], fill_rule: FillRule) -> Option<Vec<FillGroup>> {
    let areas: Vec<f64> = rings.iter().map(|ring| polygon_area_f64(ring)).collect();
    if areas
        .iter()
        .any(|area| !area.is_finite() || area.abs() <= 1e-8)
    {
        return None;
    }
    let bounds: Vec<_> = rings.iter().map(|ring| ring_bounds(ring)).collect();
    let mut parents = vec![None; rings.len()];
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
            if i_in_j && j_in_i {
                return None;
            }
            if i_in_j {
                update_parent(&mut parents, &areas, i, j)?;
            } else if j_in_i {
                update_parent(&mut parents, &areas, j, i)?;
            }
        }
    }

    let mut order: Vec<usize> = (0..rings.len()).collect();
    order.sort_by(|&a, &b| {
        areas[b]
            .abs()
            .total_cmp(&areas[a].abs())
            .then_with(|| a.cmp(&b))
    });

    let mut inside_winding = vec![None::<i32>; rings.len()];
    let mut inside_parity = vec![None::<bool>; rings.len()];
    let mut active_group = vec![None::<usize>; rings.len()];
    let mut groups = Vec::<FillGroup>::new();

    for ring in order {
        let (outside_winding, outside_parity, outside_group) = match parents[ring] {
            Some(parent) => (
                inside_winding[parent]?,
                inside_parity[parent]?,
                active_group[parent],
            ),
            None => (0, false, None),
        };
        let winding_delta = if areas[ring] > 0.0 { 1 } else { -1 };
        let winding = outside_winding.checked_add(winding_delta)?;
        let parity = !outside_parity;
        let outside_filled = match fill_rule {
            FillRule::EvenOdd => outside_parity,
            FillRule::NonZero => outside_winding != 0,
        };
        let inside_filled = match fill_rule {
            FillRule::EvenOdd => parity,
            FillRule::NonZero => winding != 0,
        };

        let group = match (outside_filled, inside_filled) {
            (false, true) => {
                if outside_group.is_some() {
                    return None;
                }
                let group = groups.len();
                groups.push(FillGroup {
                    outer: ring,
                    holes: Vec::new(),
                });
                Some(group)
            }
            (true, false) => {
                let group = outside_group?;
                groups.get_mut(group)?.holes.push(ring);
                None
            }
            (true, true) => Some(outside_group?),
            (false, false) => return None,
        };

        inside_winding[ring] = Some(winding);
        inside_parity[ring] = Some(parity);
        active_group[ring] = group;
    }

    if groups.is_empty() {
        None
    } else {
        Some(groups)
    }
}

fn update_parent(
    parents: &mut [Option<usize>],
    areas: &[f64],
    child: usize,
    candidate: usize,
) -> Option<()> {
    if areas[candidate].abs() <= areas[child].abs() {
        return None;
    }
    match parents[child] {
        Some(current) if areas[current].abs() <= areas[candidate].abs() => {}
        _ => parents[child] = Some(candidate),
    }
    Some(())
}

fn tessellate_fill_group(rings: &[Vec<Point>], group: &FillGroup) -> Option<Vec<f32>> {
    let outer = rings.get(group.outer)?;
    let expected_area =
        group
            .holes
            .iter()
            .try_fold(polygon_area_f64(outer).abs(), |area, &hole| {
                let remaining = area - polygon_area_f64(rings.get(hole)?).abs();
                remaining.is_finite().then_some(remaining)
            })?;
    if expected_area <= 1e-8 {
        return None;
    }

    let tris = if group.holes.is_empty() {
        ear_clip(outer)?
    } else {
        let vertex_count = group.holes.iter().try_fold(outer.len(), |count, &hole| {
            count.checked_add(rings.get(hole)?.len())
        })?;
        let mut vertices = Vec::<[f32; 2]>::with_capacity(vertex_count);
        vertices.extend(outer.iter().map(|point| [point.x, point.y]));
        let mut hole_indices = Vec::with_capacity(group.holes.len());
        for &hole in &group.holes {
            hole_indices.push(vertices.len());
            vertices.extend(rings.get(hole)?.iter().map(|point| [point.x, point.y]));
        }

        let mut indices = Vec::<usize>::new();
        earcut::Earcut::<f32>::new().earcut(vertices.iter().copied(), &hole_indices, &mut indices);
        if indices.len() < 3 || !indices.chunks_exact(3).remainder().is_empty() {
            return None;
        }

        let mut triangles = Vec::with_capacity(indices.len().checked_mul(2)?);
        for triangle in indices.chunks_exact(3) {
            let a = *vertices.get(triangle[0])?;
            let b = *vertices.get(triangle[1])?;
            let c = *vertices.get(triangle[2])?;
            triangles.extend_from_slice(&[a[0], a[1], b[0], b[1], c[0], c[1]]);
        }
        triangles
    };

    validate_triangle_area(&tris, expected_area).then_some(tris)
}

fn validate_triangle_area(tris: &[f32], expected_area: f64) -> bool {
    if tris.len() < 6 || !tris.chunks_exact(6).remainder().is_empty() {
        return false;
    }
    let mut triangulated_area = 0.0f64;
    for triangle in tris.chunks_exact(6) {
        if triangle.iter().any(|coordinate| !coordinate.is_finite()) {
            return false;
        }
        let (ax, ay) = (triangle[0] as f64, triangle[1] as f64);
        let (bx, by) = (triangle[2] as f64, triangle[3] as f64);
        let (cx, cy) = (triangle[4] as f64, triangle[5] as f64);
        triangulated_area += ((bx - ax) * (cy - ay) - (by - ay) * (cx - ax)).abs() * 0.5;
    }
    let tolerance = expected_area.max(1.0) * 1e-4;
    (triangulated_area - expected_area).abs() <= tolerance
}

fn polygon_area_f64(pts: &[Point]) -> f64 {
    let mut area = 0.0f64;
    for i in 0..pts.len() {
        let point = pts[i];
        let next = pts[(i + 1) % pts.len()];
        area += point.x as f64 * next.y as f64 - next.x as f64 * point.y as f64;
    }
    area * 0.5
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
        let mut cpu_covered = 0usize;
        for y in 0..height {
            for x in 0..width {
                let index = (y * width + x) as usize;
                let cpu_contains = pixels[index] >> 24 != 0;
                let mesh_contains =
                    triangle_mesh_contains(&vertices, Point::new(x as f32 + 0.5, y as f32 + 0.5));
                assert_eq!(
                    mesh_contains, cpu_contains,
                    "coverage mismatch at ({x}, {y}) for {fill_rule:?}"
                );
                cpu_covered += usize::from(cpu_contains);
            }
        }
        assert_eq!(cpu_covered, expected_area as usize);
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

        assert_unsupported_topologies_fall_back();
    }

    fn assert_unsupported_topologies_fall_back() {
        let mut touching = PathBuilder::new();
        add_outer(&mut touching);
        touching
            .move_to(0.0, 5.0)
            .line_to(0.0, 15.0)
            .line_to(10.0, 15.0)
            .line_to(10.0, 5.0)
            .close();
        assert!(tessellate_fill(&touching.build(), FillRule::EvenOdd).is_none());

        let mut intersecting_holes = PathBuilder::new();
        add_rect(&mut intersecting_holes, 0.0, 0.0, 40.0, 40.0, true);
        add_rect(&mut intersecting_holes, 5.0, 5.0, 25.0, 25.0, false);
        add_rect(&mut intersecting_holes, 15.0, 15.0, 35.0, 35.0, false);
        assert!(tessellate_fill(&intersecting_holes.build(), FillRule::NonZero).is_none());

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
}
